//! An integration's chats in Aflos's panes (PLAN-integrations Task 6): its
//! messages as timeline rows, its media through the files folder, and what
//! its events change on screen.
//!
//! Its media are web media keyed `int:<slot>:<form>:<key>` (form `o` the file
//! as it is, `p` a voice note's samples, `f` a GIF's frames), so the
//! timeline's loading, caching and drawing work as they do for WhatsApp.

use crate::{
    App, Clients, Message, MessageKind, Player, Screen, TimelineMessage, cache, load_web_media,
    notify, open_url, play_voice, show_frames, sounds, speaker, web_media_loaded,
};
use aflos_core::OwnedUserId;
use aflos_core::{MediaRef, Network};
use aflos_integration::{Cmd, Event, FetchForm, Fetched, MediaKind};
use iced::Task;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// A web media key for one of its media in `form`.
pub(crate) fn url(network: Network, form: FetchForm, key: &str) -> String {
    let Network::Integration(slot) = network else {
        return String::new();
    };
    let form = match form {
        FetchForm::Original => 'o',
        FetchForm::Pcm => 'p',
        FetchForm::Frames => 'f',
    };
    format!("int:{slot}:{form}:{key}")
}

/// The integration, form and key a [`url`] names.
pub(crate) fn parse(url: &str) -> Option<(Network, FetchForm, &str)> {
    let mut parts = url.strip_prefix("int:")?.splitn(3, ':');
    let slot = parts.next()?.parse().ok()?;
    let form = match parts.next()? {
        "o" => FetchForm::Original,
        "p" => FetchForm::Pcm,
        "f" => FetchForm::Frames,
        _ => return None,
    };
    Some((Network::Integration(slot), form, parts.next()?))
}

/// Asks the integration behind `url` for it.
pub(crate) fn fetch(clients: &Clients, url: &str) {
    if let Some((network, form, key)) = parse(url) {
        clients.integrations.send(
            network,
            Cmd::Fetch {
                key: key.to_owned(),
                form,
            },
        );
    }
}

/// Its messages as timeline rows: media first (one row each), then the
/// text; the quote above the first row, the reactions under the last.
pub(crate) fn timeline(
    network: Network,
    messages: &[aflos_integration::Message],
) -> Vec<TimelineMessage> {
    let Network::Integration(slot) = network else {
        return Vec::new();
    };
    let mut timeline = Vec::new();
    for m in messages {
        let local: String = m
            .author_id
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        let local = if local.is_empty() {
            "someone".to_string()
        } else {
            local
        };
        let Ok(sender) = OwnedUserId::try_from(format!("@int{slot}_{local}:integration.aflos"))
        else {
            continue;
        };
        let row = |body: String, kind| TimelineMessage {
            event_id: None,
            sender: sender.clone(),
            sender_name: m.author.clone(),
            sender_avatar: m
                .avatar
                .as_deref()
                .map(|k| MediaRef::web(url(network, FetchForm::Original, k))),
            sender_network: network,
            body,
            kind,
            reactions: Vec::new(),
            timestamp_ms: m.at_ms,
            web_id: Some(m.id.clone()),
            reply_to: None,
            forwarded: m.forwarded,
        };
        let first = timeline.len();
        if m.system {
            timeline.push(row(format!("{} {}", m.author, m.text), MessageKind::System));
            continue;
        }
        for media in &m.media {
            timeline.push(match media.kind {
                MediaKind::Image | MediaKind::Sticker | MediaKind::Gif => {
                    let (width, height, label) = match media.kind {
                        // Stickers are drawn 160 px wide, as WhatsApp's are.
                        MediaKind::Sticker => {
                            let scale = |v: Option<u64>| {
                                v.map(|v| v * 160 / media.width.unwrap_or(160).max(1))
                            };
                            (Some(160), scale(media.height), "sticker")
                        }
                        MediaKind::Gif => (media.width, media.height, "GIF"),
                        _ => (media.width, media.height, "photo"),
                    };
                    let form = if media.kind == MediaKind::Gif {
                        FetchForm::Frames
                    } else {
                        FetchForm::Original
                    };
                    let info = aflos_core::ImageInfo {
                        media: MediaRef::web(url(network, form, &media.key)),
                        width,
                        height,
                        size: media.size,
                        full: None,
                    };
                    row(label.to_string(), MessageKind::Image(info))
                }
                _ => {
                    let kind = match media.kind {
                        MediaKind::Video => "Video",
                        MediaKind::Voice => "Voice note",
                        MediaKind::Audio => "Audio",
                        _ => "Document",
                    };
                    let info = aflos_core::FileInfo {
                        media: MediaRef::web(url(network, FetchForm::Original, &media.key)),
                        name: media.name.clone(),
                        kind: kind.to_string(),
                        size: media.size,
                        duration: media.duration,
                        waveform: media.waveform.clone(),
                        thumbnail: media.preview.as_deref().map(|k| aflos_core::ImageInfo {
                            media: MediaRef::web(url(network, FetchForm::Original, k)),
                            width: media.width,
                            height: media.height,
                            size: None,
                            full: None,
                        }),
                    };
                    row(media.name.clone(), MessageKind::File(info))
                }
            });
        }
        if !m.text.is_empty() {
            timeline.push(row(m.text.clone(), MessageKind::Text));
        }
        if let Some(q) = &m.quote
            && let Some(row) = timeline.get_mut(first)
        {
            row.reply_to = Some(aflos_core::Quote {
                author: q.author.clone(),
                text: q.text.clone(),
            });
        }
        if timeline.len() > first
            && let Some(row) = timeline.last_mut()
        {
            row.reactions = m
                .reactions
                .iter()
                .map(|r| aflos_core::Reaction {
                    key: r.emoji.clone(),
                    count: r.count as usize,
                    mine: None,
                    by_me: r.mine,
                })
                .collect();
        }
    }
    timeline
}

