# Plan: `audio` (Aflos)

Status: **APPROVED 2026-11-26**. Built from
[SPEC-audio.md](SPEC-audio.md) (approved 2026-11-26). Ships as **0.5.2**
(small, compatible: the 0.5.x rule). The hotfix part (gate minimum, input
volume, `callGain`) went out in 0.5.1.

## The spike (done 2026-11-26)

sonora 0.3.0 at 48 kHz mono, release build, this PC: echo cancellation
cut a 40 ms half-level echo by **54.5 dB** (the bar was 20); AEC3 + AGC2 +
high-pass **74 µs per 10 ms frame, 0.7% of one core** (the budget was 5%);
`cargo check` for Windows passes with no C++ toolchain. AGC2 lifted a
synthetic speech-shaped noise only 14 dB (−61 to −47 dBFS): its voice
detector doesn't take noise for speech, so Task 3 measures it on a real
recording.

## Decisions taken while planning

- **One `AudioProcessing` per call**, owned by the send task. The mix task
  hands it what it played (the far end) over a channel, frame by frame;
  the send task feeds those to `process_render` before each capture frame.
  sonora works on 10 ms; our frames are 20 ms, so each is two halves.
- **The stream delay** sonora needs is our playback queue's length at the
  time (we know it: `PLAYBACK_MAX` bounds it), plus a fixed 20 ms for the
  device. AEC3 tolerates error in it; the test checks a wrong estimate by
  ±40 ms still cancels.
- **The chain's order is SPEC-audio A1's:** high-pass and echo
  cancellation (sonora), RNNoise, AGC2 (sonora, a second pass with only
  the gain controller on, so RNNoise sits between them), input volume,
  gate.
- **Settings reach `mesh-node` as one command**, `callAudio <json>`
  (`agc`, `ns`, `aec`, `sensitivity`: `auto` or a dBFS number, `output`
  percent), applied live; `callGain` stays for 0.5.1's slider.
- **The meter and the mic test run without a call:** `audioTest on|off`
  opens the mic (and, for "hear yourself", the speakers) through the same
  chain, and `meshAudioLevel {level, open}` comes about 15 times a second
  while a call or a test runs. Nothing is sent anywhere.
- **Per-person volume** scales each speaker's frames in the mix before
  they're summed; `callVolume <device or id> <percent>`; Aflos keeps it
  per ID in `config.toml` and sends it at each call's start.
- **Tests replace the mic with a tone** (`--tone`) as the calls tests do,
  with a new `--tone-level <dBFS>` for quiet ones, and an `--echo
  <ms>,<level>` test flag that feeds what we play back into the mic.

## Tasks

### Phase 1: the engine (Mesh-Node)

- [ ] **Task 1: sonora in the pipeline.** Echo cancellation and high-pass
  through one `AudioProcessing`, the far end from the mix, 20 ms frames
  as two 10 ms halves. **Accept:** a unit test on recorded frames: a
  played signal coming back 40 ms late at half level is cut by at least
  20 dB, and still by 15 dB with the delay estimate 40 ms off.
- [ ] **Task 2: the order and the switches.** RNNoise between them, AGC2
  after, each switchable; `callAudio`. **Accept:** unit tests: each
  switch off leaves its stage out; a call's settings change mid-call.
- [ ] **Task 3: automatic gain and sensitivity.** AGC2 adaptive on by
  default; Automatic sensitivity (floor ×3, `GATE_MIN`) or a manual dBFS
  threshold. **Accept:** a recorded quiet voice (−50 dBFS, a short WAV in
  the repo, CC0) comes out within 10 dB of −20 dBFS after 5 s; a manual
  threshold above the voice keeps it out.
- [ ] **Task 4: level reports and the mic test.** `meshAudioLevel`,
  `audioTest`. **Accept:** process test: with a tone, levels arrive about
  15 a second and say open; with `audioTest off`, they stop.
- [ ] **Task 5: per-person volume and output volume.** **Accept:** unit
  test of the mix (one speaker at 50% is half, the other unchanged);
  process test: `callVolume` to 0% plays silence for that person only.
- [ ] **Task 6: process tests.** A quiet tone (`--tone-level -50`) is
  heard on the other side with automatic gain on, and isn't with it off
  (the 0.5.0 bug, end to end); `--echo 40,0.5` is cancelled on the far
  side. **Accept:** both pass in CI on Linux and Windows.

**Checkpoint A (short report):** all Mesh-Node tests and clippy green; CPU
per call measured against 0.5.1.

### Phase 2: Aflos (Mesh-Voice)

- [ ] **Task 7: Voice & audio.** Input volume (0.5.1's), Automatic gain,
  Noise suppression, Echo cancellation, Input sensitivity (Automatic, or
  a slider over the live meter), Output volume, **Test my mic** (meter
  and hear yourself); all in `config.toml`, sent with `callAudio` at
  start and on change.
- [ ] **Task 8: per-person volume.** A slider on each person in the call
  bar and in a voice channel's member list; kept per ID.
- [ ] **Task 9: the checkpoint test.** A step: the meter moves with a
  tone, a switch reaches `mesh-node`, a person's volume is kept.

**Checkpoint B (by hand, added to CAPABILITY-MAP *Before beta*):** a call
on laptop speakers (no echo back), your friend's quiet mic heard at a
normal level, the meter and the mic test on Linux and Windows.

## Risks

- **sonora is pre-1.0.** Pinned to 0.3.0; Task 1's tests are the guard.
  If it misbehaves on Windows in CI, the C++ `webrtc-audio-processing` is
  the fallback (SPEC-audio: ask first).
- **A real voice test file** has to be CC0 or recorded by you; Task 3
  needs one short WAV.
- **Delay estimation on Windows devices** may differ (WASAPI buffers);
  Checkpoint B checks it by hand.
