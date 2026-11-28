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
