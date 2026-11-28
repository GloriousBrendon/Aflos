//! `aflos-integration-test <program> [args…]`: checks an integration the way
//! Aflos will run it. Exits 1 if anything failed.

use aflos_integration::harness::{self, Outcome};
use std::path::PathBuf;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut args = std::env::args().skip(1);
    let Some(program) = args.next() else {
        eprintln!("usage: aflos-integration-test <program> [args…]");
        std::process::exit(2);
    };
    let checks = harness::check(&PathBuf::from(program), &args.collect::<Vec<_>>()).await;
    for check in &checks {
        match &check.outcome {
            Outcome::Passed => println!("ok      {}", check.name),
            Outcome::Skipped(why) => println!("skipped {}: {why}", check.name),
            Outcome::Failed(why) => println!("FAILED  {}: {why}", check.name),
        }
    }
    let failed = checks.iter().filter(|c| !c.passed()).count();
    if failed > 0 {
        println!("\n{failed} failed");
        std::process::exit(1);
    }
    println!("\nall passed");
}
