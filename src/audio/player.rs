// Platform audio playback.
//
// Every TTS provider renders (or downloads) audio and then plays it here — this
// module is the single playback choke point for the whole crate. On macOS the
// player is `afplay`; elsewhere we probe a list of common command-line players
// and use the first one installed, so the 0-100 volume knob and the menu-bar
// avatar hook keep working on Linux instead of being macOS-only.

use std::path::{Path, PathBuf};

/// Locate `name` on `PATH` without spawning it.
///
/// Returns the first `PATH` entry that contains `name` as a regular file with an
/// execute bit set. `name` must be a bare program name: anything containing a
/// path separator is rejected, because "is this program installed" is only a
/// meaningful question for `PATH` lookups. Non-absolute `PATH` entries are
/// skipped so the returned path is always absolute.
///
/// Never fails: a missing `PATH`, an unreadable directory or a missing execute
/// bit all yield `None`.
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains(std::path::MAIN_SEPARATOR) || name.contains('/') {
        return None;
    }

    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        if !dir.is_absolute() {
            continue;
        }
        let candidate = dir.join(name);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Whether `path` is a regular file with at least one execute bit set.
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    match std::fs::metadata(path) {
        Ok(meta) => meta.is_file() && meta.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_on_path_missing_binary() {
        assert!(find_on_path("sumvox_definitely_not_a_binary_9f3").is_none());
    }

    #[test]
    fn test_find_on_path_returns_absolute_path() {
        let found = find_on_path("sh").expect("sh must exist on any machine running cargo test");
        assert!(found.is_absolute(), "expected absolute path, got {found:?}");
    }

    #[test]
    fn test_find_on_path_empty_name() {
        assert!(find_on_path("").is_none());
    }

    #[test]
    fn test_find_on_path_rejects_path_separator() {
        // A name with a separator is not a PATH lookup — reject rather than
        // silently resolving it relative to the cwd.
        assert!(find_on_path("./sh").is_none());
    }
}
