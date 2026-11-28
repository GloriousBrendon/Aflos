use super::*;

fn hello() -> Hello {
    Hello {
        protocol: PROTOCOL,
        aflos: "0.6.5".into(),
        data_dir: "/data/echo".into(),
        files_dir: "/data/echo/files".into(),
        granted: vec![Permission::Microphone, Permission::Camera],
        settings: BTreeMap::from([("hd".into(), serde_json::json!(true))]),
    }
}

fn every_cmd() -> Vec<Cmd> {
    let s = String::from;
    vec![
        Cmd::Hello(hello()),
        Cmd::Ping,
        Cmd::Quit,
        Cmd::SignIn {
            values: BTreeMap::from([(s("login"), s("me")), (s("password"), s("pw"))]),
        },
        Cmd::SignInCode { code: s("123456") },
        Cmd::SignOut,
        Cmd::Open {
            chat: Some(s("c1")),
        },
        Cmd::Open { chat: None },
        Cmd::Send {
            chat: s("c1"),
            text: s("hi **there**"),
            reply_to: Some(s("m1")),
        },
        Cmd::Send {
            chat: s("c1"),
            text: s("hi"),
            reply_to: None,
        },
        Cmd::SendFile {
            chat: s("c1"),
            path: "/files/a.png".into(),
            caption: s("look"),
        },
        Cmd::React {
            message: s("m1"),
            emoji: s("👍"),
        },
        Cmd::Edit {
            message: s("m1"),
            text: s("fixed"),
        },
        Cmd::Delete {
            message: s("m1"),
            everyone: true,
        },
        Cmd::Forward {
            message: s("m1"),
            to: s("c2"),
        },
        Cmd::Typing {
            chat: s("c1"),
            on: true,
        },
        Cmd::Fetch {
            key: s("img:m1"),
            form: FetchForm::Original,
        },
        Cmd::Fetch {
            key: s("voice:m2"),
            form: FetchForm::Pcm,
        },
        Cmd::Fetch {
            key: s("gif:m3"),
            form: FetchForm::Frames,
        },
        Cmd::Search { query: s("ann") },
        Cmd::StartChat {
            who: s("+27821234567"),
        },
        Cmd::Archive {
            chat: s("c1"),
            archived: true,
        },
        Cmd::DeleteChat { chat: s("c1") },
        Cmd::JoinByLink {
            link: s("https://discord.gg/abc"),
        },
        Cmd::Members { section: s("g1") },
        Cmd::CallStart {
            chat: s("c1"),
            video: true,
        },
        Cmd::CallAccept { video: false },
        Cmd::CallDecline,
        Cmd::CallEnd,
        Cmd::Join { room: s("v1") },
        Cmd::Leave,
        Cmd::Mute { on: true },
        Cmd::Deafen { on: false },
        Cmd::Camera { on: true },
        Cmd::Screen {
            on: true,
            source: Some(s("screen:0")),
        },
        Cmd::Screen {
            on: false,
            source: None,
        },
        Cmd::Sources,
        Cmd::Watch {
            stream: s("u9"),
            on: true,
        },
        Cmd::AudioDevices {
            input: Some(s("USB mic")),
            output: None,
        },
        Cmd::Setting {
            key: s("hd"),
            value: serde_json::json!(false),
        },
    ]
}

