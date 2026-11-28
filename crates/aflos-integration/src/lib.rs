//! The protocol between Aflos and an integration (SPEC-integrations).
//!
//! An integration is a program Aflos starts. They talk over its stdin and
//! stdout, one JSON object per line:
//!
//! - Aflos sends [`Command`]s: `{"cmd": "send", "id": 7, "chat": "…", "text": "…"}`.
//!   Every command carries an `id`.
//! - The integration sends [`Event`]s: `{"event": "messages", …}`. It answers
//!   a command with [`Event::Reply`] carrying the same `id`, and reports
//!   anything else (new messages, a call ringing) whenever it happens.
//!
//! The first line is always Aflos's [`Command::Hello`]; the integration
//! answers with [`Event::Hello`], saying which protocol version it speaks
//! and what it [`Supports`]. Aflos shows only what's supported.
//!
//! Big things (pictures, files, voice notes) don't go in lines: the
//! integration writes them into the files folder Aflos gives it in `hello`
//! and names the file ([`Event::Fetched`]).
//!
//! Lines Aflos doesn't understand (an unknown event, a malformed line) are
//! ignored, and so are unknown fields, so either side can be newer.
//! Integrations should do the same.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The protocol version this crate speaks.
pub const PROTOCOL: u32 = 1;
/// The oldest version Aflos still speaks.
pub const OLDEST: u32 = 1;

/// Whether an integration speaking `theirs` can run with this Aflos.
pub fn check_version(theirs: u32) -> Result<(), VersionError> {
    if theirs > PROTOCOL {
        Err(VersionError::Newer {
            theirs,
            ours: PROTOCOL,
        })
    } else if theirs < OLDEST {
        Err(VersionError::Older {
            theirs,
            oldest: OLDEST,
        })
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionError {
    /// It needs a newer Aflos.
    Newer { theirs: u32, ours: u32 },
    /// It's too old for this Aflos.
    Older { theirs: u32, oldest: u32 },
}

impl std::fmt::Display for VersionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Newer { theirs, ours } => write!(
                f,
                "it needs a newer Aflos (it speaks protocol {theirs}, this Aflos {ours})"
            ),
            Self::Older { theirs, oldest } => write!(
                f,
                "it's too old for this Aflos (it speaks protocol {theirs}, the oldest this Aflos speaks is {oldest})"
            ),
        }
    }
}

impl std::error::Error for VersionError {}

/// One line from Aflos to the integration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Command {
    /// Echoed back in [`Event::Reply`].
    pub id: u64,
    #[serde(flatten)]
    pub body: Cmd,
}

