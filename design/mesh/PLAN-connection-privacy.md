# Plan: `connection-privacy` (Aflos)

Status: **DRAFT 2026-11-26**, for your review. Built from
[SPEC-connection-privacy.md](SPEC-connection-privacy.md) (approved
2026-11-25). Target: **0.5.0**, with SPEC-social revision 2 (PR #9). Decision
numbers (P1 to P14) are the spec's.

## Overview

The spike comes first, because it settles P3 (two endpoints). Then the engine
in Mesh-Node, with process tests on the local relay. Then the Aflos screens
and the 18+ text. Each phase ends with a short report; the one hard stop is
Checkpoint A, and only if the spike disagrees with the spec.

## Decisions taken while planning (review these)

- **Upper layers keep one key per device.** On the direct endpoint, iroh
  authenticates the *direct* key. The hello there carries the `DeviceCert`
  **and** the `DirectCert`, and `check_hello` maps the direct key back to the
  device key. So `Peer.device` is always the device key, and e2ee sessions,
  dm, calls, files and Spaces stay as they are. `Peer` gains `path_kind:
  Relay | Direct` (which endpoint, not which path).
- **One `Net` with two `Node`s.** `helper.rs` holds `relay: Node` and
  `direct: Option<Node>` (`None` with Hide my IP on). It merges
  `next_stream`, `next_peer` and `next_datagram` from both. Senders pick the
  endpoint by purpose: `net.for_person(id)` for DMs, calls, files, typing and
  e2ee, and `net.for_space(space, id)` for Space traffic (P7, P14). About 36
  call sites in `calls`, `dm`, `files`, `social`, `spaces`, `e2ee`, `mail`,
  `media` and `routing` change from `net.node` to one of the two. `for_person`
  gives the direct node only for a current friend whose record has a direct
  key. Otherwise, and for everyone else, it gives the relay node.
- **The direct key rides in the address record, format 3.** The record is
  already signed by the device key and sealed per pair. Format 3 appends
  `direct: Option<DirectCert>` (104 bytes: key 32, issued 8, signature 64),
  well inside the 4,096-byte seed limit. Format 2 records still open, with no
  direct key. Only records sealed for a friend pair or the self pair carry it (Task 5
  checks that no Space path puts one), and own devices get it through the self
  pair's record, which already exists.
- **The direct secret is in the keystore**, next to the device key, as a new
  field that a store without it reads as "none yet". It's made at first
  unlock and replaced on rotation. Saving it uses the keystore's existing
  format bump and migration pattern (codes.bin format 2 did the same).
- **Hide my IP is an Aflos setting, passed to the helper.** It goes in
  `config.toml` as `hide_ip`, is sent as `--hide-ip` at start, and is changed
  with a `netPrivacy { hideIp }` command that restarts networking (P6). The
  helper sends no address records with a direct key while it's on.
- **A report is a message in a private `<space>:reports` chat** whose audience
  is the owner's devices only (P11). Space channels already use the dm path
  with `<space>:<channel>` IDs and a per-chat audience. So a report gets e2ee,
  mail for an offline owner, and storage on the owner's side without a new
  service. Its body is ordinary text plus quoted messages when the box is
  ticked. *(To verify in Task 9 that an audience can be narrower than the
  member list; if not, a `report` service is the fallback.)*
- **P7 is a new Space change, `DirectAllowed(bool)`**, appended at the end of
  `Change` as `VoiceChannelCreate` was. Only the owner may sign it. An older
  engine refuses it and stays relayed, which is the safe side.
- **Open questions, as recommended unless you say otherwise:**
  - Q2: use P3 (two endpoints) now. Filing the upstream iroh issue is
    outward-facing, so I'll draft it and you post it.
  - Q3: reports go to the owner only.
  - Q5: no "except N members" line. The per-call status (P8) already shows
    each person's path.
  - Q6: decided later, when there's a mobile client.

## Tasks

### Phase 0: the spike (Mesh-Node, about 3 hours)

- [ ] **Task 1: Spike, `tests/two_endpoints.rs` (ignored by default).** On
  the local relay: endpoint A is normal, A′ is relay-only, and B is a peer.
  Capture what B sees, through iroh's remote info and B's candidates (the
  `tests/candidates.rs` method). Check (a) B connected to A′ gets no
  candidates; (b) B connected to A gets them, even when A refuses it in
  `after_handshake`; (c) A and A′ on one relay with different keys don't
  evict each other.
  **Accept:** a short report of (a), (b) and (c).

**Checkpoint A (hard stop only if the spike disagrees).** If (a), (b) and (c)
all hold, P3 stands and I go on. If (b) is false, one endpoint with a reject
hook may be enough, and I stop to ask you.

### Phase 1: the engine (Mesh-Node)

- [ ] **Task 2: `DirectCert`** (`mesh-identity/src/direct.rs`): issue and check
  (signed by the device key, domain-separated, issued time). Keystore field
  and migration.
  **Accept:** unit tests for issue and check, a wrong signer, a damaged cert,
  and an old store opening with no direct key.
- [ ] **Task 3: Address record format 3** (`addr.rs`): an optional
  `DirectCert`; format 2 still opens.
  **Accept:** unit tests with and without the field, a format 2 record, and a
  cert for a different device being refused.
- [ ] **Task 4: Hello on the direct endpoint** (`handshake.rs`): it sends and
  checks `DeviceCert` + `DirectCert`, and `Peer.device` becomes the device
  key. `Allow` plus an `after_handshake` hook admit only friends and own
  devices there.
  **Accept:** unit tests: a friend is admitted with the mapped device key; a
  non-friend holding the key is refused (code 6); a `DirectCert` from another
  device is `KeyMismatch`.
- [ ] **Task 5: `Net` with two nodes** (`helper.rs`), plus `for_person` and
  `for_space`. Every `net.node` call site is moved over. Records for friends
  carry the direct key; Space peers' never do.
  **Accept:** all existing tests pass unchanged, and the spec's process test
  "a friend pair goes direct; a non-friend Space pair is relayed and neither
  holds the other's IP" passes, together with "a non-friend who dials the
  device key gets no candidates".
- [ ] **Task 6: Rotation and block** (P4, P5): rotation every 7 days and on
  unfriend or block. A `direct-moved` frame goes over current friend
  connections, then they close, and records are put again at once. Block
  closes both endpoints' connections.
  **Accept:** the spec's process tests for block (closes within 1 s; the old
  key fails), rotation (a connected friend moves; a friend offline for more
  than 7 days, with the clock moved as the mailbox tests do, reconnects
  direct), and "a non-friend with the direct key is refused".
