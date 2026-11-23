//! Voice: drives songbird (voice gateway v8, UDP, RTP/Opus, GCM/XChaCha and
//! DAVE E2EE all live inside it) with the endpoint the main gateway hands over,
//! wires the mic in and the decoded audio out, and reports who's speaking.

use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use async_trait::async_trait;
use songbird::driver::{Channels, CryptoMode, DecodeConfig, DecodeMode, SampleRate};
use songbird::events::context_data::{VoiceData, VoiceTick};
use songbird::events::{Event, EventContext, EventHandler};
use songbird::id::{ChannelId, GuildId, UserId};
use songbird::model::payload::{ClientDisconnect, Speaking};
use songbird::{Config, ConnectionInfo, Driver};
use tokio::sync::{Mutex, mpsc};

use crate::audio;
use crate::gateway::State;

/// A live voice connection: the driver plus the maps the handlers keep.
pub struct Voice {
    /// Shared with the mic's gate, which adds a track per burst of speech.
    driver: Arc<std::sync::Mutex<Driver>>,
    shared: Arc<Shared>,
    /// Stops the mic's gate.
    mic_stop: Arc<std::sync::atomic::AtomicBool>,
}

struct Shared {
    state: Arc<Mutex<State>>,
    /// ssrc -> user id, learned from speaking updates.
    ssrc: Mutex<HashMap<u32, String>>,
    speaker: async_channel::Sender<Vec<i16>>,
    notify: mpsc::UnboundedSender<()>,
}

impl Voice {
    /// Connects to the channel the gateway's `VoiceServer` event described.
    /// `notify` fires when the speaking set changes, so the snapshot refreshes.
    pub async fn connect(
        state: Arc<Mutex<State>>,
        guild_id: Option<String>,
        channel_id: String,
        endpoint: String,
        token: String,
        session_id: String,
        notify: mpsc::UnboundedSender<()>,
    ) -> anyhow::Result<Voice> {
        let me = state.lock().await.me_id().ok_or_else(|| anyhow::anyhow!("no user id yet"))?;
        // A DM call has no guild; songbird still needs a GuildId slot, so the
        // channel id stands in (Discord accepts it for private calls).
        let gid = guild_id.as_deref().unwrap_or(&channel_id);
        let info = ConnectionInfo {
            channel_id: ChannelId(parse(&channel_id)?),
            endpoint,
            guild_id: GuildId(parse(gid)?),
            session_id,
            token,
            user_id: UserId(parse(&me)?),
        };

        let mut config = Config::default()
            .crypto_mode(CryptoMode::Aes256Gcm)
            .decode_mode(DecodeMode::Decode(DecodeConfig::new(Channels::Mono, SampleRate::Hz48000)));
        // Discord video (design/DISCORD-VIDEO.md), not yet on by default:
        // MESH_DC_VIDEO=1 receives others' cameras into Mesh's call tiles;
        // MESH_DC_VIDEO_SPIKE=<folder> (V0) does too, and saves the first ten
        // frames there. Off, the voice connection is as before.
        let save = std::env::var_os("MESH_DC_VIDEO_SPIKE").map(std::path::PathBuf::from);
        if save.is_some() || std::env::var_os("MESH_DC_VIDEO").is_some_and(|v| v == "1") {
            let (tx, rx) = flume::bounded(64);
            config = config.video_sink(tx);
            video_receive(rx, save);
        }
        let mut driver = Driver::new(config);

        let speaker = audio::open_speaker()?;
        let shared = Arc::new(Shared { state, ssrc: Mutex::new(HashMap::new()), speaker, notify });

        driver.add_global_event(Event::Core(songbird::CoreEvent::SpeakingStateUpdate), Handler(shared.clone()));
        driver.add_global_event(Event::Core(songbird::CoreEvent::VoiceTick), Handler(shared.clone()));
        driver.add_global_event(Event::Core(songbird::CoreEvent::ClientDisconnect), Handler(shared.clone()));

        driver.connect(info).await.map_err(|e| anyhow::anyhow!("voice connect: {e}"))?;

        // Mic in, a track per burst of speech, so Discord hears "speaking"
        // each time (its AFK timer counts that); songbird resamples the
        // native-rate mono f32 to 48 kHz Opus.
        let driver = Arc::new(std::sync::Mutex::new(driver));
        let mic_stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        match audio::open_mic() {
            Ok(mic) => {
                let (rate, d) = (mic.rate, driver.clone());
                mic.gate(mic_stop.clone(), move |talk| {
                    let input = songbird::input::RawAdapter::new(talk, rate, 1);
                    d.lock().unwrap().play_input(input.into());
                });
            }
            Err(e) => log::warn!("no microphone, listen-only: {e}"),
        }

        {
            let mut s = shared.state.lock().await;
            s.rtc_connected = true;
            s.voice_channel = Some(channel_id);
            s.voice_guild = guild_id;
        }
        Ok(Voice {
            driver,
            shared,
            mic_stop,
        })
    }

    pub fn set_mute(&mut self, mute: bool) {
        self.driver.lock().unwrap().mute(mute);
    }

    pub async fn leave(self) {
        self.mic_stop
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.driver.lock().unwrap().leave();
        let mut s = self.shared.state.lock().await;
        s.rtc_connected = false;
        s.speaking.clear();
    }
}

/// How often a camera's picture goes to Mesh at most (the app's tiles).
const TILE_EVERY: std::time::Duration = std::time::Duration::from_millis(100);

