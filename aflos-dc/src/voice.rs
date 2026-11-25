//! Voice: drives songbird (voice gateway v8, UDP, RTP/Opus, GCM/XChaCha and
//! DAVE E2EE all live inside it) with the endpoint the main gateway hands over,
//! wires the mic in and the decoded audio out, and reports who's speaking.

use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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
    /// Our video's sender, filled when the connection is up (video only).
    video_tx: Option<songbird::driver::VideoTxSlot>,
    /// Stops our camera, while it's on.
    camera: Option<Arc<AtomicBool>>,
}

/// Whether Discord video is on (design/DISCORD-VIDEO.md): not by default
/// yet. `AFLOS_DC_VIDEO=1`, or the V0 spike's `AFLOS_DC_VIDEO_SPIKE`.
pub fn video_on() -> bool {
    std::env::var_os("AFLOS_DC_VIDEO_SPIKE").is_some()
        || std::env::var_os("AFLOS_DC_VIDEO").is_some_and(|v| v == "1")
}

/// What our video shows.
#[derive(Clone, Copy, PartialEq)]
enum Source {
    Camera,
    /// A screen or window, from the desktop's own picker.
    Screen,
}

/// Captures `source`, encodes it as H264 and sends it through `slot` until
/// the returned flag is set: op 12 says it's on with the first frame and off
/// at the end. A camera also shows in Aflos as our self view.
fn send_video(slot: songbird::driver::VideoTxSlot, source: Source) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    std::thread::spawn(move || {
        let (fps, max_width, bitrate) = match source {
            Source::Camera => (30, 1280, 2_000_000),
            Source::Screen => (15, 1920, 2_500_000),
        };
        let opts = aflos_media::Opts { fps, max_width };
        let capture = match source {
            Source::Camera => aflos_media::Capture::camera("", opts),
            Source::Screen => aflos_media::Capture::source("", opts),
        };
        let capture = match capture {
            Ok(c) => c,
            Err(e) => return log::warn!("video: couldn't capture: {e}"),
        };
        let mut encoder = match aflos_media::H264Encoder::new(fps, bitrate) {
            Ok(e) => e,
            Err(e) => return log::warn!("video: no encoder: {e}"),
        };
        // Any starting timestamp will do; the clock's nanoseconds are as good as random.
        let base = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        let started = std::time::Instant::now();
        let (mut size, mut last_view) = (None, None::<std::time::Instant>);
        let mut keyframe_at = std::time::Instant::now();
        while !stopped.load(Ordering::Relaxed) {
            let Ok(Some(frame)) = capture.recv_timeout(std::time::Duration::from_millis(500))
            else {
                continue;
            };
            // A keyframe now and then, so someone joining sees a picture soon.
            let key = size.is_none() || keyframe_at.elapsed() > std::time::Duration::from_secs(2);
            if key {
                keyframe_at = std::time::Instant::now();
            }
            let Ok(annexb) = encoder.encode(&frame, key) else {
                continue;
            };
            let ts = base.wrapping_add((started.elapsed().as_millis() as u64 * 90) as u32);
            let mut guard = slot.lock().unwrap();
            let Some(tx) = guard.as_mut() else { continue };
            if size != Some((frame.width, frame.height)) {
                size = Some((frame.width, frame.height));
                tx.announce(true, frame.width, frame.height, fps);
            }
            if let Err(e) = tx.send_frame(&annexb, ts) {
                log::warn!("video: send: {e}");
            }
            drop(guard);
            if source == Source::Camera && last_view.is_none_or(|t| t.elapsed() >= TILE_EVERY) {
                last_view = Some(std::time::Instant::now());
                if let Ok(jpeg) = frame.jpeg(480, 70) {
                    use base64::Engine as _;
                    let data = base64::engine::general_purpose::STANDARD.encode(&jpeg);
                    crate::emit(
                        "meshVideo",
                        &serde_json::json!({ "id": "dc-self", "local": true, "jpeg": data }),
                    );
                }
            }
        }
        if let (Some(tx), Some((w, h))) = (slot.lock().unwrap().as_ref(), size) {
            tx.announce(false, w, h, fps);
        }
        if source == Source::Camera {
            crate::emit(
                "meshVideo",
                &serde_json::json!({ "id": "dc-self", "ended": true }),
            );
        }
    });
    stop
}

/// A Go Live stream (design/DISCORD-VIDEO.md V3, V4): its own voice
/// connection, either watching someone's screen or sending ours.
pub struct Stream {
    pub key: String,
    driver: Driver,
    sending: Option<Arc<AtomicBool>>,
}

