//! Mesh's own Spaces (SPEC-spaces *Mesh-Voice*, PLAN-spaces Task 12): a tile
//! each on the rail, their channels by category, and each channel in the
//! chat pane on the DM path (its lines are a DM page's). Message text and
//! IDs are never logged.

use super::*;
use iced::widget::{column, row};

/// A channel's chat ID in the chat pane: `<space>:<channel>`. A DM's never
/// holds a `:` (a `mesh1…` ID or a group's hex).
pub(crate) fn chat_id(space: &str, channel: &str) -> String {
    format!("{space}:{channel}")
}

/// The Space and channel of a chat ID, if it's a channel's.
pub(crate) fn split(chat: &str) -> Option<(&str, &str)> {
    chat.split_once(':')
}

// Each chat action, to a channel's command or a DM's.
pub(crate) fn open(node: &mesh::Node, chat: &str) {
    match split(chat) {
        Some((s, c)) => node.space_open(s, c, None),
        None => node.dm_open(chat, None),
    }
}
pub(crate) fn send(
    node: &mesh::Node,
    spaces: &[mesh::Space],
    chat: &str,
    text: &str,
    reply_to: Option<&str>,
) {
    match split(chat) {
        Some((s, c)) => {
            let mentions = spaces
                .iter()
                .find(|x| x.id == s)
                .map(|x| mentions_in(x, text))
                .unwrap_or_default();
            node.space_send(s, c, text, reply_to, &mentions);
        }
        None => node.dm_send(chat, text, reply_to),
    }
}
pub(crate) fn typing(node: &mesh::Node, chat: &str) {
    match split(chat) {
        Some((s, c)) => node.space_typing(s, c),
        None => node.dm_typing(chat),
    }
}
pub(crate) fn send_file(node: &mesh::Node, chat: &str, path: &str, preview: Option<&str>) {
    match split(chat) {
        Some((s, c)) => node.space_send_file(s, c, path, preview),
        None => node.dm_send_file(chat, path, preview),
    }
}
pub(crate) fn fetch(node: &mesh::Node, chat: &str, message: &str, index: usize) {
    match split(chat) {
        Some((s, c)) => node.space_fetch(s, c, message, index),
        None => node.dm_fetch(chat, message, index),
    }
}
pub(crate) fn read(node: &mesh::Node, chat: &str, up_to: &str) {
    match split(chat) {
        Some((s, c)) => node.space_read(s, c, up_to),
        None => node.dm_read(chat, up_to),
    }
}
pub(crate) fn edit(node: &mesh::Node, chat: Option<&str>, message: &str, text: &str) {
    match chat.and_then(split) {
        Some((s, c)) => node.space_edit(s, c, message, text),
        None => node.dm_edit(message, text),
    }
}
/// A channel's delete is ours for everyone, or anyone's as an admin.
pub(crate) fn delete(node: &mesh::Node, chat: Option<&str>, message: &str, everyone: bool) {
    match chat.and_then(split) {
        Some((s, c)) => node.space_delete(s, c, message),
        None => node.dm_delete(message, everyone),
    }
}
pub(crate) fn react(node: &mesh::Node, chat: Option<&str>, message: &str, emoji: &str, on: bool) {
    match chat.and_then(split) {
        Some((s, c)) => node.space_react(s, c, message, emoji, on),
        None => node.dm_react(message, emoji, on),
    }
}

/// The members a line names as `@name` (any case), and `@everyone` if we're
/// an admin (the helper refuses it from anyone else, S12).
// ponytail: names typed out, no picker; two members with one name are both named.
pub(crate) fn mentions_in(space: &mesh::Space, text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut out: Vec<String> = (space.members.iter())
        .filter(|m| !m.name.is_empty() && lower.contains(&format!("@{}", m.name.to_lowercase())))
        .map(|m| m.id.clone())
        .collect();
    if lower.contains("@everyone") && matches!(space.role.as_deref(), Some("owner" | "admin")) {
        out.push("@everyone".to_string());
    }
    out
}

