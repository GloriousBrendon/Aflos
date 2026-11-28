//! Running an integration: Aflos's side (feature `host`, spec T7).
//!
//! [`start`] runs one integration's program for as long as its [`Handle`]
//! lives: the handshake, commands in, events out, a ping that catches a
//! hang, and restarts. Whatever the program does (crashes, hangs, floods,
//! writes garbage, speaks the wrong version), this ends in a [`Report`],
//! never a panic or a block.

use crate::{Cmd, Command, Event, Hello, HelloBack, check_version};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin};
use tokio::sync::mpsc;

/// The longest line Aflos reads (a video frame is about 100 KB); a longer
/// one is skipped, so a flood can't eat memory.
pub const MAX_LINE: usize = 16 * 1024 * 1024;

/// How to run one integration.
#[derive(Debug, Clone)]
pub struct Spec {
    /// Its id (the manifest's), echoed in every report.
    pub id: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Extra environment for its process.
    pub env: Vec<(String, String)>,
    /// Sent first, every start.
    pub hello: Hello,
    /// Aflos pings this often; a ping still unanswered at the next one is a
    /// hang (spec: 10 s).
    pub ping_every: Duration,
    /// How long it has to answer `hello`.
    pub hello_within: Duration,
    /// Restarts after a crash or hang before it's stopped for good.
    pub restarts: u32,
    /// Running this long without trouble forgives earlier restarts.
    pub stable_after: Duration,
}

impl Spec {
    pub fn new(id: impl Into<String>, program: impl Into<PathBuf>, hello: Hello) -> Self {
        Self {
            id: id.into(),
            program: program.into(),
            args: Vec::new(),
            env: Vec::new(),
            hello,
            ping_every: Duration::from_secs(10),
            hello_within: Duration::from_secs(10),
            restarts: 3,
            stable_after: Duration::from_secs(60),
        }
    }
}

/// What happened to an integration.
#[derive(Debug, Clone, PartialEq)]
pub enum Report {
    /// Running, past the handshake.
    Ready(HelloBack),
    /// Something it sent (replies to `ping` aren't passed on).
    Event(Event),
    /// It crashed, hung or quit, and is starting again.
    Restarting { attempt: u32, why: String },
    /// Stopped for good: after its restarts, or it can't run with this Aflos.
    /// `why` is in words a person can read.
    Stopped { why: String },
}

/// Commands to a running integration. Dropping the last clone stops the
/// integration: `quit`, then a kill if it hasn't exited within 2 s.
#[derive(Debug, Clone)]
pub struct Handle {
    commands: mpsc::UnboundedSender<Command>,
    next_id: Arc<AtomicU64>,
}

impl Handle {
    /// Sends a command (queued while it restarts); its id, for the reply.
    pub fn send(&self, body: Cmd) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let _ = self.commands.send(Command { id, body });
        id
    }
}

/// Starts `spec`'s program on the current tokio runtime. Reports go to
/// `reports` with `spec.id`.
pub fn start(spec: Spec, reports: mpsc::UnboundedSender<(String, Report)>) -> Handle {
    let (commands, rx) = mpsc::unbounded_channel();
    let next_id = Arc::new(AtomicU64::new(1));
    tokio::spawn(run(spec, rx, reports, next_id.clone()));
    Handle { commands, next_id }
}

enum Outcome {
    /// Asked to stop, and it did.
    Quit,
    /// It can't run with this Aflos: no restart.
    Fatal(String),
    /// Crashed, hung or misbehaved, after running this long.
    Failed(String, Duration),
}

async fn run(
    spec: Spec,
    mut commands: mpsc::UnboundedReceiver<Command>,
    reports: mpsc::UnboundedSender<(String, Report)>,
    next_id: Arc<AtomicU64>,
) {
    let report = |r: Report| {
        let _ = reports.send((spec.id.clone(), r));
    };
    let mut failures = 0;
    loop {
        match once(&spec, &mut commands, &report, &next_id).await {
            Outcome::Quit => return,
            Outcome::Fatal(why) => return report(Report::Stopped { why }),
            Outcome::Failed(why, ran) => {
                if ran >= spec.stable_after {
                    failures = 0;
                }
                failures += 1;
                if failures > spec.restarts {
                    let n = spec.restarts;
                    let times = if n == 1 { "once" } else { "times" };
                    let why = if n == 0 {
                        why
                    } else {
                        format!("{why} (restarted {n} {times})")
                    };
                    return report(Report::Stopped { why });
                }
                report(Report::Restarting {
                    attempt: failures,
                    why,
                });
                let wait = Duration::from_millis(500) * failures;
                // A stop asked for while waiting is still a stop.
                tokio::select! {
                    () = tokio::time::sleep(wait) => {}
                    c = commands.recv() => if c.is_none() { return },
                }
            }
        }
    }
}

