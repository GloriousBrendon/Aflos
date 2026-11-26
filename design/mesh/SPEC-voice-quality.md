# Spec: `voice-quality` (Aflos)

Status: **APPROVED 2026-11-26** ("go with recommendations": Q1 to Q5 as
recommended). 0.5.2 ships as built; this is **0.5.3**. Plan:
[PLAN-voice-quality.md](PLAN-voice-quality.md). Depends on [`audio`](SPEC-audio.md) as built
for 0.5.2 (Mesh-Node PR #7, Aflos PR #20).

## Objective

People who care how they sound can make their voice sound great, and
people can choose how much bandwidth their audio and video use.

**User stories**

- I have an EQ, compressor or de-esser plugin I like (VST3 or CLAP): I add
  it to my voice in Aflos, open its own window to tune it, and hear the
  result in the mic test before a call.
- A plugin I added crashes: my call keeps going without it, and I'm told.
- I want my voice sent at higher quality than the default, or lower when
  my connection is poor.
- When I share my screen or camera into Discord, I pick resolution,
  frame rate and bitrate instead of the fixed values.

## What exists

| Piece | Today |
|---|---|
| Voice chain | `mesh-node/src/media.rs` `Chain::step`: echo cancel + high-pass (sonora) → RNNoise → automatic gain → input volume → gate → Opus. 48 kHz mono, 20 ms frames. |
| Voice encoder | Opus, `Application::Voip`, **fixed 32 kb/s**, in-band FEC. Encoded once and sent to every device in the call, so upload = bitrate × listeners. |
| Aflos's own video | **None.** Aflos-native video/screen share is ROADMAP Phase 2 (0.6.0). |
| Discord video | `aflos-dc/src/voice.rs` `send_video`: **fixed** camera 1280 wide, 30 fps, 2 Mb/s; screen 1920 wide, 15 fps, 2.5 Mb/s. H.264 via openh264. |
| Discord / WhatsApp voice | Their own mic paths in `aflos-dc` / `aflos-wa`, not `Chain`. |

## Decisions (review these)

### Plugins

| # | Decision | Why |
|---|---|---|
| F1 | **VST3 and CLAP.** VST3 through `vst3` 0.3.0 (MIT/Apache bindings; Steinberg's SDK is MIT since 3.8). CLAP through `clack-host` 0.2.0 (MIT/Apache). No VST2 (Steinberg no longer licenses it), no LV2/AU for now. | VST3 is what most voice plugins ship as on Windows and Linux; CLAP is the open format growing fastest and is easier to host well. Both licences fit AGPL. `rack` was looked at and rejected: untested on Windows and Linux. |
| F2 | **Plugins run in their own process, `aflos-fx`**, started by mesh-node, never inside mesh-node. Audio crosses over a pipe, one 20 ms frame at a time. | A plugin is someone else's native code. Inside mesh-node it could read the identity keys and decrypted messages in memory, and a crash would end the call. In its own process it can do neither. It still runs with your user's file access, like it would in any DAW: Aflos only loads plugins you added yourself. |
| F3 | **Place in the chain: after echo cancel and noise suppression, before automatic gain.** Echo → noise → **your plugins, in your order** → gain → volume → gate → Opus. | The canceller needs the raw mic, so effects can't go before it. Gain after the plugins keeps you at a normal level whatever an EQ does to loudness; turn gain off if your chain has its own compressor. |
| F4 | **Never stall the call.** If `aflos-fx` hasn't answered a frame within 20 ms, that frame goes out dry. Three late frames in a row, or a crash, bypass the chain and tell you, with a "Restart plugins" button. | A slow or broken plugin must cost a moment of unprocessed voice, never silence or a dropped call. |
| F5 | **Latency budget: +20 ms** for the pipe hop, plus whatever the plugin reports (shown next to it). | One frame of buffering; plugin look-ahead is the plugin's own choice. |
| F6 | **The plugin's own window** opens from Settings → Voice & audio ("Edit"), owned by `aflos-fx`. Plugins without a window, or where it can't open, get generic sliders for their parameters. On Wayland, VST3 windows go through XWayland. | An EQ is unusable without its curve. Generic sliders keep every plugin usable. |
| F7 | **Mono in, mono out.** Stereo plugins get the mic on both channels; their two outputs are averaged. 48 kHz, 960-frame blocks, fixed. | The call is mono; this avoids a resampler. |
| F8 | **Found in the standard folders**, plus folders you add: VST3 in `~/.vst3`, `/usr/lib/vst3`, `/usr/local/lib/vst3`, `%CommonProgramFiles%\VST3`; CLAP in `~/.clap`, `/usr/lib/clap`, `%CommonProgramFiles%\CLAP`. Scanning runs in a throwaway `aflos-fx`, so a plugin that crashes on load is just listed as broken. | Same as DAWs. Scan crashes are common and must not hurt Aflos. |
| F9 | **Saved per device**: the chain (paths, order, on/off) in `config.toml`, each plugin's state blob in `fx/` beside it. Not synced to other devices. | Plugins are installed per machine; a path from one PC means nothing on another. |
| F10 | **The mic test includes the plugins.** "Test and hear myself" plays the processed voice back, so tuning happens before a call. | It already runs `Chain`. |
| F11 | **Aflos calls first.** Discord and WhatsApp voice don't go through `Chain` and stay as they are (Q4). | Smallest working slice. |
| F12 | Wording follows Steinberg's VST trademark guidelines (text "VST3" only, no logo, until a licence agreement is signed). | The SDK is MIT now; the trademark isn't. |

### Quality

| # | Decision | Why |
|---|---|---|
| Q-a | **Voice quality setting**, for what you send: **Low 16 kb/s**, **Standard 32 kb/s** (default, today's), **High 64 kb/s**, **Studio 128 kb/s**. High and Studio switch Opus to `Application::Audio` (better for processed and music-like voice); FEC stays on. | Receivers' Opus decoders read any bitrate, so there's no protocol change and 0.5.1/0.5.2 peers hear it fine. 128 kb/s in 20 ms frames is 320 bytes, well within a datagram. |
| Q-b | **Upload shown:** next to the choice, "about N kb/s up in a call of 5" (bitrate × listeners). | Upload is per listener, so Studio in a 10-person channel is ~1.3 Mb/s up. People should see that. |
| Q-c | **Applies live**, mid-call, like the other Voice & audio settings. | Opus changes bitrate per frame. |
| Q-d | **Discord video settings** (camera and screen separately): resolution (480p / 720p / 1080p; screen also "source"), frame rate (camera 15/30; screen 5/15/30/60), bitrate (Auto, or 0.5 to 8 Mb/s). Defaults stay today's values. | `send_video` takes these already; they're just constants today. |
| Q-e | **Discord's own limits are respected**: without Nitro, screen share is capped at 720p 30 fps, as in Discord's client (Q5). | Going past what the account pays for is going around a paid feature, which risks the account under Discord's terms. |
| Q-f | **Aflos-native video** (0.6.0) will use the same settings screen and names when it's built. | One place for quality. |

## Out of scope

- VST2, LV2, AU; MIDI; instrument plugins; plugins on what you *hear*.
- Plugins for Discord/WhatsApp voice (Q4).
- A per-channel bitrate cap set by a Space owner (later, with Space relays).
- Syncing plugin chains between devices.

## Testing

- `aflos-fx` unit tests with a built-in test plugin (a fixed gain), both
  formats; a CLAP and a VST3 test plugin built in CI.
- mesh-node process tests: a +6 dB plugin is heard 6 dB louder; a plugin
  that sleeps 50 ms leaves the call going dry and reports bypass; killing
  `aflos-fx` mid-call bypasses within 100 ms.
- Bitrate test: Studio's datagrams are ~4× Standard's; a 0.5.1 engine
  hears a Studio caller.
- By hand: a real EQ (e.g. a free one such as LSP or TDR Nova) on Linux
  and Windows, window opens, tuning heard in the mic test; Discord screen
  share at 1080p60 with Nitro, capped without.

## Questions

1. **Release order.** Ship 0.5.2 now as built (echo, gain, mic test,
   per-person volume; once its CI is green), and this as **0.5.3**?
   *Recommended*: yes, so your friend's quiet mic gets fixed sooner and
   this lands as its own tested release.
2. **Formats.** VST3 + CLAP (*recommended*), or VST3 only?
3. **Plugin windows.** The plugin's own window plus generic sliders as
   fallback (*recommended*), or generic sliders only (much less work, but
   most EQs are painful without their curve)?
4. **Discord/WhatsApp voice.** Aflos calls only for now (*recommended*),
   or plugins and voice quality in Discord/WhatsApp calls too?
5. **Discord tier.** Cap Discord video at what the account's tier allows
   (*recommended*), or let you choose anything?