/// Our messages, and the marks beside their times: ticks for ours,
/// "edited" for an edited one.
pub(crate) fn marks(
    messages: &[aflos_integration::Message],
) -> (HashSet<String>, HashMap<String, String>) {
    let mine = messages
        .iter()
        .filter(|m| m.mine)
        .map(|m| m.id.clone())
        .collect();
    let marks = messages
        .iter()
        .filter_map(|m| {
            let mark = match (m.mine, m.ticks, m.edited) {
                (true, 1, _) => "✓ sent",
                (true, 2, _) => "✓✓ delivered",
                (true, 3, _) => "✓✓ read",
                (_, _, true) => "edited",
                _ => return None,
            };
            Some((m.id.clone(), mark.to_string()))
        })
        .collect();
    (mine, marks)
}

/// What an event changes on screen, beyond what `Integrations::apply` keeps.
pub(crate) fn on_event(state: &mut App, network: Network, event: &Event) -> Task<Message> {
    super::calls::on_event(state, network, event);
    let name = network.spec().name;
    match event {
        Event::Messages { chat, messages } => {
            if let Screen::LoggedIn(session) = &mut state.screen
                && session
                    .web_chat
                    .as_ref()
                    .is_some_and(|c| c.network == network && c.id == *chat)
            {
                (session.mesh_mine, session.web_marks) = marks(messages);
                session.timeline = timeline(network, messages);
                let clients = Clients {
                    discord: state.discord.as_ref(),
                    whatsapp: state.whatsapp.as_ref(),
                    mesh: state.mesh.node.as_ref(),
                    spaces: &state.mesh.spaces,
                    integrations: &state.integrations,
                };
                load_web_media(session, &clients);
            }
        }
        Event::Notify { chat, title, body } => {
            let open = match &state.screen {
                Screen::LoggedIn(session) => session
                    .web_chat
                    .as_ref()
                    .is_some_and(|c| c.network == network && c.id == *chat),
                Screen::Booting => false,
            };
            let quiet = super::chat(state, network, chat).is_some_and(|c| c.muted || c.archived);
            if !(state.window_focused && open) && !quiet {
                sounds::play(
                    sounds::Sound::Message(network),
                    &state.sounds,
                    speaker(state),
                );
                notify(title.clone(), body.clone());
            }
        }
        Event::Fetched { key, form, result } => return fetched(state, network, key, *form, result),
        Event::Reply {
            ok: false,
            error: Some(error),
            ..
        } => {
            if let Screen::LoggedIn(session) = &mut state.screen {
                session.status = Some(format!("{name}: {error}"));
            }
        }
        _ => {}
    }
    Task::none()
}

