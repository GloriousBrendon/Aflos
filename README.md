<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="design/logo/aflos-horizontal-dark.png">
    <img src="design/logo/aflos-horizontal-light.png" alt="Aflos: Encrypted. Decentralized. Yours." width="480">
  </picture>
</p>

<p align="center"><b>Encrypted. Decentralized. Yours.</b></p>

Aflos (Afrikaans for "relay") is a chat app for Linux and Windows, written in
Rust. It has its own peer-to-peer network, with direct messages, group chats,
Discord-style Spaces and voice calls, end-to-end encrypted and with no central
server. It can also sign in to your Matrix, Discord and WhatsApp accounts, so
those chats sit in the same window.

Aflos began as a Matrix client called Mesh. Some older notes still use that
name (see [design/mesh/README.md](design/mesh/README.md)).

## What works today

The Aflos network runs in `mesh-node`, a helper from the Mesh-Node repo that
Aflos starts beside itself. Calls and the full set of Space features need
Mesh-Node's current `main`, which is what the release builds bundle.

- **Identity.** A username and password unlock a keystore on your device
  (Argon2id, XChaCha20). There's no email and no phone number. You can link a
  second device with a one-time code, and save or restore a `.mks` backup.
- **Friends.** Add friends by friend code or ID. You can see who is online and
  block people.
- **Direct and group messages.** Edits, deletes, reactions, typing, read marks
  and files. Group chats hold up to 10 people.
- **Spaces.** Discord-style servers with up to 50 members and 100 channels,
  plus categories, roles (owner, admin, member), admin-only channels,
  mentions, replies, files, kicks and bans.
- **Voice calls.** 1:1 calls in DMs, and voice channels in Spaces for up to
  10 people. Push to talk and noise suppression are built in. Calls are voice
  only, with no echo cancellation yet, so use headphones.
- **End-to-end encryption.** Messages, group messages, Space events and files
  are encrypted per device with Olm (vodozemac). Each file gets its own key.
  Call audio goes straight between devices over encrypted QUIC.
- **Decentralized, not serverless.** Devices connect to each other directly
  (iroh QUIC, with hole punching). When a direct path fails, they fall back to
  relay servers. Friend codes, address lookup and offline delivery (kept about
  30 days) go through seed nodes. Relays and seeds never see message
  contents, though a seed does see lookup IDs, timing and sizes. Aflos ships
  with the project's own relay and seed, which are currently reachable only
  from the home network, plus n0's public relays. You can run your own seed
  (`MESH_SEEDS`).
