//! The Aflos integration protocol (SPEC-integrations) at aflos-wa's edge.
//!
//! Protocol commands become this helper's own calls (`{fn, args}`), and its
//! reports (`emit(binding, payload)`) become protocol events. WhatsApp's own
//! code stays as it was; this is the only place that knows the protocol.

use aflos_integration as ai;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// What a protocol command turned into.
pub enum Turned {
    /// One of this helper's own calls.
    Call {
        func: String,
        args: Vec<String>,
    },
    /// Nothing to do beyond the reply already sent.
    Done,
    Quit,
}

/// What the translation remembers between a command and its report.
#[derive(Default)]
struct Edge {
    me: String,
    files: PathBuf,
    /// Video previews come inline with messages; kept to write on fetch.
    thumbs: HashMap<String, String>,
    /// Commands waiting on a report, oldest first: sends (text and files),
    /// forwards, a chat started, and chat actions by chat.
    sends: VecDeque<u64>,
    forwards: VecDeque<u64>,
    start: Option<u64>,
    actions: HashMap<String, u64>,
    /// Fetches waiting, by this helper's own key: the protocol key and form.
    fetches: HashMap<String, (String, ai::FetchForm)>,
    signed_in: bool,
}

static EDGE: Mutex<Option<Edge>> = Mutex::new(None);

fn edge<T>(f: impl FnOnce(&mut Edge) -> T) -> T {
    let mut guard = EDGE.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(Edge::default))
}

pub fn send(event: &ai::Event) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}", event.to_line());
    let _ = out.flush();
}

fn reply(id: u64, result: Result<(), String>) {
    send(&ai::Event::Reply {
        id,
        ok: result.is_ok(),
        error: result.err(),
    });
}

/// Reads Aflos's hello (the first line), answers it, and returns it.
pub fn handshake() -> Option<ai::Hello> {
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).ok()?;
    let ai::Command {
        body: ai::Cmd::Hello(hello),
        ..
    } = ai::Command::from_line(&line)?
    else {
        return None;
    };
    if let Err(e) = ai::check_version(hello.protocol) {
        log::warn!("Aflos speaks another protocol: {e}");
    }
    edge(|e| e.files = hello.files_dir.clone());
    let _ = std::fs::create_dir_all(&hello.files_dir);
    send(&ai::Event::Hello(hello_back()));
    send(&ai::Event::Status {
        state: ai::State::Connecting,
        error: None,
    });
    Some(hello)
}

/// Its rail logo (the file starts with a credit comment, left out).
const ICON: &str = include_str!("../assets/whatsapp.svg");

pub fn hello_back() -> ai::HelloBack {
    use ai::Action::*;
    ai::HelloBack {
        protocol: ai::PROTOCOL,
        name: "WhatsApp".into(),
        tile: ai::Tile {
            icon_svg: ICON[ICON.find("<svg").unwrap_or(0)..].into(),
            tag: "wa".into(),
            color: "#9fd88b".into(),
        },
        supports: ai::Supports {
            sign_in: vec![ai::SignInKind::Qr],
            actions: vec![
                Send, Reply, React, Forward, SendFiles, Typing, Search, StartChat, Archive,
                DeleteChat,
            ],
            calls: Some(ai::Calls {
                video: true,
                screen: true,
                voice_rooms: false,
                streams: false,
            }),
            settings: Vec::new(),
        },
    }
}

fn flag(on: bool) -> String {
    if on { "true" } else { "false" }.into()
}

fn call(func: &str, args: impl IntoIterator<Item = String>) -> Turned {
    Turned::Call {
        func: func.into(),
        args: args.into_iter().collect(),
    }
}

