# Plan: `integrations` (Aflos)

Status: **APPROVED 2026-11-28** ("approve"). Built from
[SPEC-integrations.md](SPEC-integrations.md) (approved 2026-11-28).
Phases 1 to 3 ship in 0.6.x releases with WhatsApp and Discord still
bundled (M2); Phase 4 is **0.7.0**, without them.

## Where things are

| Piece | Where | Size |
|---|---|---|
| WhatsApp helper | `aflos-wa/` (own Cargo project), JSON lines `{"fn","args"}` in, `{binding, payload}` out | about 3,000 lines |
| Discord helper | `aflos-dc/`, the same protocol | about 3,700 lines |
| Shared media | `aflos-media/`, used by both helpers | |
| App side | `crates/aflos-discord/`: `whatsapp.rs`, `lib.rs` (Discord), `engine.rs` + `agent.js` + `whatsapp-agent.js` (the old browser path), and `mesh.rs` (Mesh-Node's client, which stays) | about 1,700 lines, plus `mesh.rs` |
| App coupling | `crates/aflos/src/main.rs`: about 400 places; `accounts.rs`; `aflos-core` `bridge::Network` | |
| Shipping | `release.yml`, `ci.yml`, `installer/build.ps1`, `packaging/aur/PKGBUILD` and `aflos-chat.install` | |

Mesh-Node has no WhatsApp or Discord code.

## Decisions taken while planning

- **A new crate, `crates/aflos-integration`**: the protocol's types
  (`hello`, capabilities, commands, events, each building block) with
  serde, so the app, the test harness and Rust integrations share one
  definition. Integrations in other languages use the JSON it documents.
- **The vocabulary is the union of what WhatsApp and Discord do today**:
  every variant of `whatsapp::Event` and `aflos_discord::Event` maps to a
  building block, and each command either helper takes maps to one. If one
  doesn't fit, the block is widened, not special-cased.
- **Two small additions to the spec's blocks**, both things Discord does
  today: **Join by link** (an optional chat-list action: Discord invites)
  and **Watch a stream** (an optional calls action: Discord's Go Live). The
  spec is updated to match (Changes while planning).
- **`Network` becomes data**: `aflos-core` keeps `Matrix` and `Mesh` and
  replaces `Discord` and `WhatsApp` with `Integration(id)`. Its tag, colour
  and icon come from the integration's `hello`.
- **Matrix and Mesh don't go through the protocol.** They stay built in as
  they are (M4).
- **Installed integrations live in** `<data>/integrations/<id>/` (the
  program, its manifest, the granted permissions) and their data in
  `<data>/integrations/<id>/data/`. Removing one deletes the folder.
- **The manifest** (`aflos-integration.toml`): `id` (reverse-DNS), `name`,
  `version`, `publisher`, `protocol` (the version it speaks), `permissions`,
  and per platform an asset name and its SHA-256. A release has the
  manifest and the per-platform `.zip`s; a zip install has the manifest
  inside.
- **Official signing**: an Ed25519 signature of the manifest
  (`aflos-integration.toml.sig`), checked against a public key built into
  Aflos. The private key stays offline with the maintainer.
- **The private copies (M3)** live in `~/Documents/Integrations/` on the
  maintainer's machine: `aflos-wa/` and `aflos-dc/`, each a local git
  repository with no remote, building zips that install like any other.

## Tasks

### Phase 1: the protocol

- [x] **Task 1: `aflos-integration`**: the protocol types and their JSON,
  the handshake, the building blocks, protocol version 1. **Accept:** unit
  tests: every message round-trips; an unknown field or event is ignored,
  never fatal; a version mismatch is reported.
- [x] **Task 2: the host** in the app: start, handshake, read and write
  lines, the files folder for media (P3), restart up to three times then
  stop with its error (T7), a hang (no answer to `ping` in 10 s) treated as
  a crash. **Accept:** tests with stand-in programs that answer, crash,
  hang, and send garbage: Aflos never panics or blocks; each ends in the
  right state.
- [x] **Task 3: the echo integration and `aflos-integration-test`.** Echo:
  a rail tile, a form sign-in, one chat that repeats what you send.
  The harness runs any integration through the handshake and each block it
  claims. **Accept:** echo passes the harness; a deliberately broken echo
  fails it with a clear message.

### Phase 2: the app goes generic (WhatsApp and Discord still bundled)

- [x] **Task 4: networks as data.** `Network::Integration(id)`, the rail
  tile, status dot and badge from the integration. **Accept:** echo's tile
  appears with its icon and colour; the existing tiles look as before.
- [x] **Task 5: sign-in and Accounts.** QR, code, form and web-page
  sign-in; Accounts lists integrations with sign out, turn off and remove.
  **Accept:** echo signs in by form; unit tests for each sign-in kind.
- [x] **Task 6: chats.** Chat list sections, the timeline (text with the
  fixed formatting, images, video, files, voice notes, stickers, GIFs,
  replies, forwards, reactions, edits, deletes, ticks), the hover bar from
  declared actions, typing and presence, notifications, search, Join by
  link. **Accept:** echo's chat works end to end; unit tests map each
  block to what the panes draw.
- [x] **Task 7: calls.** Ringing and answer, mute, camera, screen with its
  source picker, video frames both ways (including Bigger video), voice
  channels with members, Watch a stream. **Accept:** unit tests with a
  stand-in that rings, sends frames and ends; the call bar and voice
  channels drawn from blocks.
- [x] **Task 8: settings.** The settings block, stored per integration;
  Discord's video settings move into it. **Accept:** a setting survives a
  restart and reaches the integration.
- [x] **Task 9: WhatsApp through the protocol.** `aflos-wa` speaks it; the
  app's WhatsApp-only code goes, and WhatsApp runs as a bundled
  integration. **Accept:** the checkpoint parity list below, WhatsApp half,
  by hand; no `Network::WhatsApp` left.
- [ ] **Task 10: Discord through the protocol.** The same for `aflos-dc`;
  the old browser path (`engine.rs`, `agent.js`, `whatsapp-agent.js`)
  goes too. **Accept:** the parity list, Discord half; `crates/aflos-discord`
  holds only `mesh.rs`.

**Checkpoint A (by hand, a release):** a 0.6.x release with WhatsApp and
Discord bundled but running as integrations. Parity list: sign in by QR;
chats, groups, servers and channels; send text, a picture, a file, a voice
note; react, reply, forward, edit, delete; notifications; a voice and a
video call on each, screen share, Bigger video; Discord voice channels, a
Go Live stream, invites, 2FA sign-in. Phase 3 starts once this passes.

### Phase 3: install, trust, update

- [ ] **Task 11: install.** Settings → Integrations → Install: a GitHub
  repository or release link (its latest release, through GitHub's API) or
  a `.zip`; manifest checks, this platform's download, SHA-256, unpack.
  **Accept:** echo installs from a test release and from its zip; a wrong
  checksum, a missing platform and a newer protocol than Aflos speaks are
  each refused with a plain message.
- [ ] **Task 12: consent and permissions.** The consent screen (T2) with
  the unofficial warning; granted permissions kept; Aflos hands over only
  what's granted (no camera frames without camera). **Accept:** unit
  tests: a denied permission's blocks stay hidden.
- [ ] **Task 12b: crash recovery** (spec T7 to T10). The stopped
  message with Restart, Turn off and Remove; the "handling" note around
  each integration's events and blocks, a panic hook that writes it to
  `<data>/integrations/crashed`, and the next start's question; the
  start-up crash count and safe start (`--safe`); Integrations on the
  sign-in screen. **Accept:** tests: a stand-in whose event makes the app
  panic is named on the next start and removing it works; two unnamed
  start-up crashes bring a safe start; `--safe` starts with all off.
- [ ] **Task 13: updates, turn off, remove.** "Update available" from the
  pinned release; Update asks again if permissions changed (T4); turn off
  and remove (T5). **Accept:** tests with two echo releases: no update
  without pressing; changed permissions ask again; remove leaves nothing.
- [ ] **Task 14: official integrations.** Signature check and the
  **Official** mark; Settings → Integrations lists official ones from a
  list built into Aflos (empty until the first one exists, and hidden while
  empty). **Accept:** a signed echo shows Official; a tampered one is
  refused.

### Phase 4: the split (0.7.0)

- [ ] **Task 15: the private copies.** `aflos-wa`, `aflos-dc` and
  `aflos-media` copied to `~/Documents/Integrations/` as local git
  repositories, each with a manifest and a script that builds its zips.
  **Accept:** both install from their zips into a copy of Aflos with them
  removed, and keep the sign-in from a 0.6.x install (M1's data move).
- [ ] **Task 16: removal.** Delete `aflos-wa/`, `aflos-dc/`, `aflos-media/`
  and the WhatsApp and Discord parts of release.yml, ci.yml, build.ps1,
  PKGBUILD and `aflos-chat.install`; rename `crates/aflos-discord` to
  `crates/aflos-mesh`. The first-start notice (M1). README and CHANGELOG
  say they were removed because using personal WhatsApp and Discord
  accounts this way is against their terms (M3). **Accept:** `grep`
  finds no WhatsApp or Discord code; CI and a release dry run pass.
- [ ] **Task 17: Build an integration.** The guide (`docs/integrations/`),
  the template repository (echo in Rust and in Python), the harness as a
  release download. **Accept:** following the guide from scratch, the
  Python echo installs and passes the harness.

**Checkpoint B (by hand, the 0.7.0 release):** update a 0.6.x install;
the notice shows; install WhatsApp and Discord from the private zips;
the parity list passes again.

## Risks

- **Size.** Phase 2 touches most of `main.rs`: the largest change Aflos
  has had. Each task lands on its own branch with the app working, and
  Checkpoint A is a whole release of parity before anything is removed.
- **The vocabulary misses something** WhatsApp or Discord does. Found in
  Tasks 9 and 10 at the latest, with both still bundled: the block is
  widened then.
- **Windows and downloaded programs.** SmartScreen and antivirus may warn
  about an unsigned program from GitHub. The consent screen says so; code
  signing for official integrations is part of Task 14.
- **GitHub's API limit** (60 requests an hour without a token) is plenty
  for installs and update checks once a day.
- **Public templates.** Task 17's template repository is new and public:
  the maintainer creates it, or says to.

## Changes while building

- **Task 2:** the host lives in `aflos-integration` behind its `host`
  feature (not in the app), so its tests run against the real echo
  program; the app depends on it with `host` on. Restarts are forgiven
  after a minute's stable running. Lines over 16 MB are skipped.
- **Task 3:** echo is also the host's stand-in: `AFLOS_ECHO_MODE` makes it
  crash, hang, stay silent, write garbage, speak a newer protocol, or
  break sending.
- **Task 4:** `Network::Discord` stays for Discord rooms bridged into
  Matrix (decided: bridges keep working); integrations are
  `Network::Integration(slot)`, the slot from a registry in `aflos-core`
  so tags and colours read like any network's. Sections gained `home`:
  a home section's chats go in Home, the others get rail tiles (a server,
  "Groups"). Until Task 11, integrations to run are listed in config.toml
  as `[[integrations]]` (`id`, `program`, `args`).
- **Task 5:** every integration has a row in Accounts' connected list and
  its own panel, including one that never started (so it can always be
  removed). The panel draws the step it asks for (QR, code, form, web),
  keeps a failed step up with the reason, and has Restart (when stopped),
  Turn off and Remove; Remove asks once and deletes its folder. Part of
  spec T7 landed here; Task 12b adds the crash-on-next-start side.
- **Task 6:** an integration's media are web media keyed
  `int:<slot>:<o|p|f>:<key>` (the file, a voice note's samples, a GIF's
  frames), so the timeline's loading, caching and drawing are WhatsApp's.
  Its home sections' chats sit with everyone's direct messages; other
  sections open their own pane, by category, with Join by link when it
  takes them. The hover bar follows its declared actions; files go out
  through its files folder; the forward picker lists its chats. Moved to
  Task 9, where WhatsApp needs them: contact search and starting a chat
  (the new-chat picker), and archived chats' row in Home (archived chats
  are left out of Home until then). "Echo's chat end to end" is the
  harness's send check (Task 3); the app's side is unit-tested.
- **Task 7:** an integration's call and voice room bars sit with the
  others above the chat (Bigger video fills the pane with any of them);
  its video tracks are `int-<slot>-<track>` in the call video, kept up to
  10 s without a frame as WhatsApp's are, since integrations say when a
  video ends. Its rings join the ringer (a call or room ringing us, else
  ringback for ours), a room's ringing can be dismissed here, and a DM
  header gets call buttons when it declares calls. The share picker is
  shared with WhatsApp's (it now takes its pick and cancel messages).
- **Task 8:** settings are kept in `settings.json` in the integration's
  folder (so Remove takes them too), sent in every `hello` and on each
  change; a text setting is sent when it's submitted. They show on its
  Accounts panel. Discord's video settings move into it with Discord
  itself (Task 10).
- **Task 9:** `aflos-wa` speaks the protocol through a translation layer
  at its edge (`aflos-wa/src/proto.rs`): protocol commands become its own
  calls and its reports become events, so WhatsApp's own code is
  untouched. It reads Aflos's hello before anything else; while bundled
  it keeps 0.6's data folder (`--data`), so nobody is signed out. Aflos
  runs it as the integration `aflos.whatsapp` when it's installed beside
  Aflos. The protocol gained `show` (an integration asks Aflos to open a
  chat it started or joined). Gone from the app: `Network::WhatsApp`, the
  WhatsApp state, events, panes, call bar and messages, the engine
  browser's WhatsApp tab, WhatsApp's registry entry (Matrix rooms bridged
  from WhatsApp now read as Matrix) and its sync switches. Made generic
  on the way: the new-chat picker (contact search, start a chat), the
  Archived row, Archive and Delete in a chat's header, Ctrl K, closing an
  integration chat while Aflos is unfocused (so it isn't marked read), a
  DM's last-seen line, and Home's badge. Integrations ring and chime as
  WhatsApp did; custom sound files keyed `message_wa`/`ring_wa` still
  apply. Not carried over: WhatsApp's "groups"/"chats" sync switches and
  its bigger profile pictures. Removing the bundled WhatsApp in Accounts
  lasts until the next start (it's re-added while bundled); Turn off
  sticks.

## Changes while planning

- 2026-11-28: crash recovery added (spec T7 extended, T8 to T10 new;
  Task 12b), from the maintainer: a broken integration must be easy to
  remove, from the sign-in screen and after a crash, named.

- Spec building blocks gained **Join by link** (chat list) and **Watch a
  stream** (calls), both things Discord does today.
