//! Connects to the running engine and prints the first snapshot; or accepts
//! an invite and prints how it went; or (`read:<channel id>`) prints a
//! channel's latest messages.
//! `cargo run -p aflos-discord --example probe [socket path | host:port] [invite | read:<id>]`
#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let socket = args.next().map(std::path::PathBuf::from).unwrap_or_else(|| {
        std::path::PathBuf::from(std::env::var("XDG_RUNTIME_DIR").unwrap_or_default()).join("discord-engine.sock")
    });
    let invite = args.next();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(aflos_discord::run(socket, tx));
    while let Some(event) = rx.recv().await {
        match event {
            aflos_discord::Event::Connected(engine) => {
                println!("connected");
                match invite.as_deref().map(|a| a.strip_prefix("read:")) {
                    Some(Some(channel)) => engine.open_channel(channel),
                    Some(None) => engine.accept_invite(invite.as_deref().unwrap_or_default()),
                    None => {}
                }
            }
            aflos_discord::Event::Snapshot(s) if invite.is_none() => {
                println!("me={:?} voice={:?} guilds={}", s.me.map(|m| m.name), s.voice.channel_id, s.guilds.len());
                return;
            }
            aflos_discord::Event::Snapshot(_) => {}
            aflos_discord::Event::Qr(_) => println!("logged out: showing a QR code"),
            aflos_discord::Event::Video(_) => {}
            aflos_discord::Event::SignIn(result) => println!("sign-in: {result:?}"),
            aflos_discord::Event::DmOpened { id, .. } => println!("DM opened: {id}"),
            aflos_discord::Event::Invite(result) => {
                println!("invite: {result:?}");
                if result != aflos_discord::Invite::Waiting {
                    return;
                }
            }
            aflos_discord::Event::Messages(m) => {
                for msg in &m.messages {
                    println!("{} {}: {}", msg.timestamp_ms, msg.author, msg.content);
                }
                return;
            }
            aflos_discord::Event::Notify(n) => println!("notify: {}: {}", n.title, n.body),
            aflos_discord::Event::Media { url, bytes } => println!("media {url}: {:?}", bytes.map(|b| b.len())),
            aflos_discord::Event::Disconnected => println!("disconnected"),
        }
    }
}
