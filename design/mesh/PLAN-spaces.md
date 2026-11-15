# Plan: `spaces` (Mesh)

Status: **APPROVED 2026-11-14**, with three fixes:
- a returning device says hello (spec S14, test 12);
- `spaceForge` truly kept out of release builds;
- what an engine without Spaces does (*Mixed versions*), with group DMs'
  formats unchanged (Task 2).

Both questions are answered yes. Building, in plan order.
Built from [SPEC-spaces.md](SPEC-spaces.md) (approved 2026-11-14,
revision 3). Test numbers below are the spec's.

## Overview

The pure parts come first, in `mesh-dm` with unit tests: the signed event,
the chain with roles, forks and redo, the history check, and the store.
Then the contact kind and routing. Then the helper: live delivery, batched
mail, history pull and moderation, ending in the process tests and the
measurements. The Mesh-Voice screens come last.

There are three checkpoints:
- **A**, after the pure parts: a short report, no stop.
- **B**, after the process tests and measurements: a hard stop with the
  figures, before any screen work.
- **C**, after the screens: by hand, added to *Before beta*.

## Architecture decisions

- **Spaces live in `mesh-dm`**, next to group DMs: a `space` module for the
  event, chain and check, and the store's existing segments for channels.
  There's no new crate, because Spaces share the event format, ordering,
  counters and store with dm.
- **The chain is dm's group chain generalised, not a copy.** `group.rs`
  gains roles and the role-ranked fork rule. Group DMs use it with two
  roles, the creator and members, and their tests keep passing unchanged.
  Redo is Space-only (spec S7), so group DMs still drop a losing branch's
  changes, as dm D7 says.
- **The per-Space log** (S11) is the device-wide log type in `history.rs`,
  one instance per Space, opened on first use. 1:1 chats and group DMs keep
  the one they have.
- **Space peers** are an account `Kind` in `mesh-identity` (`account.rs`).
  Routing, mail and the handshake already take "contacts"; the change is
  which kinds count as contacts, plus `MAX_CONTACTS` counting each person
  once.
- **The helper gains one service**, `space-history` (S10). Live Space events
  ride the existing `e2ee` service, as group DMs do.
- **The modified clients in the forging tests** (tests 5 and 7) are a helper
  command, `spaceForge`. It forges what the test names: a ground-low ID, a
  back-dated `seq`, an altered history line, or a line into an
  admins-post-only channel.
  - **It's behind `#[cfg(debug_assertions)]`,** so `cargo build --release`
    leaves it out entirely: the command, its parsing and its code. The
    process tests run the debug binary that `cargo test` builds, so they
    have it. `cargo test --release` would not, and those tests say so when
    they fail. *(Question 1, yes.)*
  - *Corrected 2026-11-14:* the draft said `--crash-at` is test-only. It
    isn't. It's parsed in every build (`main.rs`) and only documented as
    for tests. `spaceForge` doesn't follow it.
- **No new crate dependencies.**

## Task list

## Mixed versions

Friends will run 0.2.0, which has no Spaces, while Spaces is built. What
such an engine does:

- **A Space add or event** reaches it as an ordinary e2ee message, so the
  session is unharmed. Its body uses the friend layout's next version
  (`FRIEND_VERSION` 2), and forwards to your own devices use
  `FORWARD_VERSION` 2. A 0.2.0 engine reads that first byte as "newer"
  (`is_newer`, within `NEWER_WITHIN`) and answers `failed`, as built
  (bb5ae79). It never answers `ok`, so the sender's counter doesn't move
  past it. The sender keeps the event and resends it on dm D5's schedule
  (live on each connection, by mail with `failed`'s back-off), until the
  other side updates or D5's 30 days or 2,000 events run out. Nothing it
  has stored changes.
- **Batched Space mail** (Task 8) is one more body layout of the same kind,
  with a version byte first, so the same holds: `failed` in the receipt.
- **What the adder is shown:** the new member stays *added, waiting* in the
  member list until one of their devices answers a Space event `ok`. If
  every answer so far is `failed`, it reads *waiting: they may need to update
  Mesh*. Other members show the same for them.
- **Group DMs** are untouched: their events keep `VERSION` 1 and the
  friend layout 1. Task 2 keeps both formats byte for byte, and checks that
  bytes written before the change still read.
- **The account state is where it would break** (*found 2026-11-14 while
  writing this; needs your OK before Task 5*):
  - Spec S2, as approved, makes Space peers a new account `Kind`, synced
    between your own devices (social D11).
  - The account syncs as one postcard value. A 0.2.0 device can't parse an
    account holding a kind it doesn't know, so the whole sync to it would
    fail: friends, blocks and status included.
  - **Proposed:** peers aren't kept in the account at all. They're computed
    from the Spaces held: every member of every Space you're in, less
    friends and blocks. Routing, mail and the handshake take account
    contacts plus that set, and `MAX_CONTACTS` counts the union once per
    person.
  - Own devices learn Spaces through own-device sync (dm D8) anyway, so they
    compute the same set, and nothing new is synced.
  - This changes S2's mechanism and approved change 2 (social D8): no new
    kind, and social's account format is untouched. **Approved 2026-11-14**,
    written into S2, decision change 2 and the social note (Task 0).

### Phase 0: The approved changes, written into their specs

- [x] **Task 0: Four dated notes, docs only.**
  - **What:** a dated "Changed 2026-11-14 by `spaces`" section in each of:
    - SPEC-routing: D9 lets in `space` peers;
    - SPEC-social: D8's `space` kind, counted once against `MAX_CONTACTS`
      (*written 2026-11-14 to match S2 as changed: D8 and D11 unchanged,
      Space members counted against `MAX_CONTACTS` with contacts*);
    - SPEC-e2ee: D6, per-device Olm for Spaces in this version;
    - SPEC-mailbox: M2, Space events mailed in batches;
    - SPEC-dm: D3, an admin may delete anyone's message in a Space.

    Plus CAPABILITY-MAP's status line.
  - **Accept:** each note names the spec row it changes, and nothing else in
    those specs moves.
  - **Files:** the five specs and the map, in Mesh-Voice.
  - Size XS.

### Phase 1: The pure parts (`mesh-dm`)

- [x] **Task 1: The signed Space event** (S1, S3, S5, S12)
  - **What:**
    - `SpaceEvent`: Space ID, channel ID, dm's `Body` (with a mention list
      on `Text`), and the signing device's `DeviceCert`, signed by that
      device over all of it (domain `mesh/v1/space-event`).
    - The change kinds:
      - create;
      - add and leave;
      - kick and ban;
      - make admin, demote and transfer;
      - rename;
      - channel create, rename, move, delete and post switch;
      - category create, rename, move and delete;
      - whether members may add.
    - Each change is signed with its author's ID key, as group changes are.
    - Size checks against dm D14 and the Space limits.
  - **Accept:** unit tests cover:
    - sign and verify round trips;
    - an altered byte fails;
    - a certificate from another ID fails;
    - each limit is refused at its edge;
    - postcard round trips.
  - **Files:** `mesh-dm/src/space.rs` (new), `event.rs`.
  - Size M.

- [x] **Task 2: The chain with roles, forks and redo** (S7, S8)
  - **What:** `group.rs`'s chain takes a role table: who may make each kind
    of change in a version. Group DMs keep creator and members. Spaces get
    the owner, admins and members, with only the owner removing, banning or
    demoting an admin.
    - **The fork rule:** a branch with an owner change wins. If both have
      one, the lower (`seq`, ID) of each branch's first owner change wins.
      Then the same with admin changes, then with any change.
    - **Redo:** after each re-pick of the winner, the chain reports which of
      *our own* changes were dropped and are still allowed and still change
      something. The caller redoes them, once per fork lost.
  - **Accept:** unit tests cover:
    - every right and every refusal in the role table;
    - an ex-admin's fork with a ground-low ID and a back-dated `seq` losing
      to the owner's removal (7.1);
    - two admins apart, both changes kept after redo (7.2);
    - a late fork that drops a ban, with the ban redone (7.3);
    - a redone change dropped by a second fork and redone again;
    - no redo when the change no longer changes anything;
    - group DMs' existing chain tests, unchanged and passing;
    - **group DMs' wire and saved formats unchanged:** fixed bytes of a
      group change event, a friend-layout body and a saved chain, captured
      before the change, still read the same and still write the same.
  - **Files:** `mesh-dm/src/group.rs`, `space.rs`.
  - Size L.