impl Stream {
    /// Connects to a stream's voice server. Watching, its frames go to Aflos
    /// as `stream-<owner>` tiles; sharing, our screen goes out.
    pub async fn connect(
        me: &str,
        key: String,
        rtc_server_id: &str,
        endpoint: String,
        token: String,
        session_id: String,
        share: bool,
    ) -> anyhow::Result<Stream> {
        // A stream's RTC server id stands for the guild; its DAVE group is the
        // id before it (as Discord's clients do).
        let server: u64 = rtc_server_id
            .parse()
            .map_err(|_| anyhow::anyhow!("bad rtc server id"))?;
        let info = ConnectionInfo {
            channel_id: ChannelId(
                NonZeroU64::new(server.saturating_sub(1))
                    .ok_or_else(|| anyhow::anyhow!("bad rtc server id"))?,
            ),
            endpoint,
            guild_id: GuildId(parse(rtc_server_id)?),
            session_id,
            token,
            user_id: UserId(parse(me)?),
        };
        let slot: songbird::driver::VideoTxSlot = Arc::default();
        let mut config = Config::default()
            .crypto_mode(CryptoMode::Aes256Gcm)
            .video_screen(true);
        if share {
            config = config.video_tx(slot.clone());
        } else {
            let (tx, rx) = flume::bounded(64);
            config = config
                .video_sink(tx)
                .decode_mode(DecodeMode::Decode(DecodeConfig::new(
                    Channels::Mono,
                    SampleRate::Hz48000,
                )));
            video_receive(rx, None, "stream-");
        }
        let mut driver = Driver::new(config);
        // Watching, the stream's sound plays too, as voice does; through a
        // state of its own, so it doesn't light up the streamer's ring.
        if !share {
            let (notify, _) = mpsc::unbounded_channel();
            let shared = Arc::new(Shared {
                state: Arc::default(),
                ssrc: Mutex::new(HashMap::new()),
                speaker: audio::open_speaker()?,
                notify,
            });
            driver.add_global_event(
                Event::Core(songbird::CoreEvent::SpeakingStateUpdate),
                Handler(shared.clone()),
            );
            driver.add_global_event(Event::Core(songbird::CoreEvent::VoiceTick), Handler(shared));
        }
        driver
            .connect(info)
            .await
            .map_err(|e| anyhow::anyhow!("stream connect: {e}"))?;
        let sending = share.then(|| send_video(slot, Source::Screen));
        Ok(Stream {
            key,
            driver,
            sending,
        })
    }

    pub fn leave(mut self) {
        if let Some(stop) = self.sending.take() {
            stop.store(true, Ordering::Relaxed);
        }
        self.driver.leave();
    }
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
        let me = state
            .lock()
            .await
            .me_id()
            .ok_or_else(|| anyhow::anyhow!("no user id yet"))?;
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
            .decode_mode(DecodeMode::Decode(DecodeConfig::new(
                Channels::Mono,
                SampleRate::Hz48000,
            )));
        // Discord video (design/DISCORD-VIDEO.md), not yet on by default:
        // AFLOS_DC_VIDEO=1 receives others' cameras into Aflos's call tiles;
        // AFLOS_DC_VIDEO_SPIKE=<folder> (V0) does too, and saves the first ten
        // frames there. Off, the voice connection is as before.
        let save = std::env::var_os("AFLOS_DC_VIDEO_SPIKE").map(std::path::PathBuf::from);
        let video_tx = video_on().then(songbird::driver::VideoTxSlot::default);
        if let Some(slot) = &video_tx {
            let (tx, rx) = flume::bounded(64);
            config = config.video_sink(tx).video_tx(slot.clone());
            video_receive(rx, save, "dc-");
        }
        let mut driver = Driver::new(config);

        let speaker = audio::open_speaker()?;
        let shared = Arc::new(Shared {
            state,
            ssrc: Mutex::new(HashMap::new()),
            speaker,
            notify,
        });

        driver.add_global_event(
            Event::Core(songbird::CoreEvent::SpeakingStateUpdate),
            Handler(shared.clone()),
        );
        driver.add_global_event(
            Event::Core(songbird::CoreEvent::VoiceTick),
            Handler(shared.clone()),
        );
        driver.add_global_event(
            Event::Core(songbird::CoreEvent::ClientDisconnect),
            Handler(shared.clone()),
        );