/// A protocol line from Aflos, as this helper's own call. Commands that
/// get no report of their own are answered here.
pub fn turn(line: &str) -> Option<Turned> {
    let ai::Command { id, body } = ai::Command::from_line(line)?;
    use ai::Cmd as C;
    let ok = |t: Turned| {
        reply(id, Ok(()));
        t
    };
    Some(match body {
        C::Hello(_) => ok(Turned::Done),
        C::Ping => ok(Turned::Done),
        C::Quit => Turned::Quit,
        C::SignIn { .. } | C::SignInCode { .. } => ok(Turned::Done),
        C::SignOut => ok(call("signOut", [])),
        C::Open { chat } => ok(call("open", chat)),
        C::Send {
            chat,
            text,
            reply_to,
        } => {
            edge(|e| e.sends.push_back(id));
            call("send", [chat, text, reply_to.unwrap_or_default()])
        }
        C::SendFile {
            chat,
            path,
            caption,
        } => match std::fs::read(&path) {
            Ok(data) => {
                edge(|e| e.sends.push_back(id));
                let name = sent_name(&path);
                let mime = mime_of(&name).to_string();
                call("sendFile", [chat, name, mime, B64.encode(data), caption])
            }
            Err(e) => {
                reply(id, Err(format!("couldn't read the file: {e}")));
                Turned::Done
            }
        },
        C::React { message, emoji } => ok(call("react", [message, emoji])),
        C::Forward { message, to } => {
            edge(|e| e.forwards.push_back(id));
            call("forward", [message, to])
        }
        C::Typing { chat, on } => ok(call("typing", [chat, flag(on)])),
        C::Fetch { key, form } => {
            reply(id, Ok(()));
            fetch(key, form)
        }
        C::Search { query } => ok(call("searchContacts", [query])),
        C::StartChat { who } => {
            edge(|e| e.start = Some(id));
            call("startChat", [who])
        }
        C::Archive { chat, archived } => {
            edge(|e| e.actions.insert(chat.clone(), id));
            call(if archived { "archive" } else { "unarchive" }, [chat])
        }
        C::DeleteChat { chat } => {
            edge(|e| e.actions.insert(chat.clone(), id));
            call("deleteChat", [chat])
        }
        C::CallStart { chat, video } => ok(call("callStart", [chat, flag(video)])),
        C::CallAccept { video } => ok(call("callAccept", [flag(video)])),
        C::CallDecline => ok(call("callDecline", [])),
        C::CallEnd => ok(call("callEnd", [])),
        C::Mute { on } => ok(call("callMute", [flag(on)])),
        C::Camera { on } => ok(call("callCamera", [flag(on)])),
        C::Screen { on, source } => ok(call("callScreen", std::iter::once(flag(on)).chain(source))),
        C::Sources => ok(call("listSources", [])),
        C::Setting { .. } | C::AudioDevices { .. } => ok(Turned::Done),
        _ => {
            reply(id, Err("WhatsApp doesn't do that".into()));
            Turned::Done
        }
    })
}

/// A fetch, by what the key names: a video preview kept here, a voice note's
/// samples, a GIF's frames, a file to open, or a picture.
fn fetch(key: String, form: ai::FetchForm) -> Turned {
    if let Some(id) = key.strip_prefix("wa-thumb:") {
        let thumb = edge(|e| e.thumbs.get(id).cloned());
        let result = match thumb.and_then(|b| B64.decode(b).ok()) {
            Some(jpeg) => write(&key, "jpg", &jpeg),
            None => Err("no preview".into()),
        };
        fetched(key, form, result.map(|path| (path, None)));
        return Turned::Done;
    }
    let own = match (form, key.strip_prefix("wa-file:")) {
        (ai::FetchForm::Pcm, Some(id)) => ("voicePcm", id.to_owned(), format!("pcm:{id}")),
        (ai::FetchForm::Original, Some(id)) => ("saveMedia", id.to_owned(), format!("file:{id}")),
        _ => ("fetchMedia", key.clone(), key.clone()),
    };
    edge(|e| e.fetches.insert(own.2, (key, form)));
    call(own.0, [own.1])
}

fn fetched(key: String, form: ai::FetchForm, result: Result<(PathBuf, Option<u64>), String>) {
    let result = match result {
        Ok((path, delay_ms)) => ai::Fetched::Done { path, delay_ms },
        Err(error) => ai::Fetched::Failed { error },
    };
    send(&ai::Event::Fetched { key, form, result });
}

/// Writes a fetched file into the files folder, named for its key.
fn write(key: &str, ext: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    let safe: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let path = edge(|e| e.files.clone()).join(format!("{safe}.{ext}"));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

/// The name Aflos gave a file it hands over (`out-<pid>-<n>-<name>`), less
/// its prefix.
fn sent_name(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut parts = name.splitn(4, '-');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("out"), Some(_), Some(_), Some(rest)) if !rest.is_empty() => rest.to_owned(),
        _ => name,
    }
}

fn mime_of(name: &str) -> &'static str {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "ogg" | "opus" => "audio/ogg",
        "mp3" => "audio/mpeg",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// One of this helper's reports, as protocol events.
