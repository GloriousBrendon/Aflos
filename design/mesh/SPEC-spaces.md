# Spec: `spaces` (Mesh)

Status: **APPROVED 2026-11-14** (revision 3), with
[PLAN-spaces.md](PLAN-spaces.md) for your review. Module 9 of the build order in
[CAPABILITY-MAP.md](CAPABILITY-MAP.md). It depends on [`dm`](SPEC-dm.md),
[`mailbox`](SPEC-mailbox.md), [`e2ee`](SPEC-e2ee.md),
[`routing`](SPEC-routing.md) and [`social`](SPEC-social.md), all built.

**Revision 3, after your review of revision 2:**
- S5 keeps only the device certificate (a). The `device-out` change and the
  `after` link are cut. History counts an event by its certificate and
  signature, the author's membership somewhere in the chain held, and their
  right to post in that channel in some version of it. Test 4 is rewritten
  and test 5 gains the announcements case.
- S14 gives the lookup rate for peers that can't be reached. S9 says what the
  quit flush fits. S7 redoes a change each time it's dropped.
- Decision changes 1 to 4 are approved. So are the fork fix, the redo rule,
  the cap of 10 Spaces, and the known limit on addresses and online times.

**Revision 2, after your review of revision 1:**
- Only the owner removes, bans or demotes an admin. Honest changes on a
  losing branch are redone. Test 7 is rewritten (S7).
- Old events are checked against author-signed device certificates, with a
  time for each removed device (S5). Test 4 is new.
- The connection model (S14), the mail worst case (S9) and the contact cap
  (S2) are given in numbers.
- What members can learn about each other is stated plainly (*Known
  limits*).
- Cut from this version: invite codes, and admins-only channels for seeing.
- Your answers are recorded: 50 members, full history, Olm, batched mail,
  add-by-ID only, and "move to a new ID" not in this version.
