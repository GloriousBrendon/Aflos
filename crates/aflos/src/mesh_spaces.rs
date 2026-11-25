//! Aflos's own Spaces (SPEC-spaces *Mesh-Voice*, PLAN-spaces Task 12): a tile
//! each on the rail, their channels by category, and each channel in the
//! chat pane on the DM path (its lines are a DM page's). Message text and
//! IDs are never logged.

use super::*;
use iced::widget::{column, pick_list, row, text_input};

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
    let gear = button(ui::mono("settings", 11.0, p.text_muted()))
        .on_press(Message::MeshSpace(Msg::Settings(Some(space.id.clone()))))
        .padding([2, 6])
        .style(button::text);
    let header = column![
        row![
            ui::sans(&space.name, 15.0, Weight::Semibold, p.text_bright()).width(Length::Fill),
            mute,
            gear
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
            if channel.voice {
                list = list.push(crate::mesh_calls::voice_row(mesh, space, channel, p, l));
                continue;
            }
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
    if may(space).admin {
        list = list.push(quick_channel(&mesh.space_form, &space.id, p, l));
    }
    list = list.push(members(mesh, space, p));
    column![
        header,
        ui::hline(p),
        scrollable(list).height(Length::Fill),
        ui::hline(p),
        footer
    ]
    .into()
}

/// "+ New channel" at the end of a Space's channels (admins), typed in place.
fn quick_channel<'a>(
    f: &'a Form,
    space: &str,
    p: &'a theme::Palette,
    l: &theme::Layout,
) -> Element<'a, Message> {
    let Some((_, name)) = f.quick.as_ref().filter(|(s, _)| s == space) else {
        let line = row![
            ui::icon(ui::Icon::Plus, 14.0, p.text_muted()),
            ui::sans("New channel", 14.0, Weight::Normal, p.text_muted()),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);
        let open = Message::MeshSpace(Msg::Quick(Some((space.to_owned(), String::new()))));
        return ui::row_button(line, open, false, p, l).into();
    };
    let space = space.to_owned();
    let typed = text_input("channel name", name)
        .on_input(move |v| Message::MeshSpace(Msg::Quick(Some((space.clone(), v)))))
        .on_submit(Message::MeshSpace(Msg::QuickCreate))
        .padding(6)
        .width(Length::Fill);
    let ready = !name.trim().is_empty();
    row![
        typed,
        act("Add", p.accent(), ready.then_some(Msg::QuickCreate), p),
        act("Cancel", p.text_muted(), Some(Msg::Quick(None)), p),
    ]
    .spacing(4)
    .align_y(iced::Alignment::Center)
    .padding([4, 0])
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

/// What's typed and picked on a Space's settings, and on New Space.
#[derive(Default)]
pub(crate) struct Form {
    /// The Space whose settings show in the chat pane.
    pub settings: Option<String>,
    name: String,
    channel: String,
    channel_category: Option<String>,
    channel_admins: bool,
    channel_voice: bool,
    /// A channel or category being renamed: its ID, and the name typed.
    renaming: Option<(String, String)>,
    category: String,
    add: Option<String>,
    /// `end`, `leave` or `transfer:<id>`, waiting for a second click.
    confirm: Option<String>,
    /// New Space's name, while it's open (Home).
    new_space: Option<String>,
    /// "+ New channel" under a Space's channels: the Space, and the name typed.
    quick: Option<(String, String)>,
}

