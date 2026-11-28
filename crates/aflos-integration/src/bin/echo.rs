//! Echo: the smallest whole integration (PLAN-integrations Task 3), and the
//! template's starting point. A rail tile, a sign-in form asking for a
//! name, and one chat that says back what you send.
//!
//! Plain blocking reads and writes: an integration needs no async runtime.
//!
//! `AFLOS_ECHO_MODE` makes it misbehave, for Aflos's tests: `crash` (exits
//! at once), `hang` (answers hello, then nothing), `silent` (never answers
//! hello), `garbage` (junk and an oversized line before its hello), `newer`
//! (speaks a protocol from the future), `broken` (claims Send, never sends
//! the chat back).

use aflos_integration::{
    Action, Chat, ChatKind, Chats, Cmd, Command, Event, Field, HelloBack, Message, PROTOCOL,
    Section, Setting, SettingKind, SignIn, SignInKind, State, Supports, Tile,
};
use std::io::{BufRead, Write};
use std::time::{SystemTime, UNIX_EPOCH};

const ICON: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="currentColor" d="M4 4h16v12H7l-3 3z"/></svg>"#;
const CHAT: &str = "echo";

fn main() {
    let mode = std::env::var("AFLOS_ECHO_MODE").unwrap_or_default();
    match mode.as_str() {
        "crash" => std::process::exit(3),
        "garbage" => {
            let mut out = std::io::stdout().lock();
            let _ = writeln!(out, "not json");
            let _ = writeln!(out, r#"{{"event":"hologram"}}"#);
            let _ = out.write_all(&vec![b'x'; 17 * 1024 * 1024]);
            let _ = writeln!(out);
            let _ = out.write_all(&[0xff, 0xfe, b'\n']);
            let _ = out.flush();
        }
        _ => {}
    }
    let mut echo = Echo {
        mode,
        name: None,
        shout: false,
        messages: Vec::new(),
    };
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Some(command) = Command::from_line(&line) else {
            continue;
        };
        if echo.mode == "hang" && !matches!(command.body, Cmd::Hello(_)) {
            continue;
        }
        if !echo.handle(command) {
            break;
        }
    }
}

struct Echo {
    mode: String,
    name: Option<String>,
    shout: bool,
    messages: Vec<Message>,
}

fn send(event: &Event) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}", event.to_line());
    let _ = out.flush();
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

impl Echo {
    /// Handles one command; `false` to stop.
    fn handle(&mut self, command: Command) -> bool {
        let reply = |ok: bool, error: Option<&str>| {
            send(&Event::Reply {
                id: command.id,
                ok,
                error: error.map(str::to_owned),
            });
        };
        match command.body {
            Cmd::Hello(hello) => {
                if self.mode == "silent" {
                    return true;
                }
                self.shout = hello
                    .settings
                    .get("shout")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                send(&Event::Hello(HelloBack {
                    protocol: if self.mode == "newer" {
                        PROTOCOL + 1
                    } else {
                        PROTOCOL
                    },
                    name: "Echo".into(),
                    tile: Tile {
                        icon_svg: ICON.into(),
                        tag: "ec".into(),
                        color: "#3fa7d6".into(),
                    },
                    supports: Supports {
                        sign_in: vec![SignInKind::Form],
                        actions: vec![Action::Send, Action::Reply, Action::React],
                        calls: None,
                        settings: vec![Setting {
                            key: "shout".into(),
                            label: "Shout back".into(),
                            help: Some("Echo answers in capitals.".into()),
                            kind: SettingKind::Switch { default: false },
                        }],
                    },
                }));
                send(&Event::Status {
                    state: State::SignedOut,
                    error: None,
                });
                send(&Event::SignIn(SignIn::Form {
                    fields: vec![Field {
                        name: "name".into(),
                        label: "Your name".into(),
                        secret: false,
                    }],
                }));
            }
            Cmd::Ping => reply(true, None),
            Cmd::Quit => return false,
            Cmd::SignIn { values } => match values
                .get("name")
                .map(|n| n.trim())
                .filter(|n| !n.is_empty())
            {
                Some(name) => {
                    self.name = Some(name.to_owned());
                    reply(true, None);
                    send(&Event::Status {
                        state: State::Online,
                        error: None,
                    });
                    self.chats();
                }
                None => {
                    reply(false, Some("Enter a name."));
                    send(&Event::SignIn(SignIn::Failed {
                        error: "Enter a name.".into(),
                    }));
                }
            },
            Cmd::SignOut => {
                self.name = None;
                self.messages.clear();
                reply(true, None);
                send(&Event::Status {
                    state: State::SignedOut,
                    error: None,
                });
            }
            Cmd::Open { chat } => {
                reply(true, None);
                if chat.as_deref() == Some(CHAT) {
                    self.show();
                }
            }
            Cmd::Send {
                chat,
                text,
                reply_to,
            } if chat == CHAT && self.name.is_some() => {
                reply(true, None);
                if self.mode == "broken" {
                    return true;
                }
                let quote = reply_to
                    .and_then(|id| self.messages.iter().find(|m| m.id == id))
                    .map(|m| aflos_integration::Quote {
                        id: Some(m.id.clone()),
                        author: m.author.clone(),
                        text: m.text.clone(),
                    });
                let me = self.name.clone().unwrap_or_default();
                let n = self.messages.len();
                self.messages.push(Message {
                    id: format!("m{n}"),
                    author_id: "me".into(),
                    author: me,
                    text: text.clone(),
                    mine: true,
                    at_ms: now_ms(),
                    quote,
                    ticks: 3,
                    ..Message::default()
                });
                let back = if self.shout {
                    text.to_uppercase()
                } else {
                    text
                };
                self.messages.push(Message {
                    id: format!("m{}", n + 1),
                    author_id: "echo".into(),
                    author: "Echo".into(),
                    text: back,
                    at_ms: now_ms(),
                    ..Message::default()
                });
                self.show();
                self.chats();
            }
            Cmd::React { message, emoji } => {
                match self.messages.iter_mut().find(|m| m.id == message) {
                    Some(m) => {
                        m.reactions.retain(|r| !r.mine);
                        if !emoji.is_empty() {
                            m.reactions.push(aflos_integration::Reaction {
                                emoji,
                                count: 1,
                                mine: true,
                            });
                        }
                        reply(true, None);
                        self.show();
                    }
                    None => reply(false, Some("No such message.")),
                }
            }
            Cmd::Setting { key, value } => {
                if key == "shout" {
                    self.shout = value.as_bool().unwrap_or(false);
                }
                reply(true, None);
            }
            _ => reply(false, Some("Echo doesn't do that.")),
        }
        true
    }

    fn chats(&self) {
        let last = self.messages.last();
        send(&Event::Chats(Chats {
            me: "me".into(),
            sections: vec![Section {
                id: "direct".into(),
                name: "Direct".into(),
                home: true,
                chats: vec![Chat {
                    id: CHAT.into(),
                    name: "Echo".into(),
                    kind: ChatKind::Direct,
                    last_ms: last.map_or(0, |m| m.at_ms),
                    ..Chat::default()
                }],
                ..Section::default()
            }],
        }));
    }

    fn show(&self) {
        send(&Event::Messages {
            chat: CHAT.into(),
            messages: self.messages.clone(),
        });
    }
}
