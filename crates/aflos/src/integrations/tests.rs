use super::*;
use aflos_integration::{Chat, Section, Supports, Tile};

fn hello(name: &str, tag: &str, color: &str) -> HelloBack {
    HelloBack {
        protocol: PROTOCOL,
        name: name.into(),
        tile: Tile {
            icon_svg: "<svg/>".into(),
            tag: tag.into(),
            color: color.into(),
        },
        supports: Supports::default(),
    }
}

fn chats() -> Chats {
    let chat = |id: &str, unread| Chat {
        id: id.into(),
        name: id.into(),
        unread,
        ..Chat::default()
    };
    Chats {
        me: "me".into(),
        sections: vec![
            Section {
                id: "direct".into(),
                name: "Direct".into(),
                home: true,
                chats: vec![chat("ann", 2)],
                ..Section::default()
            },
            Section {
                id: "band".into(),
                name: "Band".into(),
                chats: vec![chat("general", 3)],
                ..Section::default()
            },
            Section {
                id: "work".into(),
                name: "Work".into(),
                chats: vec![chat("standup", 0)],
                ..Section::default()
            },
        ],
    }
}

fn event(e: Event) -> Update {
    Update::Report(Report::Event(e))
}

#[test]
fn hello_registers_its_network_with_its_tag_and_colour() {
    let mut all = Integrations::new(vec![]);
    all.apply(
        "test.int.tile".into(),
        Update::Report(Report::Ready(hello("Tiles", "ti", "#3fa7d6"))),
    );
    let live = &all.live["test.int.tile"];
    let network = live.network.expect("a network after hello");
    assert!(matches!(network, Network::Integration(_)));
    assert_eq!(network.tag(), "ti");
    assert_eq!(network.spec().name, "Tiles");
    assert_eq!(network.spec().color, "#3fa7d6");
    assert!(live.icon.is_some());
}

#[test]
fn signed_out_it_gets_one_tile_online_one_per_section_outside_home() {
    let (p, l) = (theme::Palette::default(), theme::Layout::default());
    let mut all = Integrations::new(vec![]);
    let id = "test.int.rail".to_string();
    all.apply(
        id.clone(),
        Update::Report(Report::Ready(hello("Rail", "ra", "#112233"))),
    );
    all.apply(
        id.clone(),
        event(Event::Status {
            state: State::SignedOut,
            error: None,
        }),
    );
    all.apply(id.clone(), event(Event::Chats(chats())));
    let live = &all.live[&id];
    let network = live.network.unwrap();
    assert_eq!(
        rail_tiles(None, live, network, &p, &l).len(),
        1,
        "signed out: one tile to sign in from"
    );
    all.apply(
        id.clone(),
        event(Event::Status {
            state: State::Online,
            error: None,
        }),
    );
    let live = &all.live[&id];
    assert_eq!(
        rail_tiles(None, live, network, &p, &l).len(),
        2,
        "Band and Work; Direct goes to Home"
    );
    assert_eq!(unread(live), 5);
}

#[test]
fn trouble_is_kept_and_cleared_by_the_next_hello() {
    let mut all = Integrations::new(vec![]);
    let id = "test.int.trouble".to_string();
    all.apply(
        id.clone(),
        Update::Report(Report::Restarting {
            attempt: 1,
            why: "it crashed".into(),
        }),
    );
    assert_eq!(
        all.live[&id].trouble,
        Some(Trouble::Restarting("it crashed".into()))
    );
    all.apply(
        id.clone(),
        Update::Report(Report::Ready(hello("Back", "bk", "#000000"))),
    );
    assert_eq!(all.live[&id].trouble, None);
    all.apply(
        id.clone(),
        Update::Report(Report::Stopped {
            why: "it crashed (restarted 3 times)".into(),
        }),
    );
    assert_eq!(
        all.live[&id].trouble,
        Some(Trouble::Stopped("it crashed (restarted 3 times)".into()))
    );
}