#[derive(Debug, Clone)]
pub(crate) enum Msg {
    Settings(Option<String>),
    Name(String),
    Rename,
    ChannelName(String),
    ChannelCategory(Pick),
    ChannelAdmins(bool),
    /// The new channel is a voice channel (SPEC-calls C8).
    ChannelVoice(bool),
    /// Rename a channel or category (its ID, its name now), type, save.
    RenameStart(String, String),
    RenameText(String),
    RenameDone,
    /// A channel (or category) one up (true) or down among its neighbours.
    Shift(String, bool),
    ChannelCreate,
    ChannelDelete(String),
    /// A channel, and whether only admins post.
    ChannelPost(String, bool),
    ChannelMove(String, Pick),
    CategoryName(String),
    CategoryCreate,
    CategoryDelete(String),
    MembersMayAdd(bool),
    AddPick(Pick),
    Add,
    /// A member, made admin (true) or member.
    Role(String, bool),
    Kick(String),
    Ban(String),
    Unban(String),
    /// Asks before `end`, `leave` or `transfer:<id>`, and does it on the
    /// second click; `None` cancels.
    Confirm(Option<String>),
    NewSpace(bool),
    NewSpaceName(String),
    NewSpaceCreate,
    /// Opens (with the Space and the name so far), types in, or closes "+ New channel".
    Quick(Option<(String, String)>),
    QuickCreate,
}

/// A pick list's entry: an ID (`None`: no category), and its label.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Pick {
    pub id: Option<String>,
    pub label: String,
}

impl std::fmt::Display for Pick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// What we may do in a Space, as mesh-dm's rules allow: admins change the
/// settings, the owner roles, transfer and the end; nothing once it's ended.
#[derive(Debug, PartialEq)]
struct May {
    admin: bool,
    owner: bool,
    add: bool,
}

fn may(space: &mesh::Space) -> May {
    let (owner, admin) = match space.role.as_deref() {
        Some("owner") => (true, true),
        Some("admin") => (false, true),
        _ => (false, false),
    };
    let live = !space.ended && space.role.is_some();
    May {
        admin: admin && live,
        owner: owner && live,
        add: (admin || space.members_may_add) && live,
    }
}

/// Whether we may kick or ban someone with `role`: the owner anyone else,
/// an admin members only.
fn over(may: &May, role: &str) -> bool {
    match role {
        "owner" => false,
        "admin" => may.owner,
        _ => may.admin,
    }
}