- [x] **Task 3: The history check** (S5)
  - **What:** `check_live(event, current lists, version)` and
    `check_history(event, chain)`:
    - **live:** the device is on the author's current list, and the author
      may post in that channel now;
    - **history:** the certificate and signature check, the author is a
      member in some version of the chain, and the author may post in that
      channel in some version of it.
  - **Accept:** unit tests cover:
    - a removed device's line refused live and taken in history;
    - a departed author's line taken in history;
    - an altered line refused;
    - a never-admin's line in an admins-post-only channel refused in
      history;
    - an ex-admin's line there taken (the known limit, asserted as such).
  - **Files:** `mesh-dm/src/space.rs`.
  - Size S.

- [x] **Task 4: The Space store and its log** (S3, S11)
  - **What:**
    - a Space folder in `<data>/dm/`, with one chat per channel (segments,
      index, read marker), and the chain saved beside them;
    - a per-Space `History` log, opened on first use;
    - formats numbered, as every saved file is.
  - **Accept:**
    - unit tests: round trips across reopen, a channel deleted, and a log
      opened only when used;
    - an `--ignored` measurement: unlock-side open time with a 100,000-event
      Space against an empty one (test 10's pure half).
  - **Files:** `mesh-dm/src/store.rs`, `history.rs`, `space.rs`.
  - Size M.

**Checkpoint A (a short report, no stop):** the unit test counts, and Task
4's numbers.

**Checkpoint A, reached 2026-11-14** (Mesh-Node `4533f1c` to `cbcb7f7`,
Mesh-Voice `4792c4c`):
- **Tests:** `mesh-dm` has 100 unit tests passing (2 ignored measurements),
  up from 81. 19 are new:
  - 5 for the event (Task 1);
  - 10 for the chain (Task 2), plus the group DM format test;
  - 2 for the checks (Task 3);
  - 3 for the store (Task 4).

  Every group DM test passes unchanged, and the full workspace, clippy and
  fmt are clean.
- **Task 4's measurement** (release, NVMe under `target/`):
  - unlock reads only the list of Spaces: 23 µs with a 100,000-event Space,
    against 15 µs with an empty one;
  - the Space's first use reads its log and changes in 176 ms, then
    `#general`'s newest segment in 0.8 ms.
- **Changed from this plan while building:**
  - **Changes are signed like every Space event**, by the author's device,
    with its certificate. They're not signed with the ID key as group
    changes are. One check covers everything, and the ID signs the
    certificate. Every device holds the root key anyway (identity D2), so a
    stolen device could sign either way.
  - **`n` counts per channel**, not per Space, as dm's `Chat` checks it per
    chat.
  - **The store is its own file**, `space_store.rs`, on dm's `Store`, `Log`
    and `Chat`, not a `History` per Space. `History` keeps dm `Event`s, which
    would drop the signatures that history (S10) passes on, and its group
    rules would refuse channel events.
  - **The history check reads the versions on the winning line**, not every
    branch held. A dishonest admin's losing fork can't then open an
    announcements channel to members' forged lines.
  - **Storing one event at a time costs about 1.9 ms**, because each store
    syncs the channel file and the log. That's fine live, but 100,000 took
    187 s. Task 9 stores history in batches, synced once, as dm's intake
    does.
  - **Test first:** Task 2's format test was captured and passing before the
    refactor. For Tasks 1, 3 and 4 the tests were written with the code, not
    run failing first.
- **Stopping here** for the account question in *Mixed versions*, before
  Task 5.

### Phase 2: Contacts and routing

- [x] **Task 5: Peers worked out from the Spaces held** (S2 as changed
  2026-11-14)
  - **What:**
    - **The peer set at unlock:** unlock reads the list of Spaces, and for
      each Space only its changes (the chain). It doesn't read the event
      log, which S11 leaves to first use. The set is every member of every
      Space's current version, less yourself, your contacts and blocked
      IDs. It's kept again whenever a change is stored.
    - **Peers' routing state** (their device list and the address that last
      worked) is a `Contact` held by the helper in memory, not in the
      keystore. It's learned again from records after each unlock.
    - **One list for the lower layers:** routing, mail and the handshake
      take contacts plus peers. Every one of the helper's 22 uses of
      `store.contacts()` is reviewed, and each one that means "who may reach
      us and whom we reach" takes both.
    - **The cap:** `MAX_CONTACTS` counts contacts and peers together, once
      per person. A Space add that would take this device past it is refused
      here, with that reason.
    - No account kind, so social sees nothing new: a peer is never promoted
      to friend, because promotion needs an `outgoing` entry.
  - **Tests first:**
    - `mesh-dm`: the chain read without the log matches the full open's
      version, and reads no channel or log file;
    - `mesh-node` unit tests: the set is worked out less self, contacts and
      blocks; the union cap refuses at 1,000; storing a change updates the
      set;
    - the social and routing process tests still pass.
  - **Files:** `mesh-dm/src/space_store.rs`, `mesh-identity/src/contacts.rs`
    (a public constructor for a peer's `Contact`), `mesh-node/src/spaces.rs`
    (new), `helper.rs`, `routing.rs`, `mail.rs`, `mailrec.rs`.
  - Size M.

- [x] **Task 6: Peers on the wire** (S14)
  - **What:**
    - the handshake allow-list and records cover `space` peers;
    - peers are dialled to send, not at unlock or refresh;
    - `PEER_IDLE`: the helper closes a peer-only connection 60 s after its
      last stream (friends' connections untouched);
    - `PEER_RETRY`: no lookup of an away person for 30 minutes;
    - **the hello** (S14): on coming online, each Space member is dialled
      once, with lookups at most `HELLO_LOOKUPS` a minute, `HELLO_DIALS` at
      once, and a pause above `HELLO_ROOM`. An incoming connection from a
      person clears their `PEER_RETRY`.
  - **Accept:** a routing process test covers:
    - two peers connecting, then disconnecting after `PEER_IDLE` (with a
      test-short setting);
    - a friend's connection staying up;
    - an away peer looked up once per `PEER_RETRY` (counted at the seed);
    - the hello's pace held to its limits (counted at the seed).
  - **Files:** `mesh-node/src/routing.rs`, `helper.rs`.
  - Size M.

### Phase 3: The helper

- [x] **Task 7: Spaces, live** (S1, S4, S6, S12)
  - **What:** helper commands:
    - `spaceCreate`, `spaceAdd`, `spaceLeave`;
    - channel and category commands;
    - `spaceSend`, `spaceEdit`, `spaceDelete`, `spaceReact`, `spaceRead`;
    - `spaceTyping`, `spaceMute`;
    - and notifications for each.

    Sending encrypts for every current member device in one
    `encrypt_device_batch`, over the `e2ee` service. Delivery counters are
    dm D5's. Own-device sync carries Spaces as dm D8 does.
  - **Accept:** process tests 1, 6 (roles and the post switch) and 12 (a
    returning member sees A's next line in under 5 s without posting).
  - **Files:** `mesh-node/src/dm.rs` (or a new `spaces.rs` beside it),
    `helper.rs`, `tests/spaces.rs` (new).
  - Size L.

- [x] **Task 8: Batched mail** (S9)
  - **What:**
    - for each away reader device, the events waiting for it from every
      Space go in one envelope at the refresh and at the flush;
    - DMs keep mailbox M2;
    - the order at the limit is DMs first, then the devices mailed longest
      ago;
    - `slow-down` leaves events waiting, with counters unmoved.
  - **Accept:**
    - process test 2, with at most one put per away device for three lines;
    - a unit test of the order at the limit;
    - a flush test: events that don't fit stay waiting, and go at the next
      unlock's first refresh.
  - **Files:** `mesh-node/src/mail.rs`, `tests/spaces.rs`.
  - Size M.

- [x] **Task 9: History from any member** (S10)
  - **What:**
    - the `space-history` service: changes first, then each channel newest
      first, in batches of 200, by a per-(Space, channel) counter;
    - asked on joining, and after more than mail covers;
    - served only to a current member;
    - each event checked with `check_history` (Task 3).
  - **Accept:** process tests 3, 4 and 5. Test 5 includes the announcements
    case, with `spaceForge`.
  - **Files:** `mesh-node/src/dm.rs` or `spaces.rs`, `helper.rs`,
    `tests/spaces.rs`.
  - Size M.

- [ ] **Task 10: Moderation and forks end to end** (S7, S8)
  - **What:**
    - admin delete, kick, ban, and the owner-only admin changes, through
      the helper;
    - a ban drops the person as a peer everywhere (unless they're a friend);
    - redo wired to Task 2's report.
  - **Accept:** process tests 7 (all three parts, with `spaceForge`) and 8.
  - **Files:** as Task 9.
  - Size M.

- [ ] **Task 11: Measurements** (tests 9, 10, 11)
  - **What:** `--ignored` process tests:
    - a 50-member Space, one device each: a line to all 49; unlock with 5
      such Spaces; for one line, the sender's CPU and bytes, open
      connections and seed puts; and over an hour (clock offset) with 45
      away, the seed's address lookups; a returning device's hello, its
      lookups and its time to reach everyone online;
    - **test 10, kept honest:** unlock with a 100,000-event Space against an
      empty one, both with the peer set worked out (the list, then each
      chain), so the time includes what unlock really reads;
    - a check that no ID or text is logged, as the other modules have.
  - **Accept:** numbers recorded here. If a line to 49 takes over 1 s on
    this box, or unlock with 5 Spaces is over 1 s after "online", stop at
    Checkpoint B with the figures.
  - **Files:** `tests/spaces.rs`.
  - Size M.

**Progress, 2026-11-15** (Mesh-Node `97c32a1`, `7b4256a`, `7800b52`,
`acebfc7`, `c8d57e6`): Tasks 5 to 9 are done. Seven process tests pass in
`tests/spaces.rs`: tests 1, 2, 3, 4 (the departure half), 5, 6 (the local
refusals) and 12. Changed while building:
- **A new peer** gets our records and mail claims at once, and a hello. A
  failed hello is tried again three times, 10 s apart; only a failed
  dial-to-send starts `PEER_RETRY`.
- **Keys for a peer never met** come only from their records. A delivery
  that finds none fetches them and tries again at the refresh, and the
  refresh's key fetch covers peers without keys as well as contacts.
- **Mail from a sender we know nothing of yet** stays untaken at the holder
  until their record is read. Before, it was dropped and its box taken,
  which lost it. *This changes mailbox M7 for that case and needs your OK.*
- **Space mail marks** are in memory only (`ponytail:`). After a restart,
  unanswered Space mail is mailed again, and readers drop the copies by
  event ID.
- **Space events use a real millisecond clock.** `seq` is still dm D2's
  count from what's held, so lines sent before either side saw the other's
  are concurrent, and sort by ID.
- **Test 4's removed-device half** stays a `mesh-dm` unit test. A
  process-level device swap needs the link flow.
- **Own-device sync of Spaces** is built, but no test covers it yet.
- **Known flakiness:** under the full workspace's parallel load, a peer's
  first live line still sometimes misses the 60 s wait (test 3 failed once
  in four full runs after the fixes). Alone, the Space suite passed 14 runs
  of 14.

**Checkpoint B (hard stop, needs your review):** the process tests pass,
and Task 11's figures are recorded here. No screen work before your OK.

### Phase 4: Mesh-Voice

- [ ] **Task 12: The rail and channels** (tests 1, 14)
  - **What:**
    - Mesh Spaces on the existing rail, beside Matrix spaces and Discord
      guilds (the rail groups by space already);
    - the channel list with categories;
    - each channel's timeline using the existing chat view and composer;
    - unread counts, mention badges, and muting a Space.
  - **Accept:**
    - `cargo test -p mesh-app` passes, with no new clippy findings and no
      rustfmt diffs (no whole-repo reformat);
    - the checkpoint test's Spaces step (Task 14) drives it.
  - **Files:** `crates/mesh-app/src/mesh_dm.rs`, `mesh_ui.rs`, `main.rs`.
  - Size L.

- [ ] **Task 13: Members and settings**
  - **What:**
    - the member list with roles (presence for friends only);
    - "Add member" from your friends;
    - Space settings: name, channels and categories, the post switch,
      whether members may add, and admins (owner only);
    - kick, ban and unban, transfer ownership, delete the Space, leave.
  - **Accept:** as Task 12.
  - **Files:** as Task 12.
  - Size M.

- [ ] **Task 14: The checkpoint test's Spaces step**
  - **What:** `checkpoint.rs` gains a step. O creates a Space with two
    channels and adds B and C, who aren't friends. They post, react and
    reply. C locks and gets mail on unlock. O deletes a line. B can't post
    in the announcements channel.
  - **Accept:** `cargo test -p mesh-app checkpoint -- --ignored` passes.
  - **Files:** `crates/mesh-app/src/checkpoint.rs`.
  - Size S.

**Checkpoint C (by hand, added to *Before beta*):** test 14, the solo look
on Linux and Windows; and test 13, the gate with three people on two
networks, once the outside seed is up.

## Order and size

Task 0 first. Tasks 1 to 4 in order (2 needs 1, 3 needs 2, 4 needs 1). Then
Tasks 5 and 6. Tasks 7 to 10 in order. Task 11. Checkpoint B. Then Tasks
12 to 14.

Roughly 4,000 lines in Mesh-Node (a third of them tests) and 1,200 in
Mesh-Voice.

## Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Encrypting for about 100 devices per line is slow | Medium | Task 11 measures; Checkpoint B stops on over 1 s. The path past 50 (Megolm) is the fix, not a tweak. |
| More than 256 open connections in a busy account | Low | `PEER_IDLE` keeps the count to people talking now; past it, mail (S14). Task 11 records the count. |
| Generalising the chain breaks group DMs | Medium | Task 2 keeps every group DM chain test unchanged and passing. Redo is Space-only. |
| Redo loops between two honest devices | Low | Redo happens only after seeing a winning branch that lacks the change, once per fork lost. Task 2 tests a second fork. |
| History pull of a long Space is heavy | Medium | Batches of 200, newest first, resumable. A 100,000-event Space is about 30 MB (dm D11). |
| The mail limit is hit in a busy account | Low for the beta | S9's order and waiting. Task 8 tests the order. |
| Forging code leaks into a release | Medium | `spaceForge` is behind `cfg(debug_assertions)`, so a release build has none of it. Task 11 checks a release binary answers it `unknown`. |
| An old engine on a friend's PC breaks a Space or a session | Medium | *Mixed versions*: newer layouts get `failed`, not garbage; the account stays as it is. |

## Needs a new dependency

None.

## Open questions (answered 2026-11-14)

1. **`spaceForge`:** yes, behind `cfg(debug_assertions)`.
2. **The rail:** yes, the existing one.

The account question in *Mixed versions*: answered yes, peers worked out
from the Spaces held (2026-11-14).

**From Task 5 on, tests are written first and run failing before the code**
(your rule, restated 2026-11-14).
