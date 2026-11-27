# Spec: `profile` (Aflos)

Status: **APPROVED 2026-11-27**, with the recommended answers to Q1 to Q5
as written. Depends on `social` (friends, account sync
D11), `dm` (files, D12), `spaces` (the Space log, S1) and `e2ee` (1:1
sessions), all built in Mesh-Node as of Aflos 0.5.3. See
[ROADMAP.md](../../ROADMAP.md).

## Objective

People can make their profile look however they like: MySpace style, a
rice for your profile. It's free for everyone; nothing that Discord Nitro
charges for (animated avatar, banner, profile theme, per-server profiles) is
paywalled. Today an Aflos user is only a display name, from adding them
as a friend or from joining a Space, drawn as an initials circle.

The constraint is the engine: **no new rendering engine and no code from
other people.** A profile is data that Aflos draws with its own iced
widgets, built on the Rice token system (`theme.rs`: `Palette`, `Layout`,
`Extras`, `Animation`) and the existing encrypted file path (`mesh-dm`
`blob.rs`).

**User stories**

- I set an avatar and a banner, animated if I like, and my friends see them.
- I rice my profile: colours, panes, an animated background or my own
  image. Or I tick **Follow my rice** and it wears my desktop theme.
- I arrange blocks: bio, links, a gallery, a profile song, a Top 8, a status.
- Strangers and Space members who aren't my friends never see my profile.
- In a Space, I choose a small card (name, avatar, one line) for that Space.
- A Space owner can hide a member's avatar or bio in their own Space.
- A friend's huge or broken picture can't freeze or crash my app.

## Decisions (review these)

| # | Decision | Why |
|---|---|---|
| PR1 | **Friends only.** The full profile goes to current friends and your own devices, nobody else, whether the connection is direct or relayed. "Friends" is whatever SPEC-social says it is: today mutual friend requests; with consent-first friendship (revision 2, PR #9), people you've paired with. Neither a request nor a pairing carries a picture: the profile follows once you're friends. | Relays carry only ciphertext either way, so the path doesn't matter; who receives it does. Tying it to "direct" would blank your profile for friends behind CGNAT or mobile data, and for anyone using Hide my IP. |
| PR2 | **A profile is a signed, versioned bundle.** A manifest (`profile.toml` plus, for each asset, its hash, size, type and file key) signed by the device key under domain `mesh/v1/profile`, with a version that only goes up. Assets are sealed with the existing per-file encryption (`blob.rs`, D12) and fetched by hash like message files. | Reuses what's built and tested. A signed manifest means a friend can't be shown a faked profile, and the version means they fetch only when it changes. |
| PR3 | **Delivery: the version on connect, the manifest on demand.** When a friend's device or one of ours connects, and when we publish, each side with a profile opens a `profile` stream and sends its version; the other answers whether it wants it, and the manifest follows. Then the files it doesn't hold come on `blob`. No seed and no mailbox: an offline friend gets it the next time you're both online. *Changed 2026-11-27 (Mesh-Node #12): the manifest rides its own stream on the device connection, like presence and account sync, not the 1:1 e2ee session; that connection is already end-to-end encrypted and authenticated by device key, the manifest is signed, and each file is sealed with its own key.* | No new infrastructure. Profiles change rarely, so on connect is enough. |
| PR4 | **`profile.toml`: the theme section uses `theme.toml`'s keys.** `[theme]` takes `Palette`, `Layout` and `Extras` (animation, background image as an asset, translucent panes, density), exactly as Rice settings writes them. `follow_rice = true` snapshots your current theme when you publish. Values a viewer can't use are dropped, not errors. | One token system, and every rice preset already works as a profile theme. |
| PR5 | **Blocks, in the order you set (`[[block]]`):** `bio` (markdown subset: bold, italic, links, lists, up to 4,000 characters), `links`, `gallery` (up to 12 pictures), `song` (one track), `top8`, `status` (up to 128 characters). Avatar and banner are fixed slots, not blocks. | MySpace's set, plus a status line. A fixed list keeps the renderer small. More block kinds come later as plain additions. |
| PR6 | **Nothing is fetched from the internet.** Every picture and the song come from the bundle. Links are shown as the full URL and open only on click, after a confirm. No remote image URLs, ever. | A remote image would leak the viewer's IP and online time to whoever hosts it (the tracking-pixel problem). |
| PR7 | **Generous caps, for safety:** the bundle at most **25 MB** in total; avatar up to 8 MB and banner up to 12 MB (PNG, JPEG, WebP, GIF, APNG; animated allowed); gallery pictures up to 4 MB each; the song up to 10 MB and 6 minutes (Opus, Vorbis, MP3, FLAC). The app re-encodes on the sender's side to fit where it can. | Profiles go only to friends, fetched once per version and cached, so tens of MB is fine on home connections. |
| PR8 | **Decoding a friend's files is untrusted input.** Memory-safe Rust decoders only (`image`, `symphonia`), in the helper or a worker thread, with limits applied before allocating: at most 4096×4096 pixels a frame, 300 frames and 30 s an animation, 64 MB decoded; anything over is shown as a placeholder. | No decompression bombs, and a bad file can't take the app down. |
| PR9 | **Fonts come from a bundled set** (Inter, Michroma, JetBrains Mono, plus a few display fonts chosen in the plan). A profile names one; it never ships a font file. | Font parsers have a long history of security bugs, and bundled fonts keep the app small. |
| PR10 | **The song never autoplays.** A play button on the profile; a setting *Autoplay profile songs* (off by default). | MySpace's autoplay is the part nobody misses. |
| PR11 | **Top 8 needs consent.** A friend appears in your Top 8 only after they accept a signed "you may show me" from you, which they can withdraw. Viewers see a shown friend's display name and avatar, not their ID. *Settled while building (Mesh-Node, Task 5): the ask, the yes and the withdrawal ride a `top8` stream on the device connection, as PR3's manifest does, so they're authenticated by the friend's device key rather than signed separately (nobody else ever checks them, since viewers never see the ID); they wait in a saved outbox until the friend's device takes them. A no is never sent, as social's silent decline. The yes carries the name the friend wants shown (their username by default). You choose who's shown, in order, with `top8Set`, from those who said yes. Your signed manifest carries a `top8` list of {name, avatar}: the avatar is the friend's own profile avatar, passed on as they sealed it, within the avatar's caps and outside the 25 MB bundle cap. A withdrawal, an unfriend or a new avatar of theirs re-signs your profile as the next version. The lists live on the device that made them, for now; own-device sync comes later.* | A Top 8 reveals who your friends are to other people. |
| PR12 | **Per-Space card.** By default a Space shows your display name and an initials circle. You can set a card per Space: display name, a still avatar (up to 1 MB) and one line (up to 190 characters). It's a signed `card` change in the Space log (spaces S1), so every member gets it, through relays like any Space event. No banner, theme or blocks. *Settled while building (Mesh-Node, Task 4): "still" means PNG, JPEG or WebP; the avatar is kept and fetched as Space files are (`cardFetch`), so only members can fetch it; a new card replaces the whole last one.* | Space members who aren't friends get only what you chose, and it rides the existing log. |
| PR13 | **Owner moderation in a Space:** an owner or admin can hide a member's card avatar or line in that Space with a signed change; clients honour it. Reports (connection-privacy P11) can point at a card. *Settled while building (Task 4): only over a member they could kick (an admin can't hide another admin's or the owner's card); a hide outlasts the member's next card until it's lifted; a hidden avatar or line isn't passed to Aflos at all.* | The owner is the only moderator there is (P11). |
| PR14 | **Your devices share one profile** through account sync (social D11): the latest manifest wins by `(version, device)`. | Every device publishes the same profile. |
| PR15 | **Unfriend or block removes their cached profile** from your devices. | Their profile was given to a friend, not to a former friend. |
| PR16 | **The editor is Rice settings aimed at your profile**, with a live preview of how friends will see it, and **Publish** to send a new version. | The screens and controls already exist. |