pub(crate) fn update(state: &mut App, msg: Msg) -> Task<Message> {
    let m = &mut state.mesh;
    let f = &mut m.space_form;
    let Some(node) = m.node.clone() else {
        return Task::none();
    };
    let space = f.settings.clone().unwrap_or_default();
    let s = space.as_str();
    match msg {
        Msg::Settings(open) => {
            let name = (open.as_ref())
                .and_then(|id| m.spaces.iter().find(|x| x.id == *id))
                .map(|x| x.name.clone());
            *f = Form {
                settings: open,
                name: name.unwrap_or_default(),
                new_space: f.new_space.take(),
                ..Form::default()
            };
        }
        Msg::Name(name) => f.name = name,
        Msg::Rename if !f.name.trim().is_empty() => {
            node.space_change("spaceRename", &[s, f.name.trim()])
        }
        Msg::ChannelName(name) => f.channel = name,
        Msg::ChannelCategory(pick) => f.channel_category = pick.id,
        Msg::ChannelAdmins(on) => f.channel_admins = on,
        Msg::ChannelVoice(on) => f.channel_voice = on,
        Msg::RenameStart(id, name) => f.renaming = Some((id, name)),
        Msg::RenameText(text) => {
            if let Some((_, name)) = &mut f.renaming {
                *name = text;
            }
        }
        Msg::RenameDone => {
            if let Some((id, name)) = f.renaming.take().filter(|(_, n)| !n.trim().is_empty()) {
                let category = (m.spaces.iter().find(|x| x.id == s))
                    .is_some_and(|x| x.categories.iter().any(|c| c.id == id));
                if category {
                    node.space_change("spaceCategoryRename", &[s, &id, name.trim()]);
                } else {
                    node.space_change("spaceRename", &[s, name.trim(), &id]);
                }
            }
        }
        Msg::Shift(id, up) => {
            let Some(space) = m.spaces.iter().find(|x| x.id == s) else {
                return Task::none();
            };
            if space.categories.iter().any(|c| c.id == id) {
                let mut cats: Vec<&mesh::SpaceCategory> = space.categories.iter().collect();
                cats.sort_by_key(|c| c.position);
                let ids: Vec<String> = cats.iter().map(|c| c.id.clone()).collect();
                for (at, cid) in reordered(&ids, &id, up).into_iter().flatten().enumerate() {
                    if cats
                        .iter()
                        .any(|c| c.id == cid && usize::from(c.position) != at)
                    {
                        node.space_change("spaceCategoryMove", &[s, &cid, &at.to_string()]);
                    }
                }
            } else if let Some(channels) = sections(space)
                .into_iter()
                .map(|(_, cs)| cs)
                .find(|cs| cs.iter().any(|c| c.id == id))
            {
                let ids: Vec<String> = channels.iter().map(|c| c.id.clone()).collect();
                let category = channels[0].category.clone().unwrap_or_default();
                // Positions came from when each was made, across the Space:
                // the whole section is numbered again.
                for (at, cid) in reordered(&ids, &id, up).into_iter().flatten().enumerate() {
                    node.space_change("spaceChannelMove", &[s, &cid, &category, &at.to_string()]);
                }
            }
        }
        Msg::ChannelCreate if !f.channel.trim().is_empty() => {
            let who = if f.channel_admins {
                "admins"
            } else {
                "everyone"
            };
            let category = f.channel_category.clone().unwrap_or_default();
            if f.channel_voice {
                node.space_change("spaceVoiceChannel", &[s, f.channel.trim(), &category]);
            } else {
                node.space_change("spaceChannel", &[s, f.channel.trim(), who, &category]);
            }
            f.channel.clear();
        }
        Msg::ChannelDelete(channel) => node.space_change("spaceChannelDelete", &[s, &channel]),
        Msg::ChannelPost(channel, admins) => {
            let who = if admins { "admins" } else { "everyone" };
            node.space_change("spaceChannelPost", &[s, &channel, who]);
        }
        Msg::ChannelMove(channel, pick) => {
            // Last in its new category.
            let at = (m.spaces.iter().find(|x| x.id == s)).map_or(0, |x| {
                x.channels.iter().filter(|c| c.category == pick.id).count()
            });
            let category = pick.id.as_deref().unwrap_or("");
            node.space_change(
                "spaceChannelMove",
                &[s, &channel, category, &at.to_string()],
            );
        }
        Msg::CategoryName(name) => f.category = name,
        Msg::CategoryCreate if !f.category.trim().is_empty() => {
            node.space_change("spaceCategory", &[s, f.category.trim()]);
            f.category.clear();
        }
        Msg::CategoryDelete(category) => node.space_change("spaceCategoryDelete", &[s, &category]),
        Msg::MembersMayAdd(on) => node.space_change(
            "spaceMembersMayAdd",
            &[s, if on { "true" } else { "false" }],
        ),
        Msg::AddPick(pick) => f.add = pick.id,
        Msg::Add => {
            if let Some(who) = f.add.take() {
                node.space_change("spaceAdd", &[s, &who]);
            }
        }
        Msg::Role(who, admin) => {
            let role = if admin { "admin" } else { "member" };
            node.space_change("spaceRole", &[s, &who, role]);
        }
        Msg::Kick(who) => node.space_change("spaceKick", &[s, &who]),
        Msg::Ban(who) => node.space_change("spaceBan", &[s, &who]),
        Msg::Unban(who) => node.space_change("spaceUnban", &[s, &who]),
        Msg::Confirm(Some(what)) if f.confirm.as_deref() == Some(what.as_str()) => {
            f.confirm = None;
            match what.split_once(':') {
                Some(("transfer", who)) => node.space_change("spaceTransfer", &[s, who]),
                _ if what == "leave" => {
                    node.space_change("spaceLeave", &[s]);
                    f.settings = None;
                }
                _ => node.space_change("spaceEnd", &[s]),
            }
        }
        Msg::Confirm(what) => f.confirm = what,
        Msg::NewSpace(open) => f.new_space = open.then(String::new),
        Msg::NewSpaceName(name) => f.new_space = Some(name),
        Msg::NewSpaceCreate => {
            // It shows on the rail once mesh-node has made it (`Spaces`).
            if let Some(name) = f.new_space.take().filter(|n| !n.trim().is_empty()) {
                node.space_change("spaceCreate", &[name.trim()]);
            }
        }
        Msg::Quick(open) => f.quick = open,
        Msg::QuickCreate => {
            if let Some((space, name)) = f.quick.take().filter(|(_, n)| !n.trim().is_empty()) {
                node.space_change("spaceChannel", &[&space, name.trim(), "everyone", ""]);
            }
        }
        // An empty name: nothing to do.
        Msg::Rename | Msg::ChannelCreate | Msg::CategoryCreate => {}
    }
    Task::none()
}

