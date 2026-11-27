# Roadmap to Aflos 1.0

Status: **2026-11-27, at 0.5.3.** R1, R3, R4 and R5 decided on
2026-11-26 (below); R2 open. Merged on 2026-11-27 with the ROADMAP.md of
that morning (PR #23, #24): this file is the one roadmap. Written from the
capability map, the specs and what 0.5.3 does today. "1.0" here means: ready to
market and sell to businesses. Each phase gets its spec and plan as usual;
this file only orders the work. Versions follow the rule of 2026-11-26:
small, compatible updates are 0.5.x patches; a minor version only for a
change that alters the app at its foundations.

## Where 0.5.3 stands

Working: identity with password and backups, device linking; friends by
code; 1:1 and group DMs (edits, reactions, files, read marks, typing);
offline mail through seeds; Spaces with channels, categories, roles, bans;
1:1 calls and voice channels (voice only); end-to-end encryption
everywhere; connection privacy (friends direct, everyone else relayed,
Hide my IP); audio controls (automatic gain, echo cancellation, noise
suppression, mic test, per-person volume); Windows installer and Linux tarball with a self-updater; one
seed, on the homelab. Also: Matrix, Discord and WhatsApp clients in the
same window.

Not there yet: profiles; video and screen share
in Aflos calls; macOS; mobile; a second seed; code signing; anything an
organisation needs to run Aflos for its people.

## Decided 2026-11-26

- **R1: small teams of 5 to 50.** An organisation over 50 gets a
  dedicated relay (and seed) when it needs one.
- **R2: open.** Pricing and what's sold, you're planning it.
- **R3: two apps, Personal and Business.** Business never has the Matrix,
  Discord and WhatsApp clients (their terms). Personal may drop them too,
  to stay out of trouble with those companies: **to decide in Phase 1**,
  before Personal goes public.
- **R4: no mobile.** Work happens on desktops; anyone on a phone is on an
  outside network anyway. Phase 4 is macOS only.
- **R5: admin powers are Business only.** Personal keeps today's promise:
  nobody can recover a lost password or read anyone's messages. In
  Business the organisation sets up its members' accounts (so it knows
  their passwords), and an organisation device can keep every member's
  backup key, for recovery and offboarding. Business's terms must say so
  plainly: whoever holds those passwords and keys can open those accounts.

## The questions as first asked