## Commands (helper protocol)

New calls on the `mesh-node` helper (stdin `{"fn":..,"args":[..]}`):

- `profileSet(path_to_profile_dir)`: validate, re-encode, seal, sign and
  publish a new version. Returns the version or the cap that failed.
- `profileGet(id)`: the cached manifest and asset paths for a friend.
- `cardSet(space, display, line, avatar_path)` (`""` for no avatar),
  `cardHide(space, member, avatar|line|both|none)` and
  `cardFetch(space, member)` (the avatar, as `meshDmFile`).
- `top8Ask(id)`, `top8Answer(id, yes, [name])`, `top8Withdraw(id)`,
  `top8Set(ids…)` (who's shown, in order) and `top8List()`.

New bindings on stdout: `meshProfile` (a friend's profile arrived or
changed), `meshTop8` (a request, an answer or a withdrawal, with every list). A card
shows in `meshSpaces` (each member's `avatar`, `line` and `hidden`); a Top 8
in `profileGet`'s `top8` (each name and avatar path).

## Structure

```
mesh-node/crates/mesh-dm/src/profile.rs     manifest, signing, version, caps
mesh-node/crates/mesh-dm/src/space.rs       card and card-hide changes
mesh-node/crates/mesh-node/src/profile.rs   delivery on connect, fetch, cache
mesh-node/crates/mesh-node/src/decode.rs    capped image and audio decoding
aflos/crates/aflos/src/profile_ui.rs        the profile screen and editor
aflos/crates/aflos/src/theme.rs             reused; no token changes
```

## Testing

- **Unit:** `profile.toml` parsing (unknown keys dropped), manifest signing
  and versions, every cap at its limit and one over, decoder limits with
  crafted bombs (a 1×1 PNG claiming 100,000×100,000, a 10,000-frame GIF).
- **Process test** (`tests/profile.rs`): A publishes a profile; friend B
  receives it on connect and again after a change; C, a non-friend in a
  Space with A, gets only A's card; B unfriends A and A's profile leaves B's
  cache; a Top 8 ask, accept and withdraw.
- **By hand (checkpoint):** the profile screen and editor on Linux and
  Windows, an animated avatar and banner, Follow my rice, the song, a
  friend's profile over a relayed connection.

## Boundaries

- **Always:** friends only; everything from the bundle; capped decoding.
- **Ask first:** new block kinds; raising a cap.
- **Never:** HTML, CSS or scripts in profiles; remote URLs fetched
  automatically; any of it behind a payment.

## Open questions (recommended answers in bold)

- **Q1. The caps in PR7.** **As written.**
- **Q2. Top 8 in 1.0?** **Yes**, it's small once consent is built.
- **Q3. "Now playing" from the desktop (MPRIS on Linux)?** **Later**: the
  status line covers it by hand for 1.0.
- **Q4. Profile song formats.** **Opus, Vorbis, MP3, FLAC**, decoded with
  `symphonia`.
- **Q5. Which display fonts to bundle.** **Up to five, chosen in the plan**,
  open licences only (OFL).