/// `ids` with `id` swapped with its neighbour above (`up`) or below; `None`
/// when it's already at that end, or not there.
fn reordered(ids: &[String], id: &str, up: bool) -> Option<Vec<String>> {
    let at = ids.iter().position(|x| x == id)?;
    let other = if up { at.checked_sub(1)? } else { at + 1 };
    if other >= ids.len() {
        return None;
    }
    let mut out = ids.to_vec();
    out.swap(at, other);
    Some(out)
}

/// A flat text button, off when `msg` is `None`.
fn act<'a>(
    label: impl Into<String>,
    color: iced::Color,
    msg: Option<Msg>,
    p: &'a theme::Palette,
) -> Element<'a, Message> {
    button(ui::sans(label.into(), 12.0, Weight::Normal, color))
        .on_press_maybe(msg.map(Message::MeshSpace))
        .padding([4, 10])
        .style(flat_button_style(p))
        .into()
}

fn input<'a>(
    placeholder: &str,
    value: &str,
    on: fn(String) -> Msg,
    submit: Msg,
) -> Element<'a, Message> {
    text_input(placeholder, value)
        .on_input(move |v| Message::MeshSpace(on(v)))
        .on_submit(Message::MeshSpace(submit))
        .padding(8)
        .width(Length::Fill)
        .into()
}

/// A row of a label and its buttons, centred.
fn line<'a>(label: String, p: &theme::Palette) -> iced::widget::Row<'a, Message> {
    row![ui::sans(label, 13.0, Weight::Normal, p.text()).width(Length::Fill)]
        .spacing(6)
        .align_y(iced::Alignment::Center)
}

/// "New Space" on Home, under the friends.
pub(crate) fn new_space(state: &App) -> Element<'_, Message> {
    let p = &state.palette;
    match &state.mesh.space_form.new_space {
        None => act("New Space", p.accent(), Some(Msg::NewSpace(true)), p),
        Some(name) => {
            let ready = !name.trim().is_empty();
            column![
                ui::mono("NEW SPACE: ITS NAME", 11.0, p.text_muted()),
                input("", name, Msg::NewSpaceName, Msg::NewSpaceCreate),
                row![
                    act(
                        "Create",
                        p.accent(),
                        ready.then_some(Msg::NewSpaceCreate),
                        p
                    ),
                    act("Cancel", p.text_bright(), Some(Msg::NewSpace(false)), p),
                ]
                .spacing(4),
            ]
            .spacing(6)
            .into()
        }
    }
}

/// The members under a Space's channels, owner and admins first: name,
/// role, and presence for friends only (nobody else's reaches us).
fn members<'a>(
    mesh: &mesh_ui::Mesh,
    space: &mesh::Space,
    p: &theme::Palette,
) -> Element<'a, Message> {
    let title = format!("members — {}", space.members.len());
    let mut list = column![ui::section(&title, p)].spacing(2);
    let rank = |r: &str| match r {
        "owner" => 0,
        "admin" => 1,
        _ => 2,
    };
    let mut people: Vec<(u8, String, &mesh::SpaceMember)> = (space.members.iter())
        .map(|m| (rank(&m.role), name_in(mesh, Some(space), &m.id), m))
        .collect();
    people.sort_by_key(|(r, name, _)| (*r, name.to_lowercase()));
    for (_, name, m) in people {
        let mut row = line(name, p);
        if m.role != "member" {
            row = row.push(ui::mono(m.role.clone(), 10.0, p.text_muted()));
        }
        if let Some(friend) = mesh.social.friends.iter().find(|f| f.id == m.id) {
            let dot = match friend.presence.as_str() {
                "online" => p.success(),
                "idle" | "dnd" => p.warning(),
                _ => p.text_muted(),
            };
            row = row.push(ui::swatch(dot, 8.0, 8.0, 4.0));
        }
        list = list.push(container(row).padding([3, 8]));
    }
    list.into()
}