fn every_event() -> Vec<Event> {
    let s = String::from;
    let member = Member {
        id: s("u1"),
        name: s("Ann"),
        avatar: Some(s("av:u1")),
        speaking: true,
        muted: false,
        deafened: false,
    };
    vec![
        Event::Hello(HelloBack {
            protocol: PROTOCOL,
            name: s("Echo"),
            tile: Tile {
                icon_svg: s("<svg/>"),
                tag: s("ec"),
                color: s("#3fa7d6"),
            },
            supports: Supports {
                sign_in: vec![
                    SignInKind::Qr,
                    SignInKind::Code,
                    SignInKind::Form,
                    SignInKind::Web,
                ],
                actions: vec![
                    Action::Send,
                    Action::Reply,
                    Action::React,
                    Action::DeleteForEveryone,
                    Action::JoinByLink,
                    Action::Members,
                ],
                calls: Some(Calls {
                    video: true,
                    screen: true,
                    voice_rooms: true,
                    streams: true,
                }),
                settings: vec![
                    Setting {
                        key: s("hd"),
                        label: s("HD video"),
                        help: Some(s("Uses more data")),
                        kind: SettingKind::Switch { default: true },
                    },
                    Setting {
                        key: s("fps"),
                        label: s("Frame rate"),
                        help: None,
                        kind: SettingKind::Choice {
                            choices: vec![(s("30"), s("30 fps")), (s("60"), s("60 fps"))],
                            default: s("30"),
                        },
                    },
                    Setting {
                        key: s("nick"),
                        label: s("Nickname"),
                        help: None,
                        kind: SettingKind::Text { default: s("") },
                    },
                ],
            },
        }),
        Event::Reply {
            id: 7,
            ok: true,
            error: None,
        },
        Event::Reply {
            id: 8,
            ok: false,
            error: Some(s("No such chat")),
        },
        Event::Status {
            state: State::Online,
            error: None,
        },
        Event::Status {
            state: State::Offline,
            error: Some(s("no network")),
        },
        Event::SignIn(SignIn::Qr {
            png: Some(s("iVBOR")),
            svg: None,
        }),
        Event::SignIn(SignIn::Code {
            prompt: s("The code from your app"),
        }),
        Event::SignIn(SignIn::Form {
            fields: vec![
                Field {
                    name: s("login"),
                    label: s("Email"),
                    secret: false,
                },
                Field {
                    name: s("password"),
                    label: s("Password"),
                    secret: true,
                },
            ],
        }),
        Event::SignIn(SignIn::Web {
            url: s("https://example.com/login"),
        }),
        Event::SignIn(SignIn::Failed {
            error: s("Wrong password"),
        }),
        Event::Chats(Chats {
            me: s("u0"),
            sections: vec![
                Section {
                    id: s("direct"),
                    name: s("Direct"),
                    icon: None,
                    chats: vec![Chat {
                        id: s("c1"),
                        name: s("Ann"),
                        kind: ChatKind::Direct,
                        avatar: Some(s("av:u1")),
                        unread: 2,
                        last_ms: 1_700_000_000_000,
                        typing: vec![s("Ann")],
                        status: Some(s("online")),
                        ..Chat::default()
                    }],
                    rooms: vec![],
                },
                Section {
                    id: s("g1"),
                    name: s("Band"),
                    icon: Some(s("icon:g1")),
                    chats: vec![Chat {
                        id: s("t1"),
                        name: s("general"),
                        kind: ChatKind::Channel,
                        category: Some(s("Text")),
                        mentions: 1,
                        locked: false,
                        ..Chat::default()
                    }],
                    rooms: vec![Room {
                        id: s("v1"),
                        name: s("Lounge"),
                        category: None,
                        members: vec![member.clone()],
                        locked: false,
                    }],
                },
            ],
        }),
        Event::Messages {
            chat: s("c1"),
            messages: vec![Message {
                id: s("m1"),
                author_id: s("u1"),
                author: s("Ann"),
                avatar: Some(s("av:u1")),
                text: s("look"),
                system: false,
                mine: false,
                at_ms: 1_700_000_000_000,
                media: vec![
                    Media {
                        key: s("img:m1"),
                        kind: MediaKind::Image,
                        name: s("a.jpg"),
                        width: Some(640),
                        height: Some(480),
                        ..Media::default()
                    },
                    Media {
                        key: s("voice:m1"),
                        kind: MediaKind::Voice,
                        duration: Some(4.5),
                        waveform: vec![0, 50, 100],
                        ..Media::default()
                    },
                ],
                quote: Some(Quote {
                    id: Some(s("m0")),
                    author: s("You"),
                    text: s("Photo"),
                }),
                forwarded: true,
                edited: true,
                reactions: vec![Reaction {
                    emoji: s("❤️"),
                    count: 2,
                    mine: true,
                }],
                ticks: 3,
            }],
        },
        Event::Notify {
            chat: s("c1"),
            title: s("Ann"),
            body: s("hi"),
        },
        Event::Typing {
            chat: s("c1"),
            who: s("Ann"),
            on: true,
        },
        Event::Presence {
            who: s("u1"),
            online: false,
            last_seen: Some(1_700_000_000_000),
        },
        Event::Fetched {
            key: s("img:m1"),
            form: FetchForm::Original,
            result: Fetched::Done {
                path: "/files/m1.jpg".into(),
                delay_ms: None,
            },
        },
        Event::Fetched {
            key: s("gif:m3"),
            form: FetchForm::Frames,
            result: Fetched::Done {
                path: "/files/m3".into(),
                delay_ms: Some(40),
            },
        },
        Event::Fetched {
            key: s("img:m9"),
            form: FetchForm::Original,
            result: Fetched::Failed { error: s("gone") },
        },
        Event::Contacts {
            query: s("ann"),
            contacts: vec![Contact {
                id: s("u1"),
                name: s("Ann"),
                detail: s("+27 82 123 4567"),
            }],
        },
        Event::Members {
            section: s("g1"),
            rows: vec![
                MemberRow::Group {
                    name: s("Online"),
                    count: 1,
                },
                MemberRow::Person {
                    id: s("u1"),
                    name: s("Ann"),
                    avatar: None,
                    status: Some(s("idle")),
                },
            ],
        },
        Event::Call {
            call: Some(Call {
                id: s("k1"),
                chat: s("c1"),
                name: s("Ann"),
                phase: Phase::Active,
                video: true,
                group: false,
                muted: true,
                camera: true,
                screen: false,
            }),
        },
        Event::Call { call: None },
        Event::Voice(Voice {
            room: Some(s("v1")),
            room_name: s("Lounge"),
            connected: true,
            muted: false,
            deafened: false,
            camera: false,
            screen: true,
            members: vec![member],
            streams: vec![Stream {
                id: s("u1"),
                name: s("Ann"),
                watching: true,
            }],
            ringing: vec![Ring {
                room: s("dm9"),
                name: s("Bob"),
            }],
        }),
        Event::Video {
            track: s("u1"),
            local: false,
            jpeg: s("/9j/"),
        },
        Event::VideoEnded { track: s("u1") },
        Event::Sources {
            cameras: vec![Source {
                id: s("cam0"),
                name: s("Webcam"),
                thumbnail: None,
            }],
            screens: vec![Source {
                id: s("screen:0"),
                name: s("Screen 1"),
                thumbnail: Some(s("/9j/")),
            }],
        },
    ]
}

