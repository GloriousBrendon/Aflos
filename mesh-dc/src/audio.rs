//! cpal mic/speaker for Discord voice. The mic is a blocking `MediaSource` of
//! mono f32 PCM that songbird wraps in a `RawAdapter` (it resamples to 48 kHz
//! itself); the speaker is fed the mono i16 songbird decodes per 20 ms tick.
//! Same cpal shape as `mesh-wa/src/audio.rs` (MIT, (c) João Lucas); kept
//! separate because the sample formats differ.

use std::collections::VecDeque;
use std::io::{Read, Result as IoResult, Seek, SeekFrom};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{I24, Sample, SampleFormat, U24};
use log::error;
use ringbuf::HeapRb;
use ringbuf::traits::{Consumer as _, Producer as _, Split as _};
use songbird::input::core::io::MediaSource;

/// A live microphone: cpal fills the ring buffer; `Mic::gate` reads it.
pub struct Mic {
    cons: std::sync::Mutex<ringbuf::HeapCons<f32>>,
    alive: Arc<AtomicBool>,
    pub rate: u32,
}

/// Opens the default mic, at its native sample rate (songbird's
/// `RawAdapter::new(talk, rate, 1)` resamples).
pub fn open_mic() -> Result<Mic> {
    let device = cpal::default_host().default_input_device().ok_or_else(|| anyhow!("no microphone"))?;
    let supported = device.default_input_config().map_err(|e| anyhow!("mic config: {e}"))?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let rate = config.sample_rate;
    let channels = config.channels as usize;
    let (prod, cons) = HeapRb::<f32>::new(rate as usize).split();
    let alive = Arc::new(AtomicBool::new(true));
    {
        let alive = alive.clone();
        std::thread::spawn(move || {
            match build_input(&device, &config, format, channels, prod) {
                Ok(stream) => match stream.play() {
                    Ok(()) => while alive.load(Ordering::Relaxed) {
                        std::thread::park_timeout(Duration::from_millis(250));
                    },
                    Err(e) => error!("mic play: {e}"),
                },
                Err(e) => error!("mic build: {e}"),
            }
            alive.store(false, Ordering::Relaxed);
        });
    }
    Ok(Mic { cons: std::sync::Mutex::new(cons), alive, rate })
}

impl Drop for Mic {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Relaxed);
    }
}

/// Voice activity on the mic. Discord's AFK timer counts the "speaking"
/// signal, and songbird sends it only when its mixer starts or stops having
/// tracks: one mic track that never ends says "speaking" once, at join, and
/// the timer moves you to AFK however much you talk. So each burst of speech
/// is its own track (`Talk`), ended after a second of quiet.
#[derive(Debug)]
struct Gate {
    floor: f32,
    open_until: Option<std::time::Instant>,
}

/// How long the gate stays open after the last loud bit.
const HANGOVER: Duration = Duration::from_millis(1000);
/// Kept from just before speech starts, so its first sound isn't cut.
const PREROLL: Duration = Duration::from_millis(150);

impl Default for Gate {
    fn default() -> Self {
        Self {
            floor: 0.003,
            open_until: None,
        }
    }
}

impl Gate {
    /// Whether a block at `level` (RMS, -1..1 samples) is speech, or within
    /// the hangover after it. The floor follows quiet at once, noise slowly.
    fn pass(&mut self, level: f32, now: std::time::Instant) -> bool {
        self.floor = if level < self.floor {
            level.max(0.0005)
        } else {
            self.floor * 1.002
        };
        if level > (self.floor * 3.0).max(0.01) {
            self.open_until = Some(now + HANGOVER);
        }
        self.open_until.is_some_and(|until| now < until)
    }
}

fn rms(block: &[f32]) -> f32 {
    (block.iter().map(|s| s * s).sum::<f32>() / block.len().max(1) as f32).sqrt()
}

/// One burst of speech as songbird's blocking reader of mono f32 samples: it
/// ends (EOF) once the gate has closed and everything queued is read.
pub struct Talk {
    shared: Arc<TalkShared>,
    scratch: Vec<u8>,
}

