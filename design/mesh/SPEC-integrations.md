# Spec: `integrations` (Aflos)

Status: **DRAFT 2026-11-28**, for review. Recommendations are the defaults;
the open questions at the end need a decision before the plan.
Ships as **0.7.0**: it changes what Aflos is made of (see `versioning`).

## Objective

Aflos is moddable. Other networks come to Aflos as **integrations**:
programs that anyone can build, publish (on GitHub or anywhere) and
install at their own discretion. Aflos ships only what it may ship: its
own network (Mesh) and Matrix, which is open. WhatsApp and Discord leave the
Aflos release.

Official Aflos integrations, maintained by Aflos, use each service's
sanctioned API (the WhatsApp Business Platform, Discord bots, Slack and so
on). Those are what a business can rely on, and Aflos can tailor them.

**User stories**

- I want WhatsApp in Aflos: I paste the link to an integration someone
  published, Aflos shows me who made it and what it can do, I say yes, and
  WhatsApp appears on my rail like it does today.
- I don't trust an integration any more: I remove it in one click and
  everything it had goes with it.
- An integration updates: nothing changes until I say so.
- I'm a developer: I read "Build an integration", start from the template,
  and have my network's icon on the rail, its sign-in, chats and calls
  working without touching Aflos's code.
- I run a business: I install the official Aflos integration for our
  WhatsApp Business account and get support for it.

## What exists

| Piece | Today |
|---|---|
| Networks | `aflos-core` `bridge::Network`: a fixed enum, `Matrix`, `Discord`, `WhatsApp`, `Mesh`. Every pane matches on it. |
| WhatsApp | `aflos-wa` (about 3,000 lines): a native helper Aflos starts beside itself, JSON lines in and out. The app side is `crates/aflos-discord/src/whatsapp.rs` (its `Event` enum: snapshot, messages, notify, media, files, GIF frames, voice-note PCM, contacts, chat actions, QR, calls, video, typing, presence). |
| Discord | `aflos-dc` (about 3,700 lines), the same shape. The app side is `crates/aflos-discord/src/lib.rs` (snapshot, invites, messages, notify, media, QR, sign-in with 2FA, video, video quality, DMs). |
| App coupling | About 400 places in `crates/aflos/src/main.rs` name Discord or WhatsApp, plus their own panes: Discord's server list and voice channels, the WhatsApp groups pane, the call bars, Accounts. |
| Theming | Already moddable: the "rice" theme file, hot-reloaded. |
| Voice plugins | VST3 and CLAP effects (SPEC-voice-quality), in Voice & audio. A different thing with the same word, hence **integrations** here. |

The helpers already work the way integrations will: a separate program,
a line-based protocol, Aflos draws everything. The work is making that
protocol general and documented, and making the app's panes work from
what an integration declares instead of from a fixed list.

## Decisions (review these)

### What an integration is

| # | Decision | Why |
|---|---|---|
| I1 | **A separate program** Aflos starts and talks to over its stdin and stdout, one JSON object per line: the protocol below. Any language. | It's how `aflos-wa` and `aflos-dc` already work. A crash stays in the integration. No Rust needed to build one. |
| I2 | **Aflos draws everything**, from a fixed set of building blocks the integration fills in (the next table). An integration can't draw its own widgets. | Aflos is native (iced): another process can't put widgets in its window. The building blocks keep every integration in the user's theme and keyboard habits. |
| I3 | **Prebuilt, not built from source.** A release carries one program per platform (`linux-x86_64`, `windows-x86_64`, later macOS and ARM) and a manifest, `aflos-integration.toml`, listing each file's SHA-256. | Building from source needs a toolchain most Windows users don't have. Containers aren't practical on a desktop. |
| I4 | **Built in: Mesh and Matrix** (M4). Everything else is an integration. | They're Aflos's own and open. Nothing else needs to be in the release. |

### Building blocks (extension points)

What an integration can show, version 1. "Today" is where the same thing
is hard-coded now; each becomes generic.

