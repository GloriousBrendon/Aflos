# Plan: `voice-quality` (Aflos)

Status: **APPROVED 2026-11-26** ("approved, start the spike"). Built from
[SPEC-voice-quality.md](SPEC-voice-quality.md) (approved 2026-11-26).
Ships as **0.5.4** (compatible: nothing on the wire changes that 0.5.1 or
0.5.2 can't decode). 0.5.2 ships first, as built.

## The spike (done 2026-11-26, Mesh-Node PR #8)

`mesh-fx` against LSP 1.2.37's Linux CLAP and VST3 builds (198 plugins
each listed). **Processing:** both formats run 960-frame blocks at
48 kHz in microseconds (mean 2-6 µs, worst under 61 µs, debug build);
LSP's compressor cuts a loud tone (0.9 amplitude) by **−6.09 dB in
both**, so the formats agree. **Editors:** the Parametric Equalizer x8
draws its full UI in both: CLAP as a floating window it manages, VST3
embedded in our X11 window (XWayland) with an `IRunLoop` on the frame
(it registered 1 fd handler and 1 timer, fired 150 and 193 times in
8 s). Plugins with sidechains run with the sidechain bus off. **Pipe:**
a 20 ms frame's round trip through a child process, median **4 µs**,
p99 7 µs, max 0.4 ms (release). **Windows:** `cargo check` passes.
Not hard stop: Checkpoint A passes.

Changed from the plan: the exact-gain check (±0.1 dB) needs the test
plugins, which are built in Task 7 with CI; the spike proved processing
with LSP instead. Found while building: the VST3 host gives no
`IMessage` (`createInstance` returns not-implemented), which LSP doesn't
need; Task 5 adds it for plugins that do.

## Changes while building

- **Task 2:** audio messages are `[seq u32][960 × i16]`, not bare frames,
  and control JSON carries an `id`: a frame that came back after its
  deadline is skipped, never taken for the next one. The protocol is one
  file (`mesh-fx/src/proto.rs`) that `mesh-node` includes by path, so
  `mesh-node` doesn't depend on `mesh-fx` or build plugin libraries.
  `mesh-fx serve` writes the protocol to a duplicate of stdout and points
  stdout at stderr, so a plugin that prints can't corrupt it (Linux; the
  Windows equivalent is in Task 11). In `mesh-node`, a writer thread
  behind a 4-message queue owns the pipe: a full queue is `Late`, like a
  missed deadline, so a stuck `mesh-fx` never blocks the call.
- **Task 3:** the scan lists each file's plugins (id, name, vendor) from
  its descriptors; latency, editor and parameters are read when a plugin
  is added to the chain, since they need an instance and one file can
  hold hundreds of plugins. `fxScan` takes JSON (`dirs`, `fresh` for
  Rescan); `CLAP_PATH` is honoured; `MESH_FX` overrides where `mesh-fx`
  is found. Accept was checked with stand-in scanners (answering,
  crashing, hanging) and by hand on LSP; the test plugins arrive in Task 7.
- **Task 4:** plugins start with the frame after `mesh-fx` reports them
  loaded; the waiting step uses `block_in_place` so a slow plugin doesn't
  hold up other tasks on that runtime thread. Removing every plugin (or
  switching them all off) stops `mesh-fx` (`meshFx` `off`). Checked by
  hand in the mic test: LSP's compressor took a loud tone from −3.9 to
  −10.0 dBFS and back once removed (automatic gain off; with it on, AGC
  settling hides the change).

## Decisions taken while planning

- **The plugin host is a new crate and binary in Mesh-Node, `mesh-fx`**,
  beside `mesh-node` (engine binaries are `mesh-*`; the spec's `aflos-fx`
  is this). The release, installer, AUR package and updater ship it next
  to `mesh-node`. It depends on `vst3` 0.3.0 and `clack-host` 0.2.0;
  `mesh-node` never links either.
- **One `mesh-fx` per call or mic test**, started lazily when the chain
  has at least one plugin on. Nothing starts for people with no plugins,
  so their calls are exactly 0.5.2's: the built-in chain is the default
  (F0), and adding a plugin never switches a built-in stage off.
