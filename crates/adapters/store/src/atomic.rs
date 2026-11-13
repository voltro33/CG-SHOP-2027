//! Whole files or nothing: write a temporary file, then rename it over.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// OneDrive and virus scanners hold a fresh file open for a moment, which
/// makes the rename fail; Python's miner retries the same way.
const RENAME_ATTEMPTS: u32 = 10;
const RENAME_BACKOFF: Duration = Duration::from_millis(200);

/// Writes `text` to `path`, creating its directory. A crash leaves either
/// the old file or the new one, never half of one.
pub fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }

    let temp = temp_path(path);
    let written = write_synced(&temp, text.as_bytes()).and_then(|()| rename_retrying(&temp, path));
    if written.is_err() {
        // the temporary file may not exist; the first error is what matters
        let _ = fs::remove_file(&temp);
    }

    written
}

/// The label as a file name: letters, digits and `-_.=` stay, the rest
/// becomes `_`, as Python's `_safe_name`.
pub fn safe_name(label: &str) -> String {
    label
        .chars()
        .map(|c| match c.is_alphanumeric() || "-_.=".contains(c) {
            true => c,
            false => '_',
        })
        .collect()
}

fn rename_retrying(from: &Path, to: &Path) -> std::io::Result<()> {
    for attempt in 1..RENAME_ATTEMPTS {
        match fs::rename(from, to) {
            Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                std::thread::sleep(RENAME_BACKOFF * attempt);
            }
            other => return other,
        }
    }

    fs::rename(from, to)
}

/// Next to the target, unique per process, so two miners never share one.
fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.tmp", std::process::id()));
    path.with_file_name(name)
}

fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_names() {
        assert_eq!(
            safe_name("iso|groups-h/gap_factor=0.5|s0|v2"),
            "iso_groups-h_gap_factor=0.5_s0_v2"
        );
        assert_eq!(safe_name("a b:c\\d"), "a_b_c_d");
    }

    #[test]
    fn writes_replace_and_create_directories() {
        let dir = std::env::temp_dir().join(format!("mow-store-atomic-{}", std::process::id()));
        let path = dir.join("deep/er/file.json");

        write_atomic(&path, "one").unwrap();
        write_atomic(&path, "two").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "two");
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }
}
