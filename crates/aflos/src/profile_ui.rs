//! Profiles you can rice (design/mesh/SPEC-profile.md): a friend's profile
//! drawn from its `profile.toml` and files (PR4 to PR10), our editor (PR16),
//! the Top 8's asks (PR11), and Space cards (PR12, PR13). Everything comes
//! from the bundle the helper opened; nothing is fetched from the internet,
//! and the helper has run every picture and the song through its capped
//! decoder first (PR8): what failed has no path and shows as a placeholder.

use super::*;
use iced::widget::{column, row, scrollable, text_editor, text_input};
use std::collections::HashSet;

/// The blocks a profile can have, in the editor's add list (PR5).
pub(crate) const BLOCKS: [&str; 6] = ["bio", "links", "gallery", "song", "top8", "status"];
/// The picture caps (PR7), so the editor shrinks a picture before the
/// helper refuses it; a song can't be shrunk here.
const AVATAR_CAP: u64 = 8_000_000;
const BANNER_CAP: u64 = 12_000_000;
const GALLERY_CAP: u64 = 4_000_000;
const SONG_CAP: u64 = 10_000_000;
const CARD_CAP: u64 = 1_000_000;
/// How a friend's avatar is keyed in `Session::avatars`, so the timeline,
/// the chat list and the DM header find it as they find web avatars.
pub(crate) fn avatar_key(id: &str) -> String {
    format!("mesh-avatar:{id}")
}
/// A friend's avatar (their profile's), else their initials.
pub(crate) fn face<'a>(state: &'a App, id: &str, name: &str, size: f32) -> Element<'a, Message> {
    let picture = match &state.screen {
        Screen::LoggedIn(session) => web_picture(session, Some(&avatar_key(id))),
        _ => None,
    };
    ui::avatar(
        name,
        picture,
        size,
        state.palette.accent(),
        &state.palette,
        None,
    )
}

