//! Aflos's own network, through the `mesh-node` helper (Mesh-Node). It speaks
//! the same JSON lines as `aflos-wa`: `{fn, args}` in on stdin, `{binding,
//! payload}` out on stdout. Commands carry passwords, so they're never logged.

use serde::Deserialize;
use std::path::PathBuf;
use tokio::sync::mpsc;

/// What Aflos hears from `mesh-node`.
#[derive(Debug, Clone)]
pub enum Event {
    /// The helper is up; it reports `Identity` straight after.
    Connected(Node),
    Identity(Identity),
    /// The verdict on a password being typed (`Node::check_password`).
    Password { ok: bool, message: String },
    Error(Error),
    /// The safety number with someone (`Node::safety_number`).
    Safety(Safety),
    /// A command that answers with nothing else went through (`idChangePassword`, `idExport`).
    Done(String),
    /// The node's state; comes by itself whenever it changes.
    Net(Net),
    /// This device's test ticket (`Node::ticket`).
    Ticket(String),
    /// A ping's answer (`Node::ping`).
    Ping(Pong),
    /// This device's live friend codes; comes after making or revoking one,
    /// and when a seed confirms holding them.
    Codes(Codes),
    /// The ID a friend code points to (`Node::resolve`).
    Resolved(Resolved),
    /// Friends, requests both ways, blocks and our status (SPEC-social).
    /// Answers every friend command, and comes by itself when something
    /// changes (a request arrives, a friend's presence, a sync).
    Social(Social),
    /// A link code to type on the new device, once a seed holds where to
    /// find this one (`Node::link_start`); `expires` is unix seconds.
    Link { code: String, expires: u64 },
    /// The new device typed the code: confirm linking the device with this name.
    LinkConfirm(String),
    /// The device with this name is linked; an `Identity` with it follows.
    LinkDone(String),
    /// On the new device: the identity arrived; ask for its username and a
    /// new password (`Node::link_finish`).
    LinkReceived(String),
    /// The chat list (`Node::dm_list`); comes by itself whenever a chat changes.
    DmChats(Vec<Chat>),
    /// A page of the open chat, newest last (`Node::dm_open`); comes again
    /// while that chat is open when it changes.
    DmMessages(Page),
    /// A new message in a chat that isn't open.
    DmNotify(DmNotify),
    /// Someone is typing in a chat (`id`): their IDs.
    DmTyping {
        id: String,
        who: Vec<String>,
    },
    /// A file is ready to open, decrypted to `path` (`Node::dm_fetch`, or
    /// `Node::space_fetch` with the Space's ID as `id`).
    DmFile(DmFile),
    /// The Spaces held (`Node::space_list`); comes by itself whenever one
    /// changes (SPEC-spaces).
    Spaces(Vec<Space>),
    /// A page of an open channel, newest last (`Node::space_open`); comes
    /// again while it's open when it changes.
    SpaceMessages(SpacePage),
    /// The reports this device holds for a Space it owns (P11).
    SpaceReports(SpaceReports),
    /// A new post by someone else in a channel.
    SpaceNotify(SpaceNotify),
    /// Someone is typing in a channel: their IDs.
    SpaceTyping {
        space: String,
        channel: String,
        who: Vec<String>,
    },
    /// Where our call stands (SPEC-calls): a DM call or a voice channel.
    Call(Call),
    /// The mic's level after processing (dBFS), and whether it goes out:
    /// about 15 a second in a call or the mic test (SPEC-audio A5).
    AudioLevel {
        level: f32,
        open: bool,
    },
    /// Who's in one voice channel now, by ID.
    Voice {
        space: String,
        channel: String,
        people: Vec<String>,
    },
    /// The helper exited; it's restarted by itself.
    Disconnected,
}

/// A `meshCall`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Call {
    /// `calling`, `ringing`, `live` or `ended`.
    pub state: String,
    pub id: String,
    /// The friend in a DM call (ourselves in a voice channel).
    pub with: String,
    #[serde(default)]
    pub caller: bool,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub deafened: bool,
    #[serde(default)]
    pub speaking: Vec<String>,
    /// Why it ended: `ended`, `declined`, `missed`, `unreachable`,
    /// `update`, `elsewhere`, `lost`, `left`, `removed`.
    pub outcome: Option<String>,
    /// A voice channel's Space and channel, and who's in it.
    pub space: Option<String>,
    pub channel: Option<String>,
    #[serde(default)]
    pub people: Vec<String>,
}

/// A `meshDmChats` row: a 1:1 (`id` is the friend's ID) or a group.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Chat {
    pub id: String,
    pub kind: ChatKind,
    /// The friend's name, or the group's.
    pub name: String,
    pub members: Vec<String>,
    /// Milliseconds since the epoch of the newest message; 0 for none.
    pub last: u64,
    pub unread: u64,
    pub mentions: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatKind {
    Direct,
    Group,
}

