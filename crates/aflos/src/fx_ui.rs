//! A person's own voice plugins (design/mesh/SPEC-voice-quality.md, F0): a
//! collapsed "Advanced: plugins" section under Voice & audio's built-in
//! controls. Empty by default, and nothing extra runs while it is: the
//! built-in chain stays as it is either way. The chain lives in config.toml
//! and goes to mesh-node at start and on every change.

use super::*;
use iced::widget::{column, pick_list, row};
use serde::{Deserialize, Serialize};

/// One plugin in the chain, as config.toml keeps it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Plugin {
    pub path: String,
    pub id: String,
    pub name: String,
    pub format: String,
    pub on: bool,
}

/// What the scan found, to add from.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Found {
    pub path: String,
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub format: String,
}

impl std::fmt::Display for Found {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let vendor = if self.vendor.is_empty() {
            String::new()
        } else {
            format!(" · {}", self.vendor)
        };
        write!(f, "{} ({}{vendor})", self.name, self.format.to_uppercase())
    }
}

#[derive(Default)]
pub(crate) struct FxUi {
    pub chain: Vec<Plugin>,
    pub dirs: Vec<String>,
    pub open: bool,
    found: Vec<Found>,
    broken: usize,
    pick: Option<Found>,
    /// The plugins' last `meshFx` state (`starting`, `on`, `bypassed`, …).
    status: Option<serde_json::Value>,
    /// A plugin's parameters while they're shown: its place, and each
    /// `{id, name, min, max, value}`.
    params: Option<(usize, Vec<serde_json::Value>)>,
    note: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) enum Msg {
    Toggle,
    Rescan,
    AddFolder,
    FolderPicked(Option<PathBuf>),
    Pick(Found),
    Add,
    Move(usize, bool),
    On(usize, bool),
    Remove(usize),
    Edit(usize),
    Params(usize),
    /// A parameter's new value while dragging, then let go.
    Param(usize, u64, f64),
    ParamDone(usize, u64),
    Restart,
}

fn to(m: Msg) -> Message {
    Message::Fx(m)
}

/// The chain as `fxChain` takes it.
pub(crate) fn chain_json(chain: &[Plugin]) -> String {
    let list: Vec<serde_json::Value> = (chain.iter())
        .map(|p| serde_json::json!({ "path": p.path, "id": p.id, "on": p.on }))
        .collect();
    serde_json::Value::Array(list).to_string()
}

/// Sends the chain and the folders to scan: at start, and after a change.
pub(crate) fn send(state: &App) {
    if let Some(node) = &state.mesh.node {
        let fx = &state.mesh.fx;
        node.fx_chain(&chain_json(&fx.chain));
        if fx.open {
            node.fx_scan(&serde_json::json!({ "dirs": fx.dirs }).to_string());
        }
    }
}

fn save(fx: &FxUi) {
    let (chain, dirs) = (fx.chain.clone(), fx.dirs.clone());
    if let Err(err) = theme::update_config(&config_path(), move |c| {
        (c.fx_plugins, c.fx_dirs) = (chain, dirs)
    }) {
        tracing::warn!("config: {err}");
    }
}

pub(crate) fn event(state: &mut App, event: mesh::Event) {
    let fx = &mut state.mesh.fx;
    match event {
        mesh::Event::FxPlugins(v) => {
            let text =
                |p: &serde_json::Value, k: &str| p[k].as_str().unwrap_or_default().to_owned();
            fx.found = (v["plugins"].as_array().into_iter().flatten())
                .map(|p| Found {
                    path: text(p, "path"),
                    id: text(p, "id"),
                    name: text(p, "name"),
                    vendor: text(p, "vendor"),
                    format: text(p, "format"),
                })
                .collect();
            fx.broken = v["broken"].as_array().map_or(0, Vec::len);
        }
        mesh::Event::Fx(v) => match v.get("answer") {
            Some(a) => {
                if let Some(e) = a["error"].as_str() {
                    fx.note = Some(e.to_owned());
                } else if a["cmd"] == "params"
                    && let Some(list) = a["params"].as_array()
                {
                    let at = a["plugin"].as_u64().unwrap_or(0) as usize;
                    fx.params = Some((at, list.clone()));
                }
            }
            None => fx.status = Some(v),
        },
        _ => {}
    }
}

