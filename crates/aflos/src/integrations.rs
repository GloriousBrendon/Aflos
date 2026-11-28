//! Integrations (SPEC-integrations): the ones Aflos runs, what each has
//! said, and their rail tiles. Each runs under `aflos_integration::host`;
//! this keeps the latest of what it sent, for the panes to draw.

use crate::{App, Message, data_dir, theme, ui};
use aflos_core::Network;
use aflos_integration::host::{self, Report, Spec};
use aflos_integration::{Chats, Cmd, Event, Hello, HelloBack, PROTOCOL, SignIn, State};
use iced::widget::{column, container, text, tooltip};
use iced::{Element, Subscription, Task};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

/// One integration to run. For now they're listed in config.toml
/// (`[[integrations]]`); installed ones join them in PLAN Task 11.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct Entry {
    /// Its id (reverse-DNS, as in its manifest).
    pub id: String,
    pub program: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    /// Turned off: kept, not run.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub off: bool,
}

/// What one integration has said, kept for the panes.
#[derive(Debug, Default)]
pub(crate) struct Live {
    /// Its network, once it has said hello.
    pub network: Option<Network>,
    pub handle: Option<host::Handle>,
    pub hello: Option<HelloBack>,
    pub icon: Option<iced::widget::svg::Handle>,
    pub state: Option<State>,
    /// Why it's in that state, if it said.
    pub error: Option<String>,
    /// Restarting or stopped, and why (spec T7).
    pub trouble: Option<Trouble>,
    /// The sign-in step it's asking for.
    pub sign_in: Option<SignIn>,
    /// Why the last sign-in didn't work (the step stays up to try again).
    pub sign_in_error: Option<String>,
    pub chats: Chats,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Trouble {
    Restarting(String),
    Stopped(String),
}

#[derive(Debug, Default)]
pub(crate) struct Integrations {
    pub entries: Vec<Entry>,
    /// By id.
    pub live: HashMap<String, Live>,
    /// The sign-in form being filled in, by field name, and a code typed.
    pub form: BTreeMap<String, String>,
    pub code: String,
    /// Bumped by Restart, so its subscription starts over.
    pub generation: HashMap<String, u32>,
    /// Remove asked once; the next press removes it.
    pub confirm_remove: Option<String>,
}

impl Integrations {
    pub fn new(entries: Vec<Entry>) -> Self {
        Self {
            entries,
            ..Self::default()
        }
    }

    /// The running ones that have said hello, in list order.
    pub fn ready(&self) -> impl Iterator<Item = (&Entry, &Live, Network)> {
        self.entries.iter().filter_map(|e| {
            let live = self.live.get(&e.id)?;
            Some((e, live, live.network?))
        })
    }

    /// The integration behind `network`.
    pub fn by_network(&self, network: Network) -> Option<(&Entry, &Live)> {
        self.ready()
            .find(|(_, _, n)| *n == network)
            .map(|(e, l, _)| (e, l))
    }