/// One run of the program, from start to whatever ends it.
async fn once(
    spec: &Spec,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    report: &impl Fn(Report),
    next_id: &AtomicU64,
) -> Outcome {
    let started = Instant::now();
    let failed = |why: String| Outcome::Failed(why, started.elapsed());
    let mut child = match spawn(spec) {
        Ok(child) => child,
        // Missing or not runnable: restarting won't help.
        Err(e) => return Outcome::Fatal(format!("it couldn't start: {e}")),
    };
    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        return Outcome::Fatal("it couldn't start: no pipes".into());
    };
    let mut lines = BufReader::new(stdout);
    let mut buf = Vec::new();
    let id = || next_id.fetch_add(1, Ordering::Relaxed);

    let hello = Command {
        id: id(),
        body: Cmd::Hello(spec.hello.clone()),
    };
    if write(&mut stdin, &hello).await.is_err() {
        return failed(exited(&mut child).await);
    }
    let handshake = async {
        loop {
            match next_line(&mut lines, &mut buf).await {
                Some(line) => {
                    if let Some(Event::Hello(back)) = Event::from_line(&line) {
                        return Some(back);
                    }
                }
                None => return None,
            }
        }
    };
    let back = match tokio::time::timeout(spec.hello_within, handshake).await {
        Ok(Some(back)) => back,
        Ok(None) => return failed(exited(&mut child).await),
        Err(_) => {
            let _ = child.kill().await;
            let secs = spec.hello_within.as_secs_f32();
            return failed(format!("it didn't answer Aflos's hello within {secs:.0} s"));
        }
    };
    if let Err(e) = check_version(back.protocol) {
        let _ = child.kill().await;
        return Outcome::Fatal(format!("it can't run here: {e}"));
    }
    report(Report::Ready(back));

    let mut ping = tokio::time::interval(spec.ping_every);
    ping.tick().await;
    let mut waiting: Option<u64> = None;
    loop {
        tokio::select! {
            line = next_line(&mut lines, &mut buf) => match line {
                Some(line) => match Event::from_line(&line) {
                    Some(Event::Reply { id, .. }) if Some(id) == waiting => waiting = None,
                    Some(Event::Hello(_)) | None => {}
                    Some(event) => report(Report::Event(event)),
                },
                None => return failed(exited(&mut child).await),
            },
            command = commands.recv() => match command {
                Some(command) => {
                    if write(&mut stdin, &command).await.is_err() {
                        return failed(exited(&mut child).await);
                    }
                }
                None => {
                    quit(&mut child, &mut stdin, id()).await;
                    return Outcome::Quit;
                }
            },
            _ = ping.tick() => {
                if waiting.is_some() {
                    let _ = child.kill().await;
                    return failed("it stopped answering (hung)".into());
                }
                let ping = Command { id: id(), body: Cmd::Ping };
                waiting = Some(ping.id);
                if write(&mut stdin, &ping).await.is_err() {
                    return failed(exited(&mut child).await);
                }
            }
        }
    }
}

fn spawn(spec: &Spec) -> std::io::Result<Child> {
    let mut process = tokio::process::Command::new(&spec.program);
    process
        .args(&spec.args)
        .envs(spec.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(true);
    // Its own log, beside its data; nowhere if that can't be made.
    let log = std::fs::create_dir_all(&spec.hello.data_dir).and_then(|()| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(spec.hello.data_dir.join("stderr.log"))
    });
    process.stderr(log.map_or_else(|_| Stdio::null(), Stdio::from));
    #[cfg(windows)]
    process.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    process.spawn()
}

async fn write(stdin: &mut ChildStdin, command: &Command) -> std::io::Result<()> {
    let mut line = command.to_line();
    line.push('\n');
    stdin.write_all(line.as_bytes()).await?;
    stdin.flush().await
}

/// The next line, `None` at the end. Lines over [`MAX_LINE`] and lines that
/// aren't UTF-8 come back empty (ignored by the caller).
async fn next_line(reader: &mut (impl AsyncBufRead + Unpin), buf: &mut Vec<u8>) -> Option<String> {
    buf.clear();
    let mut too_long = false;
    loop {
        let chunk = match reader.fill_buf().await {
            Ok(chunk) => chunk,
            Err(_) => return None,
        };
        if chunk.is_empty() {
            // The end: a last line without its newline still counts.
            return (!buf.is_empty() || too_long).then(|| finish(buf, too_long));
        }
        let (part, done) = match chunk.iter().position(|&b| b == b'\n') {
            Some(at) => (&chunk[..at], Some(at + 1)),
            None => (chunk, None),
        };
        if buf.len() + part.len() > MAX_LINE {
            too_long = true;
            buf.clear();
        } else if !too_long {
            buf.extend_from_slice(part);
        }
        let used = done.unwrap_or(chunk.len());
        reader.consume(used);
        if done.is_some() {
            return Some(finish(buf, too_long));
        }
    }
}

fn finish(buf: &[u8], too_long: bool) -> String {
    if too_long {
        String::new()
    } else {
        String::from_utf8_lossy(buf).into_owned()
    }
}

/// Why the program ended, in words.
async fn exited(child: &mut Child) -> String {
    match tokio::time::timeout(Duration::from_secs(2), child.wait()).await {
        Ok(Ok(status)) => match status.code() {
            Some(0) => "it quit".into(),
            Some(code) => format!("it crashed (exit code {code})"),
            None => "it crashed".into(),
        },
        _ => {
            let _ = child.kill().await;
            "it stopped talking to Aflos".into()
        }
    }
}

async fn quit(child: &mut Child, stdin: &mut ChildStdin, id: u64) {
    let _ = write(
        stdin,
        &Command {
            id,
            body: Cmd::Quit,
        },
    )
    .await;
    if tokio::time::timeout(Duration::from_secs(2), child.wait())
        .await
        .is_err()
    {
        let _ = child.kill().await;
    }
}