/// A member's name: ours or a friend's as `name_of` gives it, else the name
/// they go by in the Space.
pub(crate) fn name_in(mesh: &mesh_ui::Mesh, space: Option<&mesh::Space>, id: &str) -> String {
    let known =
        mesh.my_id().as_deref() == Some(id) || mesh.social.friends.iter().any(|f| f.id == id);
    match space.and_then(|s| s.members.iter().find(|m| m.id == id)) {
        Some(m) if !known && !m.name.is_empty() => m.name.clone(),
        _ => mesh.name_of(id),
    }
}

/// The Space a chat ID's channel is in.
pub(crate) fn space_of<'a>(mesh: &'a mesh_ui::Mesh, chat: &str) -> Option<&'a mesh::Space> {
    let (s, _) = split(chat)?;
    mesh.spaces.iter().find(|x| x.id == s)
}

/// The rail's tiles, one a Space: its initials, and its mentions as a badge.
pub(crate) fn rail_tiles<'a>(
    session: &'a Session,
    mesh: &'a mesh_ui::Mesh,
    p: &theme::Palette,
    l: &theme::Layout,
) -> Vec<Element<'a, Message>> {
    (mesh.spaces.iter())
        .map(|space| {
            let active = session.active_mesh_space.as_deref() == Some(space.id.as_str());
            let face = ui::mono_bold(
                initials(&space.name),
                13.0,
                if active { p.text_bright() } else { p.text() },
            );
            let tile = rail_badge(
                rail_tile(
                    face.into(),
                    active,
                    Some(Message::MeshSpaceSelected(space.id.clone())),
                    p,
                    l,
                ),
                space.mentions(),
                p,
            );
            iced::widget::tooltip(
                tile,
                container(text(&space.name).size(12).color(p.text())).padding([4, 8]),
                iced::widget::tooltip::Position::Right,
            )
            .into()
        })
        .collect()
}

/// The channel pane for the Space picked on the rail: its name, a mute
/// switch, and its channels by category with unread and mention counts.
pub(crate) fn pane<'a>(
    session: &'a Session,
    mesh: &'a mesh_ui::Mesh,
    footer: Element<'a, Message>,
    p: &'a theme::Palette,
    l: &theme::Layout,
) -> Element<'a, Message> {
    let Some(space) = mesh
        .spaces
        .iter()
        .find(|s| Some(s.id.as_str()) == session.active_mesh_space.as_deref())
    else {
        let body = container(
            text("You're no longer in this Space.")
                .size(13)
                .color(p.text_muted()),
        )
        .padding(14)
        .height(Length::Fill);
        return column![body, ui::hline(p), footer].into();
    };
    let muted = mesh.muted_spaces.contains(&space.id);
    let mute = button(ui::mono(
        if muted { "unmute" } else { "mute" },
        11.0,
        p.text_muted(),
    ))
    .on_press(Message::MeshSpaceMute(space.id.clone(), !muted))
    .padding([2, 6])
    .style(button::text);
    let header = column![
        row![
            ui::sans(&space.name, 15.0, Weight::Semibold, p.text_bright()).width(Length::Fill),
            mute
        ]
        .align_y(iced::Alignment::Center),
        ui::mono(
            format!(
                "{} channels · {} members",
                space.channels.len(),
                space.members.len()
            ),
            11.0,
            p.text_muted()
        ),
    ]
    .spacing(4)
    .padding(iced::Padding {
        top: 14.0,
        right: 14.0,
        bottom: 10.0,
        left: 14.0,
    });
    let mut list = column![].spacing(2).padding([10, 8]);
    for (category, channels) in sections(space) {
        if let Some(name) = category {
            list = list.push(ui::section(name, p));
        }
        for channel in channels {
            let open = WebChat {
                network: Network::Mesh,
                id: chat_id(&space.id, &channel.id),
                name: channel.name.clone(),
                dm: false,
            };
            // Muted: no bold for plain unread, mentions still count.
            list = list.push(web_chat_row(
                session,
                open,
                None,
                channel.unread > 0 && !muted,
                channel.mentions,
                p,
                l,
            ));
        }
    }
    column![
        header,
        ui::hline(p),
        scrollable(list).height(Length::Fill),
        ui::hline(p),
        footer
    ]
    .into()
}

