//! An integration's calls and voice rooms (PLAN-integrations Task 7): the
//! call bar (ringing, answer, mute, camera, screen with its picker, video),
//! the voice room bar (members, streams to watch), the rooms in its section
//! pane, and the rings.

use super::Live;
use crate::{
    App, Message, ShareChoice, VideoTile, flat_button_style, net_color, theme, ui, video_live,
    video_tiles,
};
use aflos_core::Network;
use aflos_integration::{Call, Cmd, Phase, Room};
use iced::font::Weight;
use iced::widget::{button, column, container, row, text};
use iced::{Element, Length};

/// What the call and voice bars ask of an integration.
#[derive(Debug, Clone)]
pub(crate) enum CallAct {
    Start {
        chat: String,
        video: bool,
    },
    Accept(bool),
    Decline,
    End,
    Mute,
    Camera,
    /// Share (opening the picker) or stop sharing.
    Screen,
    SharePick(String),
    ShareCancel,
    Join(String),
    Leave,
    Deafen,
    Watch(String, bool),
    /// Stop ringing for a call in a room (it may ring on elsewhere).
    Dismiss(String),
}

fn msg(network: Network, a: CallAct) -> Message {
    Message::IntegrationCall(network, a)
}

/// A screen it offers, for the share picker.
pub(crate) fn choice(s: &aflos_integration::Source) -> ShareChoice {
    use base64::Engine as _;
    let jpeg = s
        .thumbnail
        .as_deref()
        .and_then(|b| base64::engine::general_purpose::STANDARD.decode(b).ok());
    ShareChoice {
        id: s.id.clone(),
        name: s.name.clone(),
        screen: true,
        thumbnail: jpeg.map(iced::widget::image::Handle::from_bytes),
    }
}

/// What a call, video or ring event changes beyond what's kept: video tiles,
/// and notifications for a call or a room starting to ring us.
pub(crate) fn on_event(state: &mut App, network: Network, event: &aflos_integration::Event) {
    use aflos_integration::Event;
    let name = network.spec().name;
    let live = state.integrations.by_network(network).map(|(_, l)| l);
    match event {
        Event::Call { call } => {
            let old = live.and_then(|l| l.call.clone());
            cues(name, old.as_ref(), call.as_ref(), &[], &[]);
            if call.as_ref().is_none_or(|c| c.phase == Phase::Ended) {
                let prefix = track_prefix(network);
                state.video.retain(|id, _| !id.starts_with(&prefix));
            }
        }
        Event::Voice(voice) => {
            let old: Vec<String> = live
                .map(|l| l.voice.ringing.iter().map(|r| r.room.clone()).collect())
                .unwrap_or_default();
            let new: Vec<(String, String)> = voice
                .ringing
                .iter()
                .map(|r| (r.room.clone(), r.name.clone()))
                .collect();
            cues(name, None, None, &old, &new);
        }
        Event::Video { track, local, jpeg } => {
            use base64::Engine as _;
            let frame = base64::engine::general_purpose::STANDARD
                .decode(jpeg)
                .ok()
                .and_then(|b| {
                    image::load_from_memory_with_format(&b, image::ImageFormat::Jpeg).ok()
                })
                .map(|i| i.into_rgba8());
            if let Some(frame) = frame {
                let (w, h) = frame.dimensions();
                let handle = iced::widget::image::Handle::from_rgba(w, h, frame.into_raw());
                let tile = VideoTile {
                    frame: handle,
                    local: *local,
                    at: std::time::Instant::now(),
                    label: None,
                };
                state
                    .video
                    .insert(format!("{}{track}", track_prefix(network)), tile);
            }
        }
        Event::VideoEnded { track } => drop(
            state
                .video
                .remove(&format!("{}{track}", track_prefix(network))),
        ),
        _ => {}
    }
}

/// The prefix of its video tracks in `App::video`.
pub(crate) fn track_prefix(network: Network) -> String {
    match network {
        Network::Integration(slot) => format!("int-{slot}-"),
        _ => String::new(),
    }
}