/// A `meshDmMessages`: one chat's page, oldest first.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Page {
    pub id: String,
    pub events: Vec<DmMessage>,
    /// Older messages exist (`Node::dm_open` with `before`).
    pub more: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DmMessage {
    /// The event ID: a `TimelineMessage`'s `web_id`.
    pub id: String,
    pub from: String,
    pub text: String,
    pub edited: bool,
    pub reply_to: Option<String>,
    pub attachments: Vec<Attachment>,
    pub reactions: Vec<Reaction>,
    /// Arrived well after it was sent (SPEC-dm D2).
    pub late: bool,
    /// Milliseconds since the epoch.
    pub time: u64,
    /// Ours, and taken by another device; false while "sending".
    pub sent: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Attachment {
    pub name: String,
    pub mime: String,
    pub size: u64,
    /// A small picture preview, hex-encoded.
    pub preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Reaction {
    pub emoji: String,
    pub who: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DmNotify {
    pub id: String,
    pub from: String,
    /// Cut to 100 characters; a file's name if it has no text.
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DmFile {
    pub id: String,
    pub message: String,
    pub index: usize,
    pub path: String,
}

/// A `meshSpaces` row: a Space held, with its channels and members.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Space {
    pub id: String,
    pub name: String,
    /// Ours: `owner`, `admin` or `member`; `None` while it's on its way.
    pub role: Option<String>,
    pub channels: Vec<SpaceChannel>,
    pub members: Vec<SpaceMember>,
    #[serde(default)]
    pub categories: Vec<SpaceCategory>,
    /// Members may add their friends, not only admins.
    #[serde(default)]
    pub members_may_add: bool,
    /// The owner lets members who are friends connect directly in this
    /// Space (SPEC-connection-privacy P7); off by default.
    #[serde(default)]
    pub direct_allowed: bool,
    /// Banned IDs (shown to admins, to unban).
    #[serde(default)]
    pub bans: Vec<String>,
    /// The owner ended it: nothing changes any more.
    #[serde(default)]
    pub ended: bool,
}

impl Space {
    /// Unread lines across its channels.
    pub fn unread(&self) -> u64 {
        self.channels.iter().map(|c| c.unread).sum()
    }
    /// Unread lines that mention us, across its channels (S12).
    pub fn mentions(&self) -> u64 {
        self.channels.iter().map(|c| c.mentions).sum()
    }
}

/// A Space's text channel, in the order the Space lists them.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceChannel {
    pub id: String,
    pub name: String,
    pub category: Option<String>,
    /// Only admins may post (an announcements channel).
    pub admins_post: bool,
    /// A voice channel (SPEC-calls C8).
    #[serde(default)]
    pub voice: bool,
    #[serde(default)]
    pub unread: u64,
    #[serde(default)]
    pub mentions: u64,
}

/// A Space's category of channels.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SpaceCategory {
    pub id: String,
    pub name: String,
    pub position: u16,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SpaceMember {
    pub id: String,
    pub name: String,
    pub role: String,
}

/// A `meshSpaceMessages`: one channel's page, oldest first, its lines as a
/// DM page's.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SpacePage {
    pub space: String,
    pub channel: String,
    pub events: Vec<DmMessage>,
    /// Older lines exist (`Node::space_open` with `before`).
    #[serde(default)]
    pub more: bool,
}

/// A `meshSpaceReports`: what members reported to us, as a Space's owner.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SpaceReports {
    pub space: String,
    pub reports: Vec<SpaceReport>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SpaceReport {
    /// Who reported, and who about (`mesh1…` IDs).
    pub from: String,
    pub about: String,
    pub reason: String,
    /// The messages the reporter included.
    #[serde(default)]
    pub quoted: Vec<String>,
    /// When, Unix seconds: with `from`, what dismissing one names.
    pub at: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SpaceNotify {
    pub space: String,
    pub channel: String,
    pub from: String,
    pub text: String,
    /// It names us, or says `@everyone` from an admin (S12).
    #[serde(default)]
    pub mention: bool,
}

/// Where the identity on this install stands. Only `Unlocked` says anything
/// about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    None,
    Locked,
    Unlocked,
}

/// A `meshIdentity` report.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub state: State,
    /// `mesh1…`, while unlocked.
    #[serde(default)]
    pub id: Option<String>,
    /// `mesh://id/…`, while unlocked.
    #[serde(default)]
    pub link: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub devices: Vec<Device>,
    #[serde(default)]
    pub keep_unlocked: bool,
    /// Why "keep unlocked" can't be used on this machine, if it can't.
    #[serde(default)]
    pub keyring: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub name: String,
    /// Seconds since the epoch.
    pub created_at: u64,
    pub this_device: bool,
    /// Hex; what `Node::remove_device` takes.
    #[serde(default)]
    pub key: String,
}

/// A `meshSafety`: the 60-digit number two people compare, for the other ID.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Safety {
    /// The other person's ID, as parsed.
    pub id: String,
    /// 12 groups of 5 digits.
    pub number: String,
    /// What the QR code holds.
    pub qr: String,
}

/// Whether this device is on the Aflos network (SPEC-transport "Helper protocol").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetState {
    /// Locked: no node.
    Off,
    /// Running, no relay reached yet.
    Connecting,
    Online,
}

/// A `meshNet` report. Never holds an IP address.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Net {
    pub state: NetState,
    /// The home relay's URL, once online.
    pub relay: Option<String>,
    /// At least one connection is on a direct path.
    pub direct: bool,
    pub connections: usize,
    /// This device's address records are on a seed: contacts can find it.
    #[serde(default)]
    pub published: bool,
    /// Those records carry this device's e2ee keys: secure messages ready.
    #[serde(default)]
    pub keys: bool,
    /// Each connected person's path, `direct` or `relay`, by `mesh1…` ID
    /// (SPEC-connection-privacy P8). Never an address.
    #[serde(default)]
    pub paths: std::collections::BTreeMap<String, String>,
    #[serde(default, rename = "directCount")]
    pub direct_count: usize,
    #[serde(default, rename = "relayedCount")]
    pub relayed_count: usize,
    /// Hide my IP is on: everyone, friends too, is relayed (P6).
    #[serde(default, rename = "hideIp")]
    pub hide_ip: bool,
}

/// A `meshPing`: who answered, by which path, and how fast.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Pong {
    /// Their `mesh1…` ID.
    pub id: String,
    /// Their device's name.
    pub name: String,
    /// `direct` or `relay`.
    pub path: String,
    pub ms: f64,
}

/// A `meshCodes` report.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Codes {
    pub codes: Vec<Code>,
    /// The code just made, if this report answers `create_code`.
    pub created: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Code {
    /// `K7QM-3XPA`.
    pub code: String,
    /// `mesh://code/…`.
    pub link: String,
    pub created_at: u64,
    /// Seconds since the epoch; `None` never expires.
    pub expires_at: Option<u64>,
    /// `None` is unlimited.
    pub max_uses: Option<u32>,
    /// A seed holds it, so others can look it up.
    #[serde(default)]
    pub held: bool,
}

/// A `meshSocial` report (SPEC-social).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Social {
    /// The command it answers; `None` when it came by itself.
    #[serde(rename = "fn")]
    pub answers: Option<String>,
    pub friends: Vec<Friend>,
    /// Requests to us, waiting for Accept or Decline.
    pub incoming: Vec<Request>,
    /// People we asked, or added by ID, who haven't connected yet.
    pub outgoing: Vec<Known>,
    pub blocked: Vec<Known>,
    /// `online`, `idle`, `dnd` or `invisible`.
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Friend {
    /// Their `mesh1…` ID.
    pub id: String,
    /// The name they gave themselves: a hint only.
    pub name: String,
    /// Our name for them; empty if none.
    pub nick: String,
    pub verified: bool,
    /// `online`, `idle`, `dnd` or `offline`.
    pub presence: String,
    /// When one of their devices last answered a dial (seconds since the epoch).
    pub seen: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Request {
    pub id: String,
    pub name: String,
    pub note: String,
    /// When they sent it.
    pub at: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Known {
    pub id: String,
    pub nick: String,
    pub at: u64,
}

/// A `meshResolved`: the code and the ID it points to.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Resolved {
    pub code: String,
    pub id: String,
    pub link: String,
}

/// A `meshError`: the command that failed, a stable kind and a message to show.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Error {
    #[serde(rename = "fn")]
    pub func: String,
    pub kind: String,
    pub message: String,
}

