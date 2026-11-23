# Spec: native Discord video and screen share (`mesh-dc`)

Status: **DRAFT 2026-11-23, for your review.** You chose native video over
handing calls to the hidden web client (2026-11-23). Spec only: no plan or
code until you approve it, and the work starts with a spike (V0) that can
stop it cheaply.

## Objective

In a Discord voice channel or DM call, from Mesh:

- **See** other people's cameras and screen shares (Go Live).
- **Send** your camera, and **share** your screen or a window.
- The existing buttons (camera, screen share, watch a stream) do what they
  say. Today `mesh-dc` takes those commands and ignores them, so they look
  like they work and don't.

## What exists

| Piece | Today |
|---|---|
| Voice | `mesh-dc` drives **songbird 0.6.1**: voice gateway v8, UDP, transport encryption (AES-GCM), Opus, and DAVE for audio through **davey**. Audio only: no video SSRCs, no video RTP, no way in from outside the crate. |
| DAVE for video | **davey 0.1.5** already encrypts and decrypts video frames (`MediaType::VIDEO`), with codec-aware unencrypted header ranges for **H264, VP8 and AV1**. This was the biggest unknown, and it's covered. |
| Video codec, capture, display | Mesh-Voice's `mesh-media`: camera and screen capture, **H.264 in and out (openh264)**. The app already draws video tiles (WhatsApp calls and the old web-client path). |
| Main gateway | `mesh-dc` sends op 4 with `self_video`; Go Live needs ops 18 to 20 (stream create, delete, watch) and their `STREAM_*` events. |

## Decisions

| # | Decision | Why |
|---|---|---|
| V1 | **Vendor songbird** into the repo (`mesh-dc/vendor/songbird`, `[patch]`) and add a video path beside its audio one, rather than writing a new voice client. | Its connection, transport encryption, DAVE session and reconnects already work for audio, and video rides the same UDP socket and DAVE session. A rewrite redoes all of that. The cost is carrying the fork. |
| V2 | **H264 only** at first, through openh264, for both sending and receiving. | Mesh already builds and links it on Windows and Linux. Discord negotiates the codec per call, and every client offers H264. VP8 and AV1 later, if a call ends up without H264. |
| V3 | **Receiving first, then sending, then Go Live.** | Seeing others is most of the value and touches the least: op 12 (video SSRCs), RTP depacketizing, DAVE decrypt, decode, draw. Sending adds encoding, packetizing, pacing and simulcast. Go Live is a second voice connection of its own. |
| V4 | **Go Live is its own connection.** A stream gets its own voice server, token and DAVE session (ops 18 to 20 on the main gateway, then a voice connection with stream keys). The vendored songbird runs a second driver for it. | That's how Discord does it: a stream is a separate RTC session. |
| V5 | **No simulcast on sending at first**: one layer, 720p30 for a camera, the screen at its size capped at 1080p, 15 fps. | Less to get right. Discord's clients accept one layer. |
| V6 | **The app is unchanged in shape.** Frames go to Mesh-Voice as the old web-client path's `meshVideo` events did, so the existing tiles draw them. | It's already built and tested with WhatsApp. |

## Phases

- **V0, the spike (go or stop):** in one call, receive one person's camera
  and save ten decoded frames as PNGs. That proves op 12, RTP H264
  depacketizing, DAVE video decryption with davey, and decoding, end to end
  against Discord. **If it fails on DAVE or SSRCs, stop and report.**
- **V1, seeing cameras:** all participants' cameras in the call's tiles,
  through reconnects and joins and leaves.
- **V2, sending your camera:** capture, encode, packetize and pace, with op
  12 announcing our video SSRC and `self_video` on op 4.
- **V3, watching a Go Live:** ops 20 and 19, the stream's own connection,
  and its frames in a tile.
- **V4, sharing your screen:** op 18, capture through `mesh-media` (a
  screen or window picker in Mesh), encode and send.

## Risks

- **It can only be tested against Discord with your account**, which
  already gets captchas, and video is where automated clients get noticed.
  Every phase's check is by hand, by you, in a real call.
- **Discord's video protocol isn't documented** for user clients. Op 12's
  fields, RTP header extensions and simulcast are read from open clients
  (Discord's own `libdave` samples, discord.js voice, Abaddon). Expect
  breakage when Discord changes them.
- **Carrying a songbird fork:** each upstream release needs merging.
- **Upload and CPU:** encoding 720p30 H264 costs a CPU core on older
  machines, and screen share at 1080p about 2.5 Mbit/s up.

## Questions

1. **Spike first, with a stop if it fails?** *Recommended: yes.*
2. **H264 only to begin with?** *Recommended: yes.*
3. **Order: see cameras, send camera, watch streams, share screen?**
   *Recommended: yes; or tell me if you'd rather screen share comes
   earlier.*
4. **Fork songbird (V1)** rather than write a new voice client?
   *Recommended: fork.*