pub(crate) fn act(state: &mut App, network: Network, a: CallAct) {
    let all = &mut state.integrations;
    let Some((_, live)) = all.by_network(network) else {
        return;
    };
    let call = live.call.clone();
    let voice = live.voice.clone();
    let send = |cmd| {
        all.send(network, cmd);
    };
    match a {
        CallAct::Start { chat, video } => send(Cmd::CallStart { chat, video }),
        CallAct::Accept(video) => send(Cmd::CallAccept { video }),
        CallAct::Decline => send(Cmd::CallDecline),
        CallAct::End => send(Cmd::CallEnd),
        CallAct::Leave => send(Cmd::Leave),
        CallAct::Join(room) => send(Cmd::Join { room }),
        CallAct::Watch(stream, on) => send(Cmd::Watch { stream, on }),
        CallAct::Deafen => send(Cmd::Deafen {
            on: !voice.deafened,
        }),
        CallAct::Mute => {
            let muted = call.as_ref().map_or(voice.muted, |c| c.muted);
            send(Cmd::Mute { on: !muted });
        }
        CallAct::Camera => {
            let on = call.as_ref().map_or(voice.camera, |c| c.camera);
            send(Cmd::Camera { on: !on });
        }
        CallAct::Screen => {
            let sharing = call.as_ref().map_or(voice.screen, |c| c.screen);
            if sharing {
                send(Cmd::Screen {
                    on: false,
                    source: None,
                });
            } else {
                send(Cmd::Sources);
                if let Some(live) = all.live_mut(network) {
                    live.sources = Some(Vec::new());
                }
            }
        }
        CallAct::SharePick(source) => {
            send(Cmd::Screen {
                on: true,
                source: Some(source),
            });
            if let Some(live) = all.live_mut(network) {
                live.sources = None;
            }
        }
        CallAct::ShareCancel => {
            if let Some(live) = all.live_mut(network) {
                live.sources = None;
            }
        }
        CallAct::Dismiss(room) => {
            if let Some(live) = all.live_mut(network) {
                live.dismissed.push(room);
            }
        }
    }
}

/// The ring an integration wants: a call or a room ringing us, else ours
/// ringing out.
pub(crate) fn ring(all: &super::Integrations) -> Option<crate::sounds::Sound> {
    use crate::sounds::Sound;
    let mut ringback = false;
    for (_, live, network) in all.ready() {
        let rooms = live
            .voice
            .ringing
            .iter()
            .any(|r| !live.dismissed.contains(&r.room));
        match live.call.as_ref().map(|c| c.phase) {
            Some(Phase::Incoming) => return Some(Sound::Ring(network)),
            _ if rooms => return Some(Sound::Ring(network)),
            Some(Phase::Outgoing) => ringback = true,
            _ => {}
        }
    }
    ringback.then_some(Sound::Ringback)
}

/// What a call or room change says out loud: a call starting to ring us.
pub(crate) fn cues(
    name: &str,
    old: Option<&Call>,
    new: Option<&Call>,
    old_rings: &[String],
    new_rings: &[(String, String)],
) {
    let ringing = |c: Option<&Call>| c.is_some_and(|c| c.phase == Phase::Incoming);
    if ringing(new)
        && !ringing(old)
        && let Some(c) = new
    {
        let kind = if c.video { "video call" } else { "call" };
        crate::notify(format!("{name} {kind}"), format!("{} is calling", c.name));
    }
    for (room, who) in new_rings {
        if !old_rings.contains(room) {
            crate::notify(format!("{name} call"), format!("{who} is calling"));
        }
    }
}

fn video(state: &App, network: Network) -> impl Iterator<Item = &VideoTile> {
    let prefix = track_prefix(network);
    state
        .video
        .iter()
        .filter(move |(id, t)| id.starts_with(&prefix) && video_live(id, t))
        .map(|(_, t)| t)
}

/// What its calls can do, when a call can be started now (none going).
pub(crate) fn can_call(state: &App, network: Network) -> Option<&aflos_integration::Calls> {
    let (_, live) = state.integrations.by_network(network)?;
    let calls = live.hello.as_ref()?.supports.calls.as_ref()?;
    live.call.is_none().then_some(calls)
}

/// Whether its bars' video should fill the chat pane.
pub(crate) fn big(state: &App, network: Network) -> bool {
    state.video_big && video(state, network).next().is_some()
}

fn bar<'a>(
    body: iced::widget::Column<'a, Message>,
    big: bool,
    p: &theme::Palette,
) -> Element<'a, Message> {
    let (fill, edge) = (p.surface_alt(), p.border());
    container(body)
        .padding(12)
        .width(Length::Fill)
        .height(crate::fill_if(big))
        .style(move |_| container::Style {
            background: Some(fill.into()),
            border: iced::border::rounded(8).color(edge).width(1),
            ..container::Style::default()
        })
        .into()
}

/// Its bars for the chat pane: the call, the voice room, rooms ringing us.
pub(crate) fn bars<'a>(
    state: &'a App,
    p: &'a theme::Palette,
    l: &theme::Layout,
) -> Vec<(Element<'a, Message>, bool)> {
    let mut bars = Vec::new();
    for (_, live, network) in state.integrations.ready() {
        let big = big(state, network);
        if let Some(call) = &live.call {
            bars.push((call_bar(state, live, network, call, big, p, l), big));
        } else if live.voice.connected {
            bars.push((voice_bar(state, live, network, big, p, l), big));
        }
        for ring in live
            .voice
            .ringing
            .iter()
            .filter(|r| !live.dismissed.contains(&r.room))
        {
            bars.push((ring_bar(network, &ring.room, &ring.name, p, l), false));
        }
    }
    bars
}