/// A member's Space card avatar, keyed the same way.
pub(crate) fn card_key(space: &str, member: &str) -> String {
    format!("mesh-card:{space}:{member}")
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Page {
    /// A friend's profile, or ours, by ID.
    View(String),
    Edit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Slot {
    Avatar,
    Banner,
    Background,
    Gallery,
    Song,
}

#[derive(Default)]
pub(crate) struct Profiles {
    /// `profileGet` answers, by ID: friends' and ours.
    pub held: HashMap<String, mesh::Profile>,
    /// IDs already asked for, so an unknown one isn't asked again and again.
    asked: HashSet<String>,
    pub top8: mesh::Top8,
    pub page: Option<Page>,
    /// A link waiting for "Open it?" (PR6).
    confirm: Option<String>,
    pub editor: Editor,
    /// The last thing done or refused: (ok, what).
    notice: Option<(bool, String)>,
    /// The song playing: its path and the player (dropping it stops it).
    playing: Option<(String, sounds::Ringer)>,
    /// Card avatars asked for, and what each was when asked (size, mime).
    cards: HashMap<(String, String), (u64, String)>,
    /// Our card in a Space, while it's being edited.
    pub card: Option<CardForm>,
    /// Autoplay profile songs (config.toml, off by default: PR10).
    pub autoplay: bool,
}

impl Profiles {
    /// Empty, as after a lock, keeping the Autoplay setting.
    pub(crate) fn keeping(autoplay: bool) -> Self {
        Self {
            autoplay,
            ..Self::default()
        }
    }
}

#[derive(Default)]
pub(crate) struct Editor {
    /// Filled from our published profile once.
    loaded: bool,
    bio: text_editor::Content,
    status: String,
    links: Vec<(String, String)>,
    blocks: Vec<&'static str>,
    avatar: Option<String>,
    banner: Option<String>,
    background: Option<String>,
    gallery: Vec<String>,
    song: Option<String>,
    follow_rice: bool,
    busy: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CardForm {
    pub space: String,
    pub name: String,
    pub line: String,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) enum Msg {
    Open(String),
    Edit,
    Close,
    Link(String),
    LinkOpen,
    LinkCancel,
    Play(String),
    Stop,
    Ask(String),
    Answer(String, bool),
    Withdraw(String),
    /// Shows or stops showing a friend who said yes.
    Show(String, bool),
    /// Moves a shown friend earlier (`true`) or later.
    Move(String, bool),
    Bio(text_editor::Action),
    Status(String),
    LinkLabel(usize, String),
    LinkUrl(usize, String),
    AddLink,
    RemoveLink(usize),
    Block(&'static str, bool),
    BlockMove(usize, bool),
    Pick(Slot),
    Picked(Slot, Vec<PathBuf>),
    Clear(Slot),
    FollowRice(bool),
    Publish,
    Autoplay(bool),
    CardOpen(String),
    CardName(String),
    CardLine(String),
    CardPick,
    CardPicked(Option<PathBuf>),
    CardClear,
    CardSave,
    CardCancel,
    /// Hides (or shows again) a member's card: Space, member, `avatar`,
    /// `line`, `both` or `none`.
    Hide(String, String, &'static str),
}

fn to(msg: Msg) -> Message {
    Message::Profile(msg)
}

/// A profile as it's drawn: from a friend's bundle, or the editor's draft.
struct Model {
    palette: theme::Palette,
    font_mono: bool,
    blocks: Vec<Block>,
    avatar: Option<String>,
    banner: Option<String>,
    background: Option<String>,
    gallery: Vec<Option<String>>,
    song: Option<(Option<String>, Option<u32>)>,
    top8: Vec<(String, Option<String>)>,
}

enum Block {
    Bio(String),
    Links(Vec<(String, String)>),
    Gallery,
    Song,
    Top8,
    Status(String),
}

/// The palette a `[theme]` gives, over Aflos's defaults: palette keys at
/// its top or under `colors`, as Rice settings writes them (PR4). A value
/// that isn't a color is dropped, not an error.
fn palette_of(theme: Option<&toml::Value>) -> theme::Palette {
    let mut palette = theme::Palette::default();
    let Some(t) = theme.and_then(toml::Value::as_table) else {
        return palette;
    };
    let colors = t.get("colors").and_then(toml::Value::as_table).unwrap_or(t);
    for key in theme::Palette::KEYS {
        if let Some(hex) = colors
            .get(key)
            .and_then(toml::Value::as_str)
            .and_then(theme::valid_hex)
        {
            palette.set(key, Some(hex));
        }
    }
    palette
}

fn blocks_of(table: &toml::Table) -> Vec<Block> {
    let list = table
        .get("block")
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let text = |b: &toml::Value| {
        b.get("text")
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    (list.iter())
        .filter_map(|b| {
            Some(match b.get("kind")?.as_str()? {
                "bio" => Block::Bio(text(b)),
                "links" => {
                    let links = b
                        .get("links")
                        .and_then(toml::Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let pair = |l: &toml::Value| {
                        Some((
                            l.get("label")?.as_str()?.to_owned(),
                            l.get("url")?.as_str()?.to_owned(),
                        ))
                    };
                    Block::Links(links.iter().filter_map(pair).collect())
                }
                "gallery" => Block::Gallery,
                "song" => Block::Song,
                "top8" => Block::Top8,
                "status" => Block::Status(text(b)),
                // A kind a newer Aflos knows (PR5): skipped.
                _ => return None,
            })
        })
        .collect()
}

fn model_of(p: &mesh::Profile) -> Model {
    let table: toml::Table = p
        .toml
        .as_deref()
        .and_then(|t| t.parse().ok())
        .unwrap_or_default();
    let slot = |s: &str| p.assets.iter().find(|a| a.slot == s);
    let path = |s: &str| slot(s).and_then(|a| a.path.clone());
    Model {
        palette: palette_of(table.get("theme")),
        font_mono: table
            .get("font")
            .and_then(toml::Value::as_str)
            .is_some_and(|f| f.contains("Mono")),
        blocks: blocks_of(&table),
        avatar: path("avatar"),
        banner: path("banner"),
        background: path("background"),
        gallery: p
            .assets
            .iter()
            .filter(|a| a.slot == "gallery")
            .map(|a| a.path.clone())
            .collect(),
        song: slot("song").map(|a| (a.path.clone(), a.seconds)),
        top8: p
            .top8
            .iter()
            .map(|f| (f.name.clone(), f.avatar.clone()))
            .collect(),
    }
}

fn model_of_draft(e: &Editor, state: &App) -> Model {
    let bio = e.bio.text();
    let blocks = (e.blocks.iter())
        .map(|b| match *b {
            "bio" => Block::Bio(bio.trim_end().to_owned()),
            "links" => Block::Links(e.links.clone()),
            "gallery" => Block::Gallery,
            "song" => Block::Song,
            "top8" => Block::Top8,
            _ => Block::Status(e.status.clone()),
        })
        .collect();
    let shown = &state.mesh.profile.top8;
    let top8 = (shown.shown.iter())
        .map(|id| {
            let name = (shown.may_show.iter())
                .find(|m| &m.id == id)
                .map_or_else(|| state.mesh.name_of(id), |m| m.name.clone());
            let avatar = state
                .mesh
                .profile
                .held
                .get(id)
                .and_then(|p| p.assets.iter().find(|a| a.slot == "avatar"))
                .and_then(|a| a.path.clone());
            (name, avatar)
        })
        .collect();
    Model {
        palette: if e.follow_rice {
            state.palette.clone()
        } else {
            own_palette(state)
        },
        font_mono: false,
        blocks,
        avatar: e.avatar.clone(),
        banner: e.banner.clone(),
        background: e.background.clone(),
        gallery: e.gallery.iter().cloned().map(Some).collect(),
        song: e.song.clone().map(|s| (Some(s), None)),
        top8,
    }
}

/// Our published profile's palette, else the default.
fn own_palette(state: &App) -> theme::Palette {
    let me = state.mesh.my_id().unwrap_or_default();
    (state.mesh.profile.held.get(&me))
        .and_then(|p| p.toml.as_deref())
        .and_then(|t| t.parse::<toml::Table>().ok())
        .map_or_else(theme::Palette::default, |t| palette_of(t.get("theme")))
}

/// Reads a picture the helper vetted into the session's images, keyed by
/// its path, once.
fn load_picture(session: &mut Session, path: &str) {
    if session.images.contains_key(path) {
        return;
    }
    if let Ok(bytes) = std::fs::read(path) {
        show_image(&mut session.images, &mut session.animations, path, bytes);
    }
}

/// Every picture a profile shows, read in.
fn load_profile(session: &mut Session, p: &mesh::Profile) {
    let pictures = (p.assets.iter())
        .filter(|a| a.slot != "song")
        .filter_map(|a| a.path.as_deref())
        .chain(p.top8.iter().filter_map(|f| f.avatar.as_deref()));
    for path in pictures.map(str::to_owned).collect::<Vec<_>>() {
        load_picture(session, &path);
    }
    let avatar = (p.assets.iter())
        .find(|a| a.slot == "avatar")
        .and_then(|a| a.path.as_deref());
    match avatar.and_then(|path| std::fs::read(path).ok()) {
        Some(bytes) => drop(session.avatars.insert(
            avatar_key(&p.id),
            Some(iced::widget::image::Handle::from_bytes(bytes)),
        )),
        None => drop(session.avatars.remove(&avatar_key(&p.id))),
    }
}

/// Asks for the profiles of friends (and ours) not yet asked for.
fn ask_missing(state: &mut App) {
    let Some(node) = &state.mesh.node else { return };
    if state.mesh.profile.asked.is_empty() {
        node.top8("top8List", &[]);
    }
    let me = state.mesh.my_id();
    let ids = me
        .into_iter()
        .chain(state.mesh.social.friends.iter().map(|f| f.id.clone()));
    for id in ids.collect::<Vec<_>>() {
        if state.mesh.profile.asked.insert(id.clone()) {
            node.profile_get(&id);
        }
    }
}

/// Asks for the card avatars a Space list shows that aren't kept yet, or
/// changed (PR12).
pub(crate) fn spaces_changed(state: &mut App, spaces: &[mesh::Space]) {
    let Some(node) = &state.mesh.node else { return };
    for space in spaces {
        for m in &space.members {
            let key = (space.id.clone(), m.id.clone());
            match &m.avatar {
                Some(a)
                    if state.mesh.profile.cards.get(&key) != Some(&(a.size, a.mime.clone())) =>
                {
                    state
                        .mesh
                        .profile
                        .cards
                        .insert(key, (a.size, a.mime.clone()));
                    node.card_fetch(&space.id, &m.id);
                }
                Some(_) => {}
                None => {
                    if state.mesh.profile.cards.remove(&key).is_some()
                        && let Screen::LoggedIn(session) = &mut state.screen
                    {
                        session.avatars.remove(&card_key(&space.id, &m.id));
                    }
                }
            }
        }
    }
}

/// A `meshDmFile` that's a card avatar we asked for: kept, and `true`.
pub(crate) fn card_file(state: &mut App, file: &mesh::DmFile) -> bool {
    let key = (file.id.clone(), file.message.clone());
    if !state.mesh.profile.cards.contains_key(&key) {
        return false;
    }
    if let Screen::LoggedIn(session) = &mut state.screen
        && let Ok(bytes) = std::fs::read(&file.path)
    {
        // Spaces' files aren't vetted by the profile decoder: the same checks as a chat picture.
        if crate::mesh_dm::fits(&bytes, 1024) {
            session.avatars.insert(
                card_key(&file.id, &file.message),
                Some(iced::widget::image::Handle::from_bytes(bytes)),
            );
        }
    }
    true
}

pub(crate) fn event(state: &mut App, event: mesh::Event) -> Task<Message> {
    match event {
        mesh::Event::Social(_) => ask_missing(state),
        mesh::Event::Profile(p) => match p.func.as_deref() {
            Some("profileGet") => {
                if let Screen::LoggedIn(session) = &mut state.screen {
                    load_profile(session, &p);
                }
                let me = state.mesh.my_id();
                let pr = &mut state.mesh.profile;
                if me.as_deref() == Some(p.id.as_str()) && !pr.editor.loaded {
                    fill_editor(&mut pr.editor, &p);
                }
                let song = (p.assets.iter())
                    .find(|a| a.slot == "song")
                    .and_then(|a| a.path.clone());
                let viewing = pr.page == Some(Page::View(p.id.clone()));
                pr.held.insert(p.id.clone(), p);
                if viewing
                    && pr.autoplay
                    && pr.playing.is_none()
                    && let Some(song) = song
                {
                    return update(state, Msg::Play(song));
                }
            }
            Some("profileSet" | "top8") => {
                let pr = &mut state.mesh.profile;
                pr.editor.busy = false;
                if p.func.as_deref() == Some("profileSet") {
                    pr.notice = Some((
                        true,
                        format!(
                            "Saved: version {}. Your friends get it when they're next connected.",
                            p.version
                        ),
                    ));
                }
                if let Some(node) = &state.mesh.node {
                    node.profile_get(&p.id);
                }
            }
            // A friend's changed, or all its files are here now.
            _ if p.complete => {
                if let Some(node) = &state.mesh.node {
                    node.profile_get(&p.id);
                }
            }
            _ => {}
        },
        mesh::Event::Top8(t) => state.mesh.profile.top8 = t,
        mesh::Event::Error(e) => {
            let pr = &mut state.mesh.profile;
            pr.editor.busy = false;
            // A friend with no profile yet isn't news.
            if e.func != "profileGet" {
                pr.notice = Some((false, e.message));
            }
        }
        _ => {}
    }
    Task::none()
}

/// The editor from our published profile.
fn fill_editor(e: &mut Editor, p: &mesh::Profile) {
    let table: toml::Table = p
        .toml
        .as_deref()
        .and_then(|t| t.parse().ok())
        .unwrap_or_default();
    e.follow_rice = table
        .get("follow_rice")
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    e.blocks.clear();
    for b in blocks_of(&table) {
        e.blocks.push(match b {
            Block::Bio(t) => {
                e.bio = text_editor::Content::with_text(&t);
                "bio"
            }
            Block::Links(l) => {
                e.links = l;
                "links"
            }
            Block::Gallery => "gallery",
            Block::Song => "song",
            Block::Top8 => "top8",
            Block::Status(t) => {
                e.status = t;
                "status"
            }
        });
    }
    let path = |s: &str| {
        p.assets
            .iter()
            .find(|a| a.slot == s)
            .and_then(|a| a.path.clone())
    };
    (e.avatar, e.banner, e.background, e.song) = (
        path("avatar"),
        path("banner"),
        path("background"),
        path("song"),
    );
    e.gallery = p
        .assets
        .iter()
        .filter(|a| a.slot == "gallery")
        .filter_map(|a| a.path.clone())
        .collect();
    e.loaded = true;
}

fn ext(path: &str) -> String {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin")
        .to_ascii_lowercase()
}

/// Copies `from` to `to` (with `from`'s extension); a still picture over
/// `cap` is shrunk to a JPEG that fits. `Err` says what to do about it.
fn place(from: &str, to: &std::path::Path, cap: u64, what: &str) -> Result<(), String> {
    let size = std::fs::metadata(from)
        .map_err(|e| format!("Couldn't read the {what}: {e}"))?
        .len();
    let target = to.with_extension(ext(from));
    if size <= cap {
        return std::fs::copy(from, &target)
            .map(drop)
            .map_err(|e| format!("Couldn't copy the {what}: {e}"));
    }
    if matches!(ext(from).as_str(), "gif" | "apng") {
        return Err(format!(
            "The {what} is bigger than {} MB. Animations can't be shrunk here: pick a smaller one.",
            cap / 1_000_000
        ));
    }
    let picture = image::open(from)
        .map_err(|e| format!("The {what} is too big, and couldn't be read to shrink it: {e}"))?;
    let mut side = picture.width().max(picture.height());
    while side > 64 {
        side = side * 3 / 4;
        let mut bytes = Vec::new();
        let small = picture.thumbnail(side, side).to_rgb8();
        small
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Jpeg,
            )
            .map_err(|e| format!("Couldn't shrink the {what}: {e}"))?;
        if bytes.len() as u64 <= cap {
            return std::fs::write(to.with_extension("jpg"), bytes)
                .map_err(|e| format!("Couldn't save the {what}: {e}"));
        }
    }
    Err(format!(
        "The {what} couldn't be shrunk under {} MB.",
        cap / 1_000_000
    ))
}

/// The profile folder the editor publishes from: `profile.toml` and the
/// files named for their slots, as `profileSet` reads them.
fn write_draft(
    e: &Editor,
    palette: Option<&theme::Palette>,
    dir: PathBuf,
) -> Result<PathBuf, String> {
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)
        .map_err(|err| format!("Couldn't make the profile folder: {err}"))?;
    for (label, url) in &e.links {
        if !(url.starts_with("https://")
            || url.starts_with("http://")
            || url.starts_with("mailto:"))
        {
            return Err(format!(
                "The link “{label}” must start with https://, http:// or mailto:."
            ));
        }
    }
    let mut table = toml::Table::new();
    table.insert("follow_rice".into(), e.follow_rice.into());
    if let Some(p) = palette {
        let mut theme = toml::Table::new();
        for (k, v) in [
            ("background", &p.background),
            ("surface", &p.surface),
            ("surface_alt", &p.surface_alt),
            ("accent", &p.accent),
            ("text", &p.text),
        ] {
            theme.insert(k.into(), v.clone().into());
        }
        table.insert("theme".into(), theme.into());
    }
    let blocks: Vec<toml::Value> = (e.blocks.iter())
        .map(|kind| {
            let mut b = toml::Table::new();
            b.insert("kind".into(), (*kind).into());
            match *kind {
                "bio" => drop(b.insert("text".into(), e.bio.text().trim_end().into())),
                "status" => drop(b.insert("text".into(), e.status.clone().into())),
                "links" => {
                    let links: Vec<toml::Value> = (e.links.iter())
                        .map(|(label, url)| {
                            let mut l = toml::Table::new();
                            l.insert("label".into(), label.clone().into());
                            l.insert("url".into(), url.clone().into());
                            l.into()
                        })
                        .collect();
                    b.insert("links".into(), links.into());
                }
                _ => {}
            }
            b.into()
        })
        .collect();
    table.insert("block".into(), blocks.into());
    let toml =
        toml::to_string(&table).map_err(|err| format!("Couldn't write profile.toml: {err}"))?;
    std::fs::write(dir.join("profile.toml"), toml)
        .map_err(|err| format!("Couldn't write profile.toml: {err}"))?;
    for (file, name, cap, what) in [
        (&e.avatar, "avatar", AVATAR_CAP, "avatar"),
        (&e.banner, "banner", BANNER_CAP, "banner"),
        (&e.background, "background", BANNER_CAP, "background"),
    ] {
        if let Some(f) = file {
            place(f, &dir.join(name), cap, what)?;
        }
    }
    for (i, f) in e.gallery.iter().enumerate() {
        place(
            f,
            &dir.join(format!("gallery-{:02}", i + 1)),
            GALLERY_CAP,
            "gallery picture",
        )?;
    }
    if let Some(f) = &e.song {
        if std::fs::metadata(f).map_or(0, |m| m.len()) > SONG_CAP {
            return Err("The song is bigger than 10 MB: pick a shorter one, or one saved at a lower bitrate.".into());
        }
        place(f, &dir.join("song"), SONG_CAP, "song")?;
    }
    Ok(dir)
}

pub(crate) fn update(state: &mut App, msg: Msg) -> Task<Message> {
    let node = state.mesh.node.clone();
    let me = state.mesh.my_id().unwrap_or_default();
    let palette = state.palette.clone();
    let pr = &mut state.mesh.profile;
    let e = &mut pr.editor;
    match msg {
        Msg::Open(id) => {
            pr.notice = None;
            if let Some(node) = &node {
                node.profile_get(&id);
                node.top8("top8List", &[]);
            }
            pr.page = Some(Page::View(id));
        }
        Msg::Edit => {
            pr.notice = None;
            if let Some(node) = &node {
                node.profile_get(&me);
                node.top8("top8List", &[]);
            }
            if !e.loaded && e.blocks.is_empty() {
                e.blocks = vec!["bio", "links", "gallery", "status"];
            }
            pr.page = Some(Page::Edit);
        }
        Msg::Close => {
            (pr.page, pr.confirm, pr.playing) = (None, None, None);
        }
        Msg::Link(url) => pr.confirm = Some(url),
        Msg::LinkCancel => pr.confirm = None,
        Msg::LinkOpen => {
            if let Some(url) = pr.confirm.take() {
                return super::update(state, Message::OpenUrl(url));
            }
        }
        Msg::Play(path) => {
            let settings = state.sounds.clone();
            let device = speaker(state);
            state.mesh.profile.playing =
                Some((path.clone(), sounds::play_file(&path, &settings, device)));
        }
        Msg::Stop => pr.playing = None,
        Msg::Ask(id) => {
            if let Some(node) = &node {
                node.top8("top8Ask", &[&id]);
            }
        }
        Msg::Answer(id, yes) => {
            if let Some(node) = &node {
                node.top8("top8Answer", &[&id, if yes { "true" } else { "false" }]);
            }
        }
        Msg::Withdraw(id) => {
            if let Some(node) = &node {
                node.top8("top8Withdraw", &[&id]);
            }
        }
        Msg::Show(id, on) => top8_set(node.as_ref(), &pr.top8.shown, id, on, true),
        Msg::Move(id, on) => top8_set(node.as_ref(), &pr.top8.shown, id, on, false),
        Msg::Bio(action) => e.bio.perform(action),
        Msg::Status(s) => e.status = s.chars().take(128).collect(),
        Msg::LinkLabel(i, s) => {
            if let Some(l) = e.links.get_mut(i) {
                l.0 = s.chars().take(64).collect();
            }
        }
        Msg::LinkUrl(i, s) => {
            if let Some(l) = e.links.get_mut(i) {
                l.1 = s.chars().take(512).collect();
            }
        }
        Msg::AddLink if e.links.len() < 16 => e.links.push((String::new(), "https://".into())),
        Msg::AddLink => {}
        Msg::RemoveLink(i) if i < e.links.len() => drop(e.links.remove(i)),
        Msg::RemoveLink(_) => {}
        Msg::Block(kind, on) => {
            e.blocks.retain(|b| *b != kind);
            if on {
                e.blocks.push(kind);
            }
        }
        Msg::BlockMove(i, up) => {
            let j = if up { i.checked_sub(1) } else { Some(i + 1) };
            if let Some(j) = j.filter(|j| *j < e.blocks.len()) {
                e.blocks.swap(i, j);
            }
        }
        Msg::Pick(slot) => {
            let (title, exts): (&str, &[&str]) = match slot {
                Slot::Song => ("A profile song", &["ogg", "opus", "mp3", "flac"]),
                _ => ("A picture", &["png", "jpg", "jpeg", "webp", "gif", "apng"]),
            };
            let many = slot == Slot::Gallery;
            return Task::perform(
                async move {
                    let dialog = rfd::AsyncFileDialog::new()
                        .set_title(title)
                        .add_filter(title, exts);
                    let files = match many {
                        true => dialog.pick_files().await.unwrap_or_default(),
                        false => dialog.pick_file().await.into_iter().collect(),
                    };
                    files.into_iter().map(|f| f.path().to_path_buf()).collect()
                },
                move |paths| to(Msg::Picked(slot, paths)),
            );
        }
        Msg::Picked(slot, paths) => {
            let mut paths: Vec<String> = paths
                .into_iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            if paths.is_empty() {
                return Task::none();
            }
            match slot {
                Slot::Avatar => e.avatar = paths.pop(),
                Slot::Banner => e.banner = paths.pop(),
                Slot::Background => e.background = paths.pop(),
                Slot::Song => e.song = paths.pop(),
                Slot::Gallery => {
                    e.gallery.extend(paths);
                    e.gallery.truncate(12);
                }
            }
            // Our own pictures, picked here: shown in the preview as they are.
            let pictures: Vec<String> = e
                .avatar
                .iter()
                .chain(&e.banner)
                .chain(&e.background)
                .chain(&e.gallery)
                .cloned()
                .collect();
            if let Screen::LoggedIn(session) = &mut state.screen {
                for p in pictures {
                    load_picture(session, &p);
                }
            }
        }
        Msg::Clear(slot) => match slot {
            Slot::Avatar => e.avatar = None,
            Slot::Banner => e.banner = None,
            Slot::Background => e.background = None,
            Slot::Song => e.song = None,
            Slot::Gallery => e.gallery.clear(),
        },
        Msg::FollowRice(on) => e.follow_rice = on,
        Msg::Publish => {
            let theme = if e.follow_rice {
                Some(palette)
            } else {
                Some(own_palette(state))
            };
            let pr = &mut state.mesh.profile;
            match write_draft(&pr.editor, theme.as_ref(), data_dir().join("profile-draft")) {
                Ok(dir) => {
                    if let Some(node) = &node {
                        pr.editor.busy = true;
                        pr.notice = None;
                        node.profile_set(&dir.to_string_lossy());
                    }
                }
                Err(why) => pr.notice = Some((false, why)),
            }
        }
        Msg::Autoplay(on) => {
            pr.autoplay = on;
            if let Err(err) = theme::update_config(&config_path(), |c| c.autoplay_songs = on) {
                tracing::warn!("config: {err}");
            }
        }
        Msg::CardOpen(space) => {
            let mine = (state.mesh.spaces.iter())
                .find(|s| s.id == space)
                .and_then(|s| s.members.iter().find(|m| m.id == me));
            let (name, line) = mine.map_or_else(Default::default, |m| {
                (m.name.clone(), m.line.clone().unwrap_or_default())
            });
            state.mesh.profile.card = Some(CardForm {
                space,
                name,
                line,
                avatar: None,
            });
        }
        Msg::CardName(s) => {
            if let Some(c) = &mut pr.card {
                c.name = s.chars().take(64).collect();
            }
        }
        Msg::CardLine(s) => {
            if let Some(c) = &mut pr.card {
                c.line = s.chars().take(190).collect();
            }
        }
        Msg::CardPick => {
            return Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("A still picture for your card")
                        .add_filter("Picture", &["png", "jpg", "jpeg", "webp"])
                        .pick_file()
                        .await
                        .map(|f| f.path().to_path_buf())
                },
                |path| to(Msg::CardPicked(path)),
            );
        }
        Msg::CardPicked(path) => {
            if let (Some(c), Some(path)) = (&mut pr.card, path) {
                c.avatar = Some(path.to_string_lossy().into_owned());
            }
        }
        Msg::CardClear => {
            if let Some(c) = &mut pr.card {
                c.avatar = None;
            }
        }
        Msg::CardSave => {
            let Some(c) = pr.card.take() else {
                return Task::none();
            };
            let avatar = match &c.avatar {
                None => String::new(),
                Some(from) => {
                    let to = data_dir().join(format!("card-{}", c.space));
                    match place(from, &to, CARD_CAP, "card picture") {
                        Ok(()) => {
                            // `place` keeps the extension, or makes it a JPEG.
                            let kept = [to.with_extension(ext(from)), to.with_extension("jpg")];
                            kept.into_iter()
                                .find(|p| p.exists())
                                .map(|p| p.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        }
                        Err(why) => {
                            pr.notice = Some((false, why));
                            pr.card = Some(c);
                            return Task::none();
                        }
                    }
                }
            };
            if let Some(node) = &node {
                node.card_set(&c.space, c.name.trim(), c.line.trim(), &avatar);
            }
        }
        Msg::CardCancel => pr.card = None,
        Msg::Hide(space, member, what) => {
            if let Some(node) = &node {
                node.card_hide(&space, &member, what);
            }
        }
    }
    Task::none()
}

/// `top8Set` with `id` shown (or not), or moved earlier (`on`) or later.
fn top8_set(node: Option<&mesh::Node>, shown: &[String], id: String, on: bool, show: bool) {
    let Some(node) = node else { return };
    let mut shown = shown.to_vec();
    let at = shown.iter().position(|s| *s == id);
    match (show, at) {
        (true, None) if on => shown.push(id),
        (true, Some(i)) if !on => drop(shown.remove(i)),
        (false, Some(i)) if on && i > 0 => shown.swap(i, i - 1),
        (false, Some(i)) if !on && i + 1 < shown.len() => shown.swap(i, i + 1),
        _ => return,
    }
    let args: Vec<&str> = shown.iter().map(String::as_str).collect();
    node.top8("top8Set", &args);
}

fn label<'a>(t: impl text::IntoFragment<'a>, p: &theme::Palette) -> iced::widget::Text<'a> {
    ui::sans(t, 13.0, Weight::Normal, p.text())
}

/// A primary button's text, in the colour that reads on the accent.
fn on_accent<'a>(t: impl text::IntoFragment<'a>, p: &theme::Palette) -> iced::widget::Text<'a> {
    ui::sans(t, 13.0, Weight::Normal, p.accent_text())
}

