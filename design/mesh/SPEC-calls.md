# Spec: `calls` (Mesh)

Status: **APPROVED 2026-11-23** with every recommended answer (Q1–Q6); see [PLAN-calls.md](PLAN-calls.md) for the choices made while planning. Module 10 of the build order
in [CAPABILITY-MAP.md](CAPABILITY-MAP.md). It depends on
[`transport`](SPEC-transport.md), [`routing`](SPEC-routing.md),
[`social`](SPEC-social.md), [`dm`](SPEC-dm.md) and
[`spaces`](SPEC-spaces.md), all built. Spec only: no plan or code until you
approve it.

## Objective

Version 1 replaces Discord on PC for a group of friends. Text is built; voice
isn't. `calls` adds the smallest voice that does that job:

- **Voice calls in a 1:1 DM:** ring, answer or decline, talk, hang up.
- **Voice channels in a Space:** a channel you join and leave, with everyone
  in it heard at once, **up to 10 people**. Members see who's in it.
- **Mute, deafen, and a choice of microphone and speakers**, as the app
  already has for Discord voice.
- **Nothing to set up.** It works on the same connections chat uses, through
  the same relays when a direct path fails. Your friend installs and talks.

### Not in this version

- Video and screen share (`mesh-media` has capture and H.264, which a later
  slice reuses).
- Calls in group DMs (Q5).
- More than 10 in one voice channel, and stage channels (admins speak, the
  rest listen).
- Push-to-talk that works while Mesh isn't focused (Q4).
- Echo cancellation and noise suppression, unless you approve a dependency
  (Q2). Headphones are recommended until then.
- Recording, and calls to people who aren't friends or Space members.

## What exists, and what must be built

Read from the code, 2026-11-16:

| Piece | Today | For `calls` |
|---|---|---|
| Capture and playback | `mesh-core/src/audio.rs`: `cpal` input and output on their own threads, resampled to **48 kHz mono, 20 ms frames (960 samples)**; device names for pickers; a time-stretch. No echo cancellation, noise suppression or gain control. | Reused as is (C4). |
| Opus | `mesh-core` uses `opus` 0.3 (bundled Opus through `audiopus_sys`, which already links the static runtime in the Windows setup). | Reused (C4). |
| Calls in the app | One active call at a time (`CallUi`: ringing out, ringing in, connecting, in call), mute and deafen carried into a call, input and output device pickers. Discord voice channels already show in the guild list with their members. | The same screens for Mesh DMs and Space voice channels (C9). |
| 1:1 calls | Matrix only: `m.call.*` signalling and `webrtc` 0.17 with the homeserver's TURN. | Not reused: no homeserver, and Mesh already has authenticated, NAT-traversing connections (C2). |
| `mesh-media` | Video only: camera and screen capture, H.264 in and out. | Not used in this version. |
| Transport | iroh 1.4.1 connections between devices, authenticated by device key (handshake), each stream tagged with a service name. **No datagram API yet**, though iroh's connections have `send_datagram`/`read_datagram`. Caps: 256 connections, 32 streams on one connection. | A datagram API, and a `call` service (C2, C3). |
| Idle connections | A Space peer's connection closes `PEER_IDLE` (60 s) after it was last used. | A call marks its peers used (C3). |
| e2ee | Olm per device, used for every chat and Space event. | Not needed for media (C6); used for signalling where it goes by mail (C7). |

## Decisions