pub fn emit(binding: &str, payload: &Value) {
    let s = |v: &Value| v.as_str().unwrap_or_default().to_owned();
    let error = || payload["error"].as_str().map(str::to_owned);
    match binding {
        "waEmit" => {
            let me = s(&payload["me"]);
            edge(|e| e.me = me.clone());
            if !edge(|e| std::mem::replace(&mut e.signed_in, true)) {
                send(&ai::Event::Status {
                    state: ai::State::Online,
                    error: None,
                });
            }
            send(&ai::Event::Chats(chats(payload)));
        }
        "waReady" => {
            edge(|e| e.signed_in = true);
            send(&ai::Event::Status {
                state: ai::State::Online,
                error: None,
            });
        }
        "waQr" => {
            edge(|e| e.signed_in = false);
            send(&ai::Event::Status {
                state: ai::State::SignedOut,
                error: None,
            });
            send(&ai::Event::SignIn(ai::SignIn::Qr {
                png: None,
                svg: payload["svg"].as_str().map(str::to_owned),
            }));
        }
        "waMessages" => {
            let me = edge(|e| e.me.clone());
            let messages = payload["messages"]
                .as_array()
                .map(|m| m.iter().map(|m| message(m, &me)).collect())
                .unwrap_or_default();
            send(&ai::Event::Messages {
                chat: s(&payload["chat_id"]),
                messages,
            });
        }
        "waNotify" => send(&ai::Event::Notify {
            chat: s(&payload["chat_id"]),
            title: s(&payload["title"]),
            body: s(&payload["body"]),
        }),
        "waTyping" => send(&ai::Event::Typing {
            chat: s(&payload["chat_id"]),
            who: s(&payload["who"]),
            on: payload["typing"].as_bool().unwrap_or(false),
        }),
        "waPresence" => send(&ai::Event::Presence {
            who: s(&payload["id"]),
            online: payload["online"].as_bool().unwrap_or(false),
            last_seen: payload["last_seen"].as_u64(),
        }),
        "waSent" | "waForwarded" => {
            let id = edge(|e| {
                if binding == "waSent" {
                    e.sends.pop_front()
                } else {
                    e.forwards.pop_front()
                }
            });
            let ok = payload["ok"].as_bool().unwrap_or(false);
            reply(
                id.unwrap_or(0),
                if ok {
                    Ok(())
                } else {
                    Err(error().unwrap_or_else(|| "it didn't go".into()))
                },
            );
        }
        "waChatAction" => {
            let id = edge(|e| {
                e.actions
                    .remove(payload["chat_id"].as_str().unwrap_or_default())
            });
            let ok = payload["ok"].as_bool().unwrap_or(false);
            reply(
                id.unwrap_or(0),
                if ok {
                    Ok(())
                } else {
                    Err(error().unwrap_or_default())
                },
            );
        }
        "waChatStarted" => {
            let id = edge(|e| e.start.take()).unwrap_or(0);
            match payload["chat"]["id"].as_str() {
                Some(chat) => {
                    reply(id, Ok(()));
                    send(&ai::Event::Show {
                        chat: chat.to_owned(),
                    });
                }
                None => reply(
                    id,
                    Err(error().unwrap_or_else(|| "couldn't start that chat".into())),
                ),
            }
        }
        "waContacts" => {
            let contacts = payload["contacts"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|c| ai::Contact {
                    id: s(&c["id"]),
                    name: s(&c["name"]),
                    detail: format!("+{}", s(&c["number"])),
                })
                .collect();
            send(&ai::Event::Contacts {
                query: s(&payload["query"]),
                contacts,
            });
        }
        "waMedia" => {
            let own = s(&payload["url"]);
            let Some((key, form)) = edge(|e| e.fetches.remove(&own)) else {
                return;
            };
            let result = match payload["data"].as_str().and_then(|b| B64.decode(b).ok()) {
                Some(bytes) => write(&key, "img", &bytes).map(|p| (p, None)),
                None => Err(error().unwrap_or_else(|| "couldn't get it".into())),
            };
            fetched(key, form, result);
        }
        "waFrames" => {
            let own = s(&payload["url"]);
            let Some((key, form)) = edge(|e| e.fetches.remove(&own)) else {
                return;
            };
            let frames: Option<Vec<Vec<u8>>> = payload["frames"].as_array().map(|f| {
                f.iter()
                    .filter_map(|b| B64.decode(b.as_str().unwrap_or_default()).ok())
                    .collect()
            });
            let result = match frames {
                Some(frames) if !frames.is_empty() => {
                    write_frames(&key, &frames).map(|p| (p, payload["delay_ms"].as_u64()))
                }
                _ => Err(error().unwrap_or_else(|| "no frames".into())),
            };
            fetched(key, form, result);
        }
        "waFile" => {
            let id = s(&payload["id"]);
            let Some((key, form)) = edge(|e| e.fetches.remove(&format!("file:{id}"))) else {
                return;
            };
            let result = match payload["data"].as_str().and_then(|b| B64.decode(b).ok()) {
                Some(bytes) => {
                    let name: String = s(&payload["filename"])
                        .chars()
                        .filter(|c| !matches!(c, '/' | '\\' | ':'))
                        .collect();
                    let dir = edge(|e| e.files.clone()).join(format!(
                        "file-{}",
                        id.chars()
                            .filter(char::is_ascii_alphanumeric)
                            .collect::<String>()
                    ));
                    let path = dir.join(if name.is_empty() {
                        "file".to_owned()
                    } else {
                        name
                    });
                    std::fs::create_dir_all(&dir)
                        .and_then(|()| std::fs::write(&path, bytes))
                        .map(|()| (path, None))
                        .map_err(|e| e.to_string())
                }
                None => Err(error().unwrap_or_else(|| "couldn't get it".into())),
            };
            fetched(key, form, result);
        }
        "waPcm" => {
            let id = s(&payload["id"]);
            let Some((key, form)) = edge(|e| e.fetches.remove(&format!("pcm:{id}"))) else {
                return;
            };
            let result = match payload["data"].as_str().and_then(|b| B64.decode(b).ok()) {
                Some(bytes) => write(&key, "pcm", &bytes).map(|p| (p, None)),
                None => Err(error().unwrap_or_else(|| "couldn't play it".into())),
            };
            fetched(key, form, result);
        }
        "waCall" => match payload {
            Value::Null => send(&ai::Event::Call { call: None }),
            v if v.get("error").is_some() => reply(0, Err(error().unwrap_or_default())),
            v => send(&ai::Event::Call {
                call: Some(call_of(v)),
            }),
        },
        "waVideo" => {
            let track = s(&payload["id"]);
            if payload["ended"].as_bool() == Some(true) {
                send(&ai::Event::VideoEnded { track });
            } else {
                send(&ai::Event::Video {
                    track,
                    local: payload["local"].as_bool().unwrap_or(false),
                    jpeg: s(&payload["jpeg"]),
                });
            }
        }
        "waSources" => {
            let source = |v: &Value| ai::Source {
                id: s(&v["id"]),
                name: s(&v["name"]),
                thumbnail: v["thumbnail"].as_str().map(str::to_owned),
            };
            let list = |k: &str| {
                payload[k]
                    .as_array()
                    .map(|a| a.iter().map(source).collect())
                    .unwrap_or_default()
            };
            send(&ai::Event::Sources {
                cameras: list("cameras"),
                screens: list("sources"),
            });
        }
        other => log::debug!("not passed on: {other}"),
    }
}

