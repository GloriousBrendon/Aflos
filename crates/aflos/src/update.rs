//! Check for updates: the newest Aflos on GitHub Releases, downloaded,
//! checked against the release's SHA256SUMS, and installed with the same
//! installer a fresh download uses (the setup on Windows, `install-linux.sh`
//! on Linux). Then Aflos restarts into it.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const RELEASES: &str =
    "https://api.github.com/repos/GloriousBrendon/Mesh-Voice/releases?per_page=10";

/// A release newer than this build, with the file for this computer.
#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    /// "0.4.0".
    pub version: String,
    pub file: String,
    pub url: String,
    pub sums_url: String,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

/// "v0.3.0" or "0.3.0" as numbers, for comparing.
fn version(tag: &str) -> Option<(u64, u64, u64)> {
    let mut parts = tag
        .trim_start_matches('v')
        .split('.')
        .map(|n| n.parse().ok());
    Some((
        parts.next()??,
        parts.next()??,
        parts.next().flatten().unwrap_or(0),
    ))
}

/// This computer's download: the setup on Windows, the tarball on Linux.
fn ours(name: &str) -> bool {
    if cfg!(windows) {
        name.starts_with("Aflos-Setup-") && name.ends_with(".exe")
    } else {
        name.starts_with("aflos-") && name.ends_with("-linux-x86_64.tar.gz")
    }
}

/// The newest release above `current` that has our file and its checksums.
fn newest(releases: &[GhRelease], current: &str) -> Option<Release> {
    let current = version(current)?;
    releases
        .iter()
        .filter(|r| !r.draft)
        .filter_map(|r| Some((version(&r.tag_name)?, r)))
        .filter(|(v, _)| *v > current)
        .max_by_key(|(v, _)| *v)
        .and_then(|((a, b, c), r)| {
            let file = r.assets.iter().find(|a| ours(&a.name))?;
            let sums = r.assets.iter().find(|a| a.name == "SHA256SUMS")?;
            Some(Release {
                version: format!("{a}.{b}.{c}"),
                file: file.name.clone(),
                url: file.browser_download_url.clone(),
                sums_url: sums.browser_download_url.clone(),
            })
        })
}

fn client() -> Result<reqwest::Client, String> {
    // GitHub's API turns away requests without a User-Agent.
    reqwest::Client::builder()
        .user_agent(concat!("aflos/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())
}

async fn get(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    let response = client.get(url).send().await.map_err(|e| e.to_string())?;
    let response = response.error_for_status().map_err(|e| e.to_string())?;
    Ok(response.bytes().await.map_err(|e| e.to_string())?.to_vec())
}

/// A newer release, `None` when this is the newest.
pub async fn check() -> Result<Option<Release>, String> {
    let body = get(&client()?, RELEASES).await?;
    let releases: Vec<GhRelease> = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    Ok(newest(&releases, env!("CARGO_PKG_VERSION")))
}

/// `file`'s line in a SHA256SUMS ("<hex>  <name>").
fn expected_sum(sums: &str, file: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, name) = line.split_once(char::is_whitespace)?;
        (name.trim().trim_start_matches('*') == file).then(|| hash.to_ascii_lowercase())
    })
}

/// Downloads, checks and starts installing `release`. On success Aflos should
/// quit: the installer (Windows) or a relaunch (Linux) brings the new one up.
pub async fn install(release: Release) -> Result<(), String> {
    let path = download(&release).await?;
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    tokio::task::spawn_blocking(move || run(&path, &dir))
        .await
        .map_err(|e| e.to_string())?
}