/// What Aflos asks for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Cmd {
    // Lifecycle.
    /// Always first. Answered with [`Event::Hello`].
    Hello(Hello),
    /// Answered with an `ok` reply; one that doesn't answer in 10 s is
    /// treated as hung and restarted.
    Ping,
    /// Stop: finish writing, then exit.
    Quit,

    // Sign-in.
    /// Start signing in. `values` are a form's fields by name (empty for
    /// QR and web sign-in).
    SignIn {
        #[serde(default)]
        values: BTreeMap<String, String>,
    },
    /// The code a [`SignIn::Code`] step asked for.
    SignInCode {
        code: String,
    },
    SignOut,

    // Chats.
    /// Show this chat: its messages come as [`Event::Messages`] now and
    /// whenever they change, until another is opened. `None` closes it.
    Open {
        chat: Option<String>,
    },
    Send {
        chat: String,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reply_to: Option<String>,
    },
    /// Send a file Aflos wrote into the files folder.
    SendFile {
        chat: String,
        path: PathBuf,
        #[serde(default)]
        caption: String,
    },
    /// React with `emoji`; an empty one takes our reaction back.
    React {
        message: String,
        emoji: String,
    },
    Edit {
        message: String,
        text: String,
    },
    /// Delete a message: for everyone, or only here.
    Delete {
        message: String,
        everyone: bool,
    },
    Forward {
        message: String,
        to: String,
    },
    Typing {
        chat: String,
        on: bool,
    },
    /// Write `key` (a [`Media::key`], an avatar) into the files folder,
    /// as `form`. Answered with [`Event::Fetched`].
    Fetch {
        key: String,
        #[serde(default)]
        form: FetchForm,
    },
    Search {
        query: String,
    },
    /// Start a chat with a contact or a number from [`Event::Contacts`].
    StartChat {
        who: String,
    },
    Archive {
        chat: String,
        archived: bool,
    },
    DeleteChat {
        chat: String,
    },
    /// Join by an invite link.
    JoinByLink {
        link: String,
    },
    /// A section's member list (a server's people), as [`Event::Members`].
    Members {
        section: String,
    },

    // Calls and voice rooms.
    CallStart {
        chat: String,
        video: bool,
    },
    CallAccept {
        video: bool,
    },
    CallDecline,
    CallEnd,
    /// Join a voice room (or a call that's ringing there).
    Join {
        room: String,
    },
    Leave,
    Mute {
        on: bool,
    },
    Deafen {
        on: bool,
    },
    Camera {
        on: bool,
    },
    /// Share a screen or window (a [`Source::id`]), or stop.
    Screen {
        on: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
    },
    /// List cameras and screens, as [`Event::Sources`].
    Sources,
    /// Watch someone's stream in our voice room, or stop.
    Watch {
        stream: String,
        on: bool,
    },
    /// The microphone and speakers the person chose in Aflos.
    AudioDevices {
        input: Option<String>,
        output: Option<String>,
    },

    // Settings.
    /// A setting from its [`Supports::settings`] page changed.
    Setting {
        key: String,
        value: serde_json::Value,
    },
}

/// What [`Cmd::Hello`] tells the integration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hello {
    /// The protocol version Aflos speaks ([`PROTOCOL`]).
    pub protocol: u32,
    /// Aflos's version, for logs.
    pub aflos: String,
    /// Its own folder, kept until it's removed.
    pub data_dir: PathBuf,
    /// Where it writes files for Aflos, and Aflos writes files for it.
    pub files_dir: PathBuf,
    /// The permissions the person granted.
    #[serde(default)]
    pub granted: Vec<Permission>,
    /// Its settings' stored values, by key.
    #[serde(default)]
    pub settings: BTreeMap<String, serde_json::Value>,
}

/// A permission an integration asks for in its manifest (spec T3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Microphone,
    Camera,
    Screen,
    Notifications,
    Files,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FetchForm {
    /// The file as it is.
    #[default]
    Original,
    /// A voice note or audio as raw 48 kHz mono `i16` little-endian samples.
    Pcm,
    /// A GIF as numbered JPEG frames in a folder, with its frame delay.
    Frames,
}

/// One line from the integration to Aflos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// The answer to [`Cmd::Hello`].
    Hello(HelloBack),
    /// The answer to a command.
    Reply {
        id: u64,
        ok: bool,
        /// Why not, in words a person can read.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// Connected, signed out, and so on: the rail tile's dot.
    Status {
        state: State,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// The next sign-in step.
    SignIn(SignIn),
    /// Every chat, in sections; sent whole whenever something changes.
    Chats(Chats),
    /// The open chat's latest messages, oldest first.
    Messages {
        chat: String,
        messages: Vec<Message>,
    },
    /// A new message worth a notification.
    Notify {
        chat: String,
        title: String,
        body: String,
    },
    Typing {
        chat: String,
        who: String,
        on: bool,
    },
    Presence {
        who: String,
        online: bool,
        /// Last online, ms since the epoch, if they share it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_seen: Option<u64>,
    },
    /// The answer to [`Cmd::Fetch`]: the file in the files folder.
    Fetched {
        key: String,
        #[serde(default)]
        form: FetchForm,
        /// Where it is, or why not.
        #[serde(flatten)]
        result: Fetched,
    },
    Contacts {
        query: String,
        contacts: Vec<Contact>,
    },
    Members {
        section: String,
        rows: Vec<MemberRow>,
    },
    /// The call going on, or `None`.
    Call {
        call: Option<Call>,
    },
    /// Where we are in voice rooms, and what's ringing.
    Voice(Voice),
    /// A frame of someone's video (or our own, `local`), as JPEG, base64.
    Video {
        track: String,
        #[serde(default)]
        local: bool,
        jpeg: String,
    },
    VideoEnded {
        track: String,
    },
    Sources {
        cameras: Vec<Source>,
        screens: Vec<Source>,
    },
    /// Show this chat: one it just started or joined for the person.
    Show { chat: String },
}

/// What [`Event::Hello`] tells Aflos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HelloBack {
    /// The protocol version it speaks.
    pub protocol: u32,
    pub name: String,
    pub tile: Tile,
    #[serde(default)]
    pub supports: Supports,
}