pub(crate) fn update(state: &mut App, msg: Msg) -> Task<Message> {
    let node = state.mesh.node.clone();
    let fx = &mut state.mesh.fx;
    fx.note = None;
    let mut changed = false;
    match msg {
        Msg::Toggle => {
            fx.open = !fx.open;
            if fx.open
                && let Some(node) = &node
            {
                node.fx_scan(&serde_json::json!({ "dirs": fx.dirs }).to_string());
            }
        }
        Msg::Rescan => {
            if let Some(node) = &node {
                node.fx_scan(&serde_json::json!({ "dirs": fx.dirs, "fresh": true }).to_string());
            }
        }
        Msg::AddFolder => {
            return Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("A folder of plugins")
                        .pick_folder()
                        .await
                        .map(|f| f.path().to_path_buf())
                },
                |p| to(Msg::FolderPicked(p)),
            );
        }
        Msg::FolderPicked(Some(dir)) => {
            let dir = dir.to_string_lossy().into_owned();
            if !fx.dirs.contains(&dir) {
                fx.dirs.push(dir);
                save(fx);
            }
            if let Some(node) = &node {
                node.fx_scan(&serde_json::json!({ "dirs": fx.dirs }).to_string());
            }
        }
        Msg::FolderPicked(None) => {}
        Msg::Pick(found) => fx.pick = Some(found),
        Msg::Add => {
            if let Some(f) = fx.pick.take() {
                fx.chain.push(Plugin {
                    path: f.path,
                    id: f.id,
                    name: f.name,
                    format: f.format,
                    on: true,
                });
                changed = true;
            }
        }
        Msg::Move(i, up) => {
            let j = if up { i.checked_sub(1) } else { Some(i + 1) };
            if let Some(j) = j.filter(|j| *j < fx.chain.len()) {
                fx.chain.swap(i, j);
                changed = true;
            }
        }
        Msg::On(i, on) => {
            if let Some(p) = fx.chain.get_mut(i) {
                p.on = on;
                changed = true;
            }
        }
        Msg::Remove(i) if i < fx.chain.len() => {
            fx.chain.remove(i);
            fx.params = None;
            changed = true;
        }
        Msg::Remove(_) => {}
        Msg::Edit(i) => {
            if let Some(node) = &node {
                node.fx("fxEdit", &[&i.to_string()]);
            }
        }
        Msg::Params(i) => match &fx.params {
            Some((at, _)) if *at == i => fx.params = None,
            _ => {
                if let Some(node) = &node {
                    node.fx("fxParams", &[&i.to_string()]);
                }
            }
        },
        Msg::Param(i, id, value) => {
            if let Some((_, list)) = fx.params.as_mut().filter(|(at, _)| *at == i)
                && let Some(p) = list.iter_mut().find(|p| p["id"].as_u64() == Some(id))
            {
                p["value"] = value.into();
            }
        }
        Msg::ParamDone(i, id) => {
            let value = (fx.params.as_ref())
                .and_then(|(_, list)| list.iter().find(|p| p["id"].as_u64() == Some(id)))
                .and_then(|p| p["value"].as_f64());
            if let (Some(node), Some(v)) = (&node, value) {
                node.fx(
                    "fxParam",
                    &[&i.to_string(), &id.to_string(), &v.to_string()],
                );
                node.fx("fxSave", &[]);
            }
        }
        Msg::Restart => {
            if let Some(node) = &node {
                node.fx_restart();
            }
        }
    }
    if changed {
        save(&state.mesh.fx);
        send(state);
    }
    Task::none()
}

fn flat<'a>(
    label: impl text::IntoFragment<'a>,
    msg: Option<Msg>,
    p: &'a theme::Palette,
) -> Element<'a, Message> {
    button(text(label).size(12))
        .on_press_maybe(msg.map(to))
        .padding([4, 8])
        .style(flat_button_style(p))
        .into()
}