- [ ] **Task 7: Hide my IP** (P6): `--hide-ip` and `netPrivacy`, with
  networking restarted.
  **Accept:** process test: a friend pair is relayed, and `meshNet` says so.
- [ ] **Task 8: Status** (P8): `meshNet` gains `direct` and `relayed` counts;
  open chats and calls get the peer's path (`Direct`, `Relayed`, or
  `Relayed (Hide my IP)`). No addresses, and nothing new in logs (P13).
  **Accept:** unit test of the counts; process tests assert the path.
- [ ] **Task 9: Space direct switch and reports** (P7, P11): the
  `DirectAllowed` change (owner only) and `for_space` following it. A
  `spaceReport { space, member, reason, messages? }` command writes to
  `<space>:reports`.
  **Accept:** the spec's P14 and P7 process tests (Space traffic relayed
  while DMs stay direct; with P7 on, friend members go direct and others
  stay relayed; turned off, relayed again); a report reaches the owner's
  devices and no other member's.

**Checkpoint B (short report).** All Mesh-Node tests and clippy are green, and
idle CPU and relay traffic are measured with two endpoints (P3's cost),
against 0.4.1. Mesh-Node is tagged `v0.5.0` only when Aflos 0.5.0 ships.

### Phase 2: Aflos (Mesh-Voice)

- [ ] **Task 10: Privacy settings and status.** The Hide my IP switch with
  P6's line, `config.toml` `hide_ip`; the path shown in the call bar and the
  chat header; network status counts.
- [ ] **Task 11: Space screens.** The owner's "Allow direct connections with
  members" switch in Space settings; the notice on the add-to-Space screen
  and the Space header; Report on a member's profile and on a message (with
  a "include these messages" box); the owner's Reports list in Space
  settings.
- [ ] **Task 12: 18+ (P12).** `TERMS.md`, the README, the installer's welcome
  page, and the AUR `pkgdesc`.
- [ ] **Task 13: Checkpoint test step.** `checkpoint.rs` gets a step for
  direct versus relayed and for Hide my IP, through the app's state.
  **Accept:** all tests pass, with no new clippy findings and no new rustfmt
  hunks (the Mesh-Voice rule; no whole-repo reformat).

**Checkpoint C (by hand, added to CAPABILITY-MAP *Before beta*).** Two
machines on different networks, one on mobile data: friends direct, Space
strangers relayed, the status line right on both, and a packet capture on the
stranger's side showing only the relay's address.

## Risks

- **Two relay connections per device** (P3's cost). Checkpoint B measures it.
- **A friend on 0.4.x** dials the device key and gets relayed until they
  update. That's the safe side, and the changelog says so.
- **Rotation races:** a friend dialling the old key during a rotation fails
  once, looks up the fresh record, and retries. Task 6 tests it.
- **SPEC-social revision 2 changes how friendship is made, not what a friend
  is.** This plan only needs "current friend" (social D8), so the two can be
  built in either order.