fn flat<'a>(t: impl text::IntoFragment<'a>, msg: Msg, p: &theme::Palette) -> Element<'a, Message> {
    button(ui::sans(t, 12.0, Weight::Normal, p.text()))
        .on_press(to(msg))
        .padding([4, 10])
        .style(flat_style(p))
        .into()
}

/// `flat_button_style`, holding its colors rather than the palette, so a
/// profile's own palette can draw it.
fn flat_style(p: &theme::Palette) -> impl Fn(&IcedTheme, button::Status) -> button::Style + use<> {
    let (fg, hover) = (p.text(), p.surface_alt());
    move |_, status| button::Style {
        background: matches!(status, button::Status::Hovered).then(|| hover.into()),
        text_color: fg,
        border: iced::border::rounded(6),
        ..button::Style::default()
    }
}

/// A picture by its path, from the session's images, or a placeholder.
fn picture<'a>(
    session: &'a Session,
    path: Option<&str>,
    w: f32,
    h: f32,
    p: &theme::Palette,
) -> Element<'a, Message> {
    match path.and_then(|k| drawn_image(session, k)) {
        Some(img) => img
            .width(w)
            .height(h)
            .content_fit(iced::ContentFit::Cover)
            .into(),
        None => container(ui::mono("·", 12.0, p.text_muted()))
            .center_x(Length::Fixed(w))
            .center_y(Length::Fixed(h))
            .style({
                let fill = p.surface_alt();
                move |_| container::Style {
                    background: Some(fill.into()),
                    ..container::Style::default()
                }
            })
            .into(),
    }
}

