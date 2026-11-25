//! One-time move from Mesh's folders to Aflos's, after the rename. It copies
//! and never deletes: the old `mesh` folders stay as they were, a backup of
//! your identity, accounts and settings.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Copies `old` to `new` when `new` doesn't exist yet. The copy goes into a
/// sibling `<new>.migrating` first and is renamed into place, so a crash
/// mid-copy leaves no half-filled `new` behind and the next start retries.
pub fn copy_once(old: &Path, new: &Path) -> io::Result<bool> {
    if new.exists() || !old.is_dir() {
        return Ok(false);
    }
    let tmp = new.with_extension("migrating");
    if tmp.exists() {
        fs::remove_dir_all(&tmp)?;
    }
    copy_dir(old, &tmp)?;
    fs::rename(&tmp, new)?;
    Ok(true)
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let path = entry?.path();
        let dest = to.join(path.file_name().unwrap_or_default());
        if path.is_dir() {
            copy_dir(&path, &dest)?;
        } else {
            fs::copy(&path, &dest)?;
        }
    }
    Ok(())
}

/// `base/aflos`, or `base/mesh` while only that exists (a migration that
/// failed this run): Aflos then runs on the old folder rather than starting
/// empty, and copies again next start.
pub fn pick(base: &Path) -> PathBuf {
    let new = base.join("aflos");
    let old = base.join("mesh");
    if !new.exists() && old.is_dir() { old } else { new }
}

/// Copies `<data>/mesh` and `<config>/mesh` (the same folder on Windows) to
/// `aflos` beside them. Not with `AFLOS_DATA`/`MESH_DATA`: that folder is
/// used as it is.
pub fn from_mesh(bases: impl IntoIterator<Item = PathBuf>) {
    for base in bases {
        match copy_once(&base.join("mesh"), &base.join("aflos")) {
            Ok(true) => tracing::info!("copied {} to aflos (the old folder is kept)", base.join("mesh").display()),
            Ok(false) => {}
            Err(e) => tracing::warn!("couldn't copy {} to aflos, using it as it is: {e}", base.join("mesh").display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_once_and_keeps_the_old_folder() {
        let base = std::env::temp_dir().join(format!("aflos-migrate-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let (old, new) = (base.join("mesh"), base.join("aflos"));
        fs::create_dir_all(old.join("node/keys")).unwrap();
        fs::write(old.join("config.toml"), "theme = \"rice\"").unwrap();
        fs::write(old.join("node/keys/identity"), [1u8, 2, 3]).unwrap();
        // A crash in an earlier run left a partial copy behind.
        fs::create_dir_all(base.join("aflos.migrating/junk")).unwrap();

        assert_eq!(pick(&base), old, "the old folder until the copy is done");
        from_mesh([base.clone()]);
        assert_eq!(fs::read(new.join("node/keys/identity")).unwrap(), [1, 2, 3]);
        assert_eq!(fs::read_to_string(new.join("config.toml")).unwrap(), "theme = \"rice\"");
        assert!(!new.join("junk").exists() && !base.join("aflos.migrating").exists());
        assert!(old.join("node/keys/identity").is_file(), "old data is never deleted");
        assert_eq!(pick(&base), new);

        // Once Aflos has its own folder, the old one never overwrites it.
        fs::write(new.join("config.toml"), "theme = \"nord\"").unwrap();
        assert!(!copy_once(&old, &new).unwrap());
        assert_eq!(fs::read_to_string(new.join("config.toml")).unwrap(), "theme = \"nord\"");

        // A fresh install: nothing to copy.
        assert!(!copy_once(&base.join("none"), &base.join("fresh")).unwrap());
        assert_eq!(pick(&base.join("none")), base.join("none/aflos"));
        fs::remove_dir_all(&base).unwrap();
    }
}