| # | Decision | Why |
|---|---|---|
| C1 | **Full mesh, no forwarder.** Everyone in a call sends their own voice straight to every other participant's device. A voice channel holds at most 10 people (`CALL_MAX`). | Voice is small. At 32 kbit/s Opus in 20 ms frames, a stream is about **50 kbit/s on the wire** (80 B of audio plus about 40 B of QUIC, nonce and headers, 50 times a second). In a full channel of 10, each person sends 9 streams and receives 9: **about 450 kbit/s up and 450 kbit/s down**, well inside a home connection. One member forwarding for the rest would send 72 streams (about 3.6 Mbit/s) on one home upload, and adds a hop. A mixing forwarder would have to decrypt (C6). Full mesh needs no one to be special and nothing new at the seed or relay. The map's "relay tree of the nearest members" is for larger groups and video, later. |
| C2 | **Media goes as QUIC datagrams on the existing Mesh connection; control goes on a `call` stream.** Transport gains two calls, `send_datagram` and the datagrams received, both per connection, using iroh's datagrams. Each datagram starts with a 1-byte kind (`call media`) and an 8-byte call ID, then a 2-byte sequence number and the Opus packet. The `call` stream carries the control messages: invite, ringing, answer, decline, hang up, join, leave, mute state, and the participant list. | Datagrams don't retransmit: a late voice packet is worse than a lost one, and Opus hides a lost 20 ms. The connection is the one chat already made: the handshake has proved who's at the other end, and `social`'s allow list (friends, own devices, Space peers) already decides who may connect. No ICE, no TURN, no new port. |
| C3 | **A call keeps its connections up.** While a call is on, its peers are marked used every 5 s, so `PEER_IDLE` never closes them. The `call` stream stays open for the call's length. A full channel is at most 9 extra connections, against the 256 cap; dialling a participant uses `reach` as a send does. | Today an idle Space peer's connection closes 60 s after its last use, whatever is open on it. |
| C4 | **Audio runs in the engine (`mesh-node`), not the app.** It reuses `mesh-core`'s capture, playback and Opus code, moved or copied into the engine. The app sends commands (`callStart`, `callAnswer`, `callHangup`, `voiceJoin`, `voiceLeave`, `callMute`, `callDeafen`, `callDevices`) and gets events (`meshCall`: state, participants, who's speaking). | The engine holds the connections. Sending 50 packets a second per peer through the app's JSON-lines pipe would add latency and CPU. `mesh-dc` already does its audio in its helper, so this is the existing pattern. It adds `cpal` and `opus` to Mesh-Node (Q1). |
| C5 | **Playback: a small jitter buffer per participant, then a mix.** Each incoming stream waits in its own buffer of about 60 ms (`JITTER_TARGET`), grows to at most 200 ms on a bad path, and uses Opus's loss concealment for a missing frame. Decoded streams are summed with clipping and played. Sending uses Opus in-band FEC and DTX (silence costs nearly nothing). | The playback buffer in `audio.rs` is one queue for one remote. Ten speakers need one each, or one late path stalls everyone. |
| C6 | **Media isn't encrypted a second time.** Each pair's datagrams travel inside their own QUIC connection, which is TLS 1.3 between the two devices, keyed by the handshake. A relay forwards those QUIC packets and can't read them. There are no forwarders (C1), so nothing between two participants ever holds audio. | SFrame (the map's choice) exists so a forwarder can route media it can't read. Without forwarders, it adds a second key system, a key share to every participant, and a rekey on every join and leave, for no gain. A later forwarder or relay tree brings it back. |
| C7 | **A DM call rings all of the friend's online devices.** The caller opens a `call` stream to each connected or reachable device of the friend and sends an invite. Each rings for up to 45 s (`RING_TIMEOUT`). The first device to answer takes the call; the caller tells the others it was answered elsewhere, and they stop ringing. Your own other devices ring too when you're called, and stop when one answers. If no device of the friend can be reached, the caller sees "not reachable" after the dial. **A missed call** (rung out, declined or unreachable) leaves a line in the DM: a new dm event body, `Call`, with its outcome and length, which goes by mail like any line (Q3). | It's how a phone and Discord behave. The missed-call line is the only part that must reach someone who's away, so it uses the path that already does. |
| C8 | **A voice channel is a Space channel of a new kind.** The owner and admins create, rename and delete it like a text channel: a change on the Space's chain (S7). Any member may join; a member who's kicked or banned is dropped from it at once, because their connections stop passing the handshake. **Who's in it** is sent by each participant to the online devices of every Space member it's connected to, on join, on leave and every 30 s (`VOICE_STATE_EVERY`), like typing (dm D10); nothing about it is stored or mailed. A member who opens the Space asks any connected member for the current list. Joining dials every participant on that list; a full channel (10) refuses the 11th with "full". | Members see who's talking before they join, as in Discord. Keeping the state live-only means nothing about calls lands in history or at the seed. |
| C9 | **The app's call screens are reused.** A Mesh DM gets a call button and the existing ringing, in-call, mute and deafen controls. A Space's voice channels show in the channel list with their participants, as Discord's do now. The microphone and speaker pickers in settings apply to Mesh calls too. | It's what's built, and it looks the same to someone moving from Discord. |
| C10 | **Voice activity by default, with a gate.** Mesh sends while your level is above a gate that adapts to the room's noise, and marks you speaking for others. Push-to-talk is an option in settings, while Mesh is focused (Q4). | It needs nothing set up. |
| C11 | **Mixed versions fail plainly.** An engine without `calls` refuses the `call` stream's unknown tag, so the caller sees "they need to update Mesh" and nothing rings. An engine without voice channels shows nothing for that channel (an unknown channel kind is skipped, as S12's *Mixed versions* does for newer layouts). | Nobody gets half a call. |

## What others see

- **The seed:** nothing new. A call uses the address lookups a chat already
  makes, inside the same budget (S14).