/// A profile, drawn in its own colors (PR4 to PR10).
fn draw<'a>(m: Model, name: String, session: &'a Session, state: &'a App) -> Element<'a, Message> {
    let p = &m.palette;
    let pr = &state.mesh.profile;
    let body_text = |t: String| -> Element<'a, Message> {
        match m.font_mono {
            true => ui::mono(t, 13.0, p.text()).into(),
            false => ui::sans(t, 14.0, Weight::Normal, p.text()).into(),
        }
    };
    let mut col = column![].spacing(14);
    if let Some(banner) = m.banner.as_deref() {
        col = col.push(picture(session, Some(banner), 720.0, 160.0, p));
    }
    let face: Element<'a, Message> = match m.avatar.as_deref().and_then(|k| drawn_image(session, k))
    {
        Some(img) => img
            .width(96)
            .height(96)
            .border_radius(48.0)
            .content_fit(iced::ContentFit::Cover)
            .into(),
        None => ui::avatar(&name, None, 96.0, p.accent(), p, None),
    };
    col = col.push(
        row![face, ui::sans(name, 24.0, Weight::Bold, p.text_bright())]
            .spacing(16)
            .align_y(iced::Alignment::Center),
    );
    for block in m.blocks {
        let part: Element<'a, Message> = match block {
            Block::Bio(t) if !t.is_empty() => body_text(t),
            Block::Status(t) if !t.is_empty() => {
                ui::sans(format!("“{t}”"), 14.0, Weight::Normal, p.text_muted()).into()
            }
            Block::Links(links) if !links.is_empty() => {
                let mut list = column![].spacing(4);
                for (l, url) in links {
                    // The full URL shows; it opens only after a confirm (PR6).
                    let line = row![
                        ui::sans(l, 13.0, Weight::Semibold, p.accent()),
                        ui::mono(url.clone(), 11.0, p.text_muted())
                    ]
                    .spacing(10);
                    list = list.push(
                        button(line)
                            .on_press(to(Msg::Link(url)))
                            .padding([2, 4])
                            .style(flat_style(p)),
                    );
                }
                list.into()
            }
            Block::Gallery if !m.gallery.is_empty() => {
                let tiles = m
                    .gallery
                    .iter()
                    .map(|g| picture(session, g.as_deref(), 160.0, 160.0, p));
                iced::widget::Row::with_children(tiles)
                    .spacing(8)
                    .wrap()
                    .into()
            }
            Block::Song => match &m.song {
                Some((Some(path), seconds)) => {
                    let len = seconds
                        .map(|s| format!(" · {}:{:02}", s / 60, s % 60))
                        .unwrap_or_default();
                    let playing = pr.playing.as_ref().is_some_and(|(at, _)| at == path);
                    let (t, msg) = match playing {
                        true => (format!("■ Stop{len}"), Msg::Stop),
                        false => (
                            format!("▶ Play the profile song{len}"),
                            Msg::Play(path.clone()),
                        ),
                    };
                    button(ui::sans(t, 13.0, Weight::Semibold, p.accent_text()))
                        .on_press(to(msg))
                        .padding([6, 12])
                        .style(ui_primary(p))
                        .into()
                }
                Some((None, _)) => {
                    ui::mono("The song couldn't be played.", 12.0, p.text_muted()).into()
                }
                None => continue,
            },
            Block::Top8 if !m.top8.is_empty() => {
                let tiles = m.top8.iter().map(|(n, a)| -> Element<'a, Message> {
                    let face: Element<'a, Message> =
                        match a.as_deref().and_then(|k| drawn_image(session, k)) {
                            Some(img) => img
                                .width(72)
                                .height(72)
                                .content_fit(iced::ContentFit::Cover)
                                .into(),
                            None => ui::avatar(n, None, 72.0, p.surface_alt(), p, None),
                        };
                    column![face, ui::sans(n.clone(), 12.0, Weight::Normal, p.text())]
                        .spacing(4)
                        .width(88)
                        .align_x(iced::Alignment::Center)
                        .into()
                });
                column![
                    ui::section("Top 8", p),
                    iced::widget::Row::with_children(tiles).spacing(10).wrap()
                ]
                .into()
            }
            _ => continue,
        };
        col = col.push(part);
    }
    let fill = p.background();
    let page = container(col.padding(24).max_width(760))
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(fill.into()),
            ..container::Style::default()
        });
    match m
        .background
        .as_deref()
        .and_then(|k| drawn_image(session, k))
    {
        Some(img) => stack![
            img.width(Length::Fill)
                .height(Length::Fill)
                .content_fit(iced::ContentFit::Cover)
                .opacity(0.35_f32),
            page
        ]
        .into(),
        None => page.into(),
    }
}