- Change 5 (admins delete anyone's message) is approved. Changes 1 to 4 wait
  for this revision.

## Acceptance tests

Process tests in Mesh-Node (`tests/spaces.rs`) run helpers and a seed, as
`tests/dm.rs` and `tests/mailbox.rs` do. The Mesh-Voice checkpoint test
(`checkpoint.rs`) gains a Spaces step. The last two tests are by hand.

In the process tests, **O** owns the Space. **B** and **C** are O's
friends but not each other's. **D** is B's friend and nobody else's. **X**
and **Y** are admins.

1. **Strangers in one Space.**
   - O creates a Space with `#general`, and adds B and C.
   - B and C talk in `#general` both ways, live, in under 1 s (the first
     line includes a dial).
   - They still aren't friends:
     - neither shows in the other's friends list;
     - neither gets the other's presence;
     - a DM between them is refused (`not-friends`).
   - Once C leaves, C's devices no longer get through B's handshake.
   - 60 s after the last line, B and C hold no connection to each other
     (S14).
2. **Away and back.**
   - C locks. B posts three lines and locks too.
   - C unlocks with nobody else online, and gets all three by mail, in order.
   - The seed counts **at most one put per C device** for those three lines
     (S9).
3. **A friend of one member joins, and sees the past.**
   - B adds D. D gets `#general`'s history from before D joined, newest
     first, from whichever member it reaches first.
4. **History survives a replaced device and a departure** (S5).
   - C posts from their first device, links a new device and posts from it,
     removes the first device, then leaves the Space.
   - E then joins (added by O). In history, E sees C's lines from both
     devices as C's.
   - Live, a line signed by C's removed device is refused by every member
     while C is still in the Space, because it's not on C's current device
     list.
5. **History can't be forged.**
   - A member's test client alters the text of another person's event in
     the history it serves. D refuses that event (bad signature), takes the
     rest, and gets the real one from another member.
   - **The announcements case:** a member who has never been an admin signs
     a line into an admins-post-only channel and serves it in history. A new
     member refuses it, because in no version of the chain could its author
     post there.
6. **Roles.**
   - A member can't create a channel, delete someone else's message, or post
     in an admins-post-only channel. Every member's device refuses it, not
     just the sender's.
   - An admin deletes one of C's messages, and it's gone on every device.
   - An admin can't remove, ban or demote another admin. Every device refuses
     it.
7. **Forks** (S7):
   1. **An ex-admin can't win.** O removes admin X. X, apart and with a
      modified client, forks from before the removal with a ground-low event
      ID and a low `seq`, and kicks B and renames `#general`. When they meet,
      every device follows O's branch: X is out, B is in, and the name
      stands.
   2. **Two admins, apart.** X kicks C while Y renames `#general`, on two
      branches. When they meet, every device ends with both changes: the
      losing branch's author's device redoes its change on the winner.
   3. **A late fork undoes, and the author redoes.** Y, with a modified
      client, forks from an old version to drop X's ban of C. X's device
      sees its ban dropped and redoes it. Every device ends with C banned.
8. **Kick and ban.**
   - O kicks C, and C can be added again.
   - O bans C. C's later events and any add of C are refused everywhere, and
     C's devices stop getting through every member's handshake, except where
     C is also someone's friend.
9. **Fifty members** (an `--ignored` measurement, recorded in the plan). A
   Space of 50 members, one device each:
   - a line reaches all 49 others;
   - unlock with 5 such Spaces is under 1 s after "online";
   - recorded: for one line, the sender's CPU time and bytes, the
     connections open and the seed puts; over an hour with 45 members
     away, the seed's address lookups (S14); and for a returning device's
     hello, its lookups and how long it takes to reach everyone online.
12. **A returning member catches up live** (S14, *added 2026-11-14*). C
    comes back while A and B are talking. C sees A's next line in under 5 s,
    without posting anything.
10. **Unlock doesn't read Space history.** Unlock takes the same time with a
    Space holding 100,000 events as with an empty one (S11).
11. **Nothing identifying is logged.** The seed's files and logs and the
    helper's stderr hold no Space ID, channel name, member ID or message text.
13. **By hand, on the gate** (three machines, two networks, an outside seed):
    the above with real people, in Mesh-Voice.
14. **By hand, a solo look** at the screens: the Space rail, channels and
    categories, the member list with roles, Space settings, adding a member,
    unread counts and mentions, and the narrow layout.

## Objective

Today Mesh has friends, 1:1 chats and group DMs of up to 10 friends. A
Discord server is something else: people who aren't all friends, talking in
named channels, with someone in charge who can remove people. That is what
`spaces` adds.

**The smallest version that replaces a small Discord server for text:**

- A Space of **up to 50 members** (Q1), who needn't be friends.
- **Text channels in categories**, which the owner and admins create, rename,
  reorder and delete.
- **Three roles:** the owner, admins and members. Each channel has one
  switch, who may post: everyone, or admins only (for announcements).
- **Joining** by being added by a member who is your friend (Q5).
- **History for new members**: everything said before they joined (Q2).
- **Moderation:** admins delete anyone's message, and kick or ban members.
- **Everything a DM already has**, per channel: edits, deletes, reactions,
  replies, files and pictures, typing, unread counts and read state.
  **Mentions** are added: `@name`, and `@everyone` for admins.
- **It works while people are away**: by mail through the seed, and from
  whichever member is online.

### Not in this version

- **Invite codes** (cut 2026-11-14). They would add joining without first
  being anyone's friend: a Space invite code works like a friend code, whose
  knock asks to join and is admitted by the inviter's device on its next
  collect. It reuses the friend-code hold, lookup, knock box and seed limits,
  plus one new knock purpose and auto-admit.
- **Channels only admins can see** (cut 2026-11-14). They would add a mods'
  room. Its events go only to admin devices (live, mail and history); a new
  admin gets its history, and a demoted one keeps what they had. That's a
  second audience per Space and a role check when serving history.
- Voice channels (`calls`).
- Custom roles, per-person permissions and per-role channel access.
- Threads, pins, slow mode, timeouts, an audit log, server emoji, webhooks
  and bots, server discovery.
- Presence for members who aren't your friends. Friends show as they do now.
- Spaces past 50 members (*The path past 50*).
- Per-Space nicknames and avatars. Display names come from joining.
- Notification settings per channel, beyond muting a whole Space.

## What it reuses

**From `dm`** (Mesh-Node `mesh-dm` and the helper), close to unchanged:

| dm | In a Space |
|---|---|
| D1 events | The same format, with a channel and the change it follows (S5) in it, and **signed by the author's device** (S5) so other members can pass it on. |
| D2 ordering | The same `seq` and `n` rules, per channel. |
| D3 edits and deletes | The same, plus an admin may delete anyone's message (S8; change 5, approved). |
| D4 reactions | Unchanged. |
| D5 delivery counters | Unchanged: per reader device, the sender's own events. This is what makes "sending" and "sent" honest. |
| D7 signed change chain | Generalised from the creator to roles, with redo (S7). |
| D8 own-device sync | Unchanged: your own devices get your Spaces as they get your chats. |
| D9 read state | Per channel. |
| D10 typing | Per channel, to connected member devices. |
| D11 store | Segments per channel. The device-wide log changes for Spaces (S11). |
| D12, D13 files | Unchanged: members fetch from any member device that has the file. |
| D14 limits | The same, plus the Space limits (*Limits*). |
| Group DMs over 1:1 Olm sessions (e2ee D6 as changed) | **Spaces too, in this version** (S4, Q3). |

**From `mailbox`**, unchanged in format:

- A Space member is a pair like a friend is, because routing D1 needs only
  the two IDs. So mail boxes, slots, claims, mail records, receipts and the
  six-week collect all work for them as they are.
- The one change is when Space events are mailed (S9).

**From `routing`:** address records, reach and the handshake allow-list
cover Space members (S2).

**From `e2ee`:** Olm sessions per device and `encrypt_device_batch`, as
group DMs use them. The Megolm code (`mesh-e2ee` `group.rs`) is built and
stays unused in this version. It's what the path past 50 uses.

## Decisions (approved 2026-11-14)

| # | Decision | Why |
|---|---|---|
| S1 | **A Space is a signed log of changes plus channels of events.** Its ID is `blake3::derive_key("mesh/v1/space", creator ID ‖ random)`, as a group DM's is (dm D7), so nobody else can make a creation for it. The log holds: the name; the channels and categories, each with its post switch; the members, each with their role and the display name they joined with; the bans; and whether members may add people. | The group DM's design, which is built and tested, with more kinds of change. |
| S2 | **Members are "Space peers", a new kind of contact.** For each member it shares a Space with, a device keeps an account entry of kind `space`. That kind publishes address and mail records, allows the handshake, and reads their records, exactly like `outgoing`. But **a connection with one never makes a friend** (social D8 excluded), and it gets no presence and no DMs. Leaving or being removed drops the entry, unless the two share another Space or are friends. Blocking still wins: a blocked member is never a peer, and their events are hidden. **Peers count against `MAX_CONTACTS` (1,000), once per person** however many Spaces you share. An add that would take anyone past 1,000 is refused for them, with that reason. With `SPACES_PER_ACCOUNT` at 10, Spaces alone use at most 490, which leaves at least 510 for friends. <br>*Changed 2026-11-14 (mixed versions): **peers aren't account entries.** They're worked out from the Spaces held: every member of every Space you're in, less yourself, your friends and blocked IDs. Routing, mail and the handshake take your contacts plus that set, and `MAX_CONTACTS` counts the two together, once per person. Nothing new is synced: your own devices get your Spaces by own-device sync (dm D8) and work out the same set. A 0.2.0 device can't read an account holding a kind it doesn't know, so a new kind would have broken all account sync to it. Where this cell says "account entry of kind `space`", read "a member of a Space you hold".* | Every lower layer keys on "contact", and the pair secret needs only the two IDs, so this is the smallest way to let strangers in a Space talk. Counting against the one cap keeps the seed's limits true: `MOST_BOXES`, the record put and get limits and the address puts are all sized from `MAX_CONTACTS + 1`. 20 Spaces of 50 would be 980 peers before any friend, so the Space cap drops to 10. |
| S3 | **Channels.** A channel is a conversation inside the Space, with its own ID (from the Space ID and a random value), its own segments, read marker and typing. One switch: **post** for everyone, or admins only. Every member sees every channel. Categories are names that channels sit under, in order. | Announcements are the one per-channel rule small servers rely on. Hidden channels are cut (above). |
| S4 | **Encryption is per device over Olm, as group DMs** (e2ee D6 as changed 2026-11-11; Q3). A Space event is encrypted for each current member device in one `encrypt_device_batch`. | It's what's built and tested, and it lets mail stay per pair (S9). At 50 members of about 2 devices, a line is roughly 100 Olm encryptions; test 9 measures it. Megolm encrypts once, but offline members then need a shared Space box at the seed and a key-share round on every removal. That's a seed change and more than 50 members need. |
| S5 | **Space events are signed by the author's device, and history checks them without the author.** Each event carries the signing device's `DeviceCert`, which is signed by the author's ID (identity). It checks with nothing else, so an event stays checkable after the author has removed that device or left the Space. <br>**Live** (straight from the author, or by mail), the rule is today's: the signing device must be on the author's current device list, and the author must be a member allowed to post in that channel in the version you hold now. <br>**In history** (S10), an event counts if: (1) the certificate and the signature check; (2) the author is a member in some version of the chain you hold; and (3) in some version of that chain, the author was allowed to post in that channel. Events name no version, so nothing depends on which branch a change landed on. | Honest members store only what they accepted live, so history can be wrong only where its server is dishonest. Against a dishonest server, the certificate stops the forging of anyone else's lines. Check (3) stops a member who was never an admin from serving their own lines into an announcements channel's history. A `device-out` time or a per-event version link would bind only an ex-member working with a dishonest server, and back-dating beats both (*Known limits*), so they're cut. |
| S6 | **Joining is add-by-ID only** (Q5). A member whose role allows it adds one of their friends with a signed add change. Members may add by default, and the owner can make it admins only. The add carries the new member's display name. Every member's device, on accepting the change, makes the new member a peer (S2). | Group DM's add, which is built. Invite codes are cut. |
| S7 | **Roles and the change chain.** dm D7's chain, with roles instead of "the creator". <br>**The owner** (the creator) can do everything, and is the only one who removes, bans or demotes an admin, makes admins, transfers ownership, or deletes the Space. **Admins:** channels and categories, kicking and banning members, deleting messages, whether members may add, and the Space's name. **Members:** post where allowed, react, leave, and add friends if allowed. <br>A change counts only if it follows the newest change held and its author has the right in that version. **On a fork, the branch containing an owner change wins.** If both do, the lower (`seq`, event ID) of each branch's first owner change wins. Then the same with admin changes, then with any change. <br>**Honest changes on a losing branch are redone:** a device whose own change was dropped makes it again after the winner's newest change, if it's still allowed and still changes something; otherwise the device shows it as undone. **A redone change that is dropped again is redone again**, once for each fork it loses, so a second fork can't win for good. A device redoes a change only after it has seen a winning branch that lacks it, so two honest devices can't trade redos forever. | **The ex-admin hole is closed:** only the owner removes an admin, so an ex-admin's fork must start before an owner change, and its branch, holding no owner change of its own, always loses. The ex-admin can't author one, and grinding IDs changes nothing. If an owner device made a change on the ex-admin's branch, both branches hold owner changes, and the tie-break compares the owner's own `seq` and IDs, which only the owner controls. **What a current admin can still undo with a late fork:** any admin or member change made after a fork point of their choosing (kicks, bans and unbans of members, channel changes, renames, adds), by grinding a lower ID and back-dating `seq`. Never an owner change, and never an admin's removal. That's no power beyond what an admin has openly, since they could revert each change anyway, but it's quieter. And redo puts honest changes back once their authors next come online, so a late fork buys delay, not a lasting result. The owner can then remove that admin. |
| S8 | **Moderation.** *Delete:* an admin's `Delete` of anyone's event, signed, checked against the role in force, and honoured by honest clients as dm D3 (change 5, approved). *Kick:* remove a member; they may be added again. *Ban:* remove a member and add their ID to the ban list. From then on, their events and any add of them are refused everywhere, and every member drops them as a peer. Removing, banning or demoting an admin is the owner's alone (S7). | Discord's three basic tools. As everywhere in Mesh, a modified client keeps what it already has (map). |
| S9 | **Space events are mailed in batches** (Q4). For a reader device with no open connection, the events waiting for it **from every Space you share** go at the next refresh (30 minutes) or at lock and quit (the flush), **in one envelope per reader device**, packed up to the envelope limit. DMs are unchanged (mailbox M2: right after the event). Receipts and counters are as mailbox M9 and dm D5. <br>**The worst case against `PUTS_PER_KEY` (600 an hour):** you're in 10 full Spaces, which is 490 peers of about 2 devices, everyone is away, and you post in all of them. That's up to 980 puts per refresh, so about 1,960 an hour, plus one put per DM event per away device. **At the limit**, the seed answers `slow-down` to the rest. Those events stay waiting on the sender, with their counters unmoved (dm D5), and go at the next refresh, in the next hour's window. The order is DMs first, then the reader devices mailed longest ago, so none starves. Nothing is lost. The worst extra delay is a refresh or two (about an hour) for the last devices. <br>**At quit**, the flush (`MAIL_FLUSH`, 3 s, mailbox M2) sends one batch per holder, pipelined on one stream. It fits **as many puts as the hour's `PUTS_PER_KEY` budget has left**: at most 600, fewer if that hour's refreshes used some. Time isn't the limit at 600: the seed stored 256 synced puts in 225 to 242 ms (PLAN-mailbox Task 16), and 600 envelopes of a few KB is a couple of MB, which a 20 Mbit/s upload sends in about a second. Events that don't fit wait on the sender's device until Mesh is next opened. Then they go live to whoever is online, and by mail at the first refresh. The beta case is about ten people: a person in two Spaces of ten has about 40 devices to mail, about 80 puts an hour. | One put per event per away device would let a 50-member Space post about 6 lines an hour while people are away. Batching makes the cost about two puts an hour per away device, however much is said. The delay costs nothing that matters: a reader who comes back sooner meets the sender live, or another member (S10). |
| S10 | **Catch-up and history come from any member** (Q2: everything). On joining, and whenever a member device comes back after more than mail covers (30 days, or past dm D5's 2,000 events), it asks the first member device it reaches. Each channel comes newest first, in batches of 200, by a per-(Space, channel) counter like own-device sync (dm D8), so it resumes where it stopped. The changes come first, so S5's checks have the chain they need. <br>**A post from history can be news** (*added 2026-11-16 at your review*). A new member's history can hold a line written after the add, and its live copy then comes as a duplicate. So a post by someone else in history is told as a live one is when its time is no more than 2 minutes before the time of the change that added us (`JOIN_CLOCK_TOLERANCE`). Those are two devices' clocks: a post from a clock more than 2 minutes behind the adder's isn't told, and one from just before the add may be. This is a second use of the sender's time, which dm D2 calls display-only: it decides only whether a notification shows, never order or what's stored. | The map's "history comes from other members". A pull is resumable and costs nothing while nobody asks. |
| S11 | **A Space's log is read when the Space is first used, not at unlock.** dm's device-wide log (every stored event's number and every deleted ID) stays for 1:1 chats and group DMs. Each Space gets its own, read the first time the Space is opened, sent to or received in during that unlock. | The map's open question: "before `spaces` reuses the dm store, it must change how the history log is held." This is the smallest change that keeps unlock flat (test 10). A Space's log in memory is about 16 MB per 100,000 events (PLAN-dm Task 15), bounded by the Space, not the account. |
| S12 | **Mentions and unread.** A `Text` event may list mentioned member IDs, and `@everyone` if its author is an admin. Unread counts are per channel, from the read marker (dm D9). A mention also shows on the Space in the rail. One switch mutes a whole Space. | What makes a busy server usable. |
| S13 | **Nothing identifying is logged.** No Space IDs, channel names, member IDs or contents, only counts. | As routing D10, social D15 and dm D15. |
| S14 | **Connections: dialled to send, closed when quiet.** <br>**Who dials:** a device with Space events for a member's device dials it then, as dm D5 does for a friend (reach: a cached address, else the seed's record), in parallel for every member device it has events for. Space peers aren't dialled at unlock or at each refresh, as friends are for presence. <br>**When connections close:** 60 s after the last stream on it, the helper closes a connection to a device that's only a Space peer (no friend, not your own). Friends' connections stay open as now. <br>**How a line reaches 49 members live:** it's encrypted for each member device (S4) and sent on each open connection, dialling the rest in parallel. A cold dial took 0.64 s on the routing smoke test. Members already in the conversation within the minute reuse their connection. Members who are away get it by mail (S9). <br>**Looking up a peer who can't be reached:** one address lookup at the seed covers all of a person's devices (routing D2). After a lookup that finds no record or a failed dial, that person isn't looked up again for 30 minutes (`PEER_RETRY`), whatever is sent meanwhile. Their events wait and go by mail. A busy 50-member Space with 45 members away costs **45 lookups in the minute it gets busy, then about 90 an hour**, against the seed's 120 a minute per device (`GETS_PER_DEVICE`, so 7,200 an hour). At the limit of 10 busy Spaces, 441 away peers would want looking up in the same minute. Past 120, the seed answers `slow-down`, and the rest are tried in the following minutes. That delays only a live try for people who are away anyway. Steady state is then about 880 an hour. A peer who comes back while the sender is quiet gets the lines at the sender's next refresh by mail, or live on the sender's next line once `PEER_RETRY` has passed: up to 30 minutes either way. *Replaced 2026-11-14 at your review of the plan by the hello below.* <br>**A returning device says hello** (*added 2026-11-14*). When a device comes online, it dials each of its Spaces' members once: that's the hello, an ordinary connection with no new message. Lookups stay under `GETS_PER_DEVICE` (at most 100 a minute, `HELLO_LOOKUPS`). At most 32 dials are under way at once (`HELLO_DIALS`), and dialling pauses while open connections are above 192 (`HELLO_ROOM`, three quarters of the 256 cap). <br>On the other side, an incoming connection from a person clears `PEER_RETRY` for them, and dm D5's "when a connection opens" sends them what waits. The connection then closes at `PEER_IDLE` as any other, and the sender's next line dials them again, because the retry was cleared. Members who are away cost one lookup each. A full Space of 49 others is 49 lookups and up to 49 dials, about 30 s at the 100-a-minute pace. Ten full Spaces (490 peers) is about 5 minutes, which test 9 records. *Corrected 2026-11-15 by the budget below.* <br>**One budget for every address get** (*corrected 2026-11-15 at your review*). A lookup is two gets at the seed (today's record and yesterday's, routing D2), so the hello's 100 people a minute was 200 gets against the seed's 120. The key lookup, a send's dial and the short retry ask the seed too. In the fifty-member set-up, the seed refused 21,816 of 39,988 address gets, and a refused get was taken as "away". Every address get a helper makes now comes from one budget: 110 in any 60 s (`GETS_A_MINUTE`, a sliding minute, so it holds wherever the seed's minute starts). What doesn't fit waits its turn. The hello takes only what leaves 20 free (`SPARE_KEEPS`), so sends and key lookups go first. That paces the hello at 45 people a minute at most: a full Space of 49 others takes about 65 s, and ten full Spaces (490 peers) about 11 minutes after unlock. Members already connected cost nothing, and friends' lookups at unlock come first, at up to 55 people a minute. A get the seed refuses anyway is asked again after 10 s, three times. It's told apart from "away": it starts no `PEER_RETRY`, costs no hello try, and the person is dialled again soon. <br>**A budget per seed** (*corrected 2026-11-16 at your review*). A lookup asks every seed, and each seed counts its own limit, so each seed has its own 110 a minute. A second seed then doesn't halve the pace above. <br>**A miss backs off to the refresh** (*added 2026-11-16 at your review*). A Space send that didn't go (no keys for the device yet, no answer, or a batch answered `failed`) and a hello that found nobody are tried again after 10 s (`BACK_OFF_FIRST`), the wait doubling after each miss: 10, 20, 40 s and so on, up to 1,280 s. When the next wait would reach the 30-minute refresh, the refresh takes over. Before, the send was tried six times 10 s apart and the hello three times, so both gave up after about a minute. In the fifty-member set-up, every member then waited on the refresh with sends to 36 to 48 of the other 49 outstanding, and the first line to all didn't reach one member in 300 s. A lookup the seed refused is no miss: it's dialled again after 10 s and takes no step of the back-off. Neither starts `PEER_RETRY`. <br>**How many are open:** at 50 members, while a channel is busy, up to 49 times their online devices (about 75 at 1.5 devices online each), plus friends. At rest, none for peers. At the limit of 10 busy Spaces, up to about 735. Past the helper's 256 open connections (transport `Limits`), new incoming ones are refused, and the events for those devices wait and go by mail at the next refresh. The cap stays, and test 9 records the count and memory. <br>**A relay-only member** (no hole punch) sends and receives everything through their home relay. A line they post is one envelope per member device, about 1 KB each, so about 100 KB up through the relay for a full Space. They receive each line once per device of theirs. That costs the relay's operator bandwidth (100 lines a day is about 10 MB), adds the relay's round trip to every line, and is capped per client at 2 MB/s (`relay.toml`). | Dialling every peer at unlock, as friends are dialled, would hold up to 490 connections open all day for nothing, past the 256 cap. Dialling to send costs one dial per quiet minute and keeps the count at the people you're actually talking with. |

## Limits

| Name | Value | Why |
|---|---|---|
| `SPACE_MAX_MEMBERS` | 50 (Q1) | S4, S9 and S14 are sized for it |
| `SPACES_PER_ACCOUNT` | 10 | 490 peers at most, inside `MAX_CONTACTS` with room for friends (S2) |
| `SPACE_MAX_CHANNELS` | 100, in up to 20 categories | Discord's own cap is 500 |
| `PEER_IDLE` | 60 s | S14 |
| `PEER_RETRY` | 30 minutes | S14: one lookup per away person per refresh |
| `HELLO_LOOKUPS` | 100 a minute | S14: under the seed's 120 (`GETS_PER_DEVICE`). *Replaced 2026-11-15 by `GETS_A_MINUTE` and `SPARE_KEEPS`: it counted people, and each costs two gets.* |
| `GETS_A_MINUTE` | 110 address gets in any 60 s, at each seed | S14 (*added 2026-11-15*; *per seed 2026-11-16*): every lookup a helper makes, under each seed's 120 |
| `SPARE_KEEPS` | 20 gets | S14 (*added 2026-11-15*): the hello leaves these for sends and key lookups, so 45 people a minute |
| `BACK_OFF_FIRST` | 10 s, doubling each miss, until the refresh | S14 (*added 2026-11-16*): a failed Space send, and a hello that found nobody |
| `HELLO_DIALS` | 32 at once | S14 |
| `HELLO_ROOM` | 192 open connections | S14: hello pauses above three quarters of the 256 cap |
| `HISTORY_BATCH` | 200 events | As dm D8 |
| Text, files, reactions | dm D14, D12 | Unchanged |

## Known limits

- **Every member can learn every other member's network address and when
  they're online.** "No presence" holds only for an honest client.
  - A member's device publishes address records for every peer (S2), so a
    member can read your records whenever they like. Records exist only
    while you're unlocked, so that tells them when you're online. Dialling
    you tells them too.
  - A direct connection (after hole punching) shows them your IP address.
  - The record itself names only your home relay, not your IP.
  - **What hiding it would cost:**
    - *IP:* peers would connect through a relay only, never directly.
      Every Space line would cross a relay, with its bandwidth and round
      trip, and the relay's operator sees the addresses instead.
    - *Online times:* no address records for peers at all. Live chat
      between non-friends would go, and everything would travel by mail and
      member pull, with a delay of up to the 30-minute refresh.

    Neither is in this version. A Space is for people you're willing to be
    reachable by, as a Discord server's members can see you're online and
    message you.
- **History is only as honest as the member who serves it.** Signatures stop
  anyone forging another person's lines. But a removed member who signs
  back-dated lines with an old `after`, together with a current member who
  serves them, can put that ex-member's lines into a new member's history.
  Honest members never stored them, so only the new member sees them.
  **A member who was once an admin can serve their own lines into an
  admins-post-only channel's history, dated to any time.**
- **A past owner can take the Space back:** they can fork from before a
  transfer with a back-dated owner change of their own, and that branch wins
  (S7's owner rule ranks it first).
- **A stolen owner device plus one member can take a Space over:** a removed
  device's signed changes are still accepted when a member serves them as
  history (S5 checks the certificate there, not the current device list).
- **A removed device that's still unlocked** can sign lines that a dishonest
  server then serves as history. Live, every member refuses them (not on the
  current device list). A new member can't tell them from history. This is
  identity's "move to a new ID" question, which is not in this version (your
  call, 2026-11-14).

## Changes to approved decisions

Each would be written into its own spec, dated, once this one is approved.

1. **routing D9:** the handshake also lets in `space` peers (S2).
   *Approved 2026-11-14.*
2. **social D8: unchanged** (*changed 2026-11-14*). Approved first as a new
   account kind, `space`. Replaced at your review of mixed versions: peers are
   worked out from the Spaces held (S2), so the account has no new kind and
   its format doesn't change. Social's note says only that Space members
   count against `MAX_CONTACTS` with contacts.
3. **e2ee D6:** Spaces use per-device Olm in this version, not sender chains
   (S4). Megolm returns with the path past 50. *Approved 2026-11-14.*
4. **mailbox M2:** Space events are mailed in batches (S9). *Approved
   2026-11-14.*
5. **dm D3:** in a Space, an admin may delete anyone's message (S8).
   *Approved 2026-11-14.*

## The path past 50

Megolm for Space events (e2ee D6 as first approved), and a shared Space box
at the seed: one put per line, collected by every member, with no per-reader
take. That's a new seed service and a mailbox change. Until a Space needs
more than 50, neither is worth building. Both fit on top of this design: the
signed events (S5) and the member pull (S10) stay as they are.

## What it needs from you

1. **Review of PLAN-spaces.md.**
2. **For the gate (test 13): three people on two networks**, and a seed
   reachable from outside (the VPS). The process tests need neither.
3. **A look at the screens** (test 14), on Linux and Windows.

## Questions (answered 2026-11-14)

1. **The size cap:** 50 members.
2. **History for new members:** yes, everything.
3. **Per-device Olm instead of Megolm for now:** yes.
4. **Batched mail for Space events:** yes.
5. **Invite codes:** no. Joining is add-by-ID only.

Also decided: **"move to a new ID" is not in this version.**