#[test]
fn signing_in_clears_the_sign_in_step() {
    let mut all = Integrations::new(vec![]);
    let id = "test.int.signin".to_string();
    all.apply(
        id.clone(),
        event(Event::SignIn(SignIn::Code {
            prompt: "Code?".into(),
        })),
    );
    assert!(all.live[&id].sign_in.is_some());
    all.apply(
        id.clone(),
        event(Event::Status {
            state: State::Online,
            error: None,
        }),
    );
    assert!(all.live[&id].sign_in.is_none());
}

#[test]
fn each_sign_in_step_sends_its_answer() {
    use aflos_integration::Field;
    let form: BTreeMap<String, String> = [
        ("login".to_string(), "ann@example.com".to_string()),
        ("password".to_string(), "pw".to_string()),
        ("stray".to_string(), "not asked for".to_string()),
    ]
    .into();
    let fields = vec![
        Field {
            name: "login".into(),
            label: "Email".into(),
            secret: false,
        },
        Field {
            name: "password".into(),
            label: "Password".into(),
            secret: true,
        },
        Field {
            name: "otp".into(),
            label: "One-time code".into(),
            secret: false,
        },
    ];
    let Some(Cmd::SignIn { values }) = sign_in_command(&SignIn::Form { fields }, &form, "") else {
        panic!("a form sends sign_in")
    };
    // Only what it asked for; an empty field is sent empty, for it to judge.
    let want: BTreeMap<String, String> = [
        ("login", "ann@example.com"),
        ("password", "pw"),
        ("otp", ""),
    ]
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .into();
    assert_eq!(values, want);

    let code = SignIn::Code {
        prompt: "Code?".into(),
    };
    assert_eq!(
        sign_in_command(&code, &form, " 123456 "),
        Some(Cmd::SignInCode {
            code: "123456".into()
        })
    );
    assert_eq!(
        sign_in_command(&code, &form, "  "),
        None,
        "no code typed: nothing sent"
    );
    assert_eq!(
        sign_in_command(
            &SignIn::Qr {
                png: None,
                svg: None
            },
            &form,
            ""
        ),
        None
    );
    assert_eq!(
        sign_in_command(
            &SignIn::Web {
                url: "https://x".into()
            },
            &form,
            ""
        ),
        None
    );
}

#[test]
fn a_failed_sign_in_keeps_the_step_and_says_why() {
    let mut all = Integrations::new(vec![]);
    let id = "test.int.failed".to_string();
    let form = SignIn::Form { fields: vec![] };
    all.apply(id.clone(), event(Event::SignIn(form.clone())));
    all.apply(
        id.clone(),
        event(Event::SignIn(SignIn::Failed {
            error: "Wrong password".into(),
        })),
    );
    assert_eq!(all.live[&id].sign_in, Some(form.clone()));
    assert_eq!(
        all.live[&id].sign_in_error.as_deref(),
        Some("Wrong password")
    );
    // A new step clears the old error.
    all.apply(
        id.clone(),
        event(Event::SignIn(SignIn::Code {
            prompt: "Code?".into(),
        })),
    );
    assert_eq!(all.live[&id].sign_in_error, None);
}

#[test]
fn turning_off_and_removing() {
    let entry = |id: &str| Entry {
        id: id.into(),
        program: "/bin/true".into(),
        args: vec![],
        off: false,
    };
    let mut all = Integrations::new(vec![entry("a"), entry("b")]);
    all.apply(
        "a".into(),
        Update::Report(Report::Ready(hello("A", "aa", "#000000"))),
    );
    all.set_off("a", true);
    assert!(all.entries[0].off);
    assert!(
        !all.live.contains_key("a"),
        "what it said goes when it's off"
    );
    all.set_off("a", false);
    assert!(!all.entries[0].off);
    all.confirm_remove = Some("b".into());
    all.remove("b");
    assert_eq!(
        all.entries
            .iter()
            .map(|e| e.id.as_str())
            .collect::<Vec<_>>(),
        ["a"]
    );
    assert_eq!(all.confirm_remove, None);
}