/// The release's file in a temp folder, once it matches its checksum.
async fn download(release: &Release) -> Result<PathBuf, String> {
    let client = client()?;
    let sums = String::from_utf8_lossy(&get(&client, &release.sums_url).await?).into_owned();
    let want = expected_sum(&sums, &release.file)
        .ok_or("the release lists no checksum for its download")?;
    let bytes = get(&client, &release.url).await?;
    let got: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if got != want {
        return Err("the download didn't match its checksum; nothing was installed".into());
    }
    let dir = std::env::temp_dir().join(format!("aflos-update-{}", release.version));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(&release.file);
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Windows: the setup, silent; it closes Aflos and starts the new one itself.
#[cfg(windows)]
fn run(setup: &Path, _dir: &Path) -> Result<(), String> {
    std::process::Command::new(setup)
        .args(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"])
        .spawn()
        .map(drop)
        .map_err(|e| format!("couldn't start the setup: {e}"))
}

/// Linux: unpack, run the release's own install-linux.sh (into ~/.local/bin),
/// then start the new Aflos a moment after this one has quit.
#[cfg(not(windows))]
fn run(tarball: &Path, dir: &Path) -> Result<(), String> {
    use std::process::Command;
    let ok = |what: &str, status: std::io::Result<std::process::ExitStatus>| match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("{what} failed ({s})")),
        Err(e) => Err(format!("{what} failed: {e}")),
    };
    ok(
        "unpacking",
        Command::new("tar")
            .arg("xzf")
            .arg(tarball)
            .arg("-C")
            .arg(dir)
            .status(),
    )?;
    let folder = tarball
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix(".tar.gz"))
        .ok_or("odd file name")?;
    ok(
        "installing",
        Command::new(dir.join(folder).join("install-linux.sh")).status(),
    )?;
    let app: PathBuf = dirs::home_dir()
        .ok_or("no home folder")?
        .join(".local/bin/aflos");
    Command::new("sh")
        .arg("-c")
        .arg("sleep 2; exec \"$0\"")
        .arg(app)
        .spawn()
        .map(drop)
        .map_err(|e| format!("installed, but couldn't restart Aflos: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, files: &[&str]) -> GhRelease {
        let assets = files
            .iter()
            .map(|n| GhAsset {
                name: n.to_string(),
                browser_download_url: format!("https://x/{n}"),
            })
            .collect();
        GhRelease {
            tag_name: tag.into(),
            draft: false,
            assets,
        }
    }

    #[test]
    fn picks_the_newest_newer_release_with_our_file() {
        let file = if cfg!(windows) {
            "Aflos-Setup-0.5.0.0.exe"
        } else {
            "aflos-0.5.0-linux-x86_64.tar.gz"
        };
        let releases = [
            release("v0.4.0", &[]),
            release("v0.5.0", &[file, "SHA256SUMS"]),
            release("v0.2.0", &[file, "SHA256SUMS"]),
        ];
        let found = newest(&releases, "0.3.0").unwrap();
        assert_eq!(
            (found.version.as_str(), found.file.as_str()),
            ("0.5.0", file)
        );
        assert_eq!(newest(&releases, "0.5.0"), None, "nothing newer");
        assert_eq!(
            newest(&[release("v0.9.0", &[file])], "0.3.0"),
            None,
            "no checksums, no update"
        );
        assert!(version("v0.10.0") > version("v0.9.1"));
    }

    /// Against the real v0.2.0 release: `cargo test -p aflos -- --ignored live_download`.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "downloads from GitHub"]
    async fn live_download_checks_out() {
        let body = get(&client().unwrap(), RELEASES).await.unwrap();
        let releases: Vec<GhRelease> = serde_json::from_slice(&body).unwrap();
        let release = newest(&releases, "0.1.0").expect("a release newer than 0.1.0");
        let path = download(&release).await.unwrap();
        assert!(path.metadata().unwrap().len() > 1_000_000);
    }

    #[test]
    fn reads_sha256sums() {
        let sums = "abc  Aflos-Setup-0.4.0.0.exe\nDEF *aflos-0.4.0-linux-x86_64.tar.gz\n";
        assert_eq!(
            expected_sum(sums, "aflos-0.4.0-linux-x86_64.tar.gz").as_deref(),
            Some("def")
        );
        assert_eq!(
            expected_sum(sums, "Aflos-Setup-0.4.0.0.exe").as_deref(),
            Some("abc")
        );
        assert_eq!(expected_sum(sums, "other"), None);
    }
}