/// Its rail tile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    /// An SVG icon, drawn in the tile's colour.
    pub icon_svg: String,
    /// Two or three letters: `wa`, `dc`.
    pub tag: String,
    /// `#rrggbb`.
    pub color: String,
}

/// The building blocks it fills in. Anything not listed is hidden.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Supports {
    /// How it signs in (empty: no sign-in).
    #[serde(default)]
    pub sign_in: Vec<SignInKind>,
    /// What can be done to messages and chats.
    #[serde(default)]
    pub actions: Vec<Action>,
    #[serde(default)]
    pub calls: Option<Calls>,
    /// Its settings page.
    #[serde(default)]
    pub settings: Vec<Setting>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignInKind {
    Qr,
    Code,
    Form,
    Web,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Send,
    Reply,
    React,
    Edit,
    Delete,
    DeleteForEveryone,
    Forward,
    SendFiles,
    Typing,
    Search,
    StartChat,
    Archive,
    DeleteChat,
    JoinByLink,
    Members,
}

/// What its calls can do.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Calls {
    #[serde(default)]
    pub video: bool,
    #[serde(default)]
    pub screen: bool,
    /// Rooms people join and leave (Discord's voice channels).
    #[serde(default)]
    pub voice_rooms: bool,
    /// Others' screen shares to watch in a voice room.
    #[serde(default)]
    pub streams: bool,
}