| Block | The integration provides | Today |
|---|---|---|
| Rail tile | An SVG icon, a short tag (`wa`, `dc`), a colour, a status dot, an unread badge | The fixed `dc`, `wa` tiles |
| Sign-in | One of: a QR code to scan, a code to enter, a form (fields it names), or a web page to open | WhatsApp's QR, Discord's QR and 2FA code |
| Chat list | Chats in sections it names (Direct, Groups, Servers → channels), with pictures, unread counts, muted, archived | The WhatsApp groups pane, Discord's server list |
| Timeline | Messages: text (with a fixed set of formatting), images, video, files, voice notes, stickers, replies, forwards, reactions, edits, deletes, ticks | Each network's `Messages` |
| Actions | Which message actions it supports: send, reply, react, edit, delete, forward, send files | The hover bar's per-network rules |
| Presence and typing | Per chat or contact | WhatsApp's `Typing`, `Presence` |
| Notifications | A new message worth a notification | `Notify` |
| Calls | Ringing, answer, decline, mute, end; video frames in and out; which cameras and screens to share | The WhatsApp and Discord call bars, voice channels |
| Voice channels | Rooms you join and leave, with who's in them | Discord voice channels |
| Settings | A page of fields it names (switches, choices, text), stored for it | Discord video settings |
| Search | Contacts and chats matching a query | WhatsApp contact search, Ctrl K |

Not in version 1: custom screens, custom message types, changing Aflos's
own panes. A later version could add custom screens through a sandboxed
layer; see Later.

### Installing, trusting and updating

| # | Decision | Why |
|---|---|---|
| T1 | **Settings → Integrations → Install**: paste a GitHub repository or release link, or choose a downloaded `.zip`. Aflos reads the manifest, downloads this platform's program over HTTPS, checks its SHA-256 and installs it into Aflos's data folder. | The two ways people share software. |
| T2 | **A consent screen before anything runs**: name, publisher (the GitHub owner, with a link), version, and what it asks for (the permissions below), with **Install** and **Cancel**. Unofficial ones say plainly: "Made by someone else, not checked by Aflos. It can read the chats it connects to." | It's code with your accounts. People should decide knowing that. |
| T3 | **Permissions are declared and shown, not enforced** in version 1: network services it connects to, microphone, camera, screen, notifications, files. Aflos only hands over what's granted (it won't send camera frames to one without camera permission), but it can't stop a native program doing what it likes on its own. The consent screen says so. | Honest. True sandboxing needs per-OS work (bubblewrap on Linux, AppContainer on Windows); see Later. |
| T4 | **Pinned versions.** Aflos checks for a new release and shows "Update available"; nothing updates until you press Update, which shows the consent screen again if the permissions changed. | A publisher can't push new code to you silently. |
| T5 | **Remove** stops it and deletes its program and data, after a confirmation. **Turn off** keeps them. | One click out. |
| T6 | **Official integrations are signed** with Aflos's key and marked **Official** on the consent screen and in Settings. Aflos checks the signature. | How a business knows it's the supported one. |
| T7 | **One crash doesn't take Aflos down.** A crashed integration is restarted up to three times, then shown as stopped with its error and a Restart button. | The same rule as voice plugins. |

### The protocol

| # | Decision | Why |
|---|---|---|
| P1 | **JSON lines** both ways: commands from Aflos `{"cmd": ..., "id": ..., ...}`, events from the integration `{"event": ..., ...}`. Replies carry the command's `id`. | Readable, debuggable with a terminal, any language. Today's helpers already speak nearly this. |
| P2 | **Handshake first**: Aflos sends `hello` with the protocol version it speaks; the integration answers with its own and what it supports (the building blocks above). Aflos shows only what's supported. | Older and newer integrations keep working; a missing feature hides its control instead of failing. |
| P3 | **Big data goes in files**, not lines: media, voice notes and GIF frames are written to a folder Aflos gives it, and the event names the file. Video frames stay inline JPEG for version 1 (as now), with a shared-memory path later. | Megabytes of base64 in one line is slow; it's what makes today's GIFs and voice notes heavy. |
| P4 | **Audio for calls is the integration's own**, as now: it opens the microphone and speakers itself, given the devices the user chose in Aflos. | It's how both helpers already work, and calls stay in the integration's control. |
| P5 | **Aflos keeps nothing it isn't given**: no copies of an integration's messages beyond the open chat's and the caches it already has. Each integration's data folder is its own. | Removing one takes its data with it (T5). |

