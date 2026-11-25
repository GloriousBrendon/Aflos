# Spec: `social` (Aflos)

Status: **DRAFT revision 2 (2026-11-25)**, for review, with the review answers
of the same day folded in (D16, D17, D21, D25). Revision 1 was
approved 2026-11-10 (requests through a live friend code). Revision 2 makes
friendship consent-first: two people become friends only when both agree,
and nothing is ever shown to someone because another person added them.
Friend requests and friend codes go away; pairing codes and an in-Space
"Add friend" replace them. Plan: PLAN-social changes (to write, version
0.5.0). Depends on [`routing`](SPEC-routing.md), [`seed`](SPEC-seed.md),
[`spaces`](SPEC-spaces.md) and
[`connection-privacy`](SPEC-connection-privacy.md).

## Objective

Aflos is for close friends, consent first. Friendship is the thing that
unlocks everything private: DMs, calls, presence and direct connections
(connection-privacy P1). So becoming friends must need both people, every
time, and asking must cost the other person nothing: no request in their
inbox, no notification, no "pending" they have to deal with.

There are three ways to become friends, all mutual:

1. **Pairing** (outside a Space): one of you taps Pair and reads out or
   sends a short code like `7-gitaar-koffie`; the other types it within 2
   minutes; both see the other's name and identicon and tap Confirm. In person,
   one shows a QR code and the other scans it, which also marks you verified.
2. **Add friend in a Space**: on a member's profile. When both of you have
   pressed it, you're friends. If only one has, the other never knows.
