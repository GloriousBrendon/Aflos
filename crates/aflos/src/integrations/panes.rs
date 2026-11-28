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