/// Commands into `mesh-node`. Cheap to clone.
#[derive(Debug, Clone)]
pub struct Node {
    commands: mpsc::UnboundedSender<String>,
}

impl Node {
    /// Makes the identity; answers with `Identity` (unlocked) or `Error`.
    pub fn create(&self, username: &str, password: &str) {
        self.call("idCreate", &[username, password]);
    }
    /// Opens the identity; a wrong username or password answers with the
    /// same `Error` either way.
    pub fn unlock(&self, username: &str, password: &str) {
        self.call("idUnlock", &[username, password]);
    }
    pub fn lock(&self) {
        self.call("idLock", &[]);
    }
    /// Turns "keep unlocked on this device" on or off; answers with `Identity`.
    pub fn keep_unlocked(&self, on: bool) {
        self.call("idKeepUnlocked", &[if on { "true" } else { "false" }]);
    }
    /// Answers with `Done` or `Error`.
    pub fn change_password(&self, username: &str, old: &str, new: &str) {
        self.call("idChangePassword", &[username, old, new]);
    }
    /// Writes an encrypted backup that opens with this username and password;
    /// answers with `Done` or `Error`.
    pub fn export(&self, path: &str, username: &str, password: &str) {
        self.call("idExport", &[path, username, password]);
    }
    /// Restores a backup as this install's identity (a new device), with a new
    /// password; answers with `Identity` (unlocked) or `Error`.
    pub fn import(&self, path: &str, username: &str, backup_password: &str, new_password: &str) {
        self.call("idImport", &[path, username, backup_password, new_password]);
    }
    /// The safety number with a pasted ID or `mesh://` link; answers with
    /// `Safety`, or an `Error` naming what's wrong with the text.
    pub fn safety_number(&self, other: &str) {
        self.call("safetyNumber", &[other]);
    }
    /// Asks whether a new password would be accepted; answers with `Password`.
    pub fn check_password(&self, username: &str, password: &str) {
        self.call("idCheckPassword", &[username, password]);
    }

    /// Makes a friend code; `expiry` is seconds, `never` or empty (7 days),
    /// `uses` a count, `unlimited` or empty (1). Answers with `Codes`.
    pub fn create_code(&self, expiry: &str, uses: &str) {
        self.call("codeCreate", &[expiry, uses]);
    }
    /// Answers with `Codes`.
    pub fn revoke_code(&self, code: &str) {
        self.call("codeRevoke", &[code]);
    }
    /// Looks a friend code (or `mesh://code/` link) up on the seeds; answers
    /// with `Resolved` or an `Error` (`not-found`, `slow-down`, `offline`, or
    /// what's wrong with the text).
    pub fn resolve(&self, code: &str) {
        self.call("codeResolve", &[code]);
    }
    /// Asks for the node's state; answers with `Net`.
    pub fn net_status(&self) {
        self.call("netStatus", &[]);
    }
    /// This device's `mesh://node/…` test ticket; answers with `Ticket`, or an
    /// `Error` (`offline`) before a relay is reached.
    pub fn ticket(&self) {
        self.call("netTicket", &[]);
    }
    /// Pings the device a ticket names; answers with `Ping` or an `Error`
    /// (`bad-ticket`, `unreachable`, `timeout`, `bad-cert`, `key-mismatch`, `offline`).
    pub fn ping(&self, ticket: &str) {
        self.call("netPing", &[ticket]);
    }

    /// Sends a friend request through someone's live friend code (it uses
    /// one of the code's uses). Answers with `Social` once a seed has it, or
    /// an `Error` (`not-found`, `self`, `blocked`, `already-friend`,
    /// `slow-down`, `offline`, `bad-note`).
    pub fn request(&self, code: &str, note: &str) {
        self.call("friendRequest", &[code, note]);
    }
    /// Adds someone by ID: they're a friend once they've added yours too.
    /// Answers with `Social`, or an `Error` (`self`, `already-contact`,
    /// `blocked`, `bad-name`, or what's wrong with the ID).
    pub fn add_friend(&self, id: &str, nick: &str) {
        self.call("friendAdd", &[id, nick]);
    }
    /// Each answers with `Social`, or an `Error`.
    pub fn accept(&self, id: &str) {
        self.call("friendAccept", &[id]);
    }
    /// Nothing tells them.
    pub fn decline(&self, id: &str) {
        self.call("friendDecline", &[id]);
    }
    /// Removes a friend, or cancels a request we sent.
    pub fn remove_friend(&self, id: &str) {
        self.call("friendRemove", &[id]);
    }
    pub fn rename_friend(&self, id: &str, nick: &str) {
        self.call("friendRename", &[id, nick]);
    }
    pub fn verify(&self, id: &str, verified: bool) {
        self.call("friendVerify", &[id, if verified { "true" } else { "false" }]);
    }
    pub fn block(&self, id: &str) {
        self.call("block", &[id]);
    }
    pub fn unblock(&self, id: &str) {
        self.call("unblock", &[id]);
    }
    pub fn social(&self) {
        self.call("socialList", &[]);
    }
    /// `online`, `idle`, `dnd` or `invisible`, for the whole account.
    pub fn set_status(&self, status: &str) {
        self.call("statusSet", &[status]);
    }
    /// Pings a friend by ID; answers with `Ping` (then `Social`), or an
    /// `Error` (`not-a-contact`, `unreachable`, `revoked-device`, `offline`).
    pub fn ping_id(&self, id: &str) {
        self.call("netPingId", &[id]);
    }
    /// Shows a link code for a new device; answers with `Link`, then
    /// `LinkConfirm` once the new device has typed it.
    pub fn link_start(&self) {
        self.call("linkStart", &[]);
    }
    /// On a device with no identity: joins the identity showing `code`,
    /// naming this device; answers with `LinkReceived` or `Error`.
    pub fn link_join(&self, code: &str, device_name: &str) {
        self.call("linkJoin", &[code, device_name]);
    }
    /// Yes or no to the device `LinkConfirm` named; yes answers with
    /// `LinkDone`, no with `Done`.
    pub fn link_confirm(&self, yes: bool) {
        self.call("linkConfirm", &[if yes { "true" } else { "false" }]);
    }
    /// Stops a link on either side; answers with `Done`.
    pub fn link_cancel(&self) {
        self.call("linkCancel", &[]);
    }
    /// On the new device: saves the identity under its username and this
    /// device's own password; answers with `Identity` (unlocked) or `Error`.
    pub fn link_finish(&self, username: &str, password: &str) {
        self.call("linkFinish", &[username, password]);
    }
    /// Deletes this device's copy of the identity, if `password` is right;
    /// answers with `Identity` (`none`) and the helper restarts, or `Error`.
    pub fn remove_identity(&self, password: &str) {
        self.call("idRemove", &[password]);
    }
    /// Removes one of our other linked devices by its key; answers with `Identity`.
    pub fn remove_device(&self, key: &str) {
        self.call("deviceRemove", &[key]);
    }