    /// Sends a command to the integration behind `network`; its id, or
    /// `None` if it isn't running.
    pub fn send(&self, network: Network, cmd: Cmd) -> Option<u64> {
        let (_, live) = self.by_network(network)?;
        live.handle.as_ref().map(|h| h.send(cmd))
    }
}

/// What the host sends the app about one integration.
#[derive(Debug, Clone)]
pub(crate) enum Update {
    /// Running: the handle to command it.
    Started(host::Handle),
    Report(Report),
}

/// Its folders: `<data>/integrations/<id>/data` and `…/files`.
pub(crate) fn dir(id: &str) -> PathBuf {
    data_dir().join("integrations").join(id)
}

pub(crate) fn subscription(state: &App) -> Subscription<Message> {
    let all = &state.integrations;
    Subscription::batch(all.entries.iter().filter(|e| !e.off).map(|e| {
        let generation = all.generation.get(&e.id).copied().unwrap_or(0);
        Subscription::run_with((e.clone(), generation), events)
    }))
}

/// Runs one integration for as long as its subscription lives.
fn events((entry, _): &(Entry, u32)) -> impl iced::futures::Stream<Item = Message> + use<> {
    let entry = entry.clone();
    iced::stream::channel(
        64,
        async move |mut output: iced::futures::channel::mpsc::Sender<Message>| {
            use iced::futures::SinkExt;
            let dir = dir(&entry.id);
            let hello = Hello {
                protocol: PROTOCOL,
                aflos: env!("CARGO_PKG_VERSION").into(),
                data_dir: dir.join("data"),
                files_dir: dir.join("files"),
                granted: Vec::new(),
                settings: BTreeMap::new(),
            };
            let _ = std::fs::create_dir_all(&hello.files_dir);
            let mut spec = Spec::new(entry.id.clone(), entry.program.clone(), hello);
            spec.args = entry.args.clone();
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let handle = host::start(spec, tx);
            if output
                .send(Message::Integration(
                    entry.id.clone(),
                    Update::Started(handle),
                ))
                .await
                .is_err()
            {
                return;
            }
            while let Some((id, report)) = rx.recv().await {
                if output
                    .send(Message::Integration(id, Update::Report(report)))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        },
    )
}

pub(crate) fn update(state: &mut App, id: String, update: Update) -> Task<Message> {
    state.integrations.apply(id, update);
    Task::none()
}

impl Integrations {
    /// Keeps what an integration said.
    pub fn apply(&mut self, id: String, update: Update) {
        let live = self.live.entry(id.clone()).or_default();
        match update {
            Update::Started(handle) => live.handle = Some(handle),
            Update::Report(Report::Ready(back)) => {
                let slot = aflos_core::networks::register_integration(
                    &id,
                    &back.name,
                    &back.tile.tag,
                    &back.tile.color,
                );
                live.network = Some(Network::Integration(slot));
                live.icon = Some(iced::widget::svg::Handle::from_memory(
                    back.tile.icon_svg.clone().into_bytes(),
                ));
                live.trouble = None;
                live.hello = Some(back);
            }
            Update::Report(Report::Restarting { why, .. }) => {
                live.trouble = Some(Trouble::Restarting(why))
            }
            Update::Report(Report::Stopped { why }) => {
                live.trouble = Some(Trouble::Stopped(why));
                live.handle = None;
            }
            Update::Report(Report::Event(event)) => match event {
                Event::Status { state: s, error } => {
                    live.state = Some(s);
                    live.error = error;
                    if s == State::Online {
                        live.sign_in = None;
                        live.sign_in_error = None;
                    }
                }
                Event::SignIn(SignIn::Failed { error }) => live.sign_in_error = Some(error),
                Event::SignIn(step) => {
                    live.sign_in = Some(step);
                    live.sign_in_error = None;
                }
                Event::Chats(chats) => live.chats = chats,
                // The rest arrive with the panes that show them (PLAN Tasks 5 to 8).
                _ => {}
            },
        }
    }
}

/// What the Accounts panel asks of integrations.
#[derive(Debug, Clone)]
pub(crate) enum Act {
    /// A sign-in form field changed.
    Field(String, String),
    /// The sign-in code changed.
    Code(String),
    /// Send the form, or the code, to the integration behind it.
    Submit(Network),
    /// Start a stopped one again.
    Restart(String),
    /// Turn it off (kept, not run) or back on.
    Off(String, bool),
    /// Remove it: asks once, then does it.
    Remove(String),
}

pub(crate) fn act(state: &mut App, act: Act) -> Task<Message> {
    let all = &mut state.integrations;
    if !matches!(act, Act::Remove(_)) {
        all.confirm_remove = None;
    }
    match act {
        Act::Field(name, value) => drop(all.form.insert(name, value)),
        Act::Code(code) => all.code = code,
        Act::Submit(network) => {
            let step = all.by_network(network).and_then(|(_, l)| l.sign_in.clone());
            if let Some(cmd) = step.and_then(|s| sign_in_command(&s, &all.form, &all.code)) {
                all.send(network, cmd);
                all.code.clear();
            }
        }
        Act::Restart(id) => {
            *all.generation.entry(id.clone()).or_default() += 1;
            if let Some(live) = all.live.get_mut(&id) {
                live.trouble = None;
            }
        }
        Act::Off(id, off) => {
            all.set_off(&id, off);
            let entries = all.entries.clone();
            crate::persist(|c| c.integrations = entries);
        }
        Act::Remove(id) => {
            if all.confirm_remove.as_deref() != Some(id.as_str()) {
                all.confirm_remove = Some(id);
                return Task::none();
            }
            all.remove(&id);
            let entries = all.entries.clone();
            crate::persist(|c| c.integrations = entries);
            // Its folder and everything it kept go with it (spec T5).
            let _ = std::fs::remove_dir_all(dir(&id));
        }
    }
    Task::none()
}

impl Integrations {
    pub fn set_off(&mut self, id: &str, off: bool) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
            e.off = off;
        }
        if off {
            self.live.remove(id);
        }
    }

