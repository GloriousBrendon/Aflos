//! The host against the echo integration, well-behaved and not
//! (PLAN-integrations Task 2): whatever it does, Aflos gets a report, never
//! a panic or a block.

#![cfg(feature = "host")]

use aflos_integration::host::{self, Report, Spec};
use aflos_integration::{Cmd, Event, Hello, PROTOCOL, State};
use std::collections::BTreeMap;
use std::time::Duration;
use tokio::sync::mpsc;

fn spec(mode: &str, dir: &std::path::Path) -> Spec {
    let hello = Hello {
        protocol: PROTOCOL,
        aflos: "test".into(),
        data_dir: dir.join("data"),
        files_dir: dir.join("files"),
        granted: vec![],
        settings: BTreeMap::new(),
    };
    let mut spec = Spec::new("test.echo", env!("CARGO_BIN_EXE_aflos-echo"), hello);
    spec.env = vec![("AFLOS_ECHO_MODE".into(), mode.into())];
    spec.ping_every = Duration::from_millis(300);
    spec.hello_within = Duration::from_secs(5);
    spec.restarts = 2;
    spec
}

fn dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("aflos-host-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// The next report, or a failure after `secs`.
async fn next(rx: &mut mpsc::UnboundedReceiver<(String, Report)>, secs: u64) -> Report {
    let (id, report) = tokio::time::timeout(Duration::from_secs(secs), rx.recv())
        .await
        .expect("a report in time")
        .expect("the host is still there");
    assert_eq!(id, "test.echo");
    report
}

/// Reports until one `done` accepts.
async fn until(
    rx: &mut mpsc::UnboundedReceiver<(String, Report)>,
    done: impl Fn(&Report) -> bool,
) -> Report {
    loop {
        let report = next(rx, 20).await;
        if done(&report) {
            return report;
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_good_one_runs_answers_and_quits() {
    let dir = dir("good");
    let (tx, mut rx) = mpsc::unbounded_channel();
    let handle = host::start(spec("", &dir), tx);
    let Report::Ready(back) = next(&mut rx, 10).await else {
        panic!("not ready")
    };
    assert_eq!(back.name, "Echo");
    let mut values = BTreeMap::new();
    values.insert("name".to_string(), "Ann".to_string());
    let id = handle.send(Cmd::SignIn { values });
    until(
        &mut rx,
        |r| matches!(r, Report::Event(Event::Reply { id: i, ok: true, .. }) if *i == id),
    )
    .await;
    until(&mut rx, |r| {
        matches!(
            r,
            Report::Event(Event::Status {
                state: State::Online,
                ..
            })
        )
    })
    .await;
    handle.send(Cmd::Send {
        chat: "echo".into(),
        text: "hello".into(),
        reply_to: None,
    });
    let Report::Event(Event::Messages { messages, .. }) = until(&mut rx, |r| {
        matches!(r, Report::Event(Event::Messages { .. }))
    })
    .await
    else {
        unreachable!()
    };
    assert_eq!(messages.last().map(|m| m.text.as_str()), Some("hello"));
    // Pings come and go without reports or restarts.
    tokio::time::sleep(Duration::from_millis(1200)).await;
    while let Ok((_, r)) = rx.try_recv() {
        assert!(matches!(r, Report::Event(_)), "{r:?}");
    }
    // Dropping the handle stops it; the host's task ends and drops the sender.
    drop(handle);
    let ended = tokio::time::timeout(Duration::from_secs(5), async {
        while rx.recv().await.is_some() {}
    })
    .await;
    assert!(
        ended.is_ok(),
        "the host didn't end after its handle was dropped"
    );
    // Its stderr went to its own log.
    assert!(dir.join("data/stderr.log").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_crashing_one_is_restarted_then_stopped() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let _handle = host::start(spec("crash", &dir("crash")), tx);
    for attempt in 1..=2 {
        let r = next(&mut rx, 10).await;
        assert_eq!(
            r,
            Report::Restarting {
                attempt,
                why: "it crashed (exit code 3)".into()
            }
        );
    }
    let Report::Stopped { why } = next(&mut rx, 10).await else {
        panic!("not stopped")
    };
    assert_eq!(why, "it crashed (exit code 3) (restarted 2 times)");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hung_one_is_caught_by_the_ping() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let _handle = host::start(spec("hang", &dir("hang")), tx);
    assert!(matches!(next(&mut rx, 10).await, Report::Ready(_)));
    let r = until(&mut rx, |r| matches!(r, Report::Restarting { .. })).await;
    assert_eq!(
        r,
        Report::Restarting {
            attempt: 1,
            why: "it stopped answering (hung)".into()
        }
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_silent_one_times_out() {
    let mut spec = spec("silent", &dir("silent"));
    spec.hello_within = Duration::from_millis(500);
    spec.restarts = 0;
    let (tx, mut rx) = mpsc::unbounded_channel();
    let _handle = host::start(spec, tx);
    let Report::Stopped { why } = next(&mut rx, 10).await else {
        panic!("not stopped")
    };
    assert!(
        why.starts_with("it didn't answer Aflos's hello within"),
        "{why}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn garbage_is_skipped_and_it_still_runs() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let handle = host::start(spec("garbage", &dir("garbage")), tx);
    let Report::Ready(back) = next(&mut rx, 20).await else {
        panic!("not ready")
    };
    assert_eq!(back.name, "Echo");
    let id = handle.send(Cmd::Ping);
    // Our own pings' replies are swallowed; this one is ours to see.
    until(
        &mut rx,
        |r| matches!(r, Report::Event(Event::Reply { id: i, .. }) if *i == id),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn one_from_the_future_is_stopped_with_no_restart() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let _handle = host::start(spec("newer", &dir("newer")), tx);
    let Report::Stopped { why } = next(&mut rx, 10).await else {
        panic!("not stopped")
    };
    assert!(why.contains("needs a newer Aflos"), "{why}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_missing_program_is_stopped_with_no_restart() {
    let mut spec = spec("", &dir("missing"));
    spec.program = "/nowhere/aflos-no-such-integration".into();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let _handle = host::start(spec, tx);
    let Report::Stopped { why } = next(&mut rx, 10).await else {
        panic!("not stopped")
    };
    assert!(why.starts_with("it couldn't start"), "{why}");
}