        driver
            .connect(info)
            .await
            .map_err(|e| anyhow::anyhow!("voice connect: {e}"))?;

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
            video_tx,
            camera: None,
        })
    }

    /// Our camera on or off (V2); nothing while video is off.
    pub fn set_video(&mut self, on: bool) {
        if let Some(stop) = self.camera.take() {
            stop.store(true, Ordering::Relaxed);
        }
        match (&self.video_tx, on) {
            (Some(slot), true) => self.camera = Some(send_video(slot.clone(), Source::Camera)),
            (None, true) => log::info!("video: off (AFLOS_DC_VIDEO=1 turns it on)"),
            _ => {}
        }
    }

    pub fn set_mute(&mut self, mute: bool) {
        self.driver.lock().unwrap().mute(mute);
    }

    pub async fn leave(mut self) {
        self.set_video(false);
        self.mic_stop
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.driver.lock().unwrap().leave();
        let mut s = self.shared.state.lock().await;
        s.rtc_connected = false;
        s.speaking.clear();
    }
}

/// How often a camera's picture goes to Aflos at most (the app's tiles).
const TILE_EVERY: std::time::Duration = std::time::Duration::from_millis(100);

/// Decodes others' camera frames and sends them to Aflos as `meshVideo`
/// (a JPEG, at most 10 a second each); with `save` (V0), logs each and
/// saves the first ten as JPEGs there. No IDs in the log: SSRCs only.
fn video_receive(
    rx: flume::Receiver<songbird::driver::VideoFrame>,
    save: Option<std::path::PathBuf>,
    prefix: &'static str,
) {
    log::info!(
        "video: receiving{}",
        save.as_ref()
            .map(|d| format!(", saving to {}", d.display()))
            .unwrap_or_default()
    );
    std::thread::spawn(move || {
        if let Some(dir) = &save {
            let _ = std::fs::create_dir_all(dir);
        }
        let mut decoders: HashMap<u32, aflos_media::H264Decoder> = HashMap::new();
        let mut shown: HashMap<u32, std::time::Instant> = HashMap::new();
        let (mut got, mut saved) = (0u32, 0u32);
        while let Ok(f) = rx.recv() {
            got += 1;
            let decoder = match decoders.entry(f.ssrc) {
                std::collections::hash_map::Entry::Occupied(d) => d.into_mut(),
                std::collections::hash_map::Entry::Vacant(slot) => {
                    match aflos_media::H264Decoder::new() {
                        Ok(d) => slot.insert(d),
                        Err(e) => return log::warn!("video spike: no decoder: {e}"),
                    }
                }
            };
            match decoder.decode(&f.annexb) {
                Ok(Some(frame)) => {
                    if save.is_some() {
                        log::info!(
                            "video spike: frame {got} (ssrc {}, {} bytes): {}x{}",
                            f.ssrc,
                            f.annexb.len(),
                            frame.width,
                            frame.height
                        );
                    }
                    let due = shown
                        .get(&f.ssrc)
                        .is_none_or(|at| at.elapsed() >= TILE_EVERY);
                    let picture =
                        image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba.clone());
                    if let (true, Some(p)) = (due, &picture) {
                        shown.insert(f.ssrc, std::time::Instant::now());
                        let mut jpeg = Vec::new();
                        let rgb = image::DynamicImage::ImageRgba8(p.clone()).to_rgb8();
                        if rgb
                            .write_to(
                                &mut std::io::Cursor::new(&mut jpeg),
                                image::ImageFormat::Jpeg,
                            )
                            .is_ok()
                        {
                            use base64::Engine as _;
                            let data = base64::engine::general_purpose::STANDARD.encode(&jpeg);
                            crate::emit(
                                "meshVideo",
                                &serde_json::json!({ "id": format!("{prefix}{}", f.user_id), "local": false, "jpeg": data }),
                            );
                        }
                    }
                    if let (true, Some(dir)) = (saved < 10, &save) {
                        saved += 1;
                        let path = dir.join(format!("frame-{saved:02}.jpg"));
                        match picture
                            .map(|p| image::DynamicImage::ImageRgba8(p).to_rgb8().save(&path))
                        {
                            Some(Ok(())) => log::info!("video spike: saved {}", path.display()),
                            Some(Err(e)) => log::warn!("video spike: couldn't save: {e}"),
                            None => log::warn!("video spike: a frame of the wrong size"),
                        }
                    }
                }
                Ok(None) => log::debug!(
                    "video: frame {got} (ssrc {}, {} bytes): no picture yet",
                    f.ssrc,
                    f.annexb.len()
                ),
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
    id.parse::<NonZeroU64>()
        .map_err(|_| anyhow::anyhow!("bad id {id}"))
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