    /// The chat list; answers with `DmChats`.
    pub fn dm_list(&self) {
        self.call("dmList", &[]);
    }
    /// Opens a chat (a friend's ID or a group's ID): its newest page, or the
    /// one before `before`; answers with `DmMessages`.
    pub fn dm_open(&self, chat: &str, before: Option<&str>) {
        self.call("dmOpen", &[chat, before.unwrap_or("")]);
    }
    /// Sends a message, maybe as a reply; answers with `DmMessages`, or an
    /// `Error` (`not-a-friend`, `too-long`, `not-member`, `locked`).
    pub fn dm_send(&self, chat: &str, text: &str, reply_to: Option<&str>) {
        self.call("dmSend", &[chat, text, reply_to.unwrap_or("")]);
    }
    /// Edits one of our messages; answers with `DmMessages` or `Error` (`not-author`).
    pub fn dm_edit(&self, message: &str, text: &str) {
        self.call("dmEdit", &[message, text]);
    }
    /// Deletes a message here, or for everyone (ours only); answers with `DmMessages`.
    pub fn dm_delete(&self, message: &str, everyone: bool) {
        self.call("dmDelete", &[message, if everyone { "true" } else { "false" }]);
    }
    /// Adds or takes back a reaction; answers with `DmMessages`.
    pub fn dm_react(&self, message: &str, emoji: &str, on: bool) {
        self.call("dmReact", &[message, emoji, if on { "true" } else { "false" }]);
    }
    /// Marks a chat read up to a message; answers with `DmChats`.
    pub fn dm_read(&self, chat: &str, up_to: &str) {
        self.call("dmRead", &[chat, up_to]);
    }
    /// Says we're typing in a chat; answers with nothing.
    pub fn dm_typing(&self, chat: &str) {
        self.call("dmTyping", &[chat]);
    }
    /// Makes a group with these friends; answers with `DmChats`, or an
    /// `Error` (`not-a-friend`, `too-many`).
    pub fn dm_group_create(&self, ids: &[String], name: &str) {
        self.call("dmGroupCreate", &[&ids.join(","), name]);
    }
    pub fn dm_group_add(&self, group: &str, id: &str) {
        self.call("dmGroupAdd", &[group, id]);
    }
    /// Only the group's creator can; answers with `DmChats` or `Error` (`not-creator`).
    pub fn dm_group_remove(&self, group: &str, id: &str) {
        self.call("dmGroupRemove", &[group, id]);
    }
    pub fn dm_group_leave(&self, group: &str) {
        self.call("dmGroupLeave", &[group]);
    }
    pub fn dm_group_rename(&self, group: &str, name: &str) {
        self.call("dmGroupRename", &[group, name]);
    }
    /// Sends the file at `path`, with an optional picture preview file;
    /// answers with `DmMessages` or `Error` (`too-long` past 100 MB).
    pub fn dm_send_file(&self, chat: &str, path: &str, preview: Option<&str>) {
        self.call("dmSendFile", &[chat, path, preview.unwrap_or("")]);
    }
    /// Gets a message's file (`index` among its attachments); answers with
    /// `DmFile`, or `Error` (`unavailable`: nobody connected has it).
    pub fn dm_fetch(&self, chat: &str, message: &str, index: usize) {
        self.call("dmFetch", &[chat, message, &index.to_string()]);
    }

    /// The Spaces held; answers with `Spaces`.
    pub fn space_list(&self) {
        self.call("spaceList", &[]);
    }
    /// Opens a channel: its newest page, or the one before `before`;
    /// answers with `SpaceMessages`.
    pub fn space_open(&self, space: &str, channel: &str, before: Option<&str>) {
        match before {
            Some(before) => self.call("spaceOpen", &[space, channel, before]),
            None => self.call("spaceOpen", &[space, channel]),
        }
    }
    /// Posts a line, maybe as a reply, naming members (or `@everyone`, an
    /// admin's only); answers with `SpaceMessages`, or `Error`
    /// (`not-allowed`, `bad-args`).
    pub fn space_send(&self, space: &str, channel: &str, text: &str, reply_to: Option<&str>, mentions: &[String]) {
        let mut args = vec![space, channel, text, reply_to.unwrap_or("")];
        args.extend(mentions.iter().map(String::as_str));
        self.call("spaceSend", &args);
    }
    /// Edits one of our lines; answers with `SpaceMessages` or `Error` (`not-allowed`).
    pub fn space_edit(&self, space: &str, channel: &str, message: &str, text: &str) {
        self.call("spaceEdit", &[space, channel, message, text]);
    }
    /// Deletes our line, or anyone's as an admin; answers with `SpaceMessages`.
    pub fn space_delete(&self, space: &str, channel: &str, message: &str) {
        self.call("spaceDelete", &[space, channel, message]);
    }
    /// Adds or takes back a reaction; answers with `SpaceMessages`.
    pub fn space_react(&self, space: &str, channel: &str, message: &str, emoji: &str, on: bool) {
        self.call("spaceReact", &[space, channel, message, emoji, if on { "true" } else { "false" }]);
    }
    /// Marks a channel read up to a line; answers with `Spaces`.
    pub fn space_read(&self, space: &str, channel: &str, up_to: &str) {
        self.call("spaceRead", &[space, channel, up_to]);
    }
    /// Says we're typing in a channel; answers with nothing.
    pub fn space_typing(&self, space: &str, channel: &str) {
        self.call("spaceTyping", &[space, channel]);
    }
    /// Posts the file at `path`, with an optional picture preview file;
    /// answers with `SpaceMessages` or `Error` (`too-long` past 100 MB).
    pub fn space_send_file(&self, space: &str, channel: &str, path: &str, preview: Option<&str>) {
        self.call("spaceSendFile", &[space, channel, path, preview.unwrap_or("")]);
    }
    /// A Space's other commands, with `mesh-node`'s arguments: `spaceCreate`,
    /// `spaceAdd`, `spaceLeave`, `spaceKick`, `spaceBan`, `spaceUnban`,
    /// `spaceRole`, `spaceRename`, `spaceChannel`, `spaceChannelDelete`,
    /// `spaceChannelPost`, `spaceChannelMove`, `spaceCategory`,
    /// `spaceCategoryDelete`, `spaceMembersMayAdd`, `spaceTransfer`,
    /// `spaceEnd`. Answers with `Spaces`, or `Error` (`not-allowed`).
    pub fn space_change(&self, func: &str, args: &[&str]) {
        debug_assert!(func.starts_with("space"));
        self.call(func, args);
    }
    /// Gets a line's file; answers with `DmFile` (its `id` the Space's), or
    /// `Error` (`unavailable`).
    pub fn space_fetch(&self, space: &str, channel: &str, message: &str, index: usize) {
        self.call("spaceFetch", &[space, channel, message, &index.to_string()]);
    }