    pub fn remove(&mut self, id: &str) {
        self.entries.retain(|e| e.id != id);
        self.live.remove(id);
        self.confirm_remove = None;
    }
}

/// The command a sign-in step's answer is: the form's fields (only the ones
/// it asked for), or the code. `None` for steps the person finishes
/// elsewhere (a QR code, a web page).
pub(crate) fn sign_in_command(
    step: &SignIn,
    form: &BTreeMap<String, String>,
    code: &str,
) -> Option<Cmd> {
    match step {
        SignIn::Form { fields } => Some(Cmd::SignIn {
            values: fields
                .iter()
                .map(|f| {
                    (
                        f.name.clone(),
                        form.get(&f.name).cloned().unwrap_or_default(),
                    )
                })
                .collect(),
        }),
        SignIn::Code { .. } if !code.trim().is_empty() => Some(Cmd::SignInCode {
            code: code.trim().to_owned(),
        }),
        _ => None,
    }
}

/// One of its chats, by id.
pub(crate) fn chat<'a>(
    state: &'a App,
    network: Network,
    id: &str,
) -> Option<&'a aflos_integration::Chat> {
    let (_, live) = state.integrations.by_network(network)?;
    live.chats
        .sections
        .iter()
        .flat_map(|s| &s.chats)
        .find(|c| c.id == id)
}

/// Unread messages across its chats, muted ones left out.
pub(crate) fn unread(live: &Live) -> u64 {
    live.chats
        .sections
        .iter()
        .flat_map(|s| &s.chats)
        .filter(|c| !c.muted && !c.archived)
        .map(|c| c.unread.max(c.mentions))
        .sum()
}

/// Its logo in its colour (or `color`, for an inactive tile).
pub(crate) fn logo<'a>(live: &Live, size: f32, color: iced::Color) -> Element<'a, Message> {
    match &live.icon {
        Some(icon) => iced::widget::svg(icon.clone())
            .width(size)
            .height(size)
            .style(move |_, _| iced::widget::svg::Style { color: Some(color) })
            .into(),
        None => ui::icon(ui::Icon::Hash, size, color),
    }
}

/// Its folder's tiles: one per section that isn't Home's (a server, its
/// groups), or, until it's signed in, one tile to sign in from.
pub(crate) fn rail_tiles<'a>(
    active: Option<(Network, &str)>,
    live: &'a Live,
    network: Network,
    p: &theme::Palette,
    l: &theme::Layout,
) -> Vec<Element<'a, Message>> {
    let color = p.network(network.spec());
    let name = live.hello.as_ref().map_or("", |h| h.name.as_str());
    if live.state != Some(State::Online) {
        let why = match &live.trouble {
            Some(Trouble::Stopped(why) | Trouble::Restarting(why)) => format!("{name}: {why}"),
            None => format!("{name}: not signed in"),
        };
        let tile = crate::rail_tile(
            logo(live, 20.0, p.text_muted()),
            false,
            Some(Message::AccountsOpen(network.spec().id)),
            p,
            l,
        );
        return vec![
            tooltip(
                tile,
                container(text(why).size(12).color(p.text())).padding([4, 8]),
                tooltip::Position::Right,
            )
            .into(),
        ];
    }
    live.chats
        .sections
        .iter()
        .filter(|s| !s.home)
        .map(|section| {
            let active = active == Some((network, section.id.as_str()));
            let unread: u64 = section
                .chats
                .iter()
                .filter(|c| !c.muted)
                .map(|c| c.unread.max(c.mentions))
                .sum();
            let face = ui::mono_bold(
                crate::initials(&section.name),
                13.0,
                if active { color } else { p.text() },
            );
            let tile = crate::rail_tile(
                face.into(),
                active,
                Some(Message::IntegrationSection(network, section.id.clone())),
                p,
                l,
            );
            tooltip(
                column![crate::rail_badge(tile, unread, p)].align_x(iced::Alignment::Center),
                container(text(section.name.clone()).size(12).color(p.text())).padding([4, 8]),
                tooltip::Position::Right,
            )
            .into()
        })
        .collect()
}

mod panel;
pub(crate) use panel::panel;

#[cfg(test)]
mod tests;