| # | Question | Why it matters |
|---|---|---|
| R1 | **Who's the first business customer?** Small teams (5 to 50 people) who want private chat and voice without a vendor holding their data is the natural fit; regulated or large organisations need far more (audits, retention, SSO). | Everything in Phases 3 and 4 depends on it. |
| R2 | **What's sold?** Options: (a) hosted seeds and relays per organisation, as a subscription, with the app free; (b) a commercial licence for organisations that won't take the AGPL; (c) support contracts; (d) per-seat. Likely (a) + (c), with (b) on request. | There are no accounts to bill, by design. A business's paid thing has to be infrastructure, licence or service, not "your account". |
| R3 | **One app, or a business edition?** The consumer app is "for close friends, 18+". A business edition would drop the Discord, WhatsApp and Matrix clients (unofficial Discord and WhatsApp clients break those services' terms; no business can be sold that) and add organisation features. | Brand, the 18+ line, what ships in which installer. |
| R4 | **Mobile before 1.0?** Businesses expect phones. A light mobile client (no always-on node, reached through seeds and relays) is the largest single item on this list. | Could be 1.0, or a 1.x after selling to desktop-first teams. |
| R5 | **What may a business admin do that the privacy design doesn't allow today?** Today nobody can recover a lost password, read a member's messages or keep a copy. Businesses ask for account recovery, offboarding, retention and export. Each needs an opt-in design that's honest about what it costs privacy. | These change promises in the specs; they're yours to decide. |

## Phase 1: stable for the public (0.5.x)

What a stranger needs to install it, use it daily and not hit a wall.

- [x] **Audio** (SPEC-audio): quiet mics heard and input volume (0.5.1),
  automatic gain, echo cancellation, mic test, per-person volume (0.5.2).
  Left: PLAN-audio Checkpoint B by hand.
- **A second seed off the homelab** (a small VPS, SERVER.md), so the
  network doesn't depend on one machine at home; seed monitoring (it's up,
  it's taking callers) with an alert to you.
- **The "Before beta" checks** (CAPABILITY-MAP), by hand: the chat and
  Spaces screens on Linux and Windows; linking the Windows PC from the
  Linux box; identity Checkpoint D and transport Checkpoint C on Windows;
  routing Checkpoint B (passed in effect); Checkpoint C for seed, routing,
  social, e2ee and dm on two networks; calls at home, across two networks
  and a group evening in a voice channel.
- **Packaging:** Windows code signing, so SmartScreen doesn't warn on
  every install and update; PLAN-packaging Tasks 1 and 4 to 7 (static C
  runtime, a clean machine, an update in Windows Sandbox); the AUR package
  `aflos-chat` published; an AppImage or Flatpak; a Linux upgrade over a
  Mesh install and X11 window grouping checked; the old Mesh-named release
  files dropped once nobody runs 0.3.1.
- **A contributor licence agreement (CLA)**, so commercial licences (R2 b)
  stay possible while others contribute.
- **Docs that match the code:** PLAN-mailbox's built tasks ticked.
- **Crash and bug reports without telemetry:** a "Copy diagnostics" button
  (version, OS, the last log lines, no message content) to paste into a
  report.
- **Reliability passes:** reconnects after sleep and network changes,
  mail after long offline spells, big files, many Spaces.
- **A website and a privacy policy** that say plainly what's stored where.
- **The Matrix, Discord and WhatsApp clients leave both apps** for a
  **plugins and themes store** (your idea, 2026-11-26), as VS Code's or
  Obsidian's work: authors publish plugins from their own repositories,
  the store is an index we can take entries off. The helpers already talk
  to Aflos over a line protocol, so that's the plugin boundary; the store
  adds signing, a permission prompt (what a plugin connects to), updates
  and a kill switch. Ask a lawyer before Discord, WhatsApp or Teams
  plugins are listed: running or curating the store may still count as
  distributing them. Until the store exists, release builds of both apps
  leave the helpers out.
- **Two editions from one codebase** (R3): a build switch for Personal and
  Business, with their own name, installer and update channel.

## Phase 2: the foundation changes (0.6.0)

- **Profiles you can rice** (SPEC-profile, approved 2026-11-27;
  PLAN-profile): MySpace style, free for everyone, nothing paywalled. A
  `profile.toml` on the Rice token system, Follow my rice, avatar, banner,
  bio, gallery, a profile song, a Top 8, a status; per-Space cards. Only
  friends get your profile; it's drawn by Aflos from declarative data,
  never code, and travels as one encrypted, versioned bundle.
- **Consent-first friendship** (SPEC-social revision 2, PR #9): pairing
  codes and QR, no friend requests; DMs and calls only between friends.
- **Space relays** (SPEC-peer-relay, to write): a Space's own relay, so
  relayed voice in South Africa isn't routed through Europe (about 350 ms
  today), and friends who relay for friends, opt-in.
- **Address records in the DHT** (pkarr), so finding someone doesn't need
  a seed; seeds stay for mail.
- **Video and screen share in Aflos calls** (the `calls` module's
  remaining part; mesh-media's H.264 capture is there to reuse), 1:1 first,
  then small groups.

## Phase 3: Aflos Business (0.7.0)

For teams of 5 to 50 (R1); Business only (R3, R5):

- **LAN mode:** an organisation's people keep chatting and calling over the
  office network when the internet is down (mDNS discovery), the case you
  sell on in South Africa (load shedding, cable theft, fibre outages). The
  switches and access points need a UPS for it to hold.
- **Organisations:** a set of Spaces and members under one owner and
  admins, with invite links (never listed or searchable) and offboarding
  (one action removes someone from every Space of the organisation).
- **The organisation's own seed and relay**, one command (Docker), so its
  traffic and mail stay on its machines; Aflos points at it from an invite.
  This is also the thing that can be sold hosted (R2 a).
- **Within-organisation directory**, opt-in: members can find each other
  inside the organisation, still never outside it.
- **Accounts the organisation sets up** (R5): an admin creates each
  member's identity and password and hands the device over; an
  organisation device (the "main PC") keeps every member's backup key, so
  a lost password or a departed member is recoverable.
- **Admin controls:** message expiry per Space (retention), an owner's
  export of a Space's history, offboarding with the kept keys.
- **An on-site archive box** (optional, sold or rented): a small machine
  that keeps an encrypted copy of the organisation's messages for audits.
  The organisation holds its key and two of its people must approve before
  it's opened; Aflos never holds it. Staff are told in their contracts.
- **A dedicated relay and seed for organisations over 50** (R1), hosted
  by us.
- **Granular roles and permissions** per channel.

## Phase 4: macOS (0.8.0)

- **macOS** (the Rust stack builds there; needs signing and notarisation).
  No mobile before 1.0 (R4).

## Phase 5: trust and scale (0.9.0)

- **An external security audit** of the cryptography and protocols
  (vodozemac use, address records, mail, the direct-key handover), and a
  published threat model.
- **Scale tests at the sizes sold:** Spaces of a few hundred, voice
  channels of 25 or more, many devices per person.
- **Accessibility** (screen readers, keyboard-only), and the UI and UX pass
  you've put off.

## Phase 6: ready to sell (1.0)

- **Licence and pricing** (R2): the commercial licence text, prices, how a
  customer pays and gets their hosted seed.
- **Legal:** terms for businesses, a data-processing agreement (POPIA,
  GDPR) for hosted seeds, the trademark registered.
- **Support:** a channel, response times, a status page for hosted seeds.
- **Sales material:** the website's business page, a one-page security
  overview, a comparison with Slack, Discord and Teams that's honest about
  what Aflos can't do (no server-side search, history from online members).

## Later (after 1.0)

- Friend-of-friend lookup (SPEC-routing Q1).
- Drag and drop on Wayland (waits on winit 0.31 and iced, #11).
- Mobile (R4).

## What this file isn't

Not dates: each phase is several specs, and their sizes come out of the
plans. A business pilot can start on Phase 3's first release, before 1.0. Not a commitment to every item: R2 (what's sold) may still reshape
Phases 3 and 6.
