# Plan: `calls` (Mesh)

Status: **APPROVED 2026-11-23** ("go": every recommended answer, no stops
until beta). Built from [SPEC-calls.md](SPEC-calls.md). Test numbers below
are the spec's.

## Overview

Engine first, in Mesh-Node, with process tests that use a tone for the
microphone and a file for the speaker. Then the Mesh-Voice screens. No hard
stop: you asked to reach beta and fix what comes up after. The by-hand
checks (spec tests 10 and 11) go to CAPABILITY-MAP *Before beta*.

## Decisions taken while planning

- **Q1–Q6 as recommended:** `cpal` and `opus` in Mesh-Node; noise
  suppression later (not in this plan: headphones until the group's first
  evening says otherwise); a missed-call line; push-to-talk only while Mesh
  is focused (not in this plan: voice activity first); no group-DM calls;
  32 kbit/s.
- **Audio is a default-on `audio` feature of `mesh-node`.** The seed's
  Docker image builds without it (no ALSA, no CMake there), and a helper
  without it answers `callStart` and `voiceJoin` with `unavailable`. CI
  installs `libasound2-dev`.
- **The missed-call line is an ordinary text line** ("📞 Missed call",
  "📞 Call, 3:12"), written by the caller. It goes by mail like any line, and
  needs no new dm event body (spec Q3's "new body" not taken: same result,
  no format change). The caller writes it; a declined or unreachable call
  writes it too.
- **Control messages go one per stream** on the `call` service, as typing
  does, not on one long stream (spec C2). An invite is answered with one
  frame, so a caller can tell "rings" from "old engine" (a reset stream, test
  7) from "nobody there".
- **A DM call rings the friend's connected devices.** Friends' connections
  stay open (social), so those are the online ones; with none, one dial is
  tried first.
- **Who's in a voice channel** goes from each participant to every Space
  member device it's connected to, on join, on leave and every 30 s.
  Participants answer a newcomer's join with their own state, so the joiner
  learns the list at once. A viewer who joins nothing waits for the next
  30 s round (ponytail note).
- **Voice channels are a new Space change, `VoiceChannelCreate`**, added at
  the end of the enum, so older events still read. An engine without it
  refuses the change (it can't read it), and shows no channel.

## Tasks

### Phase 1: the engine (Mesh-Node)

- [ ] **Task 1: Datagrams in transport.** `Node::send_datagram(device,
  bytes)` and `Node::next_datagram()`, read from every connection.
  Accept: a unit test sends one each way.
- [ ] **Task 2: The media pipeline** (`media.rs`, feature `audio`).
  Capture (cpal, or a tone with `--tone <hz>`), Opus at 32 kbit/s with FEC,
  a voice gate, datagrams `[1][call id 8][seq 2][opus]`, a jitter buffer per
  speaker (60 ms, up to 200 ms, Opus concealment for a gap), the mix, and
  playback (cpal, or `--heard <file>`). `--drop <percent>` drops incoming
  datagrams for test 5. Accept: unit tests for the buffer and the mix.
- [ ] **Task 3: DM calls.** `callStart`, `callAnswer`, `callDecline`,
  `callHangup`, `callMute`, `callDeafen`, `callDevices`; `meshCall` events;
  ring timeout 45 s, answered elsewhere, busy, the missed-call line, and
  peers kept used every 5 s. Accept: `tests/calls.rs` tests 1, 2, 7.
- [ ] **Task 4: Voice channels.** `VoiceChannelCreate`, `voice` on a
  channel in `meshSpaces`, `voiceJoin` and `voiceLeave`, `meshVoice`, full
  at 10, a kicked member dropped. Accept: tests 3 (at 4 members; 10 under
  `--ignored`), 4, 5, 8.
- [ ] **Task 5: Builds.** CI's ALSA package, the seed's Dockerfile without
  `audio`, Mesh-Voice's installers unchanged (they build the default).

### Phase 2: Mesh-Voice

- [ ] **Task 6: DM calls in the app.** A call button on a Mesh DM; the
  existing ringing, in-call, mute and deafen screens drive the engine's
  commands; the microphone and speaker picks go to `callDevices`.
- [ ] **Task 7: Voice channels in the app.** Voice channels in the Space
  pane with who's in them, join and leave, "Add voice channel" in Settings.
- [ ] **Task 8: The checkpoint test's calls step.** A calls B through the
  app, B answers, both hear the other's tone; a voice channel with three.

**Before beta (by hand):** spec tests 10 and 11, and test 6 (relay only)
and 9 (measurements), added to CAPABILITY-MAP.