/// A file it wrote for a fetch: shown, played, or opened.
fn fetched(
    state: &mut App,
    network: Network,
    key: &str,
    form: FetchForm,
    result: &Fetched,
) -> Task<Message> {
    let url = url(network, form, key);
    let opening = state.integrations.opening.remove(&url);
    let Screen::LoggedIn(session) = &mut state.screen else {
        return Task::none();
    };
    let path = match result {
        Fetched::Done { path, .. } => path,
        Fetched::Failed { error } => {
            match form {
                FetchForm::Original if !opening => {
                    return web_media_loaded(state, url, Err(error.clone()));
                }
                FetchForm::Frames => drop(session.images.remove(&url)),
                FetchForm::Pcm if session.player.as_ref().is_some_and(|pl| pl.loading) => {
                    session.player = None
                }
                _ => {}
            }
            session.status = Some(format!("Couldn't get that: {error}"));
            return Task::none();
        }
    };
    if opening {
        session.status = open_url(&path.to_string_lossy())
            .err()
            .map(|e| format!("Couldn't open that: {e}"));
        return Task::none();
    }
    match form {
        FetchForm::Original => {
            let bytes = std::fs::read(path).map_err(|e| e.to_string());
            web_media_loaded(state, url, bytes)
        }
        FetchForm::Pcm => {
            let voice = self::url(network, FetchForm::Original, key);
            let waiting = session
                .player
                .as_ref()
                .is_some_and(|pl| pl.key == voice && pl.loading);
            match std::fs::read(path) {
                Ok(bytes) => {
                    let samples = bytes
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|b| i16::from_le_bytes(*b))
                        .collect();
                    session.voice_pcm.insert(voice.clone(), Arc::new(samples));
                    if waiting {
                        return play_voice(session, voice, 0);
                    }
                }
                Err(e) => {
                    session.status = Some(format!("Couldn't play that voice note: {e}"));
                    if waiting {
                        session.player = None;
                    }
                }
            }
            Task::none()
        }
        FetchForm::Frames => {
            let delay = match result {
                Fetched::Done { delay_ms, .. } => delay_ms.unwrap_or(100),
                Fetched::Failed { .. } => 100,
            };
            let mut files: Vec<_> = std::fs::read_dir(path)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .collect();
            files.sort();
            let frames: Vec<Vec<u8>> = files.iter().filter_map(|f| std::fs::read(f).ok()).collect();
            if frames.is_empty() {
                session.images.remove(&url);
            } else {
                cache::put(
                    cache::Kind::Media,
                    &url,
                    &crate::pack_frames(delay, &frames),
                );
                show_frames(
                    &mut session.images,
                    &mut session.animations,
                    &url,
                    delay,
                    frames,
                );
            }
            Task::none()
        }
    }
}

/// Sends picked files to an integration's chat, through its files folder,
/// the caption with the first.
pub(crate) fn send_files(
    session: &mut crate::Session,
    all: &super::Integrations,
    chat: &crate::WebChat,
    first: crate::Attachment,
    caption: String,
) {
    let sends = session
        .int_actions
        .get(&chat.network)
        .is_some_and(|a| a.contains(&aflos_integration::Action::SendFiles));
    let dir = all
        .by_network(chat.network)
        .map(|(e, _)| super::dir(&e.id).join("files"));
    let Some(dir) = dir.filter(|_| sends) else {
        session.status = Some(format!("{} can't send files.", chat.network.spec().name));
        return;
    };
    let rest = std::mem::take(&mut session.queued_attachments);
    for (n, file) in std::iter::once(first).chain(rest).enumerate() {
        let name = crate::safe_filename(&file.filename);
        let path = dir.join(format!("out-{}-{n}-{name}", std::process::id()));
        if let Err(err) = std::fs::write(&path, &file.data) {
            session.status = Some(format!("Couldn't send {}: {err}", file.filename));
            continue;
        }
        let caption = if n == 0 {
            caption.clone()
        } else {
            String::new()
        };
        all.send(
            chat.network,
            Cmd::SendFile {
                chat: chat.id.clone(),
                path,
                caption,
            },
        );
    }
    session.pending_attachment = None;
    session.message_draft = iced::widget::text_editor::Content::new();
}

/// Sends the edit being made to our message in an integration's chat;
/// false if the edit isn't in one.
pub(crate) fn edit(session: &mut crate::Session, all: &super::Integrations, body: &str) -> bool {
    // Only take the edit when it's theirs: an Aflos chat's goes on below.
    let Some(network @ Network::Integration(_)) = session.web_chat.as_ref().map(|c| c.network)
    else {
        return false;
    };
    let Some(id) = session.editing.take() else {
        return false;
    };
    all.send(
        network,
        Cmd::Edit {
            message: id,
            text: body.to_owned(),
        },
    );
    session.message_draft = iced::widget::text_editor::Content::new();
    true
}

/// Starts a voice note's samples loading, for the player.
pub(crate) fn want_voice(state: &mut App, key: String) {
    let Some((network, _, media)) = parse(&key) else {
        return;
    };
    state.integrations.send(
        network,
        Cmd::Fetch {
            key: media.to_owned(),
            form: FetchForm::Pcm,
        },
    );
    if let Screen::LoggedIn(session) = &mut state.screen {
        session.player = Some(Player {
            key,
            position: 0,
            out: None,
            loading: true,
        });
    }
}