- **A relay operator** (when two participants can't punch through): QUIC
  packets between two devices at voice rates, and when they start and stop.
  Not who the people are, not the audio. A relay-only member in a full
  channel costs the relay about 450 kbit/s each way, against its 2 MB/s
  (16 Mbit/s) cap per client. For the home relay that's home upload: ten
  members all relay-only would be about 4.5 Mbit/s through it. Each member's
  home relay is the nearest one that answers, ours or n0's, so not all of
  them land on ours.
- **Space members who aren't in the channel:** who's in it, while they're
  online. Not what's said.

## Limits

| Name | Value | Why |
|---|---|---|
| `CALL_MAX` | 10 in a voice channel | C1: about 450 kbit/s each way at 10 |
| DM call | 2 people (one device each, in the call) | C7 |
| Opus | 32 kbit/s mono, 20 ms frames, FEC and DTX on | C1, C5 |
| `JITTER_TARGET` | 60 ms, up to 200 ms | C5 |
| `RING_TIMEOUT` | 45 s | C7 |
| `VOICE_STATE_EVERY` | 30 s | C8 |
| Calls at once | 1 per device (a DM call or one voice channel) | The app's single-call model; starting another hangs up the first |

## Acceptance tests

Process tests in Mesh-Node (`tests/calls.rs`) run helpers with a test flag
that reads the microphone from a tone file and writes what's played to a
file, so a test can check what each side heard. The last two are by hand.

1. **A DM call.** A calls B. Both of B's devices ring. B's laptop answers; B's
   desk stops ringing within 1 s. A tone from each side reaches the other.
   A hangs up; both see the call end, and the chat shows a call line with its
   length.
2. **Declined, missed and unreachable.** B declines: A sees "declined". B
   doesn't answer: after 45 s both see "missed". B is locked: A sees "not
   reachable", and B gets the missed-call line by mail on unlock.
3. **A voice channel of 10.** O makes a voice channel. Ten members join. Each
   hears a different tone from each of the other nine, and every member
   outside it sees the ten. The eleventh gets "full".
4. **Leave, kick and ban.** A participant leaves: the others stop hearing it,
   and the list drops it within 1 s. An admin kicks a participant: it's
   dropped from the channel within 1 s and can't rejoin.
5. **Loss.** With 10% of datagrams dropped (a test flag), a tone is still
   heard without a gap longer than 60 ms.
6. **Relay only.** Two members held to relayed paths (a new test flag;
   `--cut` today covers only the seed and live connections) talk through the
   relay.
7. **Mixed versions.** A calls an engine without `calls`: A sees "they need to
   update Mesh", and nothing rings.
8. **Nothing identifying logged.** No ID, address or audio in stderr or at the
   seed, as the other modules check.
9. **Measurements** (`--ignored`, recorded in the plan): mouth-to-ear delay
   direct and through a relay; CPU and bytes each way per person in a
   channel of 10; the jitter buffer's size over a run.
10. **By hand, two PCs:** a DM call between your PC and your friend's, on two
    networks.
11. **By hand, the group:** a voice channel with the group, for an evening.

## Known limits

- **Without echo cancellation, speakers feed back.** Someone on laptop
  speakers is heard twice by the others. Headphones avoid it, unless Q2 adds
  echo cancellation.
- **Upload limits the channel.** About 450 kbit/s at 10 is fine for nearly
  every home, but a weak mobile hotspot may struggle.
- **Who's in a voice channel is seen only by members online and connected**
  to a participant. Someone who opens a Space with nobody connected sees an
  empty channel until they connect.
- **One call at a time per device**, as the app has now.

## Changes to approved decisions

- **CAPABILITY-MAP's calls row** says "a relay tree of the nearest members in
  groups, SFrame E2EE". This spec has no relay tree and no SFrame for version
  1 (C1, C6); both come back with video or groups over 10. The row would be
  updated when you approve this.
- **dm D1** gains one event body, `Call` (C7), if Q3 is yes.
- **spaces S1** gains a channel kind, voice (C8).
- **transport** gains datagrams (C2).

## Questions

1. **Audio in the engine (C4):** add `cpal` and `opus` to Mesh-Node (both are
   in Mesh-Voice already, and `opus` 0.3's bundled Opus already links the
   static runtime in the setup)? *Recommended: yes.*
2. **Echo cancellation and noise suppression:** add a dependency for them?
   The options are WebRTC's audio processing (C++, a second CMake build for
   the Windows static runtime) or `nnnoiseless` (pure Rust, noise only, no
   echo). *Recommended: `nnnoiseless` now for noise, and headphones
   recommended. Echo cancellation after the group's first evening shows
   whether it's needed.*
3. **A missed-call line in the chat (C7):** a new dm event body, so it reaches
   someone away by mail? *Recommended: yes.*
4. **Push-to-talk:** only while Mesh is focused (works everywhere), or also a
   global hotkey (Windows only; Wayland doesn't allow it)? *Recommended:
   focused only for version 1; voice activity is the default anyway.*
5. **Calls in group DMs** (up to 10 friends): the same as a voice channel,
   started from the group? *Recommended: not in this version; a Space voice
   channel covers a group.*
6. **Bitrate:** 32 kbit/s, or 24 to save upload? *Recommended: 32. At 10
   people the difference is about 70 kbit/s up.*