/// A row's ↑, ↓ and Rename.
fn edits<'a>(
    row: iced::widget::Row<'a, Message>,
    id: &str,
    name: &str,
    p: &'a theme::Palette,
) -> iced::widget::Row<'a, Message> {
    row.push(act(
        "↑",
        p.text(),
        Some(Msg::Shift(id.to_string(), true)),
        p,
    ))
    .push(act(
        "↓",
        p.text(),
        Some(Msg::Shift(id.to_string(), false)),
        p,
    ))
    .push(act(
        "Rename",
        p.text(),
        Some(Msg::RenameStart(id.to_string(), name.to_string())),
        p,
    ))
}

/// The rename field, if `id` is the one being renamed.
fn renaming<'a>(f: &Form, id: &str, p: &'a theme::Palette) -> Option<Element<'a, Message>> {
    let (_, name) = f.renaming.as_ref().filter(|(r, _)| r == id)?;
    let save = act("Save", p.accent(), Some(Msg::RenameDone), p);
    Some(
        row![input("", name, Msg::RenameText, Msg::RenameDone), save]
            .spacing(6)
            .into(),
    )
}

/// A Space's categories as a pick list's entries, `none` first.
fn category_picks(space: &mesh::Space, none: &str) -> Vec<Pick> {
    let mut cats: Vec<&mesh::SpaceCategory> = space.categories.iter().collect();
    cats.sort_by_key(|c| c.position);
    let none = Pick {
        id: None,
        label: none.to_string(),
    };
    std::iter::once(none)
        .chain(cats.into_iter().map(|c| Pick {
            id: Some(c.id.clone()),
            label: c.name.clone(),
        }))
        .collect()
}

