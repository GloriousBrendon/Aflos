# Spec: `connection-privacy` (Aflos)

Status: **APPROVED 2026-11-25**, with your changes of the same day (P3, P4,
P7, P12, P14, open question 6). Plan: [PLAN-connection-privacy](PLAN-connection-privacy.md) (DRAFT 2026-11-26). No code yet. Depends on
[`transport`](SPEC-transport.md), [`routing`](SPEC-routing.md),
[`social`](SPEC-social.md), [`spaces`](SPEC-spaces.md) and
[`seed`](SPEC-seed.md) as built in Mesh-Node v0.4.1. Decisions marked
*(your choice)* are yours; *(approved)* marks proposals you accepted.

## Objective

Aflos is a place for close friends to call, chat and have fun, with no data
sold and no server to set up. It isn't a social network: there's no public
directory, and no way to discover strangers.

Today any two Aflos devices that connect also try for a direct path, so
anyone you share a Space with can learn your home IP address. This spec makes
the rule simple: **only friends ever get a direct path to you.** Everyone else
reaches you only through a relay. If you turn on "Hide my IP", friends are
relayed too.

It also sets the safeguards a close-friends app needs without a moderation
team (invite-only Spaces, block and report to the Space owner, an age line),
and states plainly what each person and each operator can still see.

**Not in this spec:** the Space relay (that's
[SPEC-peer-relay](SPEC-peer-relay.md), which makes relayed paths fast for
South Africa), mixing or cover traffic, and hiding *who talks to whom* from a
relay operator.

**User stories**

- I join a friend's Space with people I've never met. None of them can learn
  my home IP address, whatever they run.
- My friends and I still get direct, low-latency calls.
- I turn on "Hide my IP" before a call with someone new to me. Nobody but the
  relay operator sees my address, and I accept that calls get slower.
- I block someone. Any direct path between us ends at once, and they can
  never reach me directly again, even though they used to be my friend.
- I own a private Space of real-life friends and turn on "Allow direct
  connections with members" so our voice channel is fast. Everyone joining
  sees that it's on before they accept.
- The status line tells me whether the person I'm talking to is direct or
  relayed.
- Someone in my Space is abusive. I block them and report them to the Space
  owner. Nobody at Aflos is involved, because there's nobody at Aflos.

## Background: what iroh 1.4.1 can and can't do

Checked in iroh 1.4.1's source (`endpoint.rs`, `endpoint/quic.rs`,
`endpoint/connection.rs`, `socket/remote_map/remote_state.rs`). The spike in
*Testing* confirms it on the wire.

1. **Relay-only is per endpoint, not per connection or per peer.**
   `Builder::clear_ip_transports()` (our `Config::relay_only`) removes the IP
   sockets from the whole endpoint. `ConnectOptions` only carries a QUIC
   transport config and extra ALPNs, with no path policy. Hole punching is run
   per remote endpoint (`RemoteState::trigger_holepunching`), for every
   connection to it, and "any opened paths are opened on all other
   connections". There's no per-peer switch.
2. **Hole punching tells the peer your addresses.** iroh 1.x uses QUIC NAT
   traversal: each side sends its candidate addresses (public and LAN, from
   `local_direct_addrs`) *inside* the encrypted connection, to whoever it's
   connected to, including a peer reached only through a relay. So
   "dial them through the relay" is not enough: they still receive your IP.
   `QuicTransportConfigBuilder::max_remote_nat_traversal_addresses` won't go
   below 8, so iroh's public API can't turn this off per connection.
3. **Rejecting a peer comes too late.** An endpoint registers a connection
   (and with it, its candidates) *before* `EndpointHooks::after_handshake`
   runs, and before our own hello's `Allow` check. A peer who knows your
   endpoint key and dials it may get your candidates even if you then refuse
   it.
4. **An endpoint with no IP transports sends no candidates**, because it has
   none. That's the one dependable tool.

So the privacy line has to be drawn by **which endpoint key a peer knows**,
not by refusing them after they connect.

## Decisions this spec makes (review these)

