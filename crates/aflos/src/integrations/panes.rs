//! An integration's chats in the channel pane (PLAN-integrations Task 6):
//! its Home chats beside everyone's direct messages, and a section's own
//! pane (a server, its groups) with its chats by category.

use super::{Act, chats, typing};
use crate::{App, Message, Session, WebChat, theme, ui, user_panel, web_chat_row};
use aflos_core::Network;
use aflos_integration::{Action, ChatKind, FetchForm, State};
use iced::font::Weight;
use iced::widget::{column, row, scrollable, text_input};
use iced::{Element, Length};

fn open(network: Network, chat: &aflos_integration::Chat) -> WebChat {
    WebChat {
        network,
        id: chat.id.clone(),
        name: chat.name.clone(),
        dm: chat.kind == ChatKind::Direct,
    }
}

fn row_for<'a>(
    session: &Session,
    network: Network,
    chat: &aflos_integration::Chat,
    p: &theme::Palette,
    l: &theme::Layout,
) -> Element<'a, Message> {
    let picture = chat
        .avatar
        .as_deref()
        .map(|k| chats::url(network, FetchForm::Original, k));
    let count = chat.unread.max(chat.mentions);
    web_chat_row(
        session,
        open(network, chat),
        picture.as_deref(),
        count > 0,
        count,
        p,
        l,
    )
}

/// Its chats that belong in Home (archived ones left out), with when each
/// last moved, for sorting among everyone's.
pub(crate) fn home_rows<'a>(
    session: &'a Session,
    state: &'a App,
    shows: impl Fn(Network) -> bool,
    p: &theme::Palette,
    l: &theme::Layout,
) -> Vec<(u64, Element<'a, Message>)> {
    let mut rows = Vec::new();
    for (_, live, network) in state.integrations.ready() {
        if live.state != Some(State::Online) || !shows(network) {
            continue;
        }
        for chat in live
            .chats
            .sections
            .iter()
            .filter(|s| s.home)
            .flat_map(|s| &s.chats)
        {
            if !chat.archived {
                rows.push((chat.last_ms, row_for(session, network, chat, p, l)));
            }
        }
    }
    rows
}

/// A section's pane: its chats under their categories, newest first within
/// each, and Join by link when it takes them.
pub(crate) fn section_pane<'a>(
    session: &'a Session,
    state: &'a App,
    network: Network,
    section: &str,
    p: &'a theme::Palette,
    l: &theme::Layout,
) -> Element<'a, Message> {
    let footer = user_panel(session, None, p, l);
    let live = state.integrations.by_network(network).map(|(_, live)| live);
    let found = live.and_then(|live| live.chats.sections.iter().find(|s| s.id == section));
    let name = found.map_or("", |s| s.name.as_str());
    let count = found.map_or(0, |s| s.chats.len());
    let header = column![
        ui::sans(name, 15.0, Weight::Semibold, p.text_bright()),
        row![
            ui::mono(network.tag(), 11.0, p.network(network.spec())),
            ui::mono(format!("{count} chats"), 11.0, p.text_muted()),
        ]
        .spacing(6),
    ]
    .spacing(4)
    .padding(iced::Padding {
        top: 14.0,
        right: 14.0,
        bottom: 10.0,
        left: 14.0,
    });

    let mut list = column![].spacing(2).padding([10, 8]);
    if let Some(section) = found {
        // Categories in the order they first appear; uncategorised first.
        let mut categories: Vec<Option<&str>> = Vec::new();
        for chat in &section.chats {
            let c = chat.category.as_deref();
            if !categories.contains(&c) {
                categories.push(c);
            }
        }
        categories.sort_by_key(|c| c.is_some());
        for category in categories {
            let mut chats: Vec<_> = section
                .chats
                .iter()
                .filter(|c| c.category.as_deref() == category)
                .collect();
            // A server keeps its own order; groups go newest first.
            if category.is_none() && chats.iter().all(|c| c.kind != ChatKind::Channel) {
                chats.sort_by_key(|c| std::cmp::Reverse(c.last_ms));
            }
            if let Some(category) = category {
                list = list.push(ui::section(category, p));
            }
            for chat in chats {
                let typing = typing(state, network, &chat.id);
                list = list.push(row_for(session, network, chat, p, l));
                if !typing.is_empty() {
                    list = list.push(ui::mono(
                        format!("   {} typing…", typing.join(", ")),
                        11.0,
                        p.text_muted(),
                    ));
                }
            }
        }
    }
    if let (Some(section), Some(live)) = (found, live)
        && !section.rooms.is_empty()
    {
        let here = live.voice.room.as_deref().filter(|_| live.voice.connected);
        list = list.push(ui::section("voice", p)).push(super::calls::rooms(
            &section.rooms,
            network,
            here,
            p,
            l,
        ));
    }
    let joins = live
        .and_then(|l| l.hello.as_ref())
        .is_some_and(|h| h.supports.actions.contains(&Action::JoinByLink));
    if joins {
        list = list.push(ui::section("join by link", p)).push(
            text_input("paste an invite link", &state.integrations.join_link)
                .on_input(|s| Message::Integrations(Act::JoinLink(s)))
                .on_submit(Message::Integrations(Act::Join(network)))
                .padding([8, 10])
                .size(13),
        );
    }
    column![
        header,
        ui::hline(p),
        scrollable(list).height(Length::Fill),
        ui::hline(p),
        footer
    ]
    .width(Length::Fill)
    .into()
}