/// Decodes others' camera frames and sends them to Mesh as `meshVideo`
/// (a JPEG, at most 10 a second each); with `save` (V0), logs each and
/// saves the first ten as JPEGs there. No IDs in the log: SSRCs only.
fn video_receive(rx: flume::Receiver<songbird::driver::VideoFrame>, save: Option<std::path::PathBuf>) {
    log::info!("video: receiving{}", save.as_ref().map(|d| format!(", saving to {}", d.display())).unwrap_or_default());
    std::thread::spawn(move || {
        if let Some(dir) = &save {
            let _ = std::fs::create_dir_all(dir);
        }
        let mut decoders: HashMap<u32, mesh_media::H264Decoder> = HashMap::new();
        let mut shown: HashMap<u32, std::time::Instant> = HashMap::new();
        let (mut got, mut saved) = (0u32, 0u32);
        while let Ok(f) = rx.recv() {
            got += 1;
            let decoder = match decoders.entry(f.ssrc) {
                std::collections::hash_map::Entry::Occupied(d) => d.into_mut(),
                std::collections::hash_map::Entry::Vacant(slot) => match mesh_media::H264Decoder::new() {
                    Ok(d) => slot.insert(d),
                    Err(e) => return log::warn!("video spike: no decoder: {e}"),
                },
            };
            match decoder.decode(&f.annexb) {
                Ok(Some(frame)) => {
                    if save.is_some() {
                        log::info!("video spike: frame {got} (ssrc {}, {} bytes): {}x{}", f.ssrc, f.annexb.len(), frame.width, frame.height);
                    }
                    let due = shown.get(&f.ssrc).is_none_or(|at| at.elapsed() >= TILE_EVERY);
                    let picture = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba.clone());
                    if let (true, Some(p)) = (due, &picture) {
                        shown.insert(f.ssrc, std::time::Instant::now());
                        let mut jpeg = Vec::new();
                        let rgb = image::DynamicImage::ImageRgba8(p.clone()).to_rgb8();
                        if rgb.write_to(&mut std::io::Cursor::new(&mut jpeg), image::ImageFormat::Jpeg).is_ok() {
                            use base64::Engine as _;
                            let data = base64::engine::general_purpose::STANDARD.encode(&jpeg);
                            crate::emit("meshVideo", &serde_json::json!({ "id": format!("dc-{}", f.user_id), "local": false, "jpeg": data }));
                        }
                    }
                    if let (true, Some(dir)) = (saved < 10, &save) {
                        saved += 1;
                        let path = dir.join(format!("frame-{saved:02}.jpg"));
                        match picture.map(|p| image::DynamicImage::ImageRgba8(p).to_rgb8().save(&path)) {
                            Some(Ok(())) => log::info!("video spike: saved {}", path.display()),
                            Some(Err(e)) => log::warn!("video spike: couldn't save: {e}"),
                            None => log::warn!("video spike: a frame of the wrong size"),
                        }
                    }
                }
                Ok(None) => log::debug!("video: frame {got} (ssrc {}, {} bytes): no picture yet", f.ssrc, f.annexb.len()),
                Err(e) => log::warn!("video: frame {got} (ssrc {}) didn't decode: {e}", f.ssrc),
            }
        }
    });
}

/// Adds `pcm` onto `mix` sample by sample, saturating.
fn mix_into(mix: &mut Vec<i16>, pcm: &[i16]) {
    if mix.len() < pcm.len() {
        mix.resize(pcm.len(), 0);
    }
    for (m, &p) in mix.iter_mut().zip(pcm) {
        *m = m.saturating_add(p);
    }
}

fn parse(id: &str) -> anyhow::Result<NonZeroU64> {
    id.parse::<NonZeroU64>().map_err(|_| anyhow::anyhow!("bad id {id}"))
}

struct Handler(Arc<Shared>);

#[async_trait]
impl EventHandler for Handler {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        match ctx {
            EventContext::SpeakingStateUpdate(Speaking { ssrc, user_id, .. }) => {
                if let Some(uid) = user_id {
                    self.0.ssrc.lock().await.insert(*ssrc, uid.0.to_string());
                }
            }
            EventContext::VoiceTick(VoiceTick { speaking, .. }) => {
                self.0.play_tick(speaking).await;
            }
            EventContext::ClientDisconnect(ClientDisconnect { user_id }) => {
                let uid = user_id.0.to_string();
                self.0.ssrc.lock().await.retain(|_, v| *v != uid);
                let mut s = self.0.state.lock().await;
                if s.speaking.remove(&uid).is_some() {
                    let _ = self.0.notify.send(());
                }
            }
            _ => {}
        }
        None
    }
}

impl Shared {
    async fn play_tick(&self, speaking: &HashMap<u32, VoiceData>) {
        let ssrc = self.ssrc.lock().await;
        let mut now: HashMap<String, bool> = HashMap::new();
        let mut mix: Vec<i16> = Vec::new();
        for (s, data) in speaking {
            if let Some(pcm) = &data.decoded_voice {
                mix_into(&mut mix, pcm);
            }
            if let Some(uid) = ssrc.get(s) {
                now.insert(uid.clone(), true);
            }
        }
        drop(ssrc);
        // One mixed 20 ms frame per tick; queueing each speaker's frame in
        // turn played them back to back at N x real time (robotic).
        if !mix.is_empty() {
            let _ = self.speaker.try_send(mix);
        }
        let mut state = self.state.lock().await;
        if state.speaking != now {
            state.speaking = now;
            let _ = self.notify.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mix_into;

    #[test]
    fn mixes_speakers_into_one_frame() {
        let mut mix = Vec::new();
        mix_into(&mut mix, &[100, -200, 30_000]);
        mix_into(&mut mix, &[1, 2, 30_000]);
        assert_eq!(mix, vec![101, -198, i16::MAX]); // same length, summed, clamped
    }
}
