# Plan: `profile` (Aflos)

Status: **APPROVED 2026-11-27** with [SPEC-profile](SPEC-profile.md). Code goes in `GloriousBrendon/Mesh-Node` (Tasks 1 to 5) and
Aflos (Tasks 6 to 9). Each task is one commit with its tests; the checks
are the same as for the modules before it (`cargo test`, `cargo clippy`).

## Task list

- [ ] **Task 1: The manifest** (`mesh-dm/src/profile.rs`; PR2, PR4, PR5,
  PR7). Parse `profile.toml`, the asset list, signing and checking under
  `mesh/v1/profile`, versions, every cap. **Accept:** unit tests for parsing,
  signatures, versions and each cap at and over its limit.
- [ ] **Task 2: Capped decoding** (`mesh-node/src/decode.rs`; PR8). Images
  through `image` with limits set before decoding; audio through
  `symphonia`. **Accept:** the crafted bombs in the spec become placeholders
  within 100 ms and under 64 MB.
- [ ] **Task 3: Publish and deliver** (`mesh-node/src/profile.rs`; PR1,
  PR3, PR14, PR15). `profileSet`, the version on connect, the manifest over
  the 1:1 session, assets by hash, the cache, sync between own devices,
  removal on unfriend or block. **Accept:** the process test's first four
  steps.
- [x] **Task 4: Space cards** (Mesh-Node #13) (`mesh-dm/src/space.rs`; PR12, PR13).
  `card` and `card-hide` changes in the Space log, `cardSet`, `cardHide`.
  **Accept:** unit tests, and the process test's non-friend step.
- [x] **Task 5: Top 8 consent** (Mesh-Node #14) (PR11). `top8Ask`, `top8Answer`, withdraw.
  **Accept:** the process test's Top 8 step.

**Checkpoint A (a short report, no stop):** Mesh-Node tests and clippy
green, the process test passes, test counts.

*Passed 2026-11-27* (Mesh-Node main 09a9890): `cargo test -p mesh-dm` 128
passed; `cargo test -p mesh-node` every suite green, 120 unit tests and the
process tests, among them `tests/profile.rs` (2: delivery and unfriending,
and the Top 8 ask, accept and withdraw) and `tests/spaces.rs` (28, with
`a_card_reaches_members_who_arent_friends`); clippy and rustfmt clean. CI
needed one rerun on #13 for an unrelated timeout in
`e2ee::notes_reach_every_device_even_after_a_wait`.

- [ ] **Task 6: The profile screen** (`profile_ui.rs`; PR4, PR5, PR6,
  PR9, PR10). Theme, background, avatar, banner, then the blocks, drawn
  with the existing widgets; links confirm before opening; the song's play
  button.
- [ ] **Task 7: The editor** (PR16). Rice settings aimed at the profile,
  Follow my rice, block order, live preview, Publish. Sender-side
  re-encoding to fit the caps, with a clear message when it can't.
- [ ] **Task 8: Cards and avatars across the app.** Friends' avatars in the
  sidebar, chats, calls and member lists; Space cards in Spaces; the
  owner's hide controls; Autoplay profile songs in Settings.
- [ ] **Task 9: Release.** CHANGELOG, README ("What works today"), version
  bump.

**Checkpoint B (by hand, stop for your look):** the checks in the spec's
Testing section, on Linux and Windows.