#[test]
fn an_entry_round_trips_through_the_config() {
    let on = Entry {
        id: "org.example.chat".into(),
        program: "/opt/chat".into(),
        args: vec!["--quiet".into()],
        off: false,
    };
    let off = Entry {
        off: true,
        ..on.clone()
    };
    let text = toml::to_string(&Wrap {
        integrations: vec![on.clone(), off.clone()],
    })
    .unwrap();
    assert!(
        !text.contains("off = false"),
        "on is the default and isn't written: {text}"
    );
    let back: Wrap = toml::from_str(&text).unwrap();
    assert_eq!(back.integrations, [on, off]);

    #[derive(serde::Serialize, serde::Deserialize)]
    struct Wrap {
        integrations: Vec<Entry>,
    }
}

#[test]
fn calls_ring_and_ringback() {
    use crate::sounds::Sound;
    use aflos_integration::{Call, Phase, Ring, Voice};
    let entry = Entry {
        id: "test.int.calls".into(),
        program: "/bin/true".into(),
        args: vec![],
        off: false,
    };
    let mut all = Integrations::new(vec![entry]);
    let id = "test.int.calls".to_string();
    all.apply(
        id.clone(),
        Update::Report(Report::Ready(hello("Calls", "cl", "#123456"))),
    );
    let network = all.live[&id].network.unwrap();
    assert_eq!(calls::ring(&all), None);
    let call = |phase| Call {
        id: "k".into(),
        chat: "c".into(),
        name: "Ann".into(),
        phase,
        ..Call::default()
    };
    all.apply(
        id.clone(),
        event(Event::Call {
            call: Some(call(Phase::Outgoing)),
        }),
    );
    assert_eq!(calls::ring(&all), Some(Sound::Ringback));
    all.apply(
        id.clone(),
        event(Event::Call {
            call: Some(call(Phase::Incoming)),
        }),
    );
    assert_eq!(calls::ring(&all), Some(Sound::Ring(network)));
    // An ended call is gone, not kept.
    all.apply(
        id.clone(),
        event(Event::Call {
            call: Some(call(Phase::Ended)),
        }),
    );
    assert_eq!(all.live[&id].call, None);
    assert_eq!(calls::ring(&all), None);
    // A room ringing us rings until it's dismissed here.
    let ringing = |rooms: &[&str]| Voice {
        ringing: rooms
            .iter()
            .map(|r| Ring {
                room: r.to_string(),
                name: "Bob".into(),
            })
            .collect(),
        ..Voice::default()
    };
    all.apply(id.clone(), event(Event::Voice(ringing(&["r1"]))));
    assert_eq!(calls::ring(&all), Some(Sound::Ring(network)));
    all.live_mut(network).unwrap().dismissed.push("r1".into());
    assert_eq!(calls::ring(&all), None);
    // Once it stops ringing there, the dismissal is forgotten.
    all.apply(id.clone(), event(Event::Voice(ringing(&[]))));
    assert!(all.live[&id].dismissed.is_empty());
}

#[test]
fn screens_reach_the_picker_only_while_it_is_open() {
    use aflos_integration::Source;
    let mut all = Integrations::new(vec![]);
    let id = "test.int.share".to_string();
    let screens = vec![
        Source {
            id: "s0".into(),
            name: "Screen 1".into(),
            thumbnail: None,
        },
        Source {
            id: "w1".into(),
            name: "Firefox".into(),
            thumbnail: Some("not base64!".into()),
        },
    ];
    let sources = || {
        event(Event::Sources {
            cameras: vec![],
            screens: screens.clone(),
        })
    };
    all.apply(id.clone(), sources());
    assert!(
        all.live[&id].sources.is_none(),
        "a late answer after Cancel is dropped"
    );
    all.live.get_mut(&id).unwrap().sources = Some(Vec::new());
    all.apply(id.clone(), sources());
    let picked = all.live[&id].sources.as_ref().unwrap();
    assert_eq!(
        picked.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        ["Screen 1", "Firefox"]
    );
    assert!(
        picked[1].thumbnail.is_none(),
        "a bad thumbnail is left out, not fatal"
    );
}

#[test]
fn video_tracks_are_kept_per_integration() {
    assert_eq!(calls::track_prefix(Network::Integration(7)), "int-7-");
    assert_eq!(calls::track_prefix(Network::Matrix), "");
}