3. **Add by ID** (revision 1's D2, kept): two people who already have each
   other's IDs each add the other.

**What stays from revision 1:** friends, blocked and removed kinds, the
account sync between your own devices (D11), presence (D12, D13), names and
verified (D14), and counts-only logs (D15).

**What goes:** friend requests and their seed store (revision 1's D1, D3
to D7), the `incoming` kind with Accept and Decline, and friend codes (the
30-day `K7QM-3XPA` kind) with their seed services.

**Not in this revision:** custom status text (later); mutual friends and
suggestions (they'd show you strangers); pairing through a DHT instead of
the seed (designed below, built later).

**User stories**

- I'm on the phone with a friend. I tap Pair and read out `7-gitaar-koffie`.
  They type it; we each see the other's name and identicon, tap Confirm, and
  we're friends.
- At a braai, my friend shows a QR code and I scan it. Same result, nothing
  typed.
- Someone in a Space I'm in seems nice. I press Add friend on their profile.
  Nothing happens on their side. Weeks later they press it too, and we're
  both told we're friends.
- Someone presses Add friend on me and I never press it back. I'm never told,
  and nothing about them shows up anywhere.
- Someone overhears half my pairing code and tries to guess the rest. They
  get one guess; it's wrong; my pairing says "someone tried a wrong code;
  start again".
- In a Space, I can talk in its channels with anyone, but I can only DM or
  call my friends.

## Decisions (revision 2)

Numbers from revision 1 are kept where the decision is kept; new ones start
at D16.

| # | Decision | Why |
|---|---|---|
| D2 | **Add by ID, mutual.** *Kept, mechanism changed.* Adding an ID makes it `outgoing` and puts a **mutual token** for the pair on the seeds (D17). When both have added each other, the token matches and both become friends. Nothing is sent to the other person, and nothing shows on their side. | Revision 1 made the first connection between two `outgoing` contacts the proof that both had added each other. With Spaces, members can connect without having added each other (spaces S2), so a connection no longer proves consent; the token does. |
| D8 | **Kinds:** `outgoing` (you added them, by ID or Add friend; they don't know), `friend`, `blocked`, `removed` (a tombstone for sync). **`incoming` is gone.** Friendship comes only from a matched token (D17) or a completed pairing (D16), never from a connection. *Changed.* | Consent first: there's no state where someone else's action puts something in front of you. |
| D9 | **Cancel and remove.** Cancel drops your `outgoing` entry and stops your token being renewed; the token already on the seed lasts until it expires (7 days, D17), so the Add friend button says "If they add you back before <date>, you'll be friends; cancel stops it after that." Remove turns a friend into `removed`, closes their connections and rotates your direct key (connection-privacy P4). Accept and decline are gone. *Changed.* | A token can't be taken back from the seed without letting someone probe and retract (D17), so its short life is stated honestly instead. |
| D10 | **Blocking.** *Kept, simplified.* `block(id)` makes the ID `blocked`, drops any `outgoing` entry, closes their connections and rotates your direct key. There are no requests to filter any more. A blocked person's later tokens can never match, because you never put one for them. | As before, minus requests. |
| D11 | **Account sync.** *Kept,* without `incoming` entries or request notes. | |
| D12–D15 | **Presence, status, names, logs.** *Kept unchanged.* | |
| D16 | **Pairing codes, Magic Wormhole style, with SPAKE2** (RustCrypto `spake2`, already a dependency for device linking). (1) A taps **Pair**. A's throwaway node (relay-only, connection-privacy P9) claims a **nameplate** from the seed's pairing service: the lowest free number, so it's short (`7`). A picks **two words** at random and shows `7-gitaar-koffie`. (2) B types it within **2 minutes**. B claims that nameplate as the second side; **only one second side is ever allowed**. (3) Both run SPAKE2 with the whole code as the password and the nameplate in the identity, exchanging messages through the seed's mailbox for that nameplate. (4) With the shared key, each sends a sealed **card**: its ID, signed `DeviceList` and display name; the other side shows it with the ID's identicon (D21). (5) Both see the other's card and tap **Confirm**; each sends a sealed confirm. When both have confirmed, both add the other as `friend` (no token needed). A pairing by QR (D20) also marks the friend **verified** (D14); a typed or spoken code doesn't, and those friends can compare the safety number later. (6) The nameplate is released: done, cancelled, wrong code, or 2 minutes up. **One guess per attempt:** a wrong code (SPAKE2 key confirmation fails) ends the pairing on both sides, and A must start again with a new code. *Your choice; mechanism proposed.* | Magic Wormhole's design: a short code is safe because a wrong guess burns it, so an attacker gets one try at about 1 in 65,536 (two words of 256; D18), and A sees the failed try. The seed only relays: it sees a nameplate, two throwaway keys and ciphertext (SPAKE2 messages are random-looking). Reusing the link module's SPAKE2 setup keeps new crypto to zero. |
| D17 | **The mutual token** (for D2 and D19). For a pair of IDs, both sides can compute the same token: `blake3::derive_key("mesh/v1/mutual", pair secret)` (routing D1's pair secret, from either root key and the other's ID), plus a **side** bit (0 for the lower ID). New seed service `mutual-put(token, side)`: the seed stores `(token, side)` for **7 days** and answers `matched` if the other side's entry is there, else `waiting`. There's no get and no delete. While you have someone as `outgoing`, your device puts the token again at unlock and each refresh (routing's 30 minutes), so it learns of a match within a refresh, and the token never lapses while you still want it. On `matched`, both become `friend` and connect. *Proposed.* | The seed compares; neither side learns anything without putting its own token, and putting is pressing (so "probing" is consenting). No delete means nobody can put, read the answer, and take it back before the other notices. The seed learns that two throwaway sessions are a pair, as it already can from address records (seed D10b), but not who. |
| D18 | **Code words: Afrikaans and English lists, 256 words each**, no word on both lists, no two words within one edit of each other, all 3 to 8 letters. The code is `nameplate-word-word`; the words come from your app language's list; the typing side accepts words from either list (each word maps to its index in its own list), lowercase, with spaces or hyphens. | Two words of 8 bits is Wormhole's strength, and one guess per attempt makes it enough. Accepting both lists lets an Afrikaans speaker read a code to an English speaker. Edit distance keeps typos from becoming another valid word. |
| D19 | **Add friend inside a Space.** On a member's profile, "Add friend" makes them `outgoing` and puts the mutual token (D17). When both have pressed it, both are told "You and <name> are now friends". A one-sided press shows only on the presser's side ("Added; if they add you too, you'll be friends"). *Your choice.* | Space members already know each other's IDs, so this is D2 with a button. |
| D20 | **In person: a QR code** carrying `mesh://pair/<nameplate>/<secret>`, where the secret is 128 random bits instead of two words. Scanning runs the same flow (D16) with no typing; both still see cards and Confirm. The QR expires with its nameplate (2 minutes). *Your choice.* | A long secret needs no guess limit, but keeping the same flow means one code path. |
| D21 | **An identicon, no pictures yet.** The pairing card, the friends list, contacts and Space member lists show the name beside a pattern generated from the ID (an identicon: a symmetric 5×5 grid in a colour, both from `blake3(ID)`), so two people with the same name look different. Profile pictures come later. *Your choice (2026-11-25).* | Seeing something that belongs to that ID before confirming makes the confirm meaningful, without building pictures, their sync and their limits now. |
| D22 | **DMs and calls only between friends.** In a Space you can talk in channels with every member, but you can't DM or call someone who isn't your friend; their profile shows "Add friend" instead. Group DMs contain only friends of the creator, and each member only sees DMs from their own friends (dm D7 as is). Existing DM threads with non-friends stay readable but closed for sending. *Your choice.* | Private messages are for people who both agreed to be friends. |
| D23 | **Friend codes go away** (identity's 30-day `FriendCode`, `mesh://code/` links, the seed's `code-hold`, `code-get`, `code-revoke`). See *What replaces friend codes*. *Your choice.* | Pairing does their job with consent on both sides, and needs nothing stored for 30 days. |
| D25 | **Device linking moves onto pairing codes.** The link code becomes a pairing code (`nameplate-word-word`, D18) and linking's rendezvous moves from an address record keyed by the code to the pairing service's nameplate and mailbox (D16). Linking's own SPAKE2 messages and the identity hand-over (`link.rs`) stay as they are; only the code format and where the two sides meet change. Identity's `FriendCode` type then goes. In 0.5.0 if the plan shows it's a small change; otherwise 0.5.1, and the plan says which. *Your choice (2026-11-25).* | One kind of code is simpler for people. |
| D24 | **Seed limits for pairing and tokens** (connection-privacy: all callers are relayed, so the shared relayed buckets are what bind). Per throwaway key: 10 nameplate claims an hour and 64 token puts an hour. Everyone together: 600 nameplate claims an hour, 20,000 token puts an hour, 200,000 tokens held, 1,000 live nameplates. Per nameplate: 2 sides, 8 messages a side, 64 KiB a message. *Proposed.* | Ten users pair a handful of times a day and put tokens for their few `outgoing` people every 30 minutes: well under 1% of each. |

### Removed in revision 2

| # | Was | Why removed |
|---|---|---|
| D1 | Requests only through a live friend code; a request used up one of the code's uses. | Consent first: a request puts something in front of someone who didn't ask. |
| D3 | Only the owner reads requests (`knock-get` with the code's revoke). | No requests. |
| D4 | The sealed request format. | No requests. |
| D5 | Per-sender request slots. | No requests. |
| D6 | Seed limits for requests (20 puts an hour per device, 32 per code, 7 days). | No requests. |
| D7 | The owner's device collects requests every 5 minutes. | No requests. |

Also gone with them: `knock.rs` (identity), `knocks.rs` (seed store),
`knock-put` and `knock-get`, the `incoming` kind, `friendRequest`,
`friendAccept`, `friendDecline`, and the request half of `meshSocial`.

## Behaviour

### Seed side

- **Removed services:** `code-hold`, `code-get`, `code-revoke`,
  `knock-put`, `knock-get`, and their stores `codes.bin` and `knocks.bin`
  (deleted at the first start of a 0.5.0 seed, after logging their counts).
- **Pairing service:**
  - `pair-claim()` → `nameplate` (u16, the lowest free), live for 2 minutes.
  - `pair-join(nameplate)` → `ok`, or `gone` (not live, or a second side
    already joined; they look the same).
  - `pair-put(nameplate, side, message)` and `pair-get(nameplate, side,
    after)`: the mailbox. A get waits up to 20 s for a message newer than
    `after`, then answers what's there.
  - `pair-close(nameplate, side)`: frees it; it's freed anyway when done,
    or 2 minutes after the claim.
  - Nameplates and messages live in memory only; nothing is written to
    disk.
- **Mutual tokens:** `mutual-put(token, side)` → `matched` | `waiting` |
  `slow-down`. Stored in `mutual.bin`, 7 days from the last put, saved like
  `addrs.bin` (a damaged file starts empty: tokens come back within a
  refresh).
- The hourly counts line: claims, joins, wrong codes, token puts, matches.
- **Kept:** `addr-put`, `addr-get` (routing, and device linking's
  rendezvous), the mail services and mail records.

### Client side (the helper)

- **Commands:**
  - `pairStart()` → `meshPair {state:"code", code, qr, expires}`.
  - `pairJoin(code)` (typed, or the scanned `mesh://pair/…`) → `meshPair
    {state:"card", id, name}` once the other card arrives (Aflos draws the
    identicon from the ID).
  - `pairConfirm()`, `pairCancel()`.
  - `meshPair {state:"done", id}` or `{state:"failed", reason:"wrong-code" |
    "expired" | "cancelled" | "taken" | "offline"}`.
  - `friendAdd(id)` (D2, D19), `friendCancel(id)`, `friendRemove(id)`,
    `friendRename`, `friendVerify`, `block`, `unblock`, `statusSet`,
    `activity`, `socialList`: as before, minus requests.
- **`meshSocial`** loses `incoming`; `outgoing` entries gain `until` (when
  the token lapses if cancelled).
- **Pairing state machine** in `mesh-identity` (`pair.rs`, beside
  `link.rs`, sharing its SPAKE2 helpers): A and B roles, messages
  `Spake | Card | Confirm`, each sealed with XChaCha20-Poly1305 under keys
  derived from the SPAKE2 key (`mesh/v1/pair-a`, `mesh/v1/pair-b`).

### Aflos

- **Add friend** (Friends page tab): two buttons, *Pair* (shows the code,
  big, plus the QR and a 2-minute countdown) and *Scan or type a code*; and
  under them, *Add by ID* with its line "they need to add you too".
- A member's profile in a Space: *Add friend* / *Added (waiting)* / *Friends*.
- Non-friends' profiles have no Message or Call buttons (D22).

## What replaces friend codes

| Where friend codes are used today | Replaced by |
|---|---|
| Friends → Add friend: "your friend code" (make, list, revoke codes) | *Pair*: a code and QR for 2 minutes, nothing kept afterwards. |
| Friends → Add friend: "Check an ID" taking a code, then *Send friend request* | *Scan or type a code* runs pairing; an ID goes to *Add by ID* (D2). |
| New (+) menu: "Add a friend — By their Aflos friend code or ID" | "Add a friend — Pair, or add by ID". |
| Friends list empty states ("make a code to give someone, or type theirs") | "No friends yet. Under Add friend, tap Pair and give them the code." |
| Your ID: "Friends, requests and friend codes are on Home, under Friends." | "Friends and pairing are on Home, under Friends." |
| `mesh://code/<code>` links | Gone; a pasted one says "Friend codes were replaced by pairing; ask them to Pair with you." |
| **Onboarding** | Onboarding never showed a friend code. Its only code is the **device link code** ("I already have Aflos": type the code from your other device), which is device linking, not friendship. **D25** moves it onto the pairing service and code format, so there's one kind of code. |
| The seed's code store and limits (seed D7, D8's code limits, D9's `codes.bin`) | Removed; pairing and tokens (D24) instead. |

**Migration at the first unlock on 0.5.0:**

- **Friends, blocked, removed:** unchanged.
- **`outgoing` entries** (you added them by ID, or sent a request) become
  presses: the device starts putting their mutual tokens. If they had added
  you too (an `outgoing` entry on their side), it matches once both are on
  0.5.0.
- **`incoming` requests** that you never accepted are deleted without being
  shown again. The senders keep you as `outgoing` (a press you never see).
- **Your held codes** are dropped from the keystore; the helper stops
  holding and collecting. A 0.5.0 seed deletes its code and request stores;
  a 0.4.x seed drops them within 30 days (seed D8's hold life).
- **What a 0.4.x client does with a 0.5.0 friend or seed:** set out in the
  0.5.0 plan (PLAN-social). In short: 0.4.x code and request calls to a
  0.5.0 seed get "unknown service" and show "update Aflos to add friends";
  existing friendships between 0.4.x and 0.5.0 keep working.

## Pairing through a DHT later

Today the seed is the meeting place. To move it to a DHT (iroh's
pkarr/mainline, or a later Aflos DHT) without changing the user's flow:

- **The nameplate becomes a lookup.** A derives a key pair from the code
  (`Argon2id(code)` → seed → Ed25519) and publishes a short-lived signed
  record under that public key, holding A's throwaway endpoint key and relay
  URL (the same trick device linking already uses with the seed's address
  records). B derives the same key from the typed code, resolves the record,
  and dials A's throwaway node through the relay.
- **SPAKE2 runs over that connection** instead of the seed's mailbox, with
  the same messages (D16). The one-guess rule moves to A: A's throwaway node
  takes one session and closes.
- **What changes for privacy:** a record on a public DHT can be found by
  anyone who tries all 65,536 codes against Argon2id (minutes to hours on a
  GPU). They learn a throwaway key and a relay for 2 minutes, and still get
  only one SPAKE2 guess. The nameplate stops being a short number; the code
  becomes three words (`gitaar-koffie-braai`) so the lookup key alone has
  more entropy.
- Mutual tokens (D17) need a store that answers "is the other side there",
  which a DHT does only as "publish yours, look for theirs": each side
  publishes a record under `derive(token, side)` and looks for the other.
  That works, but anyone who can compute the token (only the two people) can
  look without publishing, so the probe protection of D17 is lost. Keep
  tokens on seeds until there's a design for that.

## Testing strategy

- **Unit:**
  - `pair.rs`: a round trip for both roles; a wrong code fails key
    confirmation on both sides; a replayed or tampered message is refused;
    cards are bound to the session (a card from another session fails).
  - Word lists: 256 words each, disjoint, edit distance ≥ 2, parse in
    either language, with spaces or hyphens, any case.
  - Mutual tokens: both sides compute the same token; the store matches
    only opposite sides; expiry; no delete; limits; damaged file starts
    empty.
  - Seed pairing service: lowest-free nameplates, one second side, 2-minute
    expiry, mailbox order, limits.
- **Process tests** (on the local relay, a 0.5.0 seed):
  - Pairing: Alice pairs with Bob by code; both confirm; friends on both
    sides and on Bob's second device. Then by QR text.
  - Wrong code: Carol joins Alice's nameplate with wrong words; both sides
    fail; Alice's nameplate is gone; Bob's correct join after that gets
    `taken`.
  - Expiry: a code typed after 2 minutes (clock offset) fails `expired`.
  - Add friend in a Space: Alice presses; Bob's helper emits nothing and
    his state has no trace of Alice's press; Bob presses; both become
    friends within one refresh.
  - Cancel: Alice presses and cancels; Bob presses 8 days later (clock
    offset): no friendship.
  - D22: a DM or call to a non-friend member is refused; channel messages
    work.
  - Migration: a 0.4.x keystore with friends, `outgoing`, `incoming` and
    held codes opens with friends kept, `outgoing` kept, `incoming` and
    codes gone.
- **By hand:** pair two machines on different networks by reading a code
  aloud; pair by QR with a phone camera pointed at the screen (the scanning
  side is the second desktop's webcam, or typing the QR's text); press Add
  friend on both sides of a Space.

## Boundaries

- **Always:** both sides consent to every friendship; one-sided presses are
  invisible to the other; one guess per pairing; counts-only logs.
- **Ask first:** any way for one person's action to show something to
  another before they've agreed; a code shorter than two words; any new
  dependency (the word lists are data, not a crate).
- **Never:** friend requests, suggestions, a public directory or search.

## Resolved questions (2026-11-25)

1. **Picture:** not in 0.5.0; the name and an identicon (D21).
2. **Device linking onto pairing:** yes, if small (D25).
3. **Pairing marks verified:** by QR in person, yes; by a typed or spoken
   code, no (D16).
4. **Token life:** 7 days (D17).

## Open questions

None.