### What changes for people using Aflos now

| # | Decision | Why |
|---|---|---|
| M1 | **0.7.0 ships without WhatsApp and Discord.** On first start after the update, Aflos says so once and links to Settings → Integrations, where installing one carries its sign-in over (the helper's data folder moves into the integration's). | Nobody is signed out without being told how to get it back. |
| M2 | **Before that, the app goes generic with WhatsApp and Discord still built in**, speaking the new protocol. One release (0.6.x) proves the protocol with the two real integrations before anything is removed. | Swapping the engine and removing the parts in one release would leave nothing to compare against. |
| M3 | **Today's WhatsApp and Discord code leaves the Aflos repos and isn't published.** The maintainer keeps it privately on their own machine, outside any repo, converted into integrations: the first real ones, used to test the protocol, the installer and every building block. *Decided 2026-11-28.* | Aflos stops distributing it, and the protocol still gets tested against the two most demanding integrations there are. |
| M4 | **Matrix stays built in**, a native part of Aflos, not an integration. *Decided 2026-11-28.* | It's an open protocol; nothing in its terms stops Aflos shipping it. |

### For people building integrations

| # | Decision | Why |
|---|---|---|
| D1 | **"Build an integration"**, a guide in the Aflos repo: the protocol, the manifest, each building block with an example, how to test, how to publish a release Aflos can install. | "Make Aflos your own" only works if it's documented. |
| D2 | **A template repository** with a working "echo" integration (a rail icon, a sign-in form, one chat that echoes what you send), in Rust, plus the same in one other language. | The fastest way to start, and proof the guide is complete. |
| D3 | **A test harness**: `aflos-integration-test`, a stand-in Aflos that runs an integration through the handshake and each block it claims, so authors find breaks before users do. | It's also Aflos's own test for the protocol. |

## Open questions

1. ~~Where do today's WhatsApp and Discord helpers go?~~ **Decided
   (M3)**: out of the repos, kept privately on the maintainer's machine as
   test integrations. (Not legal advice: if terms of service are the main
   reason, a lawyer's read is worth it.)
2. **Does Aflos list any integrations?** *Recommendation*: only official
   ones, in Settings → Integrations; community ones are installed by link.
   A directory of community integrations comes later, if ever.
3. **Matrix bridges** (mautrix-discord and similar on a homeserver): leave
   them working as Matrix rooms, as now? *Recommendation*: yes; they're the
   homeserver owner's choice, not Aflos's. (Matrix itself stays built in:
   decided, M4.)
4. **First official integrations**: which service and API first?
   *Recommendation*: WhatsApp Business Platform (Cloud API), since that's
   where a business would ask first; a separate spec.
5. **Sandboxing in version 1?** *Recommendation*: no (T3); declared
   permissions now, OS sandboxes as their own spec.

## Later

- Sandboxed integrations (Linux bubblewrap or Flatpak portals, Windows
  AppContainer), so permissions are enforced.
- Custom screens, through a sandboxed UI layer (WebAssembly components
  describing a widget tree Aflos draws).
- Video frames over shared memory instead of JPEG lines.
- A community directory.

## Done when

- Aflos 0.7.0's release, and the Aflos and Mesh-Node repos, contain no
  WhatsApp or Discord code (their history aside).
- The WhatsApp and Discord integrations, installed by link, do everything
  0.6.x did: sign-in, chats, media, voice notes, calls, video, settings.
- The template's echo integration installs from its GitHub release, passes
  `aflos-integration-test`, and shows its icon, sign-in and chat.
- An integration that crashes, hangs, or sends nonsense never takes Aflos
  down; Aflos shows it stopped and why.
- Changing an integration's permissions in an update asks again.