fn ui_primary(p: &theme::Palette) -> impl Fn(&IcedTheme, button::Status) -> button::Style + use<> {
    let (fill, fg) = (p.accent(), p.accent_text());
    move |_, _| button::Style {
        background: Some(fill.into()),
        text_color: fg,
        border: iced::border::rounded(6),
        ..button::Style::default()
    }
}

/// The profile screen or the editor, in place of the chat pane, while one
/// is open.
pub(crate) fn page<'a>(session: &'a Session, state: &'a App) -> Option<Element<'a, Message>> {
    let pr = &state.mesh.profile;
    let p = &state.palette;
    let page = pr.page.as_ref()?;
    let me = state.mesh.my_id().unwrap_or_default();
    let mut bar = row![flat("← Back", Msg::Close, p)]
        .spacing(8)
        .align_y(iced::Alignment::Center);
    let content: Element<'a, Message> = match page {
        Page::View(id) => {
            if *id == me {
                bar = bar.push(flat("Edit profile", Msg::Edit, p));
            } else {
                bar = bar.push(top8_controls(id, state));
            }
            match pr.held.get(id) {
                Some(profile) => draw(model_of(profile), state.mesh.name_of(id), session, state),
                None => container(label(
                    "No profile from them yet. It comes when you're both online.",
                    p,
                ))
                .padding(24)
                .into(),
            }
        }
        Page::Edit => editor(session, state),
    };
    let mut col = column![bar].spacing(8);
    if let Some((ok, what)) = &pr.notice {
        col = col.push(ui::sans(
            what.clone(),
            13.0,
            Weight::Normal,
            if *ok { p.success() } else { p.danger() },
        ));
    }
    let shell: Element<'a, Message> = col
        .push(scrollable(content).height(Length::Fill))
        .padding(12)
        .into();
    Some(match &pr.confirm {
        Some(url) => stack![shell, link_dialog(url, p, &state.layout)].into(),
        None => shell,
    })
}