fn write_frames(key: &str, frames: &[Vec<u8>]) -> Result<PathBuf, String> {
    let safe: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let dir = edge(|e| e.files.clone()).join(safe);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for (n, frame) in frames.iter().enumerate() {
        std::fs::write(dir.join(format!("{n:05}.jpg")), frame).map_err(|e| e.to_string())?;
    }
    Ok(dir)
}

/// The chat list: Chats (one-to-one, in Aflos's Home) and Groups (their
/// own tile), archived ones marked.
fn chats(snap: &Value) -> ai::Chats {
    let mut direct = Vec::new();
    let mut groups = Vec::new();
    for (list, archived) in [("chats", false), ("archived", true)] {
        for c in snap[list].as_array().into_iter().flatten() {
            let group = c["group"].as_bool().unwrap_or(false);
            let chat = ai::Chat {
                id: c["id"].as_str().unwrap_or_default().to_owned(),
                name: c["name"].as_str().unwrap_or_default().to_owned(),
                kind: if group {
                    ai::ChatKind::Group
                } else {
                    ai::ChatKind::Direct
                },
                avatar: c["avatar"].as_str().map(str::to_owned),
                unread: c["unread"].as_u64().unwrap_or(0),
                last_ms: c["last_ms"].as_u64().unwrap_or(0),
                muted: c["muted"].as_bool().unwrap_or(false),
                archived,
                ..ai::Chat::default()
            };
            if group {
                groups.push(chat)
            } else {
                direct.push(chat)
            }
        }
    }
    ai::Chats {
        me: snap["me"].as_str().unwrap_or_default().to_owned(),
        sections: vec![
            ai::Section {
                id: "chats".into(),
                name: "Chats".into(),
                home: true,
                chats: direct,
                ..ai::Section::default()
            },
            ai::Section {
                id: "groups".into(),
                name: "Groups".into(),
                home: false,
                chats: groups,
                ..ai::Section::default()
            },
        ],
    }
}