/// Its archived Home chats: one row that opens them, then (open) the chats.
pub(crate) fn archived<'a>(
    session: &'a Session,
    state: &'a App,
    shows: impl Fn(Network) -> bool,
    p: &theme::Palette,
    l: &theme::Layout,
) -> Vec<Element<'a, Message>> {
    let mut chats = Vec::new();
    for (_, live, network) in state.integrations.ready() {
        if live.state == Some(State::Online) && shows(network) {
            let home = live
                .chats
                .sections
                .iter()
                .filter(|s| s.home)
                .flat_map(|s| &s.chats);
            chats.extend(home.filter(|c| c.archived).map(|c| (network, c)));
        }
    }
    if chats.is_empty() {
        return Vec::new();
    }
    let unread: u64 = chats.iter().map(|(_, c)| c.unread).sum();
    let arrow = if session.show_archived { "▾" } else { "▸" };
    let mut line = row![
        ui::sans(
            format!("Archived ({})", chats.len()),
            14.0,
            Weight::Normal,
            p.text()
        )
        .width(Length::Fill)
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);
    if unread > 0 {
        line = line.push(ui::chip(unread.to_string(), p));
    }
    let line = line.push(ui::mono(arrow, 12.0, p.text_muted()));
    let mut rows =
        vec![ui::row_button(line, Message::ToggleArchived, session.show_archived, p, l).into()];
    if session.show_archived {
        chats.sort_by_key(|(_, c)| std::cmp::Reverse(c.last_ms));
        rows.extend(
            chats
                .into_iter()
                .map(|(network, chat)| row_for(session, network, chat, p, l)),
        );
    }
    rows
}

/// The open chat's Archive and Delete, when its integration takes them.
pub(crate) fn chat_actions<'a>(
    session: &Session,
    state: &App,
    channel: &WebChat,
    p: &'a theme::Palette,
) -> Vec<Element<'a, Message>> {
    let network = channel.network;
    let Some((_, live)) = state.integrations.by_network(network) else {
        return Vec::new();
    };
    let can = |a: Action| {
        live.hello
            .as_ref()
            .is_some_and(|h| h.supports.actions.contains(&a))
    };
    let flat = |label: &'static str, color: iced::Color, press: Message| -> Element<'a, Message> {
        iced::widget::button(iced::widget::text(label).size(12).color(color))
            .on_press(press)
            .padding([4, 10])
            .style(crate::flat_button_style(p))
            .into()
    };
    if session.confirm_delete.as_deref() == Some(channel.id.as_str()) {
        return vec![
            ui::mono("delete this chat?", 11.0, p.danger()).into(),
            flat("Delete", p.danger(), Message::ChatDeleteConfirm),
            flat("Cancel", p.text(), Message::ChatDeleteCancel),
        ];
    }
    let mut actions = Vec::new();
    if can(Action::Archive) {
        let archived = super::chat(state, network, &channel.id).is_some_and(|c| c.archived);
        let press = Message::ChatArchive(network, channel.id.clone(), !archived);
        actions.push(flat(
            if archived { "Unarchive" } else { "Archive" },
            p.text(),
            press,
        ));
    }
    if can(Action::DeleteChat) {
        actions.push(flat(
            "Delete",
            p.text_muted(),
            Message::ChatDeleteAsk(channel.id.clone()),
        ));
    }
    actions
}
