# Changelog

## 0.6.0

- Friends are consent first: no more friend requests. Give someone your
  code (or ID) and use theirs; you're friends once both of you have added
  the other. Adding someone shows them nothing. In a Space, Add friend is
  on each member's row. Requests sent before 0.6.0 are dropped unanswered.
- Fixed: someone you'd added could show as your friend after you shared
  a Space, before they'd added you back.
- Chat bubbles: the time and ticks sit beside each other, and the hover
  bar sits beside the bubble instead of over it.
- Profiles you can rice, MySpace style, free for everyone: an avatar, a
  banner, a background, a bio, links, a gallery of up to 12 pictures, a
  profile song and a status, in the order you set, in your own colors
  (Follow my rice takes Aflos's current ones). Friends → Your profile edits
  it, with a live preview; Publish sends it. Only your friends get it.
- A Top 8: ask a friend from their profile; they show only once they say
  yes (Friends → Pending), and they can stop it whenever they like.
  Viewers see the name they gave and their avatar, never their ID.
- Friends' avatars in the Friends list, chats, DM headers and calls.
- Space cards: a name, a still avatar and one line for each Space, which is
  all members who aren't your friends see. An owner or admin can hide a
  member's avatar or line.
- A friend's pictures and song are checked by a capped decoder before
  they're shown; anything too big or damaged shows as a placeholder. Links
  open only after you confirm, and nothing is fetched from the internet.
- Autoplay profile songs (Settings, Sounds), off by default.

## 0.5.3

- Log out (Your ID): locks your account on this device; your username and
  password open it again. If "Keep unlocked" was on, it's turned off.
- Remove from this device (Your ID): deletes this device's copy of your
  account, after your password and a second press. If it's your only
  device, a warning says the account is gone for good without a backup.

## 0.5.2

- Calls sound better: echo cancellation (no more hearing yourself through
  someone's speakers), automatic gain (quiet mics brought up to a normal
  level), and noise suppression, each with a switch in Settings, Voice &
  audio.
- Input sensitivity: Automatic, or set your own over a live level meter.
- Test my mic, and Test and hear myself, before a call.
- Output volume, and each person's own volume, in calls.
- A call that falls behind (a busy or briefly frozen PC) catches up
  instead of going quiet until the other person pauses.

## 0.5.1

- A quiet microphone is heard in Aflos calls: 0.5.0's voice gate cut off
  speech below a fixed level, however clearly you spoke.
- Input volume for Aflos calls (Settings, Voice & audio), 0 to 200%,
  changed live during a call.
- WhatsApp: a friend's chat is no longer shown as your chat with yourself
  after you write to them.

## 0.5.0

- Only friends get a direct connection to you. Everyone else, including
  people you share a Space with, reaches you only through a relay and can't
  learn your IP address. Friends on 0.4.x stay relayed until they update.
- Hide my IP (Settings, Privacy & E2EE): everyone, friends too, reaches
  you through a relay.
- The status line, a call and a DM's header say whether you're connected
  directly or through a relay.
- A Space's owner can allow direct connections between members who are
  friends.
- Report a member to the Space's owner, with a reason and their recent
  messages. Reports go to the owner only, never to Aflos.
- Aflos is for people 18 and over (TERMS.md).

## 0.4.1

- The built-in seed is now reached only through a relay. 0.4.0 can't reach
  it after the homelab change, so update.

## 0.4.0

- Mesh is now Aflos: new name, icons and installers. Your data moves over
  by itself on first start.