/// "Open this link?" with the whole URL (PR6).
fn link_dialog<'a>(url: &'a str, p: &theme::Palette, l: &theme::Layout) -> Element<'a, Message> {
    let body = column![
        ui::sans(
            "Open this link in your browser?",
            15.0,
            Weight::Semibold,
            p.text_bright()
        ),
        ui::mono(url, 12.0, p.text()),
        row![
            flat("Cancel", Msg::LinkCancel, p),
            ui::primary_button(on_accent("Open", p), Some(to(Msg::LinkOpen)), p, l)
        ]
        .spacing(8),
    ]
    .spacing(12)
    .padding(16);
    container(iced::widget::opaque(
        ui::pane(body, true, p, l, false).max_width(520),
    ))
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .style({
        let scrim = theme::alpha(p.background(), 0.6);
        move |_| container::Style {
            background: Some(scrim.into()),
            ..container::Style::default()
        }
    })
    .into()
}

/// On a friend's profile: the Top 8 ask and answer, as things stand.
fn top8_controls<'a>(id: &str, state: &'a App) -> Element<'a, Message> {
    let (t, p) = (&state.mesh.profile.top8, &state.palette);
    let id = id.to_owned();
    let has = |list: &[String]| list.contains(&id);
    let mut r = row![].spacing(8);
    if has(&t.asks) {
        r = r.push(label("They'd like to show you in their Top 8.", p));
        r = r.push(flat("Allow", Msg::Answer(id.clone(), true), p));
        r = r.push(flat("Not now", Msg::Answer(id.clone(), false), p));
    } else if has(&t.let_show) {
        r = r.push(flat(
            "Stop letting them show you",
            Msg::Withdraw(id.clone()),
            p,
        ));
    }
    if t.may_show.iter().any(|m| m.id == id) {
        r = r.push(ui::mono("in your Top 8 list", 11.0, p.text_muted()));
    } else if has(&t.asked) {
        r = r.push(ui::mono("asked for your Top 8", 11.0, p.text_muted()));
    } else {
        r = r.push(flat("Ask to show them in your Top 8", Msg::Ask(id), p));
    }
    r.into()
}

