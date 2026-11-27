# Roadmap to Aflos 1.0

Status as of 2026-11-27, at Aflos 0.5.3. What 1.0 is, and what's left before
it. Version 1 stays desktop only (Windows and Linux), as decided in
[CAPABILITY-MAP](design/mesh/CAPABILITY-MAP.md). Each item gets its own spec
and plan under `design/mesh/` before it's built, the same way as the modules
before it.

## Built (0.4.0 to 0.5.3)

- **identity**: username and password keystore, linked devices, `.mks`
  backup and restore, Log out, Remove from this device.
- **transport, seed, routing**: iroh QUIC by public key, hole punching,
  relay fallback; the home seed, reached only through a relay.
- **social**: friend codes, mutual friend requests, presence, blocking.
- **e2ee**: Olm and Megolm through vodozemac, a key per file.
- **dm and mailbox**: 1:1 and group chats (up to 10), edits, reactions,
  files, offline delivery for about 30 days.
- **spaces**: up to 50 members and 100 channels, roles, kicks and bans,
  reports to the owner.
- **calls and audio**: 1:1 calls and voice channels (up to 10), echo
  cancellation, automatic gain, noise suppression, mic test, per-person
  volume.
- **connection privacy**: only friends connect directly; everyone else goes
  through a relay. Hide my IP, and a direct or relayed indicator.
- **The rest**: the Aflos rebrand, AGPL-3.0-or-later for Aflos and Mesh-Node,
  18+ terms, the Matrix, Discord and WhatsApp bridges.

## Left for 1.0

### 1. Profiles (new module: `profile`)

Profiles you can rice, MySpace style, free for everyone. Nothing is
paywalled.

- **Friends only.** Your profile goes only to friends, whether the
  connection is direct or relayed. Strangers and non-friend Space members
  see your display name and an initials circle, unless you choose to give a
  Space a basic avatar. Friend requests carry no picture.
- **A profile is a rice.** A `profile.toml` using the Rice token system:
  colours, accent, pane style, translucency, the built-in animated
  backgrounds or your own image, layout and density. **Follow my rice**
  applies your desktop theme to your profile.
- **Blocks you arrange**: avatar, banner, bio, links, an image gallery, a
  profile song, a Top 8 (only friends who agreed to be on it), and a status
  or "now playing" line. Animated avatars and banners are allowed.
- **Per-Space profiles**: choose what each Space sees.
- **Space owners** can hide a member's avatar or bio in their own Space.
- **Built from what exists, with no new rendering engine.** It's
  declarative data that Aflos draws with its own iced widgets, and never
  code or HTML from other people. The profile and its media travel as one
  encrypted, versioned bundle over the existing file path. Friends fetch it
  when the version changes and cache it. Mesh-Node gains at most a "profile
  changed to version N" message.
- **Caps, for safety rather than to limit people**: generous size limits;
  decoding with memory-safe Rust decoders, with maximum pixel sizes and
  durations (no decompression bombs); fonts from a bundled set only.
- **Editor**: the Rice settings page aimed at your profile, with a live
  preview.

Next step: `design/mesh/SPEC-profile.md`.

### 2. The beta gate: checks by hand

The list in [CAPABILITY-MAP, "Before beta"](design/mesh/CAPABILITY-MAP.md):

- [ ] The chat screens and the Spaces screens, by eye, on Linux and Windows.
- [ ] Linking the Windows PC from the Linux box.
- [ ] Identity Checkpoint D and transport Checkpoint C on Windows.
- [ ] Routing Checkpoint B (passed in effect; tick it or rerun).
- [ ] Checkpoint C for seed, routing, social, e2ee and dm: two machines on
      different networks.
- [ ] Calls: home network, across two networks, and a group evening in a
      voice channel.
- [ ] PLAN-audio Checkpoint B.

### 3. Network

- [ ] A relay and seed that people outside the home network can reach.
- [ ] A relay in South Africa (relaying through n0's EU relay adds about
      330 ms round trip to voice).
- [ ] `SPEC-peer-relay`: opt-in relays run by friends and Space owners.
- [ ] DHT (pkarr) address records, so the seed becomes optional.
- [ ] Decide whether friend-of-friend lookup (SPEC-routing Q1) is in 1.0.

### 4. Features

- [ ] Video and screen share on Aflos calls (reuse `mesh-media`).
- [ ] Drag and drop on Wayland (waiting on winit 0.31 and iced, #11).

### 5. Packaging and release

- [ ] PLAN-packaging Tasks 1 and 4 to 7: static C runtime, a clean
      machine, an update in Windows Sandbox, the results.
- [ ] Submit the AUR package `aflos-chat`.
- [ ] Linux upgrade over a Mesh install; X11 window grouping.
- [ ] The GitHub social preview image.
- [ ] Drop the old Mesh-named release files once nobody runs 0.3.1.

### 6. Before marketing it as private

- [ ] An outside security review of `mesh-e2ee` (the envelope, sender
      chains and seed protocol are our own code).
- [ ] A contributor licence agreement (CLA), so commercial licences stay
      possible.

### 7. Docs that lag the code

- [ ] README: echo cancellation is built (0.5.2); drop it from "Planned".
- [ ] PLAN-mailbox: Tasks 1 to 5 and 10b are unticked, though mailbox is
      built and shipping; check and tick them.

## After 1.0

- Mobile apps, as light clients.
- Spaces past a few hundred members, and group video for larger groups.
- Moving to a new ID.
- **The business edition** (paid; personal stays free): LAN mode over mDNS
  when the internet is down, an admin console, company-owned accounts, and
  an on-site archive box whose key the company holds and that two people
  must approve to open. Aflos never holds that key.