- **The pipe:** `mesh-fx`'s stdin/stdout carry `[tag u8][len u32 LE]
  [payload]` messages. Audio is tag 1, 960 `i16` samples (1,920 bytes) in
  and the same back; control is tag 2, one JSON object (`chain`, `edit`,
  `params`, `param`, `save`), answered on tag 2. A dedicated thread in
  `mesh-node` owns the pipe; `Chain::step` hands it the frame and waits
  at most 20 ms (F4), else sends dry.
- **Scanning is `mesh-fx --scan <path>`**, one process per plugin file,
  10 s timeout, printing one JSON line (`name`, `vendor`, `format`,
  `latency`, `editor`, `params`), or nothing on a crash. `mesh-node`
  keeps the results in `fx/scan.json`, keyed by path + modified time, so
  only new or changed files are rescanned.
- **Plugin state** is saved by `mesh-fx` on `save` (sent when the chain
  changes, when an editor closes and at call end) to
  `<data>/fx/<sha256 of path>.state`. A state that fails to load is
  ignored and logged, never fatal.
- **Editor windows** are opened by `mesh-fx` on its own thread with its
  own event loop: X11 (XWayland on Wayland) on Linux through `x11rb`,
  Win32 on Windows. CLAP plugins that ask for a floating window get one.
  A window that fails to open falls back to `params` (F6).
- **Voice quality** is a new `callAudio` field, `quality`:
  `low|standard|high|studio` (16/32/64/128 kb/s). libopus can't change
  `application` after the first frame (`opus_encoder.c`,
  `OPUS_SET_APPLICATION_REQUEST`), so going to or from High/Studio builds
  a new encoder at the next frame; within a mode it's `set_bitrate`.
- **Discord video settings** go to `aflos-dc` as one command,
  `videoQuality <json>` (`camera`/`screen`, each `{height, fps,
  bitrate}`, `bitrate` 0 = auto), replacing `send_video`'s constants.
  `aflos-dc` reads `premium_type` from the READY user object and clamps
  to the tier (Q5); the app shows the clamped values greyed with
  "needs Nitro".
- **Test plugins** are built in CI: a CLAP gain with `clack-plugin`
  0.2.0, and a VST3 gain from `vst3-rs`'s `gain.rs` example. Each takes
  its gain and an optional per-block sleep from an env var, so the same
  plugin tests both "louder" and "too slow". No `nih-plug` (not on
  crates.io).

## Tasks

### Phase 1: the engine (Mesh-Node)

- [x] **Task 1: the spike.** `mesh-fx` loads both test plugins, processes
  960-frame blocks at 48 kHz, and opens a real plugin's editor on this
  Linux box. **Accept:** both gains measured within 0.1 dB of what they're
  set to; a block round trip through the pipe under 1 ms in release;
  `cargo check` for Windows passes. **Checkpoint A: hard stop only if a
  format can't be hosted or its editor can't open**; otherwise a short
  report and on.
- [x] **Task 2: the pipe protocol** in `mesh-fx` and its client thread in
  `mesh-node`. **Accept:** unit tests: frames round-trip in order; a
  malformed message ends `mesh-fx` cleanly; the client sees a dead
  process within 100 ms.
- [x] **Task 3: scanning.** `--scan`, `fx/scan.json` with its cache, the
  standard folders (F8) plus added ones; `fxScan` command,
  `meshFxPlugins` event. **Accept:** both test plugins listed; a file
  that isn't a plugin and one that crashes on load listed as broken; a
  second scan with nothing changed starts no processes.
- [x] **Task 4: the chain in `Chain::step`.** After noise, before gain
  (F3); mono to stereo and back (F7); the 20 ms deadline, dry frames,
  bypass after 3 late frames or a crash; `fxChain <json>`,
  `fxRestart`, `meshFx {state, plugin, latency}` events. **Accept:**
  unit tests with a fake client: late frames go dry; 3 in a row bypass;
  restart restores.
- [ ] **Task 5: state, editors, parameters.** `save`, state files,
  `fxEdit <index>`, `fxParams <index>` / `fxParam <index> <id>
  <value>`. **Accept:** a test plugin's gain set by `fxParam` survives a
  restart of `mesh-fx` through its state file; a corrupt state file is
  ignored.
- [ ] **Task 6: voice quality.** `callAudio {"quality"}`, live, with the
  encoder rebuild across modes. **Accept:** unit test: Studio's packets
  average about 4× Standard's; switching modes mid-stream decodes with no
  error.
- [ ] **Task 7: process tests** (`tests/fx.rs`, spec Testing): a +6 dB
  plugin heard 6 dB louder (±1); a 50 ms-per-block plugin leaves the call
  going dry and reports bypass; killing `mesh-fx` mid-call bypasses
  within 100 ms; the mic test plays the processed voice; an old engine
  (`--no-calls`-style 0.5.1 build) hears a Studio caller. CI builds the
  test plugins first.

### Phase 2: the app (Aflos) and Discord

- [ ] **Task 8: Plugins in Voice & audio** (F0: a collapsed "Advanced:
  plugins" section below the built-in controls, empty by default). A list (name, format,
  latency, on/off, up/down, Edit, remove), Add from the scan list, Add
  folder, Rescan; the bypass notice with Restart plugins; the chain in
  `config.toml`, sent at start. **Accept:** unit tests: the chain
  survives a `config.toml` round trip and produces the right `fxChain`
  JSON.
- [ ] **Task 9: Voice quality** choice with "about N kb/s up in a call of
  5" (Q-b). **Accept:** unit test on the estimate; `callAudio` JSON
  carries `quality`.
- [ ] **Task 10: Discord video quality.** `aflos-dc` `videoQuality`, the
  tier clamp, `send_video` reads its settings; the app's camera and
  screen settings. **Accept:** unit tests: each tier's clamp; the
  defaults equal today's constants.
- [ ] **Task 11: shipping `mesh-fx`.** release.yml builds it beside
  `mesh-node` on both OSes; the Windows installer, Linux tarball, AUR
  PKGBUILD and the self-updater include it; Aflos finds it beside
  `mesh-node`. **Accept:** a release dry run (workflow on a branch, no
  publish) has `mesh-fx` in every asset.
- [ ] **Task 12: checkpoint step** (plugins and quality) in
  `checkpoint.rs`; version 0.5.4 and CHANGELOG; CAPABILITY-MAP by-hand
  items. **Accept:** the checkpoint test passes; Aflos's rule check (no
  new clippy findings or rustfmt hunks against `main`).

**Checkpoint B (by hand, not a hard stop for building):** a real EQ
(LSP or TDR Nova) on Linux and Windows: found, window opens, tuning heard
in the mic test and in a call; Studio vs Standard by ear; Discord screen
share settings with and without Nitro.

## Risks

- **VST3 editors on Linux** need an X11 run loop host interface
  (`IRunLoop`); if the spike shows `vst3` 0.3.0 makes that impractical,
  Checkpoint A stops and the fallback is generic sliders for VST3 only.
- **Windows plugin folders** often hold 32-bit or broken plugins; the
  scanner's per-file process keeps that contained.
- **Upload at Studio** in a big voice channel (Q-b shows it; no cap yet).
