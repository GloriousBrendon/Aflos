//! An integration's Accounts panel (PLAN-integrations Task 5): how it's
//! doing, the sign-in step it asks for, sign out, and the way out when it
//! breaks: Restart, Turn off, Remove (spec T5, T7).

use super::{Act, Entry, Live, Trouble, logo};
use crate::{App, Message, flat_button_style, theme, ui};
use aflos_core::Network;
use aflos_integration::{SignIn, State};
use iced::font::Weight;
use iced::widget::{Space, button, column, container, image, row, svg, text, text_input};
use iced::{Element, Length};

fn act(a: Act) -> Message {
    Message::Integrations(a)
}

/// The panel for `entry` (an integration that never said hello has no
/// network yet, but still gets its way out).
pub(crate) fn panel<'a>(state: &'a App, entry: &'a Entry) -> Element<'a, Message> {
    let (p, l) = (&state.palette, &state.layout);
    let all = &state.integrations;
    let live = all.live.get(&entry.id);
    let network = live.and_then(|l| l.network);
    let name = live
        .and_then(|l| l.hello.as_ref())
        .map_or(entry.id.as_str(), |h| h.name.as_str());
    let status = match (
        entry.off,
        live.and_then(|l| l.trouble.as_ref()),
        live.and_then(|l| l.state),
    ) {
        (true, _, _) => ("turned off", p.text_muted()),
        (_, Some(Trouble::Stopped(_)), _) => ("stopped", p.danger()),
        (_, Some(Trouble::Restarting(_)), _) => ("restarting…", p.warning()),
        (_, None, Some(State::Online)) => ("● connected", p.success()),
        (_, None, Some(State::SignedOut)) => ("signed out", p.warning()),
        (_, None, Some(State::Offline)) => ("▲ offline", p.warning()),
        (_, None, Some(State::Connecting) | None) => ("connecting…", p.text_muted()),
    };
    let face = match (live, network) {
        (Some(live), Some(n)) => logo(live, 28.0, p.network(n.spec())),
        _ => ui::icon(ui::Icon::Hash, 28.0, p.text_muted()),
    };
    let header = row![
        container(face).center_x(40).center_y(40),
        column![
            ui::sans(name, 17.0, Weight::Semibold, p.text_bright()),
            ui::mono(status.0, 11.0, status.1),
        ]
        .spacing(2),
    ]
    .spacing(12)
    .align_y(iced::Alignment::Center);

    let mut body = column![header].spacing(18).height(Length::Fill);
    if let Some(trouble) = live.and_then(|l| l.trouble.as_ref()) {
        let why = match trouble {
            Trouble::Restarting(why) => format!("{name} stopped ({why}) and is starting again."),
            Trouble::Stopped(why) => format!("{name} stopped: {why}."),
        };
        let mut col = column![text(why).size(13).color(p.text())].spacing(10);
        if matches!(trouble, Trouble::Stopped(_)) {
            col = col.push(
                ui::primary_button(
                    ui::sans("Restart", 13.0, Weight::Semibold, p.accent_text()),
                    Some(act(Act::Restart(entry.id.clone()))),
                    p,
                    l,
                )
                .padding([6, 12]),
            );
        }
        let edge = p.warning();
        body = body.push(
            container(col)
                .padding([10, 12])
                .width(Length::Fill)
                .style(move |_| container::Style {
                    border: iced::border::rounded(8).color(edge).width(1),
                    ..container::Style::default()
                }),
        );
    }

    match (live, network) {
        (Some(live), Some(network)) if !entry.off && live.trouble.is_none() => {
            body = body.push(if live.state == Some(State::Online) {
                signed_in(state, network)
            } else {
                sign_in(state, live, network)
            });
        }
        _ => {}
    }

    body = body.push(Space::new().height(Length::Fill));
    let confirm = all.confirm_remove.as_deref() == Some(entry.id.as_str());
    let off = button(ui::sans(
        if entry.off { "Turn on" } else { "Turn off" },
        13.0,
        Weight::Normal,
        p.text(),
    ))
    .on_press(act(Act::Off(entry.id.clone(), !entry.off)))
    .padding([8, 12])
    .style(flat_button_style(p));
    let danger = p.danger();
    let remove = button(ui::sans(
        if confirm {
            "Really remove? Click again".to_string()
        } else {
            format!("Remove {name}")
        },
        13.0,
        Weight::Semibold,
        danger,
    ))
    .on_press(act(Act::Remove(entry.id.clone())))
    .padding([8, 12])
    .style(move |_, status| button::Style {
        background: matches!(status, button::Status::Hovered)
            .then(|| theme::alpha(danger, 0.12).into()),
        border: iced::border::rounded(6).color(danger).width(1),
        ..button::Style::default()
    });
    body = body
        .push(row![off, Space::new().width(Length::Fill), remove].align_y(iced::Alignment::Center))
        .push(
            text("Remove deletes it and everything it kept on this computer.")
                .size(12)
                .color(p.text_muted()),
        )
        .push(ui::mono(
            format!("integration · {}", entry.id),
            11.0,
            p.text_muted(),
        ));
    container(body)
        .padding(22)
        .width(380)
        .height(Length::Fill)
        .into()
}

