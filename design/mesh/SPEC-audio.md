# Spec: `audio` (Aflos)

Status: **APPROVED 2026-11-26** ("go with your recommendations": Q1 to Q4
as recommended). The 0.5.1 hotfix (Q1) is built; PLAN-audio for the rest
(0.5.x) to write. Depends
on [`calls`](SPEC-calls.md) as built in Mesh-Node v0.5.0.

## Objective

People must be heard, on whatever microphone they have, without tuning
anything. A friend's quiet mic was cut off in a 0.5.0 call. Today the
call pipeline (`mesh-node/src/media.rs`) is: RNNoise noise suppression
(always on), then a voice gate that opens only above a fixed level (RMS
500, about −36 dBFS) and three times the room's noise floor. There's no
gain, no automatic gain, no echo cancellation, no way to see your level,
and no way to change any of it.

**User stories**

- My mic is quiet: I'm still heard at a normal level, with nothing to set.
- My mic is too hot or my room is loud: I can turn my input down, or set
  how loud I must be before I'm sent.
- I'm on laptop speakers: the other side doesn't hear themselves back.
- Before a call, I can see my level move and check I'm being picked up.
- One friend is too loud or too quiet for me: I set their volume, for me
  only.

## Decisions (review these)

| # | Decision | Why |
|---|---|---|
| A1 | **The chain, every 10 ms frame:** high-pass filter → echo cancellation (the far end is what we play) → noise suppression → automatic gain → your input volume → the voice gate → Opus. | The order WebRTC uses: gain after noise suppression so it doesn't lift the noise, the gate last so it sees the level that's actually sent. |
| A2 | **Echo cancellation, automatic gain and the high-pass filter come from [`sonora`](https://github.com/dignifiedquire/sonora)** (pure Rust port of WebRTC's audio processing: AEC3, AGC2). Noise suppression stays **RNNoise** (`nnnoiseless`), which is in and tested. A spike first (below). | Pure Rust builds on Windows with no C++ toolchain, as everything else here. AEC3 and AGC2 are what browsers and Discord-like apps use. sonora is 0.2.0 (pre-1.0), hence the spike. |
| A3 | **Settings, in Voice & audio, all on by default except the manual ones:** *Input volume* (0 to 200%, 100% default); *Automatic gain* (on); *Noise suppression* (on); *Echo cancellation* (on); *Input sensitivity*: Automatic (default) or a manual threshold on a slider over the live level meter; *Output volume* (0 to 200%). | Everything works untouched; each piece can be turned off when it misbehaves (a good headset doesn't need echo cancellation). |
| A4 | **The gate looks at the level after gain**, and Automatic sensitivity drops the fixed RMS 500 for "clearly above your room's noise" (3× the floor, as now, with a much lower minimum). Manual sensitivity is a threshold in dBFS. | The 0.5.0 bug: a quiet mic's speech never cleared 500. With AGC before it, the gate sees speech at a normal level. |
| A5 | **A level meter and a mic test.** While Voice & audio is open or a call is live, `mesh-node` reports your input level and whether the gate is open, about 15 times a second (`meshAudioLevel`). *Test my mic* plays your processed voice back to you, half a second late, until you stop it. | You see whether you're picked up before anyone has to tell you. Hearing yourself through the whole chain is the only real check of noise suppression and gain. |
| A6 | **Per-person volume**, 0 to 200%, set from the call bar or a voice channel's member list; kept on this device by ID (`config.toml`). Only you hear the change. | The "one friend is too loud" story; Discord has it. |
| A7 | **Changes apply live**, in and out of calls (`callAudio`); settings live in `config.toml` and go to `mesh-node` at start. | No call restarts to fix a bad level. |
| A8 | **CPU budget: the whole chain under 5% of one core** at 48 kHz mono, measured in the spike on this PC and the Windows PC. | Calls already run beside games. |

## Testing strategy

- **Spike (about half a day):** sonora's AEC3 + AGC2 + HPF in the media
  pipeline on Linux and Windows; time per 10 ms frame; an echo test that
  feeds the far end into the mic with a delay and attenuation and requires
  at least 20 dB echo reduction. If sonora falls short, the fallback is
  the C++ `webrtc-audio-processing` crate (then Windows needs a C++ build),
  and I stop to ask.
- **Unit:** gain and gate on recorded frames; AGC lifts a quiet tone
  (−50 dBFS) to the target; a manual threshold holds.
- **Process (with `--tone` at low amplitude):** reproduces the 0.5.0 bug
  first (a quiet tone is never sent today), then shows it's sent with
  automatic gain on; input volume 0% sends nothing; per-person volume
  scales what's played.
- **By hand:** a call on laptop speakers (no echo heard back), the friend's
  quiet mic, the meter and the mic test on both platforms.

## Boundaries

- **Always:** defaults work untouched; nothing about audio is logged
  beyond counts; device names stay out of logs.
- **Ask first:** dropping any processing that's on by default; a C++
  dependency.
- **Never:** audio leaves the device anywhere but the call it's for (the
  mic test stays local).

## Open questions (answered 2026-11-26)

All four as recommended: a 0.5.1 hotfix first (gate minimum RMS 500 → 60,
input volume slider, `callGain`); RNNoise kept; per-person volume now;
the mic test is a meter plus "hear yourself".


1. **A 0.5.1 hotfix first?** Recommended: yes. The gate's fixed minimum
   lowered and a simple input volume slider, out this week, so quiet mics
   work now; the full chain in later 0.5.x releases.
2. **Noise suppression:** keep RNNoise (recommended: in, tested, strong on
   fans and keyboards), use sonora's (lighter, one library), or let people
   pick?
3. **Per-person volume (A6) now, or after?** Recommended: now; it's small.
4. **Mic test:** meter plus "hear yourself" (recommended), or meter only?