struct TalkShared {
    queue: std::sync::Mutex<VecDeque<f32>>,
    talking: AtomicBool,
}

impl Mic {
    /// Runs the gate on its own thread until `stop`: each time speech starts,
    /// `on_talk` gets a new `Talk` to play (with the pre-roll in it).
    pub fn gate(self, stop: Arc<AtomicBool>, on_talk: impl Fn(Talk) + Send + 'static) {
        std::thread::spawn(move || {
            let block = (self.rate / 100).max(1) as usize;
            let keep = (self.rate as u128 * PREROLL.as_millis() / 1000) as usize;
            let mut gate = Gate::default();
            let (mut pending, mut preroll) = (Vec::new(), VecDeque::new());
            let mut current: Option<Arc<TalkShared>> = None;
            let mut popped = vec![0f32; self.rate as usize / 10];
            while !stop.load(Ordering::Relaxed) && self.alive.load(Ordering::Relaxed) {
                let n = self.cons.lock().unwrap().pop_slice(&mut popped);
                pending.extend_from_slice(&popped[..n]);
                if n == 0 {
                    std::thread::sleep(Duration::from_millis(5));
                }
                while pending.len() >= block {
                    let chunk: Vec<f32> = pending.drain(..block).collect();
                    let open = gate.pass(rms(&chunk), std::time::Instant::now());
                    match (&current, open) {
                        (Some(talk), true) => talk.queue.lock().unwrap().extend(chunk),
                        (Some(talk), false) => {
                            talk.talking.store(false, Ordering::Relaxed);
                            current = None;
                            preroll.extend(chunk);
                        }
                        (None, true) => {
                            preroll.extend(chunk);
                            let shared = Arc::new(TalkShared {
                                queue: std::sync::Mutex::new(std::mem::take(&mut preroll)),
                                talking: AtomicBool::new(true),
                            });
                            on_talk(Talk {
                                shared: shared.clone(),
                                scratch: Vec::new(),
                            });
                            current = Some(shared);
                        }
                        (None, false) => preroll.extend(chunk),
                    }
                    let over = preroll.len().saturating_sub(keep);
                    preroll.drain(..over);
                }
            }
            if let Some(talk) = current {
                talk.talking.store(false, Ordering::Relaxed);
            }
        });
    }
}