/// Opens one of its files: fetched, then opened where it was written.
pub(crate) fn open(state: &mut App, url: String) {
    let Some((network, _, key)) = parse(&url) else {
        return;
    };
    state.integrations.send(
        network,
        Cmd::Fetch {
            key: key.to_owned(),
            form: FetchForm::Original,
        },
    );
    state.integrations.opening.insert(url);
    if let Screen::LoggedIn(session) = &mut state.screen {
        session.status = Some("Opening…".to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aflos_integration::{Media, Quote, Reaction};

    #[test]
    fn urls_round_trip_with_colons_in_the_key() {
        let n = Network::Integration(4);
        for form in [FetchForm::Original, FetchForm::Pcm, FetchForm::Frames] {
            let u = url(n, form, "media:abc:1");
            assert_eq!(parse(&u), Some((n, form, "media:abc:1")), "{u}");
        }
        assert_eq!(parse("wa-file:1"), None);
        assert_eq!(parse("int:x:o:k"), None);
        assert_eq!(parse("int:1:z:k"), None);
    }

    #[test]
    fn messages_become_rows_media_first_quote_on_top_reactions_last() {
        let n = Network::Integration(2);
        let m = aflos_integration::Message {
            id: "m1".into(),
            author_id: "u-1".into(),
            author: "Ann".into(),
            avatar: Some("av".into()),
            text: "look at this".into(),
            at_ms: 5,
            media: vec![
                Media {
                    key: "img".into(),
                    kind: MediaKind::Image,
                    width: Some(640),
                    height: Some(480),
                    ..Media::default()
                },
                Media {
                    key: "vn".into(),
                    kind: MediaKind::Voice,
                    duration: Some(3.0),
                    waveform: vec![1, 2],
                    ..Media::default()
                },
            ],
            quote: Some(Quote {
                id: None,
                author: "You".into(),
                text: "Photo".into(),
            }),
            reactions: vec![Reaction {
                emoji: "👍".into(),
                count: 2,
                mine: true,
            }],
            ..aflos_integration::Message::default()
        };
        let rows = timeline(n, &[m]);
        assert_eq!(rows.len(), 3);
        assert!(matches!(&rows[0].kind, MessageKind::Image(i) if i.media.key() == "int:2:o:img"));
        assert!(
            matches!(&rows[1].kind, MessageKind::File(f) if f.kind == "Voice note" && f.media.key() == "int:2:o:vn")
        );
        assert!(matches!(rows[2].kind, MessageKind::Text));
        assert!(rows[0].reply_to.is_some() && rows[2].reply_to.is_none());
        assert!(rows[2].reactions.len() == 1 && rows[0].reactions.is_empty());
        assert!(
            rows.iter()
                .all(|r| r.web_id.as_deref() == Some("m1") && r.sender_network == n)
        );
        assert_eq!(
            rows[0].sender_avatar.as_ref().map(MediaRef::key).as_deref(),
            Some("int:2:o:av")
        );
    }

    #[test]
    fn a_gif_loads_as_frames_and_a_sticker_is_160_wide() {
        let n = Network::Integration(0);
        let media = |kind, w, h| Media {
            key: "k".into(),
            kind,
            width: Some(w),
            height: Some(h),
            ..Media::default()
        };
        let m = aflos_integration::Message {
            id: "m".into(),
            author_id: "a".into(),
            author: "A".into(),
            media: vec![
                media(MediaKind::Gif, 200, 100),
                media(MediaKind::Sticker, 512, 256),
            ],
            ..aflos_integration::Message::default()
        };
        let rows = timeline(n, &[m]);
        let MessageKind::Image(gif) = &rows[0].kind else {
            panic!()
        };
        assert_eq!(gif.media.key(), "int:0:f:k");
        let MessageKind::Image(sticker) = &rows[1].kind else {
            panic!()
        };
        assert_eq!((sticker.width, sticker.height), (Some(160), Some(80)));
    }

    #[test]
    fn ticks_for_ours_and_edited_for_theirs() {
        let m = |id: &str, mine, ticks, edited| aflos_integration::Message {
            id: id.into(),
            mine,
            ticks,
            edited,
            ..aflos_integration::Message::default()
        };
        let (mine, marks) = marks(&[
            m("a", true, 3, false),
            m("b", false, 0, true),
            m("c", false, 0, false),
            m("d", true, 0, false),
        ]);
        assert_eq!(mine, HashSet::from(["a".to_string(), "d".to_string()]));
        assert_eq!(marks.get("a").map(String::as_str), Some("✓✓ read"));
        assert_eq!(marks.get("b").map(String::as_str), Some("edited"));
        assert!(!marks.contains_key("c") && !marks.contains_key("d"));
    }
}
