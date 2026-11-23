//! Mesh calls in the app (SPEC-calls C9): the call button on a Mesh DM, a
//! call bar like the other networks' (ringing, in call, mute, deafen, hang
//! up), voice channels in a Space's pane, and the ring. The audio runs in
//! `mesh-node`; this only sends its commands and shows its `meshCall` and
//! `meshVoice`. No ID or name is logged.

use super::*;
use iced::widget::row;

#[derive(Debug, Clone)]
pub(crate) enum Msg {
    /// Calls a friend, by ID.
    Start(String),
    Answer,
    /// Hangs up, declines, or leaves a voice channel.
    Hangup,
    Mute,
    Deafen,
    /// Joins a voice channel: its Space and channel.
    Join(String, String),
}

/// The microphone and speakers picked in settings, `""` for the default.
fn devices(state: &App) -> (String, String) {
    let pick = |s: &str| {
        if s == DEFAULT_DEVICE {
            String::new()
        } else {
            s.to_string()
        }
    };
    match &state.screen {
        Screen::LoggedIn(session) => (
            pick(&session.selected_input),
            pick(&session.selected_output),
        ),
        _ => (String::new(), String::new()),
    }
}

pub(crate) fn update(state: &mut App, msg: Msg) -> Task<Message> {
    let Some(node) = state.mesh.node.clone() else {
        return Task::none();
    };
    let call = state.mesh.call.as_ref();
    let with_devices = |state: &App| {
        let (input, output) = devices(state);
        node.call_devices(&input, &output);
    };
    match msg {
        Msg::Start(friend) => {
            with_devices(state);
            node.call_start(&friend);
        }
        Msg::Answer => {
            with_devices(state);
            node.call_answer();
        }
        Msg::Hangup => node.call_hangup(),
        Msg::Mute => node.call_mute(!call.is_some_and(|c| c.muted)),
        Msg::Deafen => node.call_deafen(!call.is_some_and(|c| c.deafened)),
        Msg::Join(space, channel) => {
            with_devices(state);
            node.voice_join(&space, &channel);
        }
    }
    Task::none()
}

/// What to tell someone about how a call ended; nothing for a plain end.
fn ended_note(outcome: &str) -> Option<&'static str> {
    match outcome {
        "declined" => Some("The call was declined."),
        "missed" => Some("No answer."),
        "unreachable" => Some("They aren't reachable right now; they'll see a missed call."),
        "update" => Some("They need to update Mesh to take calls."),
        "lost" => Some("The call dropped."),
        "removed" => Some("You're no longer in that voice channel."),
        _ => None,
    }
}

/// A `meshCall`: kept while the call's on, gone when it ends.
pub(crate) fn event(state: &mut App, call: mesh::Call) -> Task<Message> {
    let was_ringing = state
        .mesh
        .call
        .as_ref()
        .is_some_and(|c| c.state == "ringing");
    if call.state == "ended" {
        state.mesh.call = None;
        if let (Some(note), Screen::LoggedIn(session)) = (
            call.outcome.as_deref().and_then(ended_note),
            &mut state.screen,
        ) {
            session.status = Some(note.to_string());
        }
    } else {
        if call.state == "ringing" && !was_ringing {
            notify(
                format!("{} is calling", state.mesh.name_of(&call.with)),
                "Mesh call".to_string(),
            );
        }
        state.mesh.call = Some(call);
    }
    sync_ringer(state);
    Task::none()
}

/// The ring for a Mesh call: theirs ringing us, or ours ringing them.
pub(crate) fn ring(state: &App) -> Option<sounds::Sound> {
    match state.mesh.call.as_ref().map(|c| c.state.as_str()) {
        Some("ringing") => Some(sounds::Sound::Ring(Network::Mesh)),
        Some("calling") => Some(sounds::Sound::Ringback),
        _ => None,
    }
}