impl Read for Talk {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        // Block (briefly) until there's a sample, so songbird's blocking
        // input thread never busy-spins.
        let want = buf.len() / 4;
        if want == 0 {
            return Ok(0);
        }
        let mut got: Vec<f32> = Vec::new();
        for _ in 0..200 {
            {
                let mut q = self.shared.queue.lock().unwrap();
                let n = want.min(q.len());
                got.extend(q.drain(..n));
            }
            if !got.is_empty() {
                break;
            }
            if !self.shared.talking.load(Ordering::Relaxed) {
                return Ok(0); // Over: the track ends, and so does "speaking".
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if got.is_empty() {
            got = vec![0.0; want.min(480)];
        }
        self.scratch.clear();
        for s in &got {
            self.scratch.extend_from_slice(&s.to_le_bytes());
        }
        let len = self.scratch.len().min(buf.len());
        buf[..len].copy_from_slice(&self.scratch[..len]);
        Ok(len)
    }
}

impl Seek for Talk {
    fn seek(&mut self, _: SeekFrom) -> IoResult<u64> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
}

impl MediaSource for Talk {
    fn is_seekable(&self) -> bool {
        false
    }
    fn byte_len(&self) -> Option<u64> {
        None
    }
}

fn build_input<P>(device: &cpal::Device, config: &cpal::StreamConfig, format: SampleFormat, channels: usize, mut prod: P) -> Result<cpal::Stream>
where
    P: ringbuf::traits::Producer<Item = f32> + Send + 'static,
{
    macro_rules! build {
        ($t:ty) => {{
            let mut mono: Vec<f32> = Vec::with_capacity(2048);
            device
                .build_input_stream(
                    config.clone(),
                    move |data: &[$t], _| {
                        mono.clear();
                        for frame in data.chunks(channels) {
                            let sum: f32 = frame.iter().map(|&s| f32::from_sample(s)).sum();
                            mono.push(sum / channels as f32);
                        }
                        let _ = prod.push_slice(&mono);
                    },
                    |e| error!("mic stream error: {e}"),
                    None,
                )
                .map_err(|e| anyhow!("build input: {e}"))
        }};
    }
    match format {
        SampleFormat::I8 => build!(i8),
        SampleFormat::I16 => build!(i16),
        SampleFormat::I32 => build!(i32),
        SampleFormat::U8 => build!(u8),
        SampleFormat::U16 => build!(u16),
        SampleFormat::U32 => build!(u32),
        SampleFormat::I24 => build!(I24),
        SampleFormat::U24 => build!(U24),
        SampleFormat::F32 => build!(f32),
        SampleFormat::F64 => build!(f64),
        other => Err(anyhow!("unsupported input format {other:?}")),
    }
}

const VOICE_RATE: u32 = 48_000;

/// The default speaker, fed 48 kHz mono i16 frames.
pub fn open_speaker() -> Result<async_channel::Sender<Vec<i16>>> {
    let device = cpal::default_host().default_output_device().ok_or_else(|| anyhow!("no speaker"))?;
    let supported = device.default_output_config().map_err(|e| anyhow!("speaker config: {e}"))?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let dst_rate = config.sample_rate;
    let channels = config.channels as usize;
    let (mut prod, cons) = HeapRb::<i16>::new(dst_rate as usize).split();
    let _ = prod.push_slice(&vec![0i16; dst_rate as usize / 10]); // 100 ms pre-roll

    let (tx, rx) = async_channel::bounded::<Vec<i16>>(64);
    let done = Arc::new(AtomicBool::new(false));
    {
        let done = done.clone();
        tokio::spawn(async move {
            let mut pos = 0.0f64;
            let mut out: Vec<i16> = Vec::with_capacity(4096);
            while let Ok(frame) = rx.recv().await {
                out.clear();
                resample(&frame, VOICE_RATE, dst_rate, &mut pos, &mut out);
                let _ = prod.push_slice(&out);
            }
            done.store(true, Ordering::Relaxed);
        });
    }
    std::thread::spawn(move || {
        let stream = match build_output(&device, &config, format, channels, cons) {
            Ok(s) => s,
            Err(e) => return error!("speaker build: {e}"),
        };
        if let Err(e) = stream.play() {
            return error!("speaker play: {e}");
        }
        while !done.load(Ordering::Relaxed) {
            std::thread::park_timeout(Duration::from_millis(250));
        }
    });
    Ok(tx)
}

/// Linear resample carrying the fractional cursor across blocks.
fn resample(src: &[i16], src_rate: u32, dst_rate: u32, pos: &mut f64, out: &mut Vec<i16>) {
    if src.is_empty() || src_rate == dst_rate {
        out.extend_from_slice(src);
        return;
    }
    let step = src_rate as f64 / dst_rate as f64;
    let mut p = *pos;
    while p < src.len() as f64 {
        let i = p as usize;
        let frac = p - i as f64;
        let a = src[i] as f64;
        let b = if i + 1 < src.len() { src[i + 1] as f64 } else { a };
        out.push((a + (b - a) * frac).round() as i16);
        p += step;
    }
    *pos = p - src.len() as f64;
}

fn build_output<C>(device: &cpal::Device, config: &cpal::StreamConfig, format: SampleFormat, channels: usize, mut cons: C) -> Result<cpal::Stream>
where
    C: ringbuf::traits::Consumer<Item = i16> + Send + 'static,
{
    macro_rules! build {
        ($t:ty) => {{
            device
                .build_output_stream(
                    config.clone(),
                    move |data: &mut [$t], _| {
                        for frame in data.chunks_mut(channels) {
                            let v = <$t>::from_sample(cons.try_pop().unwrap_or(0));
                            frame.fill(v);
                        }
                    },
                    |e| error!("speaker stream error: {e}"),
                    None,
                )
                .map_err(|e| anyhow!("build output: {e}"))
        }};
    }
    match format {
        SampleFormat::I8 => build!(i8),
        SampleFormat::I16 => build!(i16),
        SampleFormat::I32 => build!(i32),
        SampleFormat::U8 => build!(u8),
        SampleFormat::U16 => build!(u16),
        SampleFormat::U32 => build!(u32),
        SampleFormat::I24 => build!(I24),
        SampleFormat::U24 => build!(U24),
        SampleFormat::F32 => build!(f32),
        SampleFormat::F64 => build!(f64),
        other => Err(anyhow!("unsupported output format {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_halves_and_doubles() {
        let mut pos = 0.0;
        let mut out = Vec::new();
        resample(&[0, 100, 200, 300], 48_000, 24_000, &mut pos, &mut out);
        assert_eq!(out, vec![0, 200]); // every other sample
        out.clear();
        pos = 0.0;
        resample(&[0, 0, 0], 48_000, 48_000, &mut pos, &mut out);
        assert_eq!(out.len(), 3); // pass-through at equal rate
    }

    /// Live check: speak while it runs. `cargo test mic_hears -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn mic_hears() {
        let mic = open_mic().unwrap();
        let rate = mic.rate;
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel();
        mic.gate(stop.clone(), move |t| {
            let _ = tx.send(t);
        });
        let mut talk = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("say something");
        let mut buf = [0u8; 3840];
        let (mut sum, mut n) = (0f64, 0usize);
        // Talk, then stay quiet: the track ends a second after.
        loop {
            let len = talk.read(&mut buf).unwrap();
            if len == 0 {
                break;
            }
            for c in buf[..len].chunks_exact(4) {
                let s = f32::from_le_bytes(c.try_into().unwrap()) as f64;
                sum += s * s;
                n += 1;
            }
        }
        stop.store(true, Ordering::Relaxed);
        let rms = (sum / n.max(1) as f64).sqrt();
        println!("rate {rate} samples {n} rms {rms:.5}");
        assert!(rms > 0.001, "mic is silent");
    }

    /// Speech opens the gate; it stays open a second after, then closes;
    /// steady room noise doesn't open it.
    #[test]
    fn the_gate_opens_on_speech() {
        let mut g = Gate::default();
        let t = std::time::Instant::now();
        let at = |ms| t + Duration::from_millis(ms);
        assert!(!g.pass(0.002, t));
        assert!(g.pass(0.2, at(10)));
        assert!(g.pass(0.002, at(900)), "the hangover");
        assert!(!g.pass(0.002, at(1100)));
        let mut hum = Gate::default();
        for ms in 0..200 {
            hum.pass(0.004, at(ms * 10));
        }
        assert!(!hum.pass(0.005, at(2010)), "steady noise stays out");
    }

    /// A talk track gives what's queued, then ends once speech is over:
    /// that end is what makes songbird say "not speaking".
    #[test]
    fn a_talk_track_ends_after_speech() {
        let shared = Arc::new(TalkShared {
            queue: std::sync::Mutex::new(VecDeque::from(vec![0.5f32; 10])),
            talking: AtomicBool::new(true),
        });
        let mut talk = Talk {
            shared: shared.clone(),
            scratch: Vec::new(),
        };
        let mut buf = [0u8; 4 * 16];
        assert_eq!(talk.read(&mut buf).unwrap(), 40, "the ten queued");
        assert_eq!(f32::from_le_bytes(buf[..4].try_into().unwrap()), 0.5);
        shared.talking.store(false, Ordering::Relaxed);
        assert_eq!(talk.read(&mut buf).unwrap(), 0, "over: EOF");
    }
}