| # | Decision | Why |
|---|---|---|
| P1 | **Friends get direct paths, with the relay as fallback.** Friends are mutual, accepted contacts (social D8). *(your choice)* | Friends already know a lot about each other; direct paths are what make calls good. |
| P2 | **Everyone else is relay-only, never direct:** members of a Space you share who aren't your friends, and anyone else. It covers chat, DMs, voice and files alike. *(your choice)* | A Space can hold strangers; their knowing your home address is the harm this spec exists to prevent. |
| P3 | **Two endpoints per device.** Because of points 1 to 3 above, the per-peer rule is made with two keys. (a) **The device key runs a relay-only endpoint** (`Config::relay_only`). It's the key every Space member already sees (Space events carry its `DeviceCert`, spaces S5), so whatever a member does with it, they get a relayed path and no candidates. (b) **A second "direct key" runs a normal endpoint**, with IP transports and hole punching. Only friends and your own devices are told it: it travels only in the encrypted address records for friends (routing D3, D9) and in your own account sync, and is certified by the device key, so a friend's device checks it belongs to you. Each endpoint only dials its own kind: your direct endpoint dials friends' direct keys, your relay endpoint dials everyone else's device keys. **The direct endpoint accepts only current friends and your own devices**, and refuses everyone else in the handshake, even someone who has the key (`Allow`, plus an `after_handshake` hook). *(approved; accept rule your change)* | It's the smallest design that holds: per-peer relay-only isn't possible inside one iroh endpoint (Background 1 to 3). A peer can only dial a key they know, so keeping the direct key away from non-friends keeps your IP from them; the accept rule covers a key that leaked anyway (though a refused peer may still get candidates, Background 3, which is why P4 rotates). **Costs:** two relay connections per device (two keep-alives, roughly twice the idle relay traffic, still small on a desktop; open question 6 for mobile); a second key to manage; a format addition to address records; and a friend on an older Aflos dials your device key and gets relayed until they update. |
| P4 | **The direct key rotates** when someone stops being a friend (removed, or blocked), **and every 7 days** regardless. Friends connected at the time are told the new key over their current connection, which then closes; everyone else gets it from fresh encrypted address records, put at once (routing D4). The old endpoint closes. A friend offline for longer than a rotation finds your current record when they come back (routing's normal lookup) and reconnects with the new key. *(approved; 7 days your change)* | A former friend knows the old key, and a leaked copy shouldn't last: with the weekly rotation it dies on its own. Address records already carry what a returning friend needs, so nothing new is needed to find you again. |
| P5 | **Blocking ends any direct path at once, and they're never dialled directly again.** `block` closes their connections on both endpoints (social D10 already closes them), rotates the direct key (P4), and from then on a blocked ID is neither admitted nor dialled. *(your choice)* | A block should be immediate and permanent, and P4 makes "never again" true even if they kept the old key. |
| P6 | **"Hide my IP": relay only for everyone, friends included.** Off by default. With it on, the direct endpoint isn't started and no direct key is published, so friends reach your device key through the relay, like anyone else. Turning it on or off restarts networking (a few seconds). The settings line reads: *"Hide my IP: everyone reaches you through a relay, friends too. Calls get slower (about a third of a second more delay in South Africa)."* *(your choice; wording proposed)* | Some people want their address hidden from everyone, for some calls or for good. One switch, and the cost stated in one line. |
| P7 | **Per Space: "Allow direct connections with members."** Off by default. Only the owner can turn it on, as a signed Space change (spaces S7). **What it allows:** Space traffic between two members who are also friends may go over their direct path. It never opens the direct endpoint to a non-friend (P3): members who aren't your friends stay relayed, and go direct only by becoming friends (SPEC-social, *Add friend* in a Space). **Without it,** Space traffic with a friend is relayed too, over the relay endpoint, even though you have a direct path for DMs and calls (P14). The add-to-Space screen and the Space's header show it: *"Direct connections are on: members who are friends connect directly in this Space."* Your own Hide my IP still wins. *(your choice; friends-only form approved 2026-11-25, no third endpoint)* | A private Space of real-life friends wants fast voice; a mixed Space doesn't. With direct limited to friends (P3), this is the only form P7 can take without a third endpoint, and it keeps the promise that a non-friend never gets your IP. |
| P8 | **The status line shows each connection's path: direct or relayed.** For a call or an open chat it names the other person's path (`Direct`, `Relayed`, `Relayed: Hide my IP is on`); the network status shows how many of each. No addresses are shown. *(your choice)* | People should be able to see what the app is doing, and transport `Status` already says direct or relay, with no IPs. |
| P9 | **The throwaway node for seeds and linking is relay-only** (`throwaway_node` in `helper.rs`, seed D7b). *(approved; ships in Mesh-Node #2, v0.4.1)* | Background 2: today a caller's candidate addresses reach the seed inside the connection, even though the seed is relay-only. The seed is reached through the relay anyway, so this costs nothing for seed traffic. Linking your own devices then needs the relay to be reachable, which it already does from outside a LAN. |
| P10 | **Spaces are invite-only, with no listing or search.** It's already so (spaces S6: add-by-ID by a member). This spec makes it a rule: no future invite link or code may be searchable or listed. *(your choice)* | No discovering strangers. |
| P11 | **Block and report, per member and per Space.** Block a member from their profile or a message (social D10). Report a member *in a Space* to that Space's owner: a signed, encrypted report event to the owner's devices only, carrying the reason and, if the reporter ticks a box, the reported messages. The owner sees it in the Space's settings and can kick, ban or delete (spaces S8). Reports never go to Aflos. A Space whose owner is the problem: leave it, block the owner. *(your choice; mechanism proposed)* | There's no moderation team and no server that could hold reports. The owner is the person who can act. |
| P12 | **Aflos is for people 18 and over.** The terms (a new `TERMS.md`), the README, the installer's welcome page and the AUR `pkgdesc` all say so. *(approved)* | (1) South Africa's POPIA counts anyone under 18 as a child and needs a guardian's consent to process a child's personal information; Aflos holds none centrally, but the line keeps us clear of it. (2) Spaces mix strangers in voice and video with no moderation team, the setting where minors most need one. (3) 16+ would follow GDPR Art. 8's default, but the beta is in South Africa. (4) There's no age check, only a self-declared line, so stricter wording costs nothing. Revisit 16+ if a moderation story exists later. |
| P14 | **A friend's direct key is only for direct connections.** DMs, calls and files with a friend use it; Space traffic with that same person follows the Space's setting (P7): relayed unless the Space allows direct. So a friend in a relay-only Space has two connections with you, one direct and one relayed. *(your choice)* | What a Space shows of you (online times in that Space, its traffic) stays the same whoever the member is; the Space's owner decides, not each friendship. The cost is a second connection per friend you share a relay-only Space with. |
| P13 | **Nothing new is logged.** No path per peer, no direct keys, no IPs in logs; counts only (direct/relayed connections), as routing D10. | As every module before. |

## Behaviour

### The two endpoints (P3)

- At unlock the helper starts the **relay endpoint** (device key,
  `relay_only`, every service) and, unless Hide my IP is on, the **direct
  endpoint** (direct key, IP transports, services for friends only).
- The **direct key** is a device-level key in the keystore, made at first
  unlock after the update, certified by the device key with a `DirectCert`
  (direct key, device key, issued time). It's replaced by P4.
- **Address records** (routing D3) for a friend gain one field: the direct
  key and its cert. Records for Space peers (spaces S2) never carry it.
  Your own devices learn each other's direct keys through account sync.
- **Rotation (P4):** a new direct key and cert every 7 days and on unfriend
  or block. Connected friends get a `direct-moved` message with the new key
  and cert over the current connection, then it closes; records for every
  friend are put again at once.
- **Dialling:** for DMs, calls and files with a friend whose record has a
  direct key, the direct endpoint dials that key, with the relay as
  fallback. Space traffic dials over the relay endpoint, unless the Space
  allows direct and the member is a friend (P7, P14). Everyone else, and an
  older friend with no direct key, is dialled on the relay endpoint.
- **Accepting:** the direct endpoint admits only current friends and your
  own devices, and refuses everyone else in `after_handshake`, key or not.
  The relay endpoint admits everyone `social` allows today.
- **Calls and files** follow the connection they run on; nothing in calls or
  dm changes.

### Settings

- *Privacy* gets the Hide my IP switch with its one line (P6).
- A Space's settings get the owner's P7 switch and the members' notice.

### Status (P8)

- `meshNet` gains `direct` and `relayed` connection counts; each open chat
  or call gets its peer's path.

## Threat model

What each party can learn about you, by setting. "IP" means your public
address and your LAN addresses (both are hole-punch candidates).

| Who | Default | Hide my IP on | Space with P7 on |
|---|---|---|---|
| **A friend** | Your IP, your device key, this week's direct key, when you're online, and your messages and calls (they're in them). | Not your IP. The rest as default. | As default; in that Space, also over the direct path. |
| **A non-friend Space member** | Your ID, display name, device key, when you're online in that Space, and what you post there. **Not your IP**, whatever client they run (they never learn the direct key, and the direct endpoint refuses them). | As default. | As default: P7 only lets friends go direct. |
| **Anyone else** | Nothing: they can't reach your endpoints (no address records for them). | As default. | As default. |
| **A Space owner** | As a member, plus the reports members send them (P11). | As default. | As default. |
| **The relay operator** (n0 today; a Space's relay in SPEC-peer-relay) | **Your IP**, both your keys (they connect from the same address, so they're linked), when you're online, which keys you talk to, how much and when. Never content. | The same, for every conversation including friends'. | The same. |
| **A seed operator** (the home seed: you) | Throwaway keys, sizes, timing (seed D10b). Today also callers' candidate IPs inside the connection, unless P9 ships. | As default. | As default. |

**What relay-only does not hide.** (1) The relay operator sees every IP and
every pairing; relay-only moves your address from your peers to the relay
operator, it doesn't remove it. (2) Timing: when you're online, and when two
keys exchange traffic, is visible to the relay operator and, roughly, to a
member who watches when you post. (3) Your other identities: a friend who is
also in a Space knows both. (4) What you say: anything you post in a Space,
members can keep. (5) A friend is trusted with your IP; a friend's device
that's compromised gives it away.

## Latency in South Africa

n0's nearest relay is in Europe (`euc1-1`). A relayed path between two
people in South Africa goes SA → EU → SA: measured **about 330 to 350 ms
round trip** (the live seed test, 2026-11-25, pinged 352 ms), against
roughly 20 to 60 ms direct (estimate; it varies by ISP). That's fine for
chat and files, noticeable in a voice call (it's like a long-distance call),
and poor for video or games.

Who pays it: every non-friend pair in a Space (P2), everyone with Hide my IP
on (P6), and friends whose hole punching fails.

**SPEC-peer-relay reduces it:** a Space owner in South Africa who opts in
relays that Space's members, so a relayed path is SA → owner → SA, roughly
2× the member-to-owner time (about 40 to 120 ms round trip, estimate), and
the relay operator for that Space is its owner instead of n0.

## Testing strategy

- **Spike first (about 3 hours):** two endpoints in one test process, one
  normal and one relay-only, plus a peer. Capture the peer's view: confirm
  (a) a peer connected to the relay-only endpoint receives no NAT-traversal
  candidates; (b) a peer connected to the normal endpoint does, even when it's
  rejected in `after_handshake` (Background 3); (c) two endpoints with
  different keys share one relay without evicting each other. The result
  settles P3; if (b) is false, a single endpoint with a reject hook may do.
- **Unit:** `DirectCert` issue and check; address records with and without
  the direct key; P4 rotation (unfriend, block, 7 days); P7 change in the
  Space log.
- **Process tests**, on the local relay:
  - A friend pair goes direct; a non-friend Space pair is relayed and
    neither side's stderr or connection state holds the other's IP.
  - A non-friend member who dials the victim's device key gets a relayed
    path and no candidates.
  - Block: the direct path closes within a second; the blocked side dialling
    the old direct key fails.
  - A non-friend who has somehow got the direct key dials it and is refused.
  - Rotation: after a rotation, a connected friend moves to the new key; a
    friend who was offline for longer than one rotation (clock moved past 7
    days, as the mailbox tests do) comes back and reconnects direct with the
    new key; the old key reaches nothing.
  - Hide my IP: a friend pair is relayed; the status says so.
  - P14: a friend pair in a Space without P7 sends Space events over a
    relayed connection while their DM connection stays direct.
  - P7: in a Space with it on, friend members' Space traffic goes direct; a
    non-friend member's stays relayed; after the owner turns it off,
    friends' Space traffic is relayed again.
  - Report: the owner's device gets the report; no other member's does.
- **By hand:** two machines on different networks (one on mobile data):
  friends direct, Space strangers relayed, the status line right in both;
  a packet capture on the stranger's side shows only the relay's address.

## Boundaries

- **Always:** a non-friend never learns your direct key; every path is
  shown as direct or relayed; nothing identifying is logged.
- **Ask first:** any change that would send the direct key or IP candidates
  to someone who isn't a friend; a different age line.
- **Never:** a public directory, Space search, or reports to Aflos.

## Open questions

1. *Resolved 2026-11-25:* P9 ships in Mesh-Node #2, before v0.4.1, with a
   test that a seed gets no candidates from a relay-only caller.
2. **The direct key or a fork?** P3 works with iroh as it is. The
   alternative is asking iroh upstream for a per-connection switch that
   turns off NAT-traversal candidates, then using one endpoint. Recommended:
   P3 now, and raise the upstream issue.
3. **Reports in a Space you own:** should a report also go to admins, or the
   owner only (as the brief says)?
4. *Resolved 2026-11-25:* 18+.
5. **P7 and Hide my IP:** if one member has Hide my IP on in a P7 Space,
   should the Space show "direct, except for N members"?
6. **Mobile (later):** two endpoints means two relay connections kept
   alive: about twice the idle radio wake-ups and keep-alive traffic, which
   costs battery on a phone, and a second endpoint to bring back after
   every network change. Options for a mobile client: start the direct
   endpoint only while the app is in the foreground or a call is open; run
   relay-only (Hide my IP) on mobile data; or, if iroh gains a
   per-connection candidate switch (question 2), go back to one endpoint.
   Not decided until there's a mobile client.