/// One field on its settings page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Setting {
    pub key: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(flatten)]
    pub kind: SettingKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SettingKind {
    Switch {
        default: bool,
    },
    /// One of `choices` (value, label).
    Choice {
        choices: Vec<(String, String)>,
        default: String,
    },
    Text {
        default: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    SignedOut,
    Connecting,
    Online,
    /// Signed in but can't reach the service.
    Offline,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum SignIn {
    /// A QR code to scan; a new one replaces it.
    Qr {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        png: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        svg: Option<String>,
    },
    /// Ask for a code (2FA, SMS), answered with [`Cmd::SignInCode`].
    Code {
        prompt: String,
    },
    /// A form, answered with [`Cmd::SignIn`]'s `values`.
    Form {
        fields: Vec<Field>,
    },
    /// Open this page in the browser.
    Web {
        url: String,
    },
    Failed {
        error: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub label: String,
    #[serde(default)]
    pub secret: bool,
}

/// Every chat it has, in sections (Direct, Groups, each server).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Chats {
    /// Our own id there.
    #[serde(default)]
    pub me: String,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub id: String,
    pub name: String,
    /// Its chats show in Aflos's Home, with everyone's direct messages;
    /// other sections get their own rail tile (a server, "Groups").
    #[serde(default)]
    pub home: bool,
    /// A [`Cmd::Fetch`] key for its icon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default)]
    pub chats: Vec<Chat>,
    #[serde(default)]
    pub rooms: Vec<Room>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Chat {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: ChatKind,
    /// The category it's under, in a server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// A [`Cmd::Fetch`] key for its picture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    #[serde(default)]
    pub unread: u64,
    #[serde(default)]
    pub mentions: u64,
    /// Last activity, ms since the epoch.
    #[serde(default)]
    pub last_ms: u64,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub archived: bool,
    /// Shown but can't be opened.
    #[serde(default)]
    pub locked: bool,
    /// Who's typing, by name.
    #[serde(default)]
    pub typing: Vec<String>,
    /// A direct chat's person: `online`, `idle`, `dnd` or `offline`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatKind {
    #[default]
    Direct,
    Group,
    Channel,
}

/// A voice room people join and leave.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Room {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default)]
    pub members: Vec<Member>,
    #[serde(default)]
    pub locked: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Member {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    #[serde(default)]
    pub speaking: bool,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub deafened: bool,
}

/// A member list row: a group header, or a person.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "row", rename_all = "snake_case")]
pub enum MemberRow {
    Group {
        name: String,
        count: u64,
    },
    Person {
        id: String,
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        avatar: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub author_id: String,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    /// The text (with Aflos's formatting: `**bold**`, `*italic*`, `` `code` ``,
    /// links), or for a system message, what happened.
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub system: bool,
    /// Ours.
    #[serde(default)]
    pub mine: bool,
    /// Sent, ms since the epoch.
    pub at_ms: u64,
    #[serde(default)]
    pub media: Vec<Media>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<Quote>,
    #[serde(default)]
    pub forwarded: bool,
    #[serde(default)]
    pub edited: bool,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
    /// Ours only: 1 sent, 2 delivered, 3 read (0 unknown).
    #[serde(default)]
    pub ticks: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Media {
    /// Its [`Cmd::Fetch`] key.
    pub key: String,
    pub kind: MediaKind,
    /// Suggested filename.
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u64>,
    /// Seconds, for audio and video.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    /// A voice note's loudness over time, 0 to 100 each.
    #[serde(default)]
    pub waveform: Vec<u8>,
    /// A small preview: a [`Cmd::Fetch`] key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Image,
    Sticker,
    Gif,
    Video,
    Voice,
    Audio,
    #[default]
    File,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub author: String,
    /// A line of its text, or its kind ("Photo").
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Reaction {
    pub emoji: String,
    pub count: u64,
    /// One of them is ours.
    #[serde(default)]
    pub mine: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Fetched {
    Done {
        path: PathBuf,
        /// A GIF's frame delay ([`FetchForm::Frames`]).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        delay_ms: Option<u64>,
    },
    Failed {
        error: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Contact {
    pub id: String,
    pub name: String,
    /// A phone number or handle, as shown.
    #[serde(default)]
    pub detail: String,
}

/// A call (one at a time).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Call {
    pub id: String,
    /// The chat it's in.
    pub chat: String,
    pub name: String,
    pub phase: Phase,
    #[serde(default)]
    pub video: bool,
    #[serde(default)]
    pub group: bool,
    #[serde(default)]
    pub muted: bool,
    /// Our camera is on.
    #[serde(default)]
    pub camera: bool,
    /// We're sharing our screen.
    #[serde(default)]
    pub screen: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Incoming,
    Outgoing,
    Connecting,
    Active,
    Ended,
}

/// Where we are in voice rooms.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Voice {
    /// The room we're in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<String>,
    #[serde(default)]
    pub room_name: String,
    #[serde(default)]
    pub connected: bool,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub deafened: bool,
    #[serde(default)]
    pub camera: bool,
    #[serde(default)]
    pub screen: bool,
    #[serde(default)]
    pub members: Vec<Member>,
    #[serde(default)]
    pub streams: Vec<Stream>,
    /// Calls ringing us in rooms we're not in: join with [`Cmd::Join`].
    #[serde(default)]
    pub ringing: Vec<Ring>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Stream {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub watching: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Ring {
    pub room: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub name: String,
    /// A small JPEG preview, base64.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
}

impl Command {
    /// The line to write (without its newline).
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).expect("a command always serializes")
    }

    /// Parses a line from Aflos; `None` for anything it doesn't understand.
    pub fn from_line(line: &str) -> Option<Self> {
        serde_json::from_str(line).ok()
    }
}

impl Event {
    /// The line to write (without its newline).
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).expect("an event always serializes")
    }

    /// Parses a line from an integration; `None` for anything this version
    /// doesn't understand, which the caller ignores.
    pub fn from_line(line: &str) -> Option<Self> {
        serde_json::from_str(line).ok()
    }
}

#[cfg(feature = "host")]
pub mod harness;
#[cfg(feature = "host")]
pub mod host;

#[cfg(test)]
mod tests;