- **Matrix.** Rooms, spaces, DMs, E2EE with device verification and key
  backup, 1:1 voice calls, attachments, reactions, read receipts and
  notifications. See [Matrix](#matrix) below.
- **Discord** (`aflos-dc`, optional). A native client that signs in to your
  own account, by QR code or email and password. It handles servers,
  channels, DMs, sending text, media and voice channels with DAVE. It can't
  upload files, and video and screen share are off. It acts as a user
  account, which Discord's terms forbid, so use it at your own risk.
- **WhatsApp** (`aflos-wa`, optional). Linked as a device with
  [whatsapp-rust](https://github.com/oxidezap/whatsapp-rust). It handles chats,
  files, reactions, forwarding and calls, including camera and screen share.

These are native clients signed in to your own accounts, not Matrix bridges.
The Discord and WhatsApp helpers have not been tested much on real accounts
yet.

**Planned:** video and screen share on Aflos calls, echo cancellation, and a
relay and seed that people outside the home network can reach.

## Layout

```
crates/
  aflos          The iced GUI: every pane, settings, theming, the Aflos
                 network screens, updates.
  aflos-core     Matrix: login, sync, rooms, timeline, E2EE, verification,
                 1:1 voice calls. No UI dependencies.
  aflos-discord  Talks to the helpers over stdio: mesh-node (the Aflos
                 network), aflos-dc, aflos-wa, and the browser fallback.
aflos-dc/        Native Discord helper (its own cargo project).
aflos-wa/        Native WhatsApp helper (its own cargo project).
aflos-media/     Camera and screen capture, H.264, shared by the helpers.
installer/       Windows setup (Inno Setup) and the Linux installer.
```

## Theming

Aflos wears your desktop. By default it follows your rice:

- **Colours** from `~/.config/themes/active/colors.conf` (`$bg2`, `$bg1`,
  `$surface`, `$accent`, `$text` as `$key = rgba(rrggbbaa)`; override the path
  with `AFLOS_THEME_FILE`). The other colours (bright and muted text, borders,
  text on accent) are derived, and muted text is lightened until it passes
  4.5:1 contrast.
- **Fonts** from fontconfig on Linux (`fc-match sans-serif` and `monospace`),
  read when Aflos starts. Set `font_from_system = false` in `config.toml` to
  use the bundled IBM Plex Sans and JetBrains Mono instead (Windows always
  does).
- **Gaps, border width and rounding** from `~/.config/hypr/hyprland.conf`
  (`general { gaps_in gaps_out border_size }`, `decoration { rounding }`,
  following `source = …` includes). Set `layout_from_hyprland = false` in
  `config.toml` to use the defaults instead.

Both files are watched: edit them and Aflos restyles within a second.
Missing files fall back to the defaults in [design/DESIGN.md](design/DESIGN.md).

Settings → **Rice** (gear icon in the user panel) switches between "Follow my
rice", the built-in presets (Catppuccin Mocha, Nord, Gruvbox Dark) and "Custom
theme.toml", shows every colour token (and which are derived), the layout
values, the message density, the background (an animation: digital rain, a
drifting mesh or stars, or none; and a background image of your own) and three
switches: translucent panes, network tags and plain-text mode (no image
downloads). With "Custom theme.toml" chosen, click any colour to change it
(hex, hue, saturation, lightness): theme.toml is saved as you go, and a first
custom theme starts from the colours you had. "Export theme" copies the current
look as a theme.toml to the clipboard. Custom theme.toml is
read from `~/.config/aflos/theme.toml` (`%APPDATA%\aflos\theme.toml` on
Windows), see [theme.example.toml](theme.example.toml). The choice persists in
`~/.config/aflos/config.toml` (`%APPDATA%\aflos\config.toml` on Windows).

## Discord and WhatsApp

### Native helpers (no browser)

`aflos-wa` (WhatsApp) and `aflos-dc` (Discord: gateway, REST and voice with
DAVE) are each their own cargo project; build them with `cargo build --release`
in `aflos-wa/` and `aflos-dc/` (they need `cmake` and `clang`, plus ALSA
headers on Linux) and put the binaries next to Aflos's own. The installers do
this for you. Link WhatsApp from the QR code on the Accounts page; sign in to
Discord there with email and password (or its QR code). WhatsApp calls can
send your camera or screen: on Linux the camera is any V4L2 webcam, and
"Choose a screen or window…" opens your desktop's own share picker
(xdg-desktop-portal, frames over PipeWire). Discord video and screen sharing
are left out on purpose.

On Linux, where Discord goes through a VPN, have `aflos-dc` run inside the
VPN's network namespace too, or it connects from your own IP. In
`~/.config/aflos/config.toml`, give the command Aflos should put in front of the
helper (it must work without a password prompt: Aflos has no terminal):

```toml
discord_native_wrapper = ["sudo", "-n", "ip", "netns", "exec", "vpn", "sudo", "-n", "-u", "you"]
# or a rootless launcher that runs its arguments inside the tunnel:
discord_native_wrapper = ["/home/you/.local/bin/discord-voice"]
```

The wrapper must pass stdin through to the helper (a shell script that starts
it with `&` hands it `/dev/null` instead), and Aflos stops it with SIGKILL, so
anything it starts should go down when the helper's stdin closes.

If the wrapper can't start, Discord stays offline; it never falls back to a
direct connection.

### Browser fallback

Without the helpers, Aflos can drive a hidden official Discord and WhatsApp
web client instead (`crates/aflos-discord`): it connects to the browser's
DevTools socket (`discord_engine_socket` in `config.toml`, default
`$XDG_RUNTIME_DIR/discord-engine.sock`), installs a small script in the page
and reads the page's own state. Set `discord_engine_command` to start that
browser when Aflos starts and it isn't running. On Linux the browser itself (a
browser behind a VPN) is set up outside Aflos; see design/DISCORD-NATIVE.md.

On Windows the engine is Edge with DevTools on `127.0.0.1:9222` (the default
there; `discord_engine_socket` also takes any `host:port`). Put this in
`%APPDATA%\aflos\config.toml`, start Aflos, log into Discord once in the window
that opens and link WhatsApp in its second tab (WhatsApp → Linked devices);
minimize it after that, but don't close the tabs:

```toml
discord_engine_command = 'start "" msedge --remote-debugging-port=9222 --user-data-dir=%LOCALAPPDATA%\aflos\discord-engine --app=https://discord.com/channels/@me --no-first-run --autoplay-policy=no-user-gesture-required --disable-background-timer-throttling --disable-renderer-backgrounding --disable-backgrounding-occluded-windows https://web.whatsapp.com/'
```

Any local program can use that port to drive the Discord and WhatsApp
sessions in it, so only run it on a PC you trust.

### Discord voice button

Add `discord_voice_url = "https://…"` to `config.toml` and a 🎧 button appears
in the channel pane that opens it in your browser, e.g. a self-hosted browser
running Discord web, for voice alongside a text-only Discord bridge. Or set
`discord_voice_command = "…"` to have the button run a command instead (say, a
script that shows or hides a voice window); it wins over the URL.

## Build & run

```sh
cargo run -p aflos
```

For the Aflos network, build `mesh-node` from a Mesh-Node checkout next to this
one and put it beside the `aflos` binary (the installers do this).

Your identity, accounts and chats (sessions, sqlite stores, E2EE keys) live in
the platform data dir: `~/.local/share/aflos` on Linux, `%APPDATA%\aflos` on
Windows. `AFLOS_DATA=<folder>` runs a second, separate copy.

**Upgrading from Mesh:** on its first start Aflos copies the old `mesh` data
and config folders to `aflos` and leaves the old ones as they were.

## Linux

`installer/install-linux.sh` builds Aflos, both helpers and `mesh-node`, and
installs them for your user: `aflos`, `aflos-wa`, `aflos-dc` and `mesh-node` in
`~/.local/bin`, Aflos's icons (hicolor) and an `aflos.desktop` entry. `aflos-dc`
is left out until `discord_native_wrapper` is set. `--uninstall` removes them
and keeps your accounts and chats. Over a Mesh install it removes Mesh's old
files and leaves `~/.local/bin/mesh` as a link to `aflos`.

## Windows

`installer\build.ps1` builds Aflos, both helpers, `mesh-node` and
`dist\Aflos-Setup-<version>.exe` (needs
[Inno Setup 6](https://jrsoftware.org/isinfo.php):
`winget install JRSoftware.InnoSetup --scope user`). The setup installs per user
without an admin prompt, adds Aflos to the Start menu and Settings → Apps, and
can add a desktop shortcut and start Aflos at sign-in. Over a Mesh install it
updates in place. Uninstalling keeps your accounts and chats in
`%APPDATA%\aflos`.

**Check for updates** (Settings footer) fetches the newest release from GitHub,
checks it against the release's `SHA256SUMS` and installs it.

## Matrix

- Password login and session restore across restarts.
- Room list sorted by recency, split into "Direct messages" and "Rooms"
  sections, with unread badges and room avatars.
- Timeline for the selected room: messages grouped by day and by author,
  anchored to the newest message, from the latest 50 events. Fenced code blocks
  and `inline code`, images (downloaded and decrypted, with a filename · size
  caption), reactions, joins, leaves and renames as small system lines, and
  "… is typing" under the composer. Three densities: `cozy` (default),
  `compact` (no avatars, one line per message) and `irc`
  (`[14:02] <nick> text`), set with `density = "compact"` in `config.toml`.
- Push-driven updates: the UI refreshes when sync delivers events (no polling).
- Sending messages (composed as Markdown — `**bold**` etc. works), attachments
  and reactions; read receipts; desktop notifications.
- E2EE device verification: incoming verification requests show a banner with
  accept/decline and the emoji SAS confirm flow. Verify Aflos from another
  client (e.g. Element) to unlock encrypted rooms.
- Secure Backup (server-side key backup + recovery): set it up, save the
  recovery key once, and restore your message keys on a new session.
- Spaces: rooms are grouped by their `m.space`; `Alt+1..9` switches space and
  `Ctrl+K` opens a jump palette over all rooms and spaces.
- Network tags: Discord rooms bridged by mautrix-discord (their `m.bridge`
  state event) and their users are tagged `dc`.
- 1:1 voice calls with the legacy Matrix VoIP events (`m.call.*`, v1), so they
  work with Element's 1:1 calls. WebRTC with Opus; ICE uses the homeserver's
  TURN server.

Not there yet for Matrix: echo cancellation, video calls, group (Element Call)
calls, history pagination on scroll-up, sending our own typing notices,
joining or creating spaces, and Discord friend requests through the bridge.

## License

Copyright (C) 2026 GloriousBrendon

Aflos and `mesh-node`, its network engine (from the Mesh-Node repo, shipped
in every release), are both free software: you can redistribute them and/or
modify them under the terms of the GNU Affero General Public License as
published by the Free Software Foundation, either version 3 of the License,
or (at your option) any later version (AGPL-3.0-or-later). See
[LICENSE](LICENSE). Contributions: see [CONTRIBUTING.md](CONTRIBUTING.md).

The vendored `songbird` and `serenity-voice-model` in `aflos-dc/vendor/` keep
their ISC license, and the Michroma font keeps the SIL Open Font License.

## Trademark

The Aflos name and logo are not covered by the code license. Forks and
redistributions of modified versions must use a different name and logo. The
brand assets are all rights reserved; see
[design/brand/aflos/LICENSE.txt](design/brand/aflos/LICENSE.txt).