/// A Space's settings, in the chat pane while they're open for the Space
/// picked on the rail.
pub(crate) fn settings<'a>(session: &'a Session, state: &'a App) -> Option<Element<'a, Message>> {
    let (mesh, p) = (&state.mesh, &state.palette);
    let f = &mesh.space_form;
    let id =
        (f.settings.as_deref()).filter(|id| session.active_mesh_space.as_deref() == Some(*id))?;
    let space = mesh.spaces.iter().find(|s| s.id == id)?;
    let may = may(space);
    let title = ui::sans(
        format!("{} settings", space.name),
        15.0,
        Weight::Semibold,
        p.text_bright(),
    );
    let close = act("Close", p.text_bright(), Some(Msg::Settings(None)), p);
    let mut col = column![row![title.width(Length::Fill), close].align_y(iced::Alignment::Center)]
        .spacing(10);
    if let Some(status) = &session.status {
        col = col.push(ui::mono(status.clone(), 11.0, p.danger()));
    }
    if space.ended {
        let ended = "The owner ended this Space: nothing in it changes any more.";
        col = col.push(text(ended).size(13).color(p.text_muted()));
    }
    let add = |label: &'static str, msg: Msg| act(label, p.accent(), Some(msg), p);
    if may.admin {
        let mark = if space.members_may_add { "☑" } else { "☐" };
        col = col
            .push(ui::section("name", p))
            .push(
                row![
                    input("", &f.name, Msg::Name, Msg::Rename),
                    add("Rename", Msg::Rename)
                ]
                .spacing(6),
            )
            .push(act(
                format!("{mark} Members may add their friends"),
                p.text(),
                Some(Msg::MembersMayAdd(!space.members_may_add)),
                p,
            ))
            .push(ui::section("categories", p));
        let mut cats: Vec<&mesh::SpaceCategory> = space.categories.iter().collect();
        cats.sort_by_key(|c| c.position);
        for c in cats {
            let delete = act(
                "Delete",
                p.danger(),
                Some(Msg::CategoryDelete(c.id.clone())),
                p,
            );
            col = col.push(match renaming(f, &c.id, p) {
                Some(editing) => editing,
                None => edits(line(c.name.clone(), p), &c.id, &c.name, p)
                    .push(delete)
                    .into(),
            });
        }
        let new = input(
            "New category",
            &f.category,
            Msg::CategoryName,
            Msg::CategoryCreate,
        );
        col = col.push(row![new, add("Add", Msg::CategoryCreate)].spacing(6));

        col = col.push(ui::section("channels", p));
        let picks = category_picks(space, "No category");
        for c in sections(space).into_iter().flat_map(|(_, cs)| cs) {
            let at = picks.iter().find(|k| k.id == c.category).cloned();
            let channel = c.id.clone();
            let moved = move |k| Message::MeshSpace(Msg::ChannelMove(channel.clone(), k));
            let who = if c.admins_post {
                "admins post"
            } else {
                "everyone posts"
            };
            if let Some(editing) = renaming(f, &c.id, p) {
                col = col.push(editing);
                continue;
            }
            col = col.push(
                edits(line(format!("# {}", c.name), p), &c.id, &c.name, p)
                    .push(pick_list(picks.clone(), at, moved).padding(4))
                    .push(act(
                        who,
                        p.text(),
                        Some(Msg::ChannelPost(c.id.clone(), !c.admins_post)),
                        p,
                    ))
                    .push(act(
                        "Delete",
                        p.danger(),
                        Some(Msg::ChannelDelete(c.id.clone())),
                        p,
                    )),
            );
        }
        let at = picks.iter().find(|k| k.id == f.channel_category).cloned();
        let mark = if f.channel_admins { "☑" } else { "☐" };
        let voice = if f.channel_voice { "☑" } else { "☐" };
        col = col.push(
            row![
                input(
                    "New channel",
                    &f.channel,
                    Msg::ChannelName,
                    Msg::ChannelCreate
                ),
                pick_list(picks, at, |k| Message::MeshSpace(Msg::ChannelCategory(k))).padding(4),
                act(
                    format!("{mark} admins only"),
                    p.text(),
                    Some(Msg::ChannelAdmins(!f.channel_admins)),
                    p
                ),
                act(
                    format!("{voice} voice"),
                    p.text(),
                    Some(Msg::ChannelVoice(!f.channel_voice)),
                    p
                ),
                add("Add", Msg::ChannelCreate),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        );
    }

    col = col.push(ui::section("members", p));
    if may.add {
        let banned = |id: &str| space.bans.iter().any(|b| b == id);
        let friends: Vec<Pick> = (mesh.social.friends.iter())
            .filter(|fr| !space.members.iter().any(|m| m.id == fr.id) && !banned(&fr.id))
            .map(|fr| Pick {
                id: Some(fr.id.clone()),
                label: mesh.name_of(&fr.id),
            })
            .collect();
        if !friends.is_empty() {
            let at = friends.iter().find(|k| k.id == f.add).cloned();
            let pick = pick_list(friends, at, |k| Message::MeshSpace(Msg::AddPick(k)))
                .placeholder("Add a friend…")
                .padding(4)
                .width(Length::Fill);
            let go = act(
                "Add member",
                p.accent(),
                f.add.as_ref().map(|_| Msg::Add),
                p,
            );
            col = col.push(row![pick, go].spacing(6).align_y(iced::Alignment::Center));
        }
    }
    let me = mesh.my_id();
    for m in &space.members {
        let mut row = line(name_in(mesh, Some(space), &m.id), p).push(ui::mono(
            m.role.clone(),
            11.0,
            p.text_muted(),
        ));
        if me.as_deref() != Some(m.id.as_str()) {
            if may.owner && m.role != "owner" {
                let admin = m.role == "admin";
                let role = if admin { "Make member" } else { "Make admin" };
                row = row.push(act(
                    role,
                    p.text(),
                    Some(Msg::Role(m.id.clone(), !admin)),
                    p,
                ));
                let what = format!("transfer:{}", m.id);
                let (label, color) = if f.confirm.as_deref() == Some(what.as_str()) {
                    ("Sure? Make owner", p.danger())
                } else {
                    ("Make owner", p.text())
                };
                row = row.push(act(label, color, Some(Msg::Confirm(Some(what))), p));
            }
            if over(&may, &m.role) {
                row = row
                    .push(act("Kick", p.text(), Some(Msg::Kick(m.id.clone())), p))
                    .push(act("Ban", p.danger(), Some(Msg::Ban(m.id.clone())), p));
            }
        }
        col = col.push(row);
    }
    if may.admin && !space.bans.is_empty() {
        col = col.push(ui::section("banned", p));
        for b in &space.bans {
            let unban = act("Unban", p.text(), Some(Msg::Unban(b.clone())), p);
            col = col.push(line(name_in(mesh, Some(space), b), p).push(unban));
        }
    }

    // The owner can't leave, only hand it on or end it.
    let (what, label, sure) = if may.owner {
        ("end", "Delete this Space", "Sure? It ends for everyone")
    } else {
        ("leave", "Leave this Space", "Sure? Leave")
    };
    if may.owner || (space.role.is_some() && space.role.as_deref() != Some("owner")) {
        let asked = f.confirm.as_deref() == Some(what);
        let mut end = row![act(
            if asked { sure } else { label },
            p.danger(),
            Some(Msg::Confirm(Some(what.to_string()))),
            p
        )];
        if asked {
            end = end.push(act("Cancel", p.text_bright(), Some(Msg::Confirm(None)), p));
        }
        col = col.push(ui::hline(p)).push(end);
    }
    let body = col.padding(iced::Padding {
        right: 12.0,
        ..iced::Padding::ZERO
    });
    Some(
        container(scrollable(body).height(Length::Fill))
            .padding(16)
            .into(),
    )
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

    /// The buttons follow mesh-dm's rules: admins set things up, the owner
    /// also roles and the end; members add only when allowed; an ended Space
    /// or one on its way allows nothing. Kick and ban go only downwards.
    #[test]
    fn what_each_role_may_do() {
        let as_role = |role: Option<&str>, may_add: bool, ended: bool| {
            let mut s = space(r#"{"id":"s","name":"club","role":null,"channels":[],"members":[]}"#);
            (s.role, s.members_may_add, s.ended) = (role.map(str::to_string), may_add, ended);
            may(&s)
        };
        let m = |admin, owner, add| May { admin, owner, add };
        assert_eq!(as_role(Some("owner"), false, false), m(true, true, true));
        assert_eq!(as_role(Some("admin"), false, false), m(true, false, true));
        assert_eq!(
            as_role(Some("member"), false, false),
            m(false, false, false)
        );
        assert_eq!(as_role(Some("member"), true, false), m(false, false, true));
        assert_eq!(as_role(Some("owner"), true, true), m(false, false, false));
        assert_eq!(as_role(None, true, false), m(false, false, false));
        let (owner, admin) = (m(true, true, true), m(true, false, true));
        assert!(over(&owner, "admin") && over(&owner, "member") && !over(&owner, "owner"));
        assert!(!over(&admin, "admin") && over(&admin, "member"));
    }

    /// Up and down swap with the neighbour, and stop at the ends.
    #[test]
    fn reorder_by_one() {
        let ids: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
        assert_eq!(reordered(&ids, "b", true).unwrap(), ["b", "a", "c"]);
        assert_eq!(reordered(&ids, "b", false).unwrap(), ["a", "c", "b"]);
        assert!(reordered(&ids, "a", true).is_none() && reordered(&ids, "c", false).is_none());
        assert!(reordered(&ids, "x", true).is_none());
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