fn signed_in<'a>(state: &'a App, network: Network) -> Element<'a, Message> {
    let p = &state.palette;
    let label = if state.accounts.confirm_sign_out {
        "Really sign out? Click again"
    } else {
        "Sign out"
    };
    let danger = p.danger();
    column![
        text("Signed in.").size(13).color(p.text()),
        button(container(ui::sans(label, 14.0, Weight::Semibold, danger)).center_x(Length::Fill))
            .on_press(Message::SignOut(network))
            .width(Length::Fill)
            .padding([10, 16])
            .style(move |_, status| button::Style {
                background: matches!(status, button::Status::Hovered)
                    .then(|| theme::alpha(danger, 0.12).into()),
                border: iced::border::rounded(6).color(danger).width(1),
                ..button::Style::default()
            }),
    ]
    .spacing(12)
    .into()
}

/// The step it asks for: a QR code, a code, a form, or a web page.
fn sign_in<'a>(state: &'a App, live: &'a Live, network: Network) -> Element<'a, Message> {
    let (p, l) = (&state.palette, &state.layout);
    let all = &state.integrations;
    let submit = |label: &'static str| {
        ui::primary_button(
            container(ui::sans(label, 14.0, Weight::Semibold, p.accent_text()))
                .center_x(Length::Fill),
            Some(act(Act::Submit(network))),
            p,
            l,
        )
        .width(Length::Fill)
        .padding([10, 16])
    };
    let mut col = column![].spacing(12);
    match &live.sign_in {
        None => {
            col = col.push(
                text("Waiting for it to say how to sign in…")
                    .size(13)
                    .color(p.text_muted()),
            )
        }
        Some(SignIn::Qr { png, svg: markup }) => {
            let code: Element<'a, Message> = match (png.as_deref().and_then(decode), markup) {
                (Some(bytes), _) => image(image::Handle::from_bytes(bytes))
                    .width(200)
                    .height(200)
                    .into(),
                (None, Some(markup)) => svg(svg::Handle::from_memory(markup.clone().into_bytes()))
                    .width(200)
                    .height(200)
                    .into(),
                (None, None) => text("Getting a code…").size(12).into(),
            };
            let light = p.text_bright();
            col = col
                .push(
                    container(
                        container(code)
                            .padding(12)
                            .style(move |_| container::Style {
                                background: Some(light.into()),
                                border: iced::border::rounded(8),
                                ..container::Style::default()
                            }),
                    )
                    .center_x(Length::Fill),
                )
                .push(
                    text("Scan it with the app on your phone.")
                        .size(13)
                        .color(p.text()),
                );
        }
        Some(SignIn::Code { prompt }) => {
            col = col
                .push(text(prompt.clone()).size(13).color(p.text()))
                .push(
                    text_input("", &all.code)
                        .on_input(|c| act(Act::Code(c)))
                        .on_submit(act(Act::Submit(network)))
                        .padding([10, 12])
                        .size(13),
                )
                .push(submit("Continue"));
        }
        Some(SignIn::Form { fields }) => {
            for f in fields {
                let name = f.name.clone();
                col = col.push(
                    column![
                        ui::mono(f.label.to_uppercase(), 11.0, p.text_muted()),
                        text_input("", all.form.get(&f.name).map_or("", String::as_str))
                            .on_input(move |v| act(Act::Field(name.clone(), v)))
                            .on_submit(act(Act::Submit(network)))
                            .secure(f.secret)
                            .padding([10, 12])
                            .size(13),
                    ]
                    .spacing(6),
                );
            }
            col = col.push(submit("Sign in"));
        }
        Some(SignIn::Web { url }) => {
            col = col.push(
                button(
                    container(ui::sans(
                        "Sign in in your browser",
                        14.0,
                        Weight::Semibold,
                        p.text_bright(),
                    ))
                    .center_x(Length::Fill),
                )
                .on_press(Message::OpenUrl(url.clone()))
                .width(Length::Fill)
                .padding(12)
                .style(flat_button_style(p)),
            );
        }
        Some(SignIn::Failed { error }) => {
            col = col.push(text(error.clone()).size(13).color(p.danger()))
        }
    }
    if let Some(error) = &live.sign_in_error {
        col = col.push(text(error.clone()).size(12).color(p.danger()));
    }
    col.into()
}

fn decode(b64: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.decode(b64).ok()
}