/// A Space's channels as the pane lists them: those in no category first,
/// then each category by position, every channel in the Space's order.
pub(crate) fn sections(space: &mesh::Space) -> Vec<(Option<&str>, Vec<&mesh::SpaceChannel>)> {
    let mut categories: Vec<&mesh::SpaceCategory> = space.categories.iter().collect();
    categories.sort_by_key(|c| c.position);
    let loose = space.channels.iter().filter(|c| {
        !categories
            .iter()
            .any(|k| c.category.as_deref() == Some(k.id.as_str()))
    });
    let mut out = vec![(None, loose.collect::<Vec<_>>())];
    for category in categories {
        let channels = space
            .channels
            .iter()
            .filter(|c| c.category.as_deref() == Some(category.id.as_str()))
            .collect();
        out.push((Some(category.name.as_str()), channels));
    }
    out.retain(|(name, channels)| name.is_some() || !channels.is_empty());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn space(json: &str) -> mesh::Space {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn a_channel_id_splits_and_a_dm_id_doesnt() {
        assert_eq!(split(&chat_id("aa", "bb")), Some(("aa", "bb")));
        assert_eq!(split("mesh1qqqq"), None);
        assert_eq!(split("00112233445566778899aabbccddeeff"), None);
    }

    /// `@name` in any case names that member; `@everyone` only as an admin.
    #[test]
    fn mentions_from_the_text() {
        let mut s = space(
            r#"{"id":"s","name":"club","role":"member","channels":[],
            "members":[{"id":"mesh1b","name":"Bea","role":"member"},{"id":"mesh1c","name":"cal","role":"admin"},{"id":"mesh1x","name":"","role":"member"}]}"#,
        );
        assert_eq!(mentions_in(&s, "hi @bea and @everyone"), ["mesh1b"]);
        assert!(mentions_in(&s, "hi bea, @ nobody").is_empty());
        s.role = Some("admin".to_string());
        assert_eq!(mentions_in(&s, "@CAL @everyone"), ["mesh1c", "@everyone"]);
    }

    /// Loose channels first, then categories by position (an empty one still
    /// shows, so admins can fill it); a channel whose category is gone is loose.
    #[test]
    fn channels_by_category() {
        let s = space(
            r#"{"id":"s","name":"club","role":"owner","members":[],
            "categories":[{"id":"k2","name":"Games","position":2},{"id":"k1","name":"Info","position":1},{"id":"k3","name":"Empty","position":3}],
            "channels":[
                {"id":"a","name":"rules","category":"k1","adminsPost":true},
                {"id":"b","name":"general","category":null,"adminsPost":false},
                {"id":"c","name":"minecraft","category":"k2","adminsPost":false},
                {"id":"d","name":"lost","category":"gone","adminsPost":false},
                {"id":"e","name":"news","category":"k1","adminsPost":true}]}"#,
        );
        let shown: Vec<(Option<&str>, Vec<&str>)> = sections(&s)
            .into_iter()
            .map(|(name, cs)| (name, cs.iter().map(|c| c.name.as_str()).collect()))
            .collect();
        assert_eq!(
            shown,
            [
                (None, vec!["general", "lost"]),
                (Some("Info"), vec!["rules", "news"]),
                (Some("Games"), vec!["minecraft"]),
                (Some("Empty"), vec![]),
            ]
        );
        assert!(!s.members_may_add && s.bans.is_empty() && !s.ended);
    }
}