/// A stored message, as the protocol's.
fn message(m: &Value, me: &str) -> ai::Message {
    let s = |v: &Value| v.as_str().unwrap_or_default().to_owned();
    let id = s(&m["id"]);
    let mut media = Vec::new();
    if let Some(image) = m["image"].as_object() {
        let gif = image.get("gif").and_then(Value::as_bool).unwrap_or(false);
        let sticker = image
            .get("sticker")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        media.push(ai::Media {
            key: format!("{}:{id}", if gif { "wa-gif" } else { "wa-msg" }),
            kind: if gif {
                ai::MediaKind::Gif
            } else if sticker {
                ai::MediaKind::Sticker
            } else {
                ai::MediaKind::Image
            },
            width: image.get("width").and_then(Value::as_u64),
            height: image.get("height").and_then(Value::as_u64),
            ..ai::Media::default()
        });
    }
    if let Some(f) = m["file"].as_object() {
        let kind = match f.get("kind").and_then(Value::as_str).unwrap_or_default() {
            "Video" => ai::MediaKind::Video,
            "Voice note" => ai::MediaKind::Voice,
            "Audio" => ai::MediaKind::Audio,
            _ => ai::MediaKind::File,
        };
        let preview = f.get("thumb").and_then(Value::as_str).map(|b64| {
            edge(|e| e.thumbs.insert(id.clone(), b64.to_owned()));
            format!("wa-thumb:{id}")
        });
        media.push(ai::Media {
            key: format!("wa-file:{id}"),
            kind,
            name: f
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            size: f.get("size").and_then(Value::as_u64),
            width: f.get("width").and_then(Value::as_u64),
            height: f.get("height").and_then(Value::as_u64),
            duration: f.get("duration").and_then(Value::as_f64),
            waveform: f
                .get("waveform")
                .and_then(Value::as_array)
                .map(|w| {
                    w.iter()
                        .filter_map(|v| v.as_u64().map(|n| n.min(100) as u8))
                        .collect()
                })
                .unwrap_or_default(),
            preview,
        });
    }
    let author_id = s(&m["author_id"]);
    ai::Message {
        mine: !me.is_empty() && author_id == me,
        id,
        author: s(&m["author"]),
        author_id,
        avatar: m["avatar"].as_str().map(str::to_owned),
        text: s(&m["content"]),
        system: m["system"].as_bool().unwrap_or(false),
        at_ms: m["timestamp_ms"].as_u64().unwrap_or(0),
        media,
        quote: m["quote"].as_object().map(|q| ai::Quote {
            id: None,
            author: q
                .get("author")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            text: q
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        }),
        forwarded: m["forwarded"].as_bool().unwrap_or(false),
        edited: false,
        reactions: m["reactions"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|r| ai::Reaction {
                emoji: r["key"].as_str().unwrap_or_default().to_owned(),
                count: r["count"].as_u64().unwrap_or(0),
                mine: r["mine"].as_bool().unwrap_or(false),
            })
            .collect(),
        ticks: m["ack"].as_u64().unwrap_or(0).min(3) as u8,
    }
}

