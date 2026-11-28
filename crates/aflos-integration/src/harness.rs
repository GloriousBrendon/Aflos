//! `aflos-integration-test`: a stand-in Aflos that runs an integration
//! through the handshake and each block it says it supports (spec D3), so
//! its author finds what's broken before anyone installs it.

use crate::host::{self, Report, Spec};
use crate::{
    Action, Cmd, Event, Hello, HelloBack, PROTOCOL, SettingKind, SignIn, SignInKind, State,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc;

/// What the harness sends through `send`, to find it in the chat.
pub const PROBE: &str = "aflos-integration-test says hi";

/// One check and how it went.
#[derive(Debug, Clone, PartialEq)]
pub struct Check {
    pub name: &'static str,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Passed,
    /// Broken: why, in words.
    Failed(String),
    /// Can't be checked by a program (a QR code to scan); why.
    Skipped(String),
}

impl Check {
    pub fn passed(&self) -> bool {
        !matches!(self.outcome, Outcome::Failed(_))
    }
}

/// How long each step may take.
const STEP: Duration = Duration::from_secs(5);

struct Run {
    rx: mpsc::UnboundedReceiver<(String, Report)>,
    handle: host::Handle,
    checks: Vec<Check>,
    /// Events that came while waiting for something else, oldest first.
    backlog: Vec<Event>,
}

impl Run {
    fn record(&mut self, name: &'static str, outcome: Outcome) -> bool {
        let ok = !matches!(outcome, Outcome::Failed(_));
        self.checks.push(Check { name, outcome });
        ok
    }

    /// The first event `want` picks within [`STEP`]. A crash or stop on the
    /// way is the failure.
    async fn wait<T>(&mut self, mut want: impl FnMut(&Event) -> Option<T>) -> Result<T, String> {
        if let Some((at, t)) = self
            .backlog
            .iter()
            .enumerate()
            .find_map(|(i, e)| want(e).map(|t| (i, t)))
        {
            self.backlog.remove(at);
            return Ok(t);
        }
        let backlog = &mut self.backlog;
        let rx = &mut self.rx;
        let waited = tokio::time::timeout(STEP, async {
            loop {
                match rx.recv().await {
                    Some((_, Report::Event(e))) => match want(&e) {
                        Some(t) => return Ok(t),
                        None => backlog.push(e),
                    },
                    Some((_, Report::Restarting { why, .. } | Report::Stopped { why })) => {
                        return Err(why);
                    }
                    Some((_, Report::Ready(_))) => {}
                    None => return Err("the host ended".into()),
                }
            }
        })
        .await;
        waited.unwrap_or_else(|_| Err(format!("nothing within {} s", STEP.as_secs())))
    }

    /// Sends `cmd` and waits for its reply.
    async fn ask(&mut self, cmd: Cmd) -> Result<(), String> {
        let id = self.handle.send(cmd);
        let reply = self
            .wait(|e| match e {
                Event::Reply { id: i, ok, error } if *i == id => Some((*ok, error.clone())),
                _ => None,
            })
            .await
            .map_err(|why| format!("no reply: {why}"))?;
        match reply {
            (true, _) => Ok(()),
            (false, error) => Err(format!(
                "it said no: {}",
                error.unwrap_or_else(|| "no reason given".into())
            )),
        }
    }
}

/// Runs `program` through every check; stops early only when the rest
/// can't run.
pub async fn check(program: &Path, args: &[String]) -> Vec<Check> {
    let dir = std::env::temp_dir().join(format!("aflos-integration-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let hello = Hello {
        protocol: PROTOCOL,
        aflos: format!("aflos-integration-test {}", env!("CARGO_PKG_VERSION")),
        data_dir: dir.join("data"),
        files_dir: dir.join("files"),
        granted: Vec::new(),
        settings: BTreeMap::new(),
    };
    let _ = std::fs::create_dir_all(&hello.files_dir);
    let mut spec = Spec::new("under-test", PathBuf::from(program), hello);
    spec.args = args.to_vec();
    spec.restarts = 0;
    let (tx, rx) = mpsc::unbounded_channel();
    let handle = host::start(spec, tx);
    let mut run = Run {
        rx,
        handle,
        checks: Vec::new(),
        backlog: Vec::new(),
    };

    // The handshake.
    let back = match tokio::time::timeout(Duration::from_secs(15), run.rx.recv()).await {
        Ok(Some((_, Report::Ready(back)))) => back,
        Ok(Some((_, Report::Stopped { why }))) => {
            run.record("answers hello", Outcome::Failed(why));
            return run.checks;
        }
        other => {
            run.record(
                "answers hello",
                Outcome::Failed(format!("no hello: {other:?}")),
            );
            return run.checks;
        }
    };
    run.record("answers hello", Outcome::Passed);
    run.record("its rail tile", tile(&back));
    let ping = run.ask(Cmd::Ping).await;
    run.record(
        "answers ping",
        ping.map_or_else(Outcome::Failed, |()| Outcome::Passed),
    );

    // Sign-in, then its chats.
    let signed_in = sign_in(&mut run, &back).await;
    let first_chat = if signed_in {
        match run
            .wait(|e| match e {
                Event::Chats(c) => Some(
                    c.sections
                        .iter()
                        .flat_map(|s| &s.chats)
                        .find(|c| !c.locked)
                        .map(|c| c.id.clone()),
                ),
                _ => None,
            })
            .await
        {
            Ok(chat) => {
                run.record("lists its chats", Outcome::Passed);
                chat
            }
            Err(why) => {
                run.record(
                    "lists its chats",
                    Outcome::Failed(format!("no chats after signing in: {why}")),
                );
                None
            }
        }
    } else {
        None
    };

    // Sending, and the actions on what was sent.
    let supports = |a: Action| back.supports.actions.contains(&a);
    if supports(Action::Send) {
        match &first_chat {
            None => {
                run.record(
                    "sends a message",
                    Outcome::Skipped("no chat to send in".into()),
                );
            }
            Some(chat) => {
                let chat = chat.clone();
                let _ = run
                    .ask(Cmd::Open {
                        chat: Some(chat.clone()),
                    })
                    .await;
                let sent = run
                    .ask(Cmd::Send {
                        chat: chat.clone(),
                        text: PROBE.into(),
                        reply_to: None,
                    })
                    .await;
                let shown = match sent {
                    Err(why) => Err(why),
                    Ok(()) => run
                        .wait(|e| match e {
                            Event::Messages { chat: c, messages } if *c == chat => messages
                                .iter()
                                .rev()
                                .find(|m| m.mine && m.text == PROBE)
                                .map(|m| m.id.clone()),
                            _ => None,
                        })
                        .await
                        .map_err(|why| format!("it says it can send, but the chat never showed the sent message ({why})")),
                };
                let probe = shown.as_ref().ok().cloned();
                run.record(
                    "sends a message",
                    shown.map_or_else(Outcome::Failed, |_| Outcome::Passed),
                );
                if let (Some(id), true) = (probe, supports(Action::React)) {
                    let reacted = run
                        .ask(Cmd::React {
                            message: id,
                            emoji: "👍".into(),
                        })
                        .await;
                    run.record(
                        "reacts",
                        reacted.map_or_else(Outcome::Failed, |()| Outcome::Passed),
                    );
                }
            }
        }
    }

    // Settings take their defaults.
    for setting in &back.supports.settings {
        let value = match &setting.kind {
            SettingKind::Switch { default } => serde_json::json!(default),
            SettingKind::Choice { default, .. } | SettingKind::Text { default } => {
                serde_json::json!(default)
            }
        };
        let set = run
            .ask(Cmd::Setting {
                key: setting.key.clone(),
                value,
            })
            .await;
        run.record(
            "takes its settings",
            set.map_or_else(Outcome::Failed, |()| Outcome::Passed),
        );
    }

    // Quitting: dropping the handle sends quit; the host ends once it's gone.
    let Run {
        mut rx,
        handle,
        mut checks,
        ..
    } = run;
    drop(handle);
    let quit = tokio::time::timeout(Duration::from_secs(5), async {
        while rx.recv().await.is_some() {}
    })
    .await;
    checks.push(Check {
        name: "quits when asked",
        outcome: if quit.is_ok() {
            Outcome::Passed
        } else {
            Outcome::Failed("still running 5 s after quit".into())
        },
    });
    let _ = std::fs::remove_dir_all(&dir);
    checks
}

fn tile(back: &HelloBack) -> Outcome {
    let t = &back.tile;
    let hex = t.color.len() == 7
        && t.color.starts_with('#')
        && t.color[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !(2..=3).contains(&t.tag.chars().count()) {
        Outcome::Failed(format!("its tag should be 2 or 3 letters, not {:?}", t.tag))
    } else if !hex {
        Outcome::Failed(format!("its colour should be #rrggbb, not {:?}", t.color))
    } else if !t.icon_svg.trim_start().starts_with("<svg") {
        Outcome::Failed("its icon should be SVG markup starting <svg".into())
    } else {
        Outcome::Passed
    }
}

async fn sign_in(run: &mut Run, back: &HelloBack) -> bool {
    let kinds = &back.supports.sign_in;
    if kinds.is_empty() {
        return true;
    }
    let step = run
        .wait(|e| {
            if let Event::SignIn(s) = e {
                Some(s.clone())
            } else {
                None
            }
        })
        .await;
    match step {
        Ok(SignIn::Form { fields }) if kinds.contains(&SignInKind::Form) => {
            let values = fields
                .iter()
                .map(|f| (f.name.clone(), "aflos-test".to_string()))
                .collect();
            if let Err(why) = run.ask(Cmd::SignIn { values }).await {
                run.record("signs in", Outcome::Failed(why));
                return false;
            }
            let online = run
                .wait(|e| match e {
                    Event::Status {
                        state: State::Online,
                        ..
                    } => Some(()),
                    _ => None,
                })
                .await;
            run.record(
                "signs in",
                online.map_or_else(
                    |why| Outcome::Failed(format!("not online after signing in: {why}")),
                    |()| Outcome::Passed,
                ),
            )
        }
        Ok(step) => {
            run.record(
                "signs in",
                Outcome::Skipped(format!("needs a person ({step:?})")),
            );
            false
        }
        Err(why) => {
            run.record(
                "signs in",
                Outcome::Failed(format!("never asked to sign in: {why}")),
            );
            false
        }
    }
}