fn controls<'a>(
    network: Network,
    (muted, camera, screen): (bool, bool, bool),
    calls: Option<&aflos_integration::Calls>,
    p: &theme::Palette,
    l: &theme::Layout,
) -> iced::widget::Row<'a, Message> {
    let mut controls = row![].spacing(8).align_y(iced::Alignment::Center);
    let mic = if muted {
        ui::Icon::MicOff
    } else {
        ui::Icon::Mic
    };
    controls = controls.push(ui::icon_button(
        mic,
        Some(msg(network, CallAct::Mute)),
        muted,
        p,
        l,
    ));
    if calls.is_some_and(|c| c.video) {
        let cam = if camera {
            ui::Icon::Video
        } else {
            ui::Icon::VideoOff
        };
        controls = controls.push(ui::icon_button(
            cam,
            Some(msg(network, CallAct::Camera)),
            !camera,
            p,
            l,
        ));
    }
    if calls.is_some_and(|c| c.screen) {
        let share = if screen {
            ui::Icon::ScreenShare
        } else {
            ui::Icon::ScreenShareOff
        };
        controls = controls.push(ui::icon_button(
            share,
            Some(msg(network, CallAct::Screen)),
            !screen,
            p,
            l,
        ));
    }
    controls
}

fn call_bar<'a>(
    state: &'a App,
    live: &'a Live,
    network: Network,
    call: &'a Call,
    big: bool,
    p: &'a theme::Palette,
    l: &theme::Layout,
) -> Element<'a, Message> {
    let calls = live.hello.as_ref().and_then(|h| h.supports.calls.as_ref());
    let status = match call.phase {
        Phase::Incoming if call.video => "video call · ringing",
        Phase::Incoming => "calling you",
        Phase::Outgoing => "ringing…",
        Phase::Connecting => "connecting…",
        Phase::Active | Phase::Ended => "in call",
    };
    let mut row_ = row![].spacing(8).align_y(iced::Alignment::Center);
    if call.phase == Phase::Incoming {
        let answer = |label: &'static str, video: bool| {
            ui::primary_button(
                ui::sans(label, 13.0, Weight::Semibold, p.accent_text()),
                Some(msg(network, CallAct::Accept(video))),
                p,
                l,
            )
            .padding([6, 12])
        };
        if call.video {
            row_ = row_.push(answer("Answer with video", true));
        }
        row_ = row_.push(answer(
            if call.video { "Voice only" } else { "Answer" },
            false,
        ));
        row_ = row_.push(ui::icon_button(
            ui::Icon::PhoneOff,
            Some(msg(network, CallAct::Decline)),
            true,
            p,
            l,
        ));
    } else {
        row_ = controls(network, (call.muted, call.camera, call.screen), calls, p, l).push(
            ui::icon_button(
                ui::Icon::PhoneOff,
                Some(msg(network, CallAct::End)),
                true,
                p,
                l,
            ),
        );
    }
    let active = call.phase == Phase::Active;
    let mut body = column![
        row![
            ui::icon(
                ui::Icon::Phone,
                17.0,
                if active { p.success() } else { p.warning() }
            ),
            ui::mono_bold(network.tag(), 12.0, net_color(network, p)),
            ui::sans(&call.name, 14.0, Weight::Semibold, p.text_bright()),
            text(status)
                .size(12)
                .color(p.text_muted())
                .width(Length::Fill),
            row_,
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center),
    ]
    .spacing(8);
    body = picker(live, network, body, p, l);
    if let Some(tiles) = video_tiles(video(state, network), big, p) {
        body = body.push(tiles);
    }
    bar(body, big, p)
}