#[test]
fn every_command_round_trips() {
    for (n, body) in every_cmd().into_iter().enumerate() {
        let cmd = Command { id: n as u64, body };
        let line = cmd.to_line();
        assert!(!line.contains('\n'), "{line}");
        assert_eq!(Command::from_line(&line).as_ref(), Some(&cmd), "{line}");
    }
}

#[test]
fn every_event_round_trips() {
    for event in every_event() {
        let line = event.to_line();
        assert!(!line.contains('\n'), "{line}");
        assert_eq!(Event::from_line(&line).as_ref(), Some(&event), "{line}");
    }
}

#[test]
fn lines_look_like_the_spec() {
    let cmd = Command {
        id: 7,
        body: Cmd::Send {
            chat: "c1".into(),
            text: "hi".into(),
            reply_to: None,
        },
    };
    assert_eq!(
        cmd.to_line(),
        r#"{"id":7,"cmd":"send","chat":"c1","text":"hi"}"#
    );
    assert_eq!(
        Command {
            id: 1,
            body: Cmd::Ping
        }
        .to_line(),
        r#"{"id":1,"cmd":"ping"}"#
    );
    let reply = Event::Reply {
        id: 7,
        ok: true,
        error: None,
    };
    assert_eq!(reply.to_line(), r#"{"event":"reply","id":7,"ok":true}"#);
}

#[test]
fn unknown_fields_are_ignored() {
    let line = r#"{"event":"notify","chat":"c1","title":"Ann","body":"hi","priority":"high","sound":"ding"}"#;
    assert_eq!(
        Event::from_line(line),
        Some(Event::Notify {
            chat: "c1".into(),
            title: "Ann".into(),
            body: "hi".into()
        })
    );
    let line = r#"{"id":3,"cmd":"typing","chat":"c1","on":true,"until_ms":5000}"#;
    assert_eq!(
        Command::from_line(line).map(|c| c.body),
        Some(Cmd::Typing {
            chat: "c1".into(),
            on: true
        })
    );
}

#[test]
fn what_it_doesnt_understand_is_none_not_a_panic() {
    for line in [
        r#"{"event":"hologram","depth":3}"#,
        r#"{"event":"reply"}"#,
        r#"{"cmd":"teleport","id":1}"#,
        "not json at all",
        "",
        "{",
        r#"[1,2,3]"#,
        r#"{"event":"messages","chat":"c1","messages":"nope"}"#,
    ] {
        assert_eq!(Event::from_line(line), None, "{line}");
    }
    assert_eq!(Command::from_line(r#"{"cmd":"teleport","id":1}"#), None);
    // A command without its id is refused: replies couldn't find it.
    assert_eq!(Command::from_line(r#"{"cmd":"ping"}"#), None);
}

#[test]
fn defaults_fill_what_an_integration_leaves_out() {
    let line = r##"{"event":"hello","protocol":1,"name":"Tiny","tile":{"icon_svg":"<svg/>","tag":"ti","color":"#000000"}}"##;
    let Some(Event::Hello(back)) = Event::from_line(line) else {
        panic!("{line}")
    };
    assert_eq!(back.supports, Supports::default());
    let line = r#"{"event":"messages","chat":"c1","messages":[{"id":"m1","author_id":"u1","author":"Ann","at_ms":5}]}"#;
    let Some(Event::Messages { messages, .. }) = Event::from_line(line) else {
        panic!("{line}")
    };
    assert_eq!(messages[0].ticks, 0);
    assert!(messages[0].media.is_empty() && !messages[0].mine);
}

#[test]
fn versions() {
    assert_eq!(check_version(PROTOCOL), Ok(()));
    let newer = check_version(PROTOCOL + 1).unwrap_err();
    assert_eq!(
        newer,
        VersionError::Newer {
            theirs: PROTOCOL + 1,
            ours: PROTOCOL
        }
    );
    assert!(newer.to_string().contains("needs a newer Aflos"), "{newer}");
    let older = check_version(0).unwrap_err();
    assert!(older.to_string().contains("too old"), "{older}");
}

/// Each variant's place in `every_cmd`/`every_event`. No wildcard: a new
/// variant doesn't compile until it's listed here, and the tests below fail
/// until it's in the round-trip lists too.
fn cmd_variant(c: &Cmd) -> usize {
    match c {
        Cmd::Hello(_) => 0,
        Cmd::Ping => 1,
        Cmd::Quit => 2,
        Cmd::SignIn { .. } => 3,
        Cmd::SignInCode { .. } => 4,
        Cmd::SignOut => 5,
        Cmd::Open { .. } => 6,
        Cmd::Send { .. } => 7,
        Cmd::SendFile { .. } => 8,
        Cmd::React { .. } => 9,
        Cmd::Edit { .. } => 10,
        Cmd::Delete { .. } => 11,
        Cmd::Forward { .. } => 12,
        Cmd::Typing { .. } => 13,
        Cmd::Fetch { .. } => 14,
        Cmd::Search { .. } => 15,
        Cmd::StartChat { .. } => 16,
        Cmd::Archive { .. } => 17,
        Cmd::DeleteChat { .. } => 18,
        Cmd::JoinByLink { .. } => 19,
        Cmd::Members { .. } => 20,
        Cmd::CallStart { .. } => 21,
        Cmd::CallAccept { .. } => 22,
        Cmd::CallDecline => 23,
        Cmd::CallEnd => 24,
        Cmd::Join { .. } => 25,
        Cmd::Leave => 26,
        Cmd::Mute { .. } => 27,
        Cmd::Deafen { .. } => 28,
        Cmd::Camera { .. } => 29,
        Cmd::Screen { .. } => 30,
        Cmd::Sources => 31,
        Cmd::Watch { .. } => 32,
        Cmd::AudioDevices { .. } => 33,
        Cmd::Setting { .. } => 34,
    }
}
const CMD_VARIANTS: usize = 35;

fn event_variant(e: &Event) -> usize {
    match e {
        Event::Hello(_) => 0,
        Event::Reply { .. } => 1,
        Event::Status { .. } => 2,
        Event::SignIn(_) => 3,
        Event::Chats(_) => 4,
        Event::Messages { .. } => 5,
        Event::Notify { .. } => 6,
        Event::Typing { .. } => 7,
        Event::Presence { .. } => 8,
        Event::Fetched { .. } => 9,
        Event::Contacts { .. } => 10,
        Event::Members { .. } => 11,
        Event::Call { .. } => 12,
        Event::Voice(_) => 13,
        Event::Video { .. } => 14,
        Event::VideoEnded { .. } => 15,
        Event::Sources { .. } => 16,
    }
}
const EVENT_VARIANTS: usize = 17;

#[test]
fn the_round_trip_lists_have_every_variant() {
    let cmds: std::collections::BTreeSet<usize> = every_cmd().iter().map(cmd_variant).collect();
    assert_eq!(
        cmds.len(),
        CMD_VARIANTS,
        "missing commands: {:?}",
        (0..CMD_VARIANTS)
            .filter(|i| !cmds.contains(i))
            .collect::<Vec<_>>()
    );
    let events: std::collections::BTreeSet<usize> =
        every_event().iter().map(event_variant).collect();
    assert_eq!(
        events.len(),
        EVENT_VARIANTS,
        "missing events: {:?}",
        (0..EVENT_VARIANTS)
            .filter(|i| !events.contains(i))
            .collect::<Vec<_>>()
    );
}