fn call_of(v: &Value) -> ai::Call {
    let s = |k: &str| v[k].as_str().unwrap_or_default().to_owned();
    let b = |k: &str| v[k].as_bool().unwrap_or(false);
    ai::Call {
        id: s("id"),
        chat: s("peer"),
        name: s("name"),
        phase: match v["phase"].as_str().unwrap_or_default() {
            "incoming" => ai::Phase::Incoming,
            "outgoing" => ai::Phase::Outgoing,
            "connecting" => ai::Phase::Connecting,
            "ended" => ai::Phase::Ended,
            _ => ai::Phase::Active,
        },
        video: b("video"),
        group: b("group"),
        muted: b("muted"),
        camera: b("camera"),
        screen: b("screen"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_snapshot_becomes_home_chats_and_groups_with_archived_marked() {
        let snap = json!({
            "me": "27820000000@s.whatsapp.net",
            "chats": [
                { "id": "a", "name": "Ann", "group": false, "unread": 2, "last_ms": 5, "muted": false, "avatar": "https://pps/a" },
                { "id": "g", "name": "Band", "group": true, "unread": 0, "last_ms": 4, "muted": true, "avatar": null }
            ],
            "archived": [ { "id": "old", "name": "Old", "group": false, "unread": 0, "last_ms": 1, "muted": false } ]
        });
        let c = chats(&snap);
        assert_eq!(c.sections.len(), 2);
        let (home, groups) = (&c.sections[0], &c.sections[1]);
        assert!(home.home && !groups.home);
        assert_eq!(
            home.chats
                .iter()
                .map(|c| (c.id.as_str(), c.archived))
                .collect::<Vec<_>>(),
            [("a", false), ("old", true)]
        );
        assert_eq!(home.chats[0].avatar.as_deref(), Some("https://pps/a"));
        assert_eq!(groups.chats[0].kind, ai::ChatKind::Group);
        assert!(groups.chats[0].muted);
    }

    #[test]
    fn a_message_keeps_its_media_quote_reactions_and_ticks() {
        let m = json!({
            "id": "M1", "author_id": "me@s", "author": "You", "content": "look", "system": false, "timestamp_ms": 9,
            "image": { "width": 640, "height": 480, "sticker": false, "gif": false },
            "file": { "name": "note.ogg", "kind": "Voice note", "size": 100, "duration": 3.5, "waveform": [10, 250], "thumb": null },
            "quote": { "author": "Ann", "text": "Photo" },
            "forwarded": true, "reactions": [ { "key": "👍", "count": 2, "mine": true } ], "ack": 3
        });
        let got = message(&m, "me@s");
        assert!(got.mine);
        assert_eq!(got.ticks, 3);
        assert_eq!(got.media[0].key, "wa-msg:M1");
        assert_eq!(got.media[1].key, "wa-file:M1");
        assert_eq!(got.media[1].kind, ai::MediaKind::Voice);
        assert_eq!(got.media[1].waveform, [10, 100], "levels top out at 100");
        assert_eq!(got.quote.map(|q| q.author).as_deref(), Some("Ann"));
        assert_eq!(got.reactions[0].emoji, "👍");
        assert!(
            !message(&m, "").mine,
            "before we know who we are, nothing is ours"
        );
    }

    #[test]
    fn a_gif_and_a_video_preview_get_their_own_keys() {
        let m = json!({
            "id": "G", "author_id": "x", "author": "X", "content": "", "system": false, "timestamp_ms": 1,
            "image": { "gif": true },
            "file": { "name": "v.mp4", "kind": "Video", "thumb": "/9j/AA==" }
        });
        let got = message(&m, "me");
        assert_eq!(
            (got.media[0].key.as_str(), got.media[0].kind),
            ("wa-gif:G", ai::MediaKind::Gif)
        );
        assert_eq!(got.media[1].preview.as_deref(), Some("wa-thumb:G"));
    }

    #[test]
    fn files_aflos_hands_over_keep_their_names() {
        assert_eq!(
            sent_name(Path::new("/f/out-1234-0-holiday photo.jpg")),
            "holiday photo.jpg"
        );
        assert_eq!(sent_name(Path::new("/f/out-1234-2-a-b-c.pdf")), "a-b-c.pdf");
        assert_eq!(sent_name(Path::new("/f/plain.png")), "plain.png");
        assert_eq!(mime_of("x.JPG"), "image/jpeg");
        assert_eq!(mime_of("x.bin"), "application/octet-stream");
    }

    #[test]
    fn calls_map_their_phases() {
        let v = json!({ "id": "k", "peer": "a", "name": "Ann", "phase": "incoming", "video": true, "group": false, "muted": false, "camera": false, "screen": false });
        let c = call_of(&v);
        assert_eq!(
            (c.phase, c.chat.as_str(), c.video),
            (ai::Phase::Incoming, "a", true)
        );
    }
}