/// "Advanced: plugins", collapsed until opened.
pub(crate) fn view<'a>(mesh: &'a mesh_ui::Mesh, p: &'a theme::Palette) -> Element<'a, Message> {
    let fx = &mesh.fx;
    let head = button(
        text(format!(
            "{} Advanced: plugins (VST3, CLAP)",
            if fx.open { "▾" } else { "▸" }
        ))
        .size(13),
    )
    .on_press(to(Msg::Toggle))
    .padding([4, 0])
    .style(flat_button_style(p));
    if !fx.open {
        return head.into();
    }
    let mut col = column![
        head,
        text("Your own effects, after echo cancellation and noise suppression and before automatic gain. The built-in tools stay on either way; with no plugins nothing extra runs.")
            .size(12)
            .color(p.text_muted()),
    ]
    .spacing(8);
    let running = mesh.call.is_some() || mesh.mic_test.is_some();
    let status = fx.status.as_ref();
    let state = status.and_then(|s| s["state"].as_str()).unwrap_or("off");
    let line = match state {
        "on" => format!(
            "Plugins on · {} samples of latency",
            status.and_then(|s| s["latency"].as_u64()).unwrap_or(0)
        ),
        "starting" => "Starting the plugins…".into(),
        "bypassed" | "failed" => format!(
            "Plugins bypassed: {}. Your voice goes out without them.",
            status
                .and_then(|s| s["error"].as_str())
                .unwrap_or("they stopped")
        ),
        _ => "The plugins run in calls and the mic test.".into(),
    };
    let mut status_row =
        row![
            text(line)
                .size(12)
                .color(if matches!(state, "bypassed" | "failed") {
                    p.warning()
                } else {
                    p.text_muted()
                })
        ]
        .spacing(8);
    if matches!(state, "bypassed" | "failed") {
        status_row = status_row.push(flat("Restart plugins", Some(Msg::Restart), p));
    }
    col = col.push(status_row);
    for (i, plugin) in fx.chain.iter().enumerate() {
        let editable = running && state == "on" && plugin.on;
        let mut r = row![
            text(format!("{}. {}", i + 1, plugin.name))
                .size(13)
                .width(Length::Fill),
            text(plugin.format.to_uppercase())
                .size(11)
                .color(p.text_muted()),
            flat(
                if plugin.on { "☑ on" } else { "☐ off" },
                Some(Msg::On(i, !plugin.on)),
                p
            ),
            flat("↑", Some(Msg::Move(i, true)), p),
            flat("↓", Some(Msg::Move(i, false)), p),
            flat("Edit", editable.then_some(Msg::Edit(i)), p),
            flat("Parameters", editable.then_some(Msg::Params(i)), p),
            flat("Remove", Some(Msg::Remove(i)), p),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center);
        if !editable && plugin.on {
            r = r.push(
                text("start the mic test to edit")
                    .size(11)
                    .color(p.text_muted()),
            );
        }
        col = col.push(r);
        if let Some((_, list)) = fx.params.as_ref().filter(|(at, _)| *at == i) {
            for param in list.iter().take(64) {
                let Some(id) = param["id"].as_u64() else {
                    continue;
                };
                let (min, max) = (
                    param["min"].as_f64().unwrap_or(0.0),
                    param["max"].as_f64().unwrap_or(1.0),
                );
                let value = param["value"].as_f64().unwrap_or(min);
                col = col.push(
                    row![
                        text(param["name"].as_str().unwrap_or("?").to_owned())
                            .size(12)
                            .width(180),
                        slider(min..=max, value, move |v| to(Msg::Param(i, id, v)))
                            .step((max - min) / 200.0)
                            .on_release(to(Msg::ParamDone(i, id)))
                            .width(240),
                        text(format!("{value:.2}")).size(11).color(p.text_muted()),
                    ]
                    .spacing(10)
                    .align_y(iced::Alignment::Center),
                );
            }
        }
    }
    let add = row![
        pick_list(fx.found.clone(), fx.pick.clone(), |f| to(Msg::Pick(f)))
            .placeholder("Add a plugin…")
            .width(320),
        flat("Add", fx.pick.as_ref().map(|_| Msg::Add), p),
        flat("Add folder…", Some(Msg::AddFolder), p),
        flat("Rescan", Some(Msg::Rescan), p),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);
    col = col.push(add);
    let found = format!(
        "{} found{}",
        fx.found.len(),
        if fx.broken > 0 {
            format!(", {} file(s) that didn't load", fx.broken)
        } else {
            String::new()
        }
    );
    col = col.push(text(found).size(11).color(p.text_muted()));
    if let Some(n) = &fx.note {
        col = col.push(text(n.clone()).size(12).color(p.warning()));
    }
    col.into()
}

/// SPEC-voice-quality's voice quality, with what it costs to send.
pub(crate) const QUALITIES: [(&str, &str, u32); 4] = [
    ("Low", "low", 16),
    ("Standard", "standard", 32),
    ("High", "high", 64),
    ("Studio", "studio", 128),
];

/// Up in a call of `people`, at `kbps`: everyone else gets their own copy.
pub(crate) fn upload(kbps: u32, people: u32) -> u32 {
    kbps * people.saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chain_round_trips_config_and_makes_fxchain_json() {
        let chain = vec![
            Plugin {
                path: "/p/a.clap".into(),
                id: "a".into(),
                name: "A".into(),
                format: "clap".into(),
                on: true,
            },
            Plugin {
                path: "/p/b.vst3".into(),
                id: "B".into(),
                name: "B".into(),
                format: "vst3".into(),
                on: false,
            },
        ];
        let config = theme::Config {
            fx_plugins: chain.clone(),
            ..Default::default()
        };
        let back: theme::Config = toml::from_str(&toml::to_string(&config).unwrap()).unwrap();
        assert_eq!(back.fx_plugins, chain);
        let v: serde_json::Value = serde_json::from_str(&chain_json(&chain)).unwrap();
        assert_eq!(
            v,
            serde_json::json!([{"path": "/p/a.clap", "id": "a", "on": true}, {"path": "/p/b.vst3", "id": "B", "on": false}])
        );
    }

    #[test]
    fn upload_in_a_call_of_five() {
        assert_eq!(upload(32, 5), 128);
        assert_eq!(upload(128, 5), 512);
        assert_eq!(upload(64, 1), 0);
    }
}