/// The editor (PR16): what's in the profile, block order, the Top 8, Follow
/// my rice, then a live preview of what friends see, and Publish.
fn editor<'a>(session: &'a Session, state: &'a App) -> Element<'a, Message> {
    let (pr, p, l) = (&state.mesh.profile, &state.palette, &state.layout);
    let e = &pr.editor;
    let file_row =
        |what: &'static str, slot: Slot, file: Option<&String>| -> Element<'a, Message> {
            let name = file.map_or("none".into(), |f| {
                std::path::Path::new(f)
                    .file_name()
                    .map_or(f.clone(), |n| n.to_string_lossy().into_owned())
            });
            row![
                label(what, p).width(110),
                ui::mono(name, 11.0, p.text_muted()).width(Length::Fill),
                flat("Choose…", Msg::Pick(slot), p),
                flat("Clear", Msg::Clear(slot), p)
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
        };
    let mut form = column![
        ui::section("Pictures and song", p),
        file_row("Avatar", Slot::Avatar, e.avatar.as_ref()),
        file_row("Banner", Slot::Banner, e.banner.as_ref()),
        file_row("Background", Slot::Background, e.background.as_ref()),
        row![
            label("Gallery", p).width(110),
            ui::mono(format!("{} of 12", e.gallery.len()), 11.0, p.text_muted())
                .width(Length::Fill),
            flat("Add…", Msg::Pick(Slot::Gallery), p),
            flat("Clear", Msg::Clear(Slot::Gallery), p)
        ]
        .spacing(8),
        file_row("Song", Slot::Song, e.song.as_ref()),
        ui::section("Bio", p),
        text_editor(&e.bio)
            .on_action(|a| to(Msg::Bio(a)))
            .height(120)
            .placeholder("A few lines about you. **bold**, *italic*, links."),
        ui::section("Status", p),
        text_input("What you're up to", &e.status)
            .on_input(|s| to(Msg::Status(s)))
            .padding(8),
        ui::section("Links", p),
    ]
    .spacing(8);
    for (i, (label_text, url)) in e.links.iter().enumerate() {
        form = form.push(
            row![
                text_input("Label", label_text)
                    .on_input(move |s| to(Msg::LinkLabel(i, s)))
                    .padding(6)
                    .width(160),
                text_input("https://…", url)
                    .on_input(move |s| to(Msg::LinkUrl(i, s)))
                    .padding(6),
                flat("Remove", Msg::RemoveLink(i), p),
            ]
            .spacing(6),
        );
    }
    form = form.push(flat("Add a link", Msg::AddLink, p));
    form = form.push(ui::section("Blocks, in order", p));
    for (i, b) in e.blocks.iter().enumerate() {
        form = form.push(
            row![
                label(*b, p).width(Length::Fill),
                flat("↑", Msg::BlockMove(i, true), p),
                flat("↓", Msg::BlockMove(i, false), p),
                flat("Remove", Msg::Block(b, false), p)
            ]
            .spacing(6),
        );
    }
    let missing: Vec<Element<'a, Message>> = BLOCKS
        .iter()
        .filter(|b| !e.blocks.contains(*b))
        .map(|b| flat(format!("+ {b}"), Msg::Block(b, true), p))
        .collect();
    form = form.push(iced::widget::Row::with_children(missing).spacing(6).wrap());
    form = form.push(ui::section("Top 8", p));
    form = form.push(top8_editor(state));
    let rice = button(
        text(format!(
            "{} Follow my rice: your profile takes Aflos's current colors when you publish",
            if e.follow_rice { "☑" } else { "☐" }
        ))
        .size(13),
    )
    .on_press(to(Msg::FollowRice(!e.follow_rice)))
    .padding([4, 0])
    .style(flat_button_style(p));
    form = form.push(rice);
    let publish = ui::primary_button(
        on_accent(if e.busy { "Saving…" } else { "Save" }, p),
        (!e.busy).then(|| to(Msg::Publish)),
        p,
        l,
    );
    form = form.push(publish);
    let name = state.mesh.name_of(&state.mesh.my_id().unwrap_or_default());
    column![
        form.padding(12).max_width(760),
        ui::section("Preview: what friends see", p),
        draw(model_of_draft(e, state), name, session, state),
    ]
    .spacing(12)
    .into()
}

/// Who said yes, to show or not and in what order; asks waiting on us.
fn top8_editor<'a>(state: &'a App) -> Element<'a, Message> {
    let (t, p) = (&state.mesh.profile.top8, &state.palette);
    let mut col = column![].spacing(6);
    if t.may_show.is_empty() {
        col = col.push(ui::mono(
            "Nobody yet. Ask a friend from their profile; they show once they say yes.",
            12.0,
            p.text_muted(),
        ));
    }
    for m in &t.may_show {
        let shown = t.shown.iter().position(|s| *s == m.id);
        let mut r = row![label(m.name.clone(), p).width(Length::Fill)].spacing(6);
        match shown {
            Some(i) => {
                r = r.push(ui::mono(format!("#{}", i + 1), 11.0, p.text_muted()));
                r = r
                    .push(flat("↑", Msg::Move(m.id.clone(), true), p))
                    .push(flat("↓", Msg::Move(m.id.clone(), false), p));
                r = r.push(flat("Don't show", Msg::Show(m.id.clone(), false), p));
            }
            None if t.shown.len() < 8 => r = r.push(flat("Show", Msg::Show(m.id.clone(), true), p)),
            None => r = r.push(ui::mono("8 shown already", 11.0, p.text_muted())),
        }
        col = col.push(r);
    }
    for id in &t.asks {
        col = col.push(
            row![
                label(
                    format!(
                        "{} asked to show you in their Top 8",
                        state.mesh.name_of(id)
                    ),
                    p
                )
                .width(Length::Fill),
                flat("Allow", Msg::Answer(id.clone(), true), p),
                flat("Not now", Msg::Answer(id.clone(), false), p)
            ]
            .spacing(6),
        );
    }
    col.into()
}

