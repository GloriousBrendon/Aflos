# Roadmap to Aflos 1.0

Status: **DRAFT 2026-11-26**, R1, R3, R4 and R5 decided the same day (below); R2 open. Written from the capability
map, the specs and what 0.5.0 does today. "1.0" here means: ready to
market and sell to businesses. Each phase gets its spec and plan as usual;
this file only orders the work. Versions follow the rule of 2026-11-26:
small, compatible updates are 0.5.x patches; a minor version only for a
change that alters the app at its foundations.

## Where 0.5.0 stands

Working: identity with password and backups, device linking; friends by
code; 1:1 and group DMs (edits, reactions, files, read marks, typing);
offline mail through seeds; Spaces with channels, categories, roles, bans;
1:1 calls and voice channels (voice only); end-to-end encryption
everywhere; connection privacy (friends direct, everyone else relayed,
Hide my IP); Windows installer and Linux tarball with a self-updater; one
seed, on the homelab. Also: Matrix, Discord and WhatsApp clients in the
same window.

Not there yet: audio controls (gain, echo, meter); video and screen share
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

- **Audio** (SPEC-audio, approved): quiet mics heard (0.5.1), input volume,
  automatic gain, echo cancellation, sensitivity with a live meter, mic
  test, per-person volume.
- **A second seed off the homelab** (a small VPS, SERVER.md), so the
  network doesn't depend on one machine at home; seed monitoring (it's up,
  it's taking callers) with an alert to you.
- **The "Before beta" checks** (CAPABILITY-MAP): linking on two machines,
  identity and transport on Windows, the solo look at every screen.
- **Windows code signing**, so SmartScreen doesn't warn on every install
  and update; the AUR package published; an AppImage or Flatpak.
- **Crash and bug reports without telemetry:** a "Copy diagnostics" button
  (version, OS, the last log lines, no message content) to paste into a
  report.
- **Reliability passes:** reconnects after sleep and network changes,
  mail after long offline spells, big files, many Spaces.
- **A website and a privacy policy** that say plainly what's stored where.
- **Whether Personal keeps the Matrix, Discord and WhatsApp clients**
  (R3), decided before the public launch; if not, release builds leave
  them out (they already build as separate helpers).
- **Two editions from one codebase** (R3): a build switch for Personal and
  Business, with their own name, installer and update channel.

## Phase 2: the foundation changes (0.6.0)

- **Consent-first friendship** (SPEC-social revision 2, PR #9): pairing
  codes and QR, no friend requests; DMs and calls only between friends.
- **Space relays** (SPEC-peer-relay, to write): a Space's own relay, so
  relayed voice in South Africa isn't routed through Europe (about 350 ms
  today).
- **Video and screen share in Aflos calls** (the `calls` module's
  remaining part; mesh-media's H.264 capture is there to reuse), 1:1 first,
  then small groups.

## Phase 3: Aflos Business (0.7.0)

For teams of 5 to 50 (R1); Business only (R3, R5):

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

## What this file isn't

Not dates: each phase is several specs, and their sizes come out of the
plans. Not a commitment to every item: R2 (what's sold) may still reshape
Phases 3 and 6.