    /// Calls a friend (SPEC-calls); answers with `Call` events.
    pub fn call_start(&self, friend: &str) {
        self.call("callStart", &[friend]);
    }
    pub fn call_answer(&self) {
        self.call("callAnswer", &[]);
    }
    /// Hangs up, cancels, declines, or leaves a voice channel.
    pub fn call_hangup(&self) {
        self.call("callHangup", &[]);
    }
    /// Input volume for calls, 0 to 200 percent (SPEC-audio A3).
    /// The switches, sensitivity and output volume, as `callAudio`'s JSON.
    pub fn call_audio(&self, json: &str) {
        self.call("callAudio", &[json]);
    }
    /// One person's volume for us, 0 to 200 percent.
    pub fn call_volume(&self, id: &str, percent: u32) {
        self.call("callVolume", &[id, &percent.to_string()]);
    }
    /// The mic test: on (and played back with `hear`), or off.
    pub fn audio_test(&self, on: bool, hear: bool) {
        match (on, hear) {
            (true, true) => self.call("audioTest", &["on", "hear"]),
            (true, false) => self.call("audioTest", &["on"]),
            (false, _) => self.call("audioTest", &["off"]),
        }
    }
    pub fn call_gain(&self, percent: u32) {
        self.call("callGain", &[&percent.to_string()]);
    }
    /// Hide my IP on or off (SPEC-connection-privacy P6).
    pub fn net_privacy(&self, hide: bool) {
        self.call("netPrivacy", &[if hide { "true" } else { "false" }]);
    }
    pub fn call_mute(&self, on: bool) {
        self.call("callMute", &[if on { "true" } else { "false" }]);
    }
    pub fn call_deafen(&self, on: bool) {
        self.call("callDeafen", &[if on { "true" } else { "false" }]);
    }
    /// Push-to-talk: `off` (voice activity), `up` (on, the key released) or
    /// `down` (held).
    pub fn call_ptt(&self, mode: &str) {
        self.call("callPtt", &[mode]);
    }
    /// Microphone and speakers by name, `""` for the default.
    pub fn call_devices(&self, input: &str, output: &str) {
        self.call("callDevices", &[input, output]);
    }
    /// Joins a voice channel; answers with `Call`, or `Error` (`full`).
    pub fn voice_join(&self, space: &str, channel: &str) {
        self.call("voiceJoin", &[space, channel]);
    }

    fn call(&self, function: &str, args: &[&str]) {
        let _ = self.commands.send(serde_json::json!({ "fn": function, "args": args }).to_string() + "\n");
    }
}

/// Runs `mesh-node --data <data>` for as long as `events` is open, restarting
/// it when it exits (the same backoff as `whatsapp::run_native`).
/// `args` gives its flags at each start, so a restart after a setting
/// changed (Hide my IP) starts it as the setting now is.
pub async fn run(
    exe: PathBuf,
    args: impl Fn() -> Vec<String>,
    data: PathBuf,
    events: mpsc::UnboundedSender<Event>,
) {
    run_with(exe, args, data, events).await;
}