/// Our card in a Space (PR12), as a small form in its settings.
pub(crate) fn card_form<'a>(space: &'a mesh::Space, state: &'a App) -> Element<'a, Message> {
    let (pr, p, l) = (&state.mesh.profile, &state.palette, &state.layout);
    let Some(c) = pr.card.as_ref().filter(|c| c.space == space.id) else {
        return flat(
            "Edit your card in this Space",
            Msg::CardOpen(space.id.clone()),
            p,
        );
    };
    let avatar = c
        .avatar
        .as_deref()
        .map_or("none (initials)".to_owned(), |a| {
            std::path::Path::new(a)
                .file_name()
                .map_or(a.to_owned(), |n| n.to_string_lossy().into_owned())
        });
    column![
        ui::mono(
            "Members who aren't your friends see only this.",
            11.0,
            p.text_muted()
        ),
        text_input("Name", &c.name)
            .on_input(|s| to(Msg::CardName(s)))
            .padding(6),
        text_input("One line (up to 190 characters)", &c.line)
            .on_input(|s| to(Msg::CardLine(s)))
            .padding(6),
        row![
            label("Avatar", p),
            ui::mono(avatar, 11.0, p.text_muted()).width(Length::Fill),
            flat("Choose…", Msg::CardPick, p),
            flat("Clear", Msg::CardClear, p)
        ]
        .spacing(8),
        row![
            flat("Cancel", Msg::CardCancel, p),
            ui::primary_button(on_accent("Save card", p), Some(to(Msg::CardSave)), p, l)
        ]
        .spacing(8),
    ]
    .spacing(6)
    .into()
}

/// An owner's or admin's hide controls on a member's row (PR13).
pub(crate) fn hide_controls<'a>(
    space: &str,
    m: &'a mesh::SpaceMember,
    p: &theme::Palette,
) -> Element<'a, Message> {
    let (space, id) = (space.to_owned(), m.id.clone());
    let h = m.hidden;
    let mut r = row![].spacing(4);
    if m.line.is_some() || h.line {
        let what = match (h.avatar, !h.line) {
            (true, true) => "both",
            (false, true) => "line",
            (true, false) => "avatar",
            (false, false) => "none",
        };
        r = r.push(flat(
            if h.line { "Show line" } else { "Hide line" },
            Msg::Hide(space.clone(), id.clone(), what),
            p,
        ));
    }
    if m.avatar.is_some() || h.avatar {
        let what = match (!h.avatar, h.line) {
            (true, true) => "both",
            (true, false) => "avatar",
            (false, true) => "line",
            (false, false) => "none",
        };
        r = r.push(flat(
            if h.avatar {
                "Show avatar"
            } else {
                "Hide avatar"
            },
            Msg::Hide(space, id, what),
            p,
        ));
    }
    r.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PR4: palette keys at `[theme]`'s top or under `colors`; anything
    /// that isn't a color is dropped, and the rest stay Aflos's defaults.
    #[test]
    fn a_theme_takes_the_colors_it_can() {
        let t: toml::Table = "[theme]\naccent = \"#123456\"\ntext = \"not a color\"\n"
            .parse()
            .unwrap();
        let p = palette_of(t.get("theme"));
        assert_eq!(p.accent, "#123456");
        assert_eq!(p.text, theme::Palette::default().text);
        let t: toml::Table = "[theme.colors]\nbackground = \"#000000\"\n"
            .parse()
            .unwrap();
        assert_eq!(palette_of(t.get("theme")).background, "#000000");
        assert_eq!(palette_of(None).accent, theme::Palette::default().accent);
    }

    /// The draft writes a profile.toml that reads back as the same blocks,
    /// in order; a block kind a newer Aflos made is skipped (PR5).
    #[test]
    fn the_draft_reads_back() {
        let mut e = Editor {
            blocks: vec!["status", "bio", "links"],
            status: "mixing".into(),
            ..Editor::default()
        };
        e.bio = text_editor::Content::with_text("Frontman of **Nagbreek**.");
        e.links = vec![("Site".into(), "https://example.org".into())];
        let tmp = std::env::temp_dir().join(format!("aflos-draft-{}", std::process::id()));
        let dir = write_draft(&e, Some(&theme::Palette::default()), tmp.clone()).unwrap();
        let toml = std::fs::read_to_string(dir.join("profile.toml")).unwrap();
        let mut table: toml::Table = toml.parse().unwrap();
        let kinds: Vec<String> = blocks_of(&table)
            .iter()
            .map(|b| match b {
                Block::Status(t) => format!("status:{t}"),
                Block::Bio(t) => format!("bio:{t}"),
                Block::Links(l) => format!("links:{}", l[0].1),
                _ => "other".into(),
            })
            .collect();
        assert_eq!(
            kinds,
            [
                "status:mixing",
                "bio:Frontman of **Nagbreek**.",
                "links:https://example.org"
            ]
        );
        assert_eq!(
            table["theme"]["accent"].as_str(),
            Some(theme::Palette::default().accent.as_str())
        );
        let blocks = table.get_mut("block").unwrap().as_array_mut().unwrap();
        blocks.push(toml::Value::Table(
            [("kind".to_string(), toml::Value::from("marquee"))]
                .into_iter()
                .collect(),
        ));
        assert_eq!(blocks_of(&table).len(), 3, "an unknown kind is skipped");
        let _ = std::fs::remove_dir_all(tmp);
    }

    /// A link that isn't https, http or mailto is refused before publishing.
    #[test]
    fn a_link_must_be_web_or_mail() {
        let e = Editor {
            blocks: vec!["links"],
            links: vec![("x".into(), "javascript:alert(1)".into())],
            ..Editor::default()
        };
        let tmp = std::env::temp_dir().join(format!("aflos-link-{}", std::process::id()));
        assert!(
            write_draft(&e, None, tmp.clone())
                .unwrap_err()
                .contains("must start with")
        );
        let _ = std::fs::remove_dir_all(tmp);
    }

    /// A still picture over its cap is shrunk to a JPEG that fits.
    #[test]
    fn a_big_picture_is_shrunk() {
        let dir = std::env::temp_dir().join(format!("aflos-shrink-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let from = dir.join("big.png");
        // Noise doesn't compress, so the PNG is well over the cap.
        let noisy = image::RgbImage::from_fn(600, 600, |x, y| {
            image::Rgb([((x * 7) ^ (y * 13)) as u8, (x * y) as u8, (x + y * 3) as u8])
        });
        noisy.save(&from).unwrap();
        let cap = 50_000;
        assert!(std::fs::metadata(&from).unwrap().len() > cap);
        place(from.to_str().unwrap(), &dir.join("avatar"), cap, "avatar").unwrap();
        assert!(std::fs::metadata(dir.join("avatar.jpg")).unwrap().len() <= cap);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