fn voice_bar<'a>(
    state: &'a App,
    live: &'a Live,
    network: Network,
    big: bool,
    p: &'a theme::Palette,
    l: &theme::Layout,
) -> Element<'a, Message> {
    let v = &live.voice;
    let calls = live.hello.as_ref().and_then(|h| h.supports.calls.as_ref());
    let deaf = if v.deafened {
        ui::Icon::HeadphoneOff
    } else {
        ui::Icon::Headphones
    };
    let controls = controls(network, (v.muted, v.camera, v.screen), calls, p, l)
        .push(ui::icon_button(
            deaf,
            Some(msg(network, CallAct::Deafen)),
            v.deafened,
            p,
            l,
        ))
        .push(ui::icon_button(
            ui::Icon::PhoneOff,
            Some(msg(network, CallAct::Leave)),
            true,
            p,
            l,
        ));
    let mut people = row![].spacing(10).align_y(iced::Alignment::Center);
    for m in &v.members {
        let color = if m.speaking { p.success() } else { p.text() };
        let mark = if m.deafened {
            " 🔇"
        } else if m.muted {
            " 🎙̸"
        } else {
            ""
        };
        people = people.push(text(format!("{}{mark}", m.name)).size(12).color(color));
    }
    let mut body = column![
        row![
            ui::icon(
                ui::Icon::Voice,
                17.0,
                if v.connected {
                    p.success()
                } else {
                    p.warning()
                }
            ),
            ui::mono_bold(network.tag(), 12.0, net_color(network, p)),
            ui::sans(&v.room_name, 14.0, Weight::Semibold, p.text_bright()).width(Length::Fill),
            controls,
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center),
        people.wrap(),
    ]
    .spacing(8);
    if !v.streams.is_empty() {
        let mut streams = row![].spacing(8).align_y(iced::Alignment::Center);
        for s in &v.streams {
            let label = if s.watching {
                format!("Stop watching {}", s.name)
            } else {
                format!("Watch {}'s stream", s.name)
            };
            streams = streams.push(
                button(text(label).size(12).color(p.text_bright()))
                    .on_press(msg(network, CallAct::Watch(s.id.clone(), !s.watching)))
                    .padding([4, 10])
                    .style(flat_button_style(p)),
            );
        }
        body = body.push(streams);
    }
    body = picker(live, network, body, p, l);
    if let Some(tiles) = video_tiles(video(state, network), big, p) {
        body = body.push(tiles);
    }
    bar(body, big, p)
}

/// The share picker, while it's open.
fn picker<'a>(
    live: &'a Live,
    network: Network,
    body: iced::widget::Column<'a, Message>,
    p: &'a theme::Palette,
    l: &theme::Layout,
) -> iced::widget::Column<'a, Message> {
    match &live.sources {
        Some(choices) => body.push(crate::share_picker(
            choices,
            move |id| msg(network, CallAct::SharePick(id)),
            msg(network, CallAct::ShareCancel),
            p,
            l,
        )),
        None => body,
    }
}

fn ring_bar<'a>(
    network: Network,
    room: &str,
    name: &'a str,
    p: &'a theme::Palette,
    l: &theme::Layout,
) -> Element<'a, Message> {
    let answer = ui::primary_button(
        ui::sans("Answer", 13.0, Weight::Semibold, p.accent_text()),
        Some(msg(network, CallAct::Join(room.to_owned()))),
        p,
        l,
    )
    .padding([6, 12]);
    let body = column![
        row![
            ui::icon(ui::Icon::Phone, 17.0, p.warning()),
            ui::mono_bold(network.tag(), 12.0, net_color(network, p)),
            ui::sans(name, 14.0, Weight::Semibold, p.text_bright()),
            text("calling you")
                .size(12)
                .color(p.text_muted())
                .width(Length::Fill),
            answer,
            ui::icon_button(
                ui::Icon::PhoneOff,
                Some(msg(network, CallAct::Dismiss(room.to_owned()))),
                true,
                p,
                l
            ),
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center),
    ];
    bar(body, false, p)
}

/// A section's voice rooms, each with who's in it; pressing one joins it.
pub(crate) fn rooms<'a>(
    rooms: &'a [Room],
    network: Network,
    current: Option<&str>,
    p: &'a theme::Palette,
    l: &theme::Layout,
) -> iced::widget::Column<'a, Message> {
    let mut col = column![].spacing(2);
    for room in rooms {
        let here = current == Some(room.id.as_str());
        let mut line = row![
            ui::icon(
                ui::Icon::Voice,
                14.0,
                if here { p.success() } else { p.text_muted() }
            ),
            ui::sans(
                &room.name,
                14.0,
                Weight::Normal,
                if room.locked {
                    p.text_muted()
                } else {
                    p.text()
                }
            )
            .width(Length::Fill),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);
        if here {
            line = line.push(ui::mono("connected", 10.0, p.success()));
        }
        let press = (!room.locked && !here).then(|| msg(network, CallAct::Join(room.id.clone())));
        let line: Element<'a, Message> = match press {
            Some(press) => ui::row_button(line, press, here, p, l).into(),
            None => container(line).padding([6, 10]).into(),
        };
        col = col.push(line);
        for m in &room.members {
            let color = if m.speaking { p.success() } else { p.text() };
            col = col.push(
                container(ui::mono(&m.name, 12.0, color)).padding(iced::Padding {
                    left: 30.0,
                    ..iced::Padding::ZERO
                }),
            );
        }
    }
    col
}