/// `run`, with more flags for `mesh-node` (a test's relay and seed).
pub async fn run_with(exe: PathBuf, args: impl Fn() -> Vec<String>, data: PathBuf, events: mpsc::UnboundedSender<Event>) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
    while !events.is_closed() {
        let mut cmd = tokio::process::Command::new(&exe);
        cmd.args(args()).arg("--data").arg(&data).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(err) => {
                tracing::warn!("could not start {}: {err}", exe.display());
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                continue;
            }
        };
        let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else { return };
        let (commands, mut command_rx) = mpsc::unbounded_channel::<String>();
        let mut lines = tokio::io::BufReader::new(stdout).lines();
        if events.send(Event::Connected(Node { commands })).is_err() {
            return;
        }
        loop {
            tokio::select! {
                line = lines.next_line() => {
                    let Ok(Some(line)) = line else { break };
                    let Some(event) = line_event(&line) else { continue };
                    if events.send(event).is_err() {
                        return;
                    }
                }
                Some(command) = command_rx.recv() => {
                    if stdin.write_all(command.as_bytes()).await.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = child.kill().await;
        if events.send(Event::Disconnected).is_err() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}

/// A helper stdout line as an event; `None` for the ones Aflos doesn't use yet.
fn line_event(line: &str) -> Option<Event> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let payload = v["payload"].as_str()?;
    match v["binding"].as_str()? {
        "meshIdentity" => serde_json::from_str(payload).ok().map(Event::Identity),
        "meshPassword" => {
            let v: serde_json::Value = serde_json::from_str(payload).ok()?;
            Some(Event::Password { ok: v["ok"].as_bool()?, message: v["message"].as_str().unwrap_or_default().to_owned() })
        }
        "meshError" => serde_json::from_str(payload).ok().map(Event::Error),
        "meshSafety" => serde_json::from_str(payload).ok().map(Event::Safety),
        "meshDone" => {
            let v: serde_json::Value = serde_json::from_str(payload).ok()?;
            Some(Event::Done(v["fn"].as_str()?.to_owned()))
        }
        "meshNet" => serde_json::from_str(payload).ok().map(Event::Net),
        "meshTicket" => {
            let v: serde_json::Value = serde_json::from_str(payload).ok()?;
            Some(Event::Ticket(v["ticket"].as_str()?.to_owned()))
        }
        "meshPing" => serde_json::from_str(payload).ok().map(Event::Ping),
        "meshCodes" => serde_json::from_str(payload).ok().map(Event::Codes),
        "meshResolved" => serde_json::from_str(payload).ok().map(Event::Resolved),
        "meshSocial" => serde_json::from_str(payload).ok().map(Event::Social),
        "meshLink" => {
            let v: serde_json::Value = serde_json::from_str(payload).ok()?;
            Some(Event::Link { code: v["code"].as_str()?.to_owned(), expires: v["expires"].as_u64()? })
        }
        binding @ ("meshLinkConfirm" | "meshLinkDone") => {
            let v: serde_json::Value = serde_json::from_str(payload).ok()?;
            let name = v["name"].as_str()?.to_owned();
            Some(if binding == "meshLinkDone" { Event::LinkDone(name) } else { Event::LinkConfirm(name) })
        }
        "meshLinkReceived" => {
            let v: serde_json::Value = serde_json::from_str(payload).ok()?;
            Some(Event::LinkReceived(v["username"].as_str()?.to_owned()))
        }
        "meshDmChats" => {
            #[derive(Deserialize)]
            struct Chats {
                chats: Vec<Chat>,
            }
            serde_json::from_str::<Chats>(payload).ok().map(|c| Event::DmChats(c.chats))
        }
        "meshDmMessages" => serde_json::from_str(payload).ok().map(Event::DmMessages),
        "meshDmNotify" => serde_json::from_str(payload).ok().map(Event::DmNotify),
        "meshDmTyping" => {
            let v: serde_json::Value = serde_json::from_str(payload).ok()?;
            let who = v["who"].as_array()?.iter().filter_map(|w| w.as_str().map(str::to_owned)).collect();
            Some(Event::DmTyping {
                id: v["id"].as_str()?.to_owned(),
                who,
            })
        }
        "meshDmFile" => serde_json::from_str(payload).ok().map(Event::DmFile),
        "meshSpaces" => {
            #[derive(Deserialize)]
            struct Spaces {
                spaces: Vec<Space>,
            }
            serde_json::from_str::<Spaces>(payload).ok().map(|s| Event::Spaces(s.spaces))
        }
        "meshSpaceMessages" => serde_json::from_str(payload).ok().map(Event::SpaceMessages),
        "meshSpaceReports" => serde_json::from_str(payload).ok().map(Event::SpaceReports),
        "meshCall" => serde_json::from_str(payload).ok().map(Event::Call),
        "meshAudioLevel" => {
            let v: serde_json::Value = serde_json::from_str(payload).ok()?;
            Some(Event::AudioLevel {
                level: v["level"].as_f64()? as f32,
                open: v["open"].as_bool().unwrap_or(false),
            })
        }
        "meshVoice" => {
            let v: serde_json::Value = serde_json::from_str(payload).ok()?;
            let people = v["people"]
                .as_array()?
                .iter()
                .filter_map(|w| w.as_str().map(str::to_owned))
                .collect();
            Some(Event::Voice {
                space: v["space"].as_str()?.to_owned(),
                channel: v["channel"].as_str()?.to_owned(),
                people,
            })
        }
        "meshSpaceNotify" => serde_json::from_str(payload).ok().map(Event::SpaceNotify),
        "meshSpaceTyping" => {
            let v: serde_json::Value = serde_json::from_str(payload).ok()?;
            let who = v["who"].as_array()?.iter().filter_map(|w| w.as_str().map(str::to_owned)).collect();
            Some(Event::SpaceTyping {
                space: v["space"].as_str()?.to_owned(),
                channel: v["channel"].as_str()?.to_owned(),
                who,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(binding: &str, payload: &str) -> String {
        serde_json::json!({ "binding": binding, "payload": payload }).to_string()
    }

    #[test]
    fn reads_helper_lines() {
        let Some(Event::Identity(locked)) = line_event(&line("meshIdentity", r#"{"state":"locked","keyring":"no Secret Service"}"#)) else {
            panic!("no identity")
        };
        assert_eq!((locked.state, locked.id, locked.keyring.as_deref()), (State::Locked, None, Some("no Secret Service")));

        let open = r#"{"state":"unlocked","id":"mesh1abc","link":"mesh://id/mesh1abc","username":"Brendon","devices":[{"name":"box","key":"ab12","createdAt":5,"thisDevice":true}],"keepUnlocked":false,"keyring":null}"#;
        let Some(Event::Identity(open)) = line_event(&line("meshIdentity", open)) else { panic!("no identity") };
        assert_eq!((open.state, open.username.as_deref(), open.devices[0].this_device, open.devices[0].key.as_str()), (State::Unlocked, Some("Brendon"), true, "ab12"));

        let Some(Event::Password { ok, message }) = line_event(&line("meshPassword", r#"{"ok":false,"message":"Add another word."}"#)) else {
            panic!("no verdict")
        };
        assert_eq!((ok, message.as_str()), (false, "Add another word."));

        let Some(Event::Error(e)) = line_event(&line("meshError", r#"{"fn":"idUnlock","kind":"unlock","message":"can't unlock"}"#)) else {
            panic!("no error")
        };
        assert_eq!((e.func.as_str(), e.kind.as_str()), ("idUnlock", "unlock"));

        let Some(Event::Done(func)) = line_event(&line("meshDone", r#"{"fn":"idExport"}"#)) else { panic!("no done") };
        assert_eq!(func, "idExport");

        let Some(Event::Safety(n)) = line_event(&line("meshSafety", r#"{"id":"mesh1abc","number":"12345 67890","qr":"q"}"#)) else {
            panic!("no safety number")
        };
        assert_eq!(n.number, "12345 67890");

        let Some(Event::Net(net)) = line_event(&line("meshNet", r#"{"state":"online","relay":"https://aps1-1.relay.n0.iroh.link./","direct":true,"connections":1}"#)) else {
            panic!("no net")
        };
        assert_eq!((net.state, net.direct, net.connections), (NetState::Online, true, 1));
        let Some(Event::Net(off)) = line_event(&line("meshNet", r#"{"state":"off","relay":null,"direct":false,"connections":0}"#)) else { panic!("no net") };
        assert_eq!((off.state, off.relay), (NetState::Off, None));

        let Some(Event::Ticket(t)) = line_event(&line("meshTicket", r#"{"ticket":"mesh://node/ab?relay=x"}"#)) else { panic!("no ticket") };
        assert_eq!(t, "mesh://node/ab?relay=x");

        let Some(Event::Ping(pong)) = line_event(&line("meshPing", r#"{"id":"mesh1abc","name":"box","path":"direct","ms":0.7}"#)) else {
            panic!("no pong")
        };
        assert_eq!((pong.id.as_str(), pong.path.as_str(), pong.ms), ("mesh1abc", "direct", 0.7));

        let codes = r#"{"codes":[{"code":"K7QM-3XPA","link":"mesh://code/K7QM-3XPA","createdAt":5,"expiresAt":null,"maxUses":1,"held":true}],"created":"K7QM-3XPA"}"#;
        let Some(Event::Codes(c)) = line_event(&line("meshCodes", codes)) else { panic!("no codes") };
        assert_eq!((c.codes[0].held, c.codes[0].expires_at, c.created.as_deref()), (true, None, Some("K7QM-3XPA")));

        let Some(Event::Resolved(r)) = line_event(&line("meshResolved", r#"{"code":"K7QM-3XPA","id":"mesh1abc","link":"mesh://id/mesh1abc"}"#)) else {
            panic!("no resolve")
        };
        assert_eq!(r.id, "mesh1abc");

        let social = r#"{"fn":"friendAccept","friends":[{"id":"mesh1abc","name":"alex","nick":"Alex","verified":true,"presence":"dnd","seen":9}],
            "incoming":[{"id":"mesh1def","name":"sam","note":"hi","at":5}],"outgoing":[{"id":"mesh1ghi","nick":"","at":6}],"blocked":[],"status":"invisible"}"#;
        let Some(Event::Social(s)) = line_event(&line("meshSocial", social)) else { panic!("no social") };
        assert_eq!((s.answers.as_deref(), s.friends[0].presence.as_str(), s.friends[0].seen), (Some("friendAccept"), "dnd", Some(9)));
        assert_eq!((s.incoming[0].note.as_str(), s.outgoing[0].at, s.blocked.len(), s.status.as_str()), ("hi", 6, 0, "invisible"));
        let unasked = r#"{"fn":null,"friends":[],"incoming":[],"outgoing":[],"blocked":[],"status":"online"}"#;
        let Some(Event::Social(s)) = line_event(&line("meshSocial", unasked)) else { panic!("no social") };
        assert_eq!(s.answers, None);
        let Some(Event::Net(published)) = line_event(&line("meshNet", r#"{"state":"online","relay":null,"direct":false,"connections":0,"published":true}"#)) else {
            panic!("no net")
        };
        assert!(published.published && !off.published && !published.keys);
        let Some(Event::Net(keys)) = line_event(&line("meshNet", r#"{"state":"online","relay":null,"direct":false,"connections":0,"published":true,"keys":true}"#)) else {
            panic!("no net")
        };
        assert!(keys.keys);


        let Some(Event::Link { code, expires }) = line_event(&line("meshLink", r#"{"code":"K7QM-3XPA","expires":99}"#)) else { panic!("no link") };
        assert_eq!((code.as_str(), expires), ("K7QM-3XPA", 99));
        let Some(Event::LinkConfirm(name)) = line_event(&line("meshLinkConfirm", r#"{"name":"Kitchen laptop"}"#)) else { panic!("no ask") };
        assert_eq!(name, "Kitchen laptop");
        let Some(Event::LinkDone(name)) = line_event(&line("meshLinkDone", r#"{"name":"Kitchen laptop"}"#)) else { panic!("no done") };
        assert_eq!(name, "Kitchen laptop");
        let Some(Event::LinkReceived(username)) = line_event(&line("meshLinkReceived", r#"{"username":"alice"}"#)) else { panic!("no identity") };
        assert_eq!(username, "alice");

        assert!(line_event(&line("meshParsed", r#"{"kind":"id"}"#)).is_none());
        assert!(line_event("not json").is_none());
    }

    /// The dm events, as the helper writes them (SPEC-dm *Client side*).
    #[test]
    fn dm_events() {
        let chats = r#"{"chats":[{"id":"mesh1bob","kind":"direct","name":"bob","members":["mesh1bob"],"last":1794428700000,"unread":2,"mentions":0},{"id":"ab12","kind":"group","name":"hike","members":["mesh1a","mesh1b","mesh1c"],"last":0,"unread":0,"mentions":0}]}"#;
        let Some(Event::DmChats(chats)) = line_event(&line("meshDmChats", chats)) else {
            panic!("no chats")
        };
        assert_eq!(
            (chats.len(), chats[0].kind, chats[0].unread, chats[0].last),
            (2, ChatKind::Direct, 2, 1794428700000)
        );
        assert_eq!((chats[1].kind, chats[1].name.as_str(), chats[1].members.len()), (ChatKind::Group, "hike", 3));

        let page = r#"{"id":"mesh1bob","more":true,"events":[{"id":"57dd","from":"mesh1me","text":"hi","edited":true,"replyTo":"11aa","attachments":[{"name":"trail.jpg","mime":"image/jpeg","size":300000,"preview":"ffd8"}],"reactions":[{"emoji":"🎉","who":["mesh1bob"]}],"late":false,"time":1794428384000,"sent":true}]}"#;
        let Some(Event::DmMessages(page)) = line_event(&line("meshDmMessages", page)) else {
            panic!("no page")
        };
        assert_eq!((page.id.as_str(), page.more, page.events.len()), ("mesh1bob", true, 1));
        let m = &page.events[0];
        assert_eq!(
            (m.id.as_str(), m.from.as_str(), m.text.as_str(), m.edited, m.reply_to.as_deref(), m.sent, m.time),
            ("57dd", "mesh1me", "hi", true, Some("11aa"), true, 1794428384000)
        );
        assert_eq!(
            (m.attachments[0].name.as_str(), m.attachments[0].size, m.attachments[0].preview.as_deref()),
            ("trail.jpg", 300000, Some("ffd8"))
        );
        assert_eq!((m.reactions[0].emoji.as_str(), m.reactions[0].who.len()), ("🎉", 1));
        let bare = r#"{"id":"ab12","more":false,"events":[{"id":"01","from":"mesh1b","text":"","edited":false,"replyTo":null,"attachments":[{"name":"a","mime":"text/plain","size":1,"preview":null}],"reactions":[],"late":true,"time":5,"sent":false}]}"#;
        let Some(Event::DmMessages(bare)) = line_event(&line("meshDmMessages", bare)) else {
            panic!("no page")
        };
        assert_eq!(
            (
                bare.events[0].reply_to.as_deref(),
                bare.events[0].late,
                bare.events[0].attachments[0].preview.as_deref()
            ),
            (None, true, None)
        );

        let Some(Event::DmNotify(n)) = line_event(&line("meshDmNotify", r#"{"id":"ab12","from":"mesh1b","text":"hello"}"#)) else {
            panic!("no notify")
        };
        assert_eq!((n.id.as_str(), n.from.as_str(), n.text.as_str()), ("ab12", "mesh1b", "hello"));
        let Some(Event::DmTyping { id, who }) = line_event(&line("meshDmTyping", r#"{"id":"mesh1bob","who":["mesh1bob"]}"#)) else {
            panic!("no typing")
        };
        assert_eq!((id.as_str(), who), ("mesh1bob", vec!["mesh1bob".to_owned()]));
        let Some(Event::DmFile(f)) = line_event(&line(
            "meshDmFile",
            r#"{"id":"ab12","message":"57dd","index":0,"path":"/data/files/57dd-0/map.png"}"#,
        )) else {
            panic!("no file")
        };
        assert_eq!((f.message.as_str(), f.index, f.path.as_str()), ("57dd", 0, "/data/files/57dd-0/map.png"));
    }

    /// The Space events (SPEC-spaces S12, PLAN-spaces Task 11b): the list
    /// with each channel's counts, a channel's page (a DM page's lines),
    /// notifications that say if they mention you, and typing.
    #[test]
    fn call_events() {
        let c = r#"{"state":"live","id":"ab","with":"mesh1b","caller":true,"muted":false,"deafened":false,"speaking":["mesh1b"],"outcome":null,"space":"s","channel":"c","people":["mesh1a","mesh1b"]}"#;
        let Some(Event::Call(call)) = line_event(&line("meshCall", c)) else {
            panic!()
        };
        assert_eq!(
            (
                call.state.as_str(),
                call.caller,
                call.speaking.len(),
                call.people.len()
            ),
            ("live", true, 1, 2)
        );
        assert_eq!(call.channel.as_deref(), Some("c"));
        let ended = r#"{"state":"ended","id":"ab","with":"mesh1b","outcome":"declined"}"#;
        let Some(Event::Call(call)) = line_event(&line("meshCall", ended)) else {
            panic!()
        };
        assert_eq!(call.outcome.as_deref(), Some("declined"));
        let v = r#"{"space":"s","channel":"c","people":["mesh1a"]}"#;
        let Some(Event::Voice {
            space,
            channel,
            people,
        }) = line_event(&line("meshVoice", v))
        else {
            panic!()
        };
        assert_eq!(
            (space.as_str(), channel.as_str(), people),
            ("s", "c", vec!["mesh1a".to_string()])
        );
    }

    #[test]
    fn space_events() {
        let spaces = r#"{"spaces":[{"id":"aa11","name":"club","role":"owner","changes":3,"channels":[{"id":"c1","name":"general","category":null,"adminsPost":false,"unread":2,"mentions":1},{"id":"c2","name":"news","category":"k1","adminsPost":true,"unread":0,"mentions":0}],"members":[{"id":"mesh1o","name":"owen","role":"owner"},{"id":"mesh1b","name":"bea","role":"member"}]}]}"#;
        let Some(Event::Spaces(spaces)) = line_event(&line("meshSpaces", spaces)) else {
            panic!("no spaces")
        };
        let club = &spaces[0];
        assert_eq!((club.id.as_str(), club.name.as_str(), club.role.as_deref()), ("aa11", "club", Some("owner")));
        let (general, news) = (&club.channels[0], &club.channels[1]);
        assert_eq!(
            (general.name.as_str(), general.category.as_deref(), general.unread, general.mentions),
            ("general", None, 2, 1)
        );
        assert_eq!((news.category.as_deref(), news.admins_post), (Some("k1"), true));
        assert_eq!((club.members[1].name.as_str(), club.members[1].role.as_str()), ("bea", "member"));
        assert_eq!((club.unread(), club.mentions()), (2, 1));
        assert!(!club.direct_allowed, "off unless meshSpaces says so");
        let on = r#"{"spaces":[{"id":"aa11","name":"club","role":"owner","channels":[],"members":[],"directAllowed":true}]}"#;
        let Some(Event::Spaces(on)) = line_event(&line("meshSpaces", on)) else {
            panic!("no spaces")
        };
        assert!(on[0].direct_allowed);
        let Some(Event::AudioLevel { level, open }) =
            line_event(&line("meshAudioLevel", r#"{"level":-31.5,"open":true}"#))
        else {
            panic!("no level")
        };
        assert_eq!((level, open), (-31.5, true));
        let reports = r#"{"space":"aa11","reports":[{"from":"mesh1b","about":"mesh1c","reason":"spam","quoted":["buy now"],"at":7}]}"#;
        let Some(Event::SpaceReports(reports)) = line_event(&line("meshSpaceReports", reports))
        else {
            panic!("no reports")
        };
        let r = &reports.reports[0];
        assert_eq!(
            (
                reports.space.as_str(),
                r.from.as_str(),
                r.about.as_str(),
                r.at
            ),
            ("aa11", "mesh1b", "mesh1c", 7)
        );
        assert_eq!(
            (r.reason.as_str(), r.quoted.as_slice()),
            ("spam", &["buy now".to_string()][..])
        );

        let page = r#"{"space":"aa11","channel":"c1","more":false,"events":[{"id":"57dd","from":"mesh1b","text":"hi","edited":false,"replyTo":null,"attachments":[],"reactions":[],"late":false,"time":5,"sent":false}]}"#;
        let Some(Event::SpaceMessages(page)) = line_event(&line("meshSpaceMessages", page)) else {
            panic!("no page")
        };
        assert_eq!(
            (page.space.as_str(), page.channel.as_str(), page.more, page.events[0].text.as_str()),
            ("aa11", "c1", false, "hi")
        );

        let n = r#"{"space":"aa11","channel":"c1","from":"mesh1b","text":"cal?","mention":true}"#;
        let Some(Event::SpaceNotify(n)) = line_event(&line("meshSpaceNotify", n)) else {
            panic!("no notify")
        };
        assert_eq!((n.space.as_str(), n.channel.as_str(), n.text.as_str(), n.mention), ("aa11", "c1", "cal?", true));

        let t = r#"{"space":"aa11","channel":"c1","who":["mesh1b"]}"#;
        let Some(Event::SpaceTyping { space, channel, who }) = line_event(&line("meshSpaceTyping", t)) else {
            panic!("no typing")
        };
        assert_eq!((space.as_str(), channel.as_str(), who), ("aa11", "c1", vec!["mesh1b".to_string()]));
    }
}
