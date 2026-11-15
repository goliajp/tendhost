use std::path::{Path, PathBuf};

use super::Labelled;

/// The account directories of one agent under one home: `<base>` and
/// every `<base>-profile-<n>`. A directory reached twice (the default
/// name being a link to a numbered one) counts once, under its real path.
pub(super) fn account_dirs(home: &Path, base: &str) -> Vec<PathBuf> {
    let prefix = format!("{base}-profile-");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(home)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| {
                    name == base
                        || name
                            .strip_prefix(&prefix)
                            .is_some_and(|n| n.parse::<u32>().is_ok())
                })
        })
        .filter_map(|path| path.canonicalize().ok())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs.dedup();
    dirs
}

/// The directories named `sub` inside the account directories, each real
/// directory once. Several accounts can share one through links.
pub(super) fn stores(accounts: &[PathBuf], sub: &str) -> Vec<PathBuf> {
    let mut stores: Vec<PathBuf> = accounts
        .iter()
        .filter_map(|dir| dir.join(sub).canonicalize().ok())
        .filter(|p| p.is_dir())
        .collect();
    stores.sort();
    stores.dedup();
    stores
}

/// Every `.jsonl` file under `dir`, at any depth, changed at or after
/// `since`.
pub(super) fn recent_logs(dir: &Path, since: std::time::SystemTime) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|x| x == "jsonl")
                && meta.modified().is_ok_and(|m| m >= since)
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// Gemini keeps the signed-in address in `google_accounts.json`, in the
/// account directory or in its `.gemini` subdirectory.
pub(super) fn gemini_label(dir: &Path) -> Option<Labelled> {
    [
        dir.join("google_accounts.json"),
        dir.join(".gemini/google_accounts.json"),
    ]
    .iter()
    .find_map(|path| {
        let text = std::fs::read_to_string(path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&text).ok()?;
        Some(Labelled {
            label: json.get("active")?.as_str()?.to_string(),
            plan: None,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_directory_and_numbered_profiles_are_accounts_and_a_link_is_not_a_second_one() {
        let home = tempfile::tempdir().unwrap();
        for dir in [
            ".claude-profile-1",
            ".claude-profile-2",
            ".claude-shared",
            ".claude-profile-x",
            ".claudette",
        ] {
            std::fs::create_dir_all(home.path().join(dir)).unwrap();
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            home.path().join(".claude-profile-1"),
            home.path().join(".claude"),
        )
        .unwrap();
        std::fs::write(
            home.path().join(".claude-profile-9"),
            "a file, not a directory",
        )
        .unwrap();
        let names: Vec<_> = account_dirs(home.path(), ".claude")
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, [".claude-profile-1", ".claude-profile-2"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_session_store_shared_by_several_accounts_is_read_once() {
        let home = tempfile::tempdir().unwrap();
        let shared = home.path().join("shared/projects");
        std::fs::create_dir_all(&shared).unwrap();
        let mut accounts = Vec::new();
        for n in 1..=2 {
            let dir = home.path().join(format!(".claude-profile-{n}"));
            std::fs::create_dir_all(&dir).unwrap();
            std::os::unix::fs::symlink(&shared, dir.join("projects")).unwrap();
            accounts.push(dir);
        }
        assert_eq!(
            stores(&accounts, "projects"),
            [shared.canonicalize().unwrap()]
        );
    }

    #[test]
    fn the_gemini_address_is_read_from_either_place() {
        let dir = tempfile::tempdir().unwrap();
        assert!(gemini_label(dir.path()).is_none());
        std::fs::create_dir_all(dir.path().join(".gemini")).unwrap();
        std::fs::write(
            dir.path().join(".gemini/google_accounts.json"),
            r#"{"active":"ada@example.com","old":[]}"#,
        )
        .unwrap();
        assert_eq!(gemini_label(dir.path()).unwrap().label, "ada@example.com");
    }
}