/// The call bar, while a Mesh call is on.
pub(crate) fn bar(state: &App) -> Option<Element<'_, Message>> {
    let call = state.mesh.call.as_ref()?;
    let (p, l) = (&state.palette, &state.layout);
    let voice = call.space.as_ref().zip(call.channel.as_ref());
    let title = match voice {
        Some((s, c)) => {
            let space = state.mesh.spaces.iter().find(|x| x.id == *s);
            let channel = space.and_then(|x| x.channels.iter().find(|ch| ch.id == *c));
            format!(
                "🔊 {} · {}",
                channel.map_or("voice", |ch| ch.name.as_str()),
                space.map_or("Space", |x| x.name.as_str())
            )
        }
        None => state.mesh.name_of(&call.with),
    };
    let status = match call.state.as_str() {
        "calling" => "Ringing…".to_string(),
        "ringing" => "is calling you".to_string(),
        _ if call.speaking.is_empty() => "In call".to_string(),
        _ => {
            let space = voice.and_then(|(s, _)| state.mesh.spaces.iter().find(|x| x.id == *s));
            let names: Vec<String> = call
                .speaking
                .iter()
                .map(|id| crate::mesh_spaces::name_in(&state.mesh, space, id))
                .collect();
            format!("In call · {} speaking", names.join(", "))
        }
    };
    let act = |msg: Msg| Message::MeshCall(msg);
    let mut controls = row![].spacing(8).align_y(iced::Alignment::Center);
    if call.state == "ringing" {
        controls = controls
            .push(
                button(text("Accept").size(13))
                    .on_press(act(Msg::Answer))
                    .padding(8),
            )
            .push(
                button(text("Decline").size(13))
                    .on_press(act(Msg::Hangup))
                    .padding(8),
            );
    } else {
        let mic = if call.muted {
            ui::Icon::MicOff
        } else {
            ui::Icon::Mic
        };
        let ears = if call.deafened {
            ui::Icon::HeadphoneOff
        } else {
            ui::Icon::Headphones
        };
        controls = controls
            .push(ui::icon_button(mic, Some(act(Msg::Mute)), call.muted, p, l))
            .push(ui::icon_button(
                ears,
                Some(act(Msg::Deafen)),
                call.deafened,
                p,
                l,
            ))
            .push(ui::icon_button(
                ui::Icon::PhoneOff,
                Some(act(Msg::Hangup)),
                true,
                p,
                l,
            ));
    }
    let bar = container(
        row![
            ui::icon(ui::Icon::Phone, 17.0, p.accent()),
            ui::mono_bold(Network::Mesh.tag(), 12.0, net_color(Network::Mesh, p)),
            text(title).size(15).color(p.text_bright()),
            text(status)
                .size(13)
                .color(p.text_muted())
                .width(Length::Fill),
            controls,
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center),
    )
    .padding([8, 12])
    .style({
        let (surface, border) = (p.surface_alt(), p.border());
        move |_| container::Style {
            background: Some(surface.into()),
            border: iced::border::rounded(8).color(border).width(1),
            ..container::Style::default()
        }
    });
    Some(bar.into())
}

/// A voice channel in a Space's pane: join on click, and who's in it.
pub(crate) fn voice_row<'a>(
    mesh: &'a mesh_ui::Mesh,
    space: &'a mesh::Space,
    channel: &'a mesh::SpaceChannel,
    p: &'a theme::Palette,
    l: &theme::Layout,
) -> Element<'a, Message> {
    let here = mesh
        .call
        .as_ref()
        .is_some_and(|c| c.channel.as_deref() == Some(channel.id.as_str()));
    let color = if here { p.text_bright() } else { p.text() };
    let label = row![
        ui::icon(ui::Icon::Headphones, 14.0, p.text_muted()),
        ui::sans(channel.name.clone(), 14.0, Weight::Normal, color).width(Length::Fill),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);
    let join = (!here).then(|| Message::MeshCall(Msg::Join(space.id.clone(), channel.id.clone())));
    let mut col = iced::widget::column![
        ui::row_button(label, join.unwrap_or(Message::Noop), here, p, l).padding(iced::Padding {
            top: 6.0,
            right: 8.0,
            bottom: 6.0,
            left: 10.0
        })
    ];
    let people = mesh.voice.get(&(space.id.clone(), channel.id.clone()));
    for id in people.into_iter().flatten() {
        let speaking = mesh
            .call
            .as_ref()
            .is_some_and(|c| here && c.speaking.contains(id));
        let dot = if speaking {
            p.success()
        } else {
            p.text_muted()
        };
        col = col.push(
            row![
                ui::swatch(dot, 6.0, 6.0, 3.0),
                ui::sans(
                    crate::mesh_spaces::name_in(mesh, Some(space), id),
                    12.0,
                    Weight::Normal,
                    p.text()
                )
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center)
            .padding(iced::Padding {
                left: 32.0,
                ..iced::Padding::ZERO
            }),
        );
    }
    col.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every way a call ends that needs saying says something; a plain end
    /// or leaving doesn't.
    #[test]
    fn how_it_ended() {
        for o in [
            "declined",
            "missed",
            "unreachable",
            "update",
            "lost",
            "removed",
        ] {
            assert!(ended_note(o).is_some(), "{o}");
        }
        for o in ["ended", "left", "elsewhere"] {
            assert!(ended_note(o).is_none(), "{o}");
        }
    }
}
