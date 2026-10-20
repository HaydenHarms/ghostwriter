//! The Claude panel's settings and the companion's status (fork addition, see
//! `docs/companion.md`): two small JSON files in the profile folder. The panel writes
//! `settings.json` and the companion reads it about every second, so a change applies at once;
//! the companion writes `status.json` and the panel shows it.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

/// The models the panel offers: (Claude Code's name, what the panel calls it).
pub const MODELS: [(&str, &str); 3] = [("haiku", "Fast"), ("sonnet", "Balanced"), ("opus", "Thorough")];

/// A status older than this (seconds) means the companion isn't running.
pub const STALE: u64 = 6;

/// Seconds since 1970 (0 where the platform has no clock, as on the web).
pub fn unix_now() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
    }
    #[cfg(target_arch = "wasm32")]
    {
        0
    }
}

fn settings_path(dir: &Path) -> PathBuf {
    dir.join("settings.json")
}

fn status_path(dir: &Path) -> PathBuf {
    dir.join("status.json")
}

fn read_object(path: &Path) -> Map<String, Value> {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_default()
}

/// Write `v` to `path` through a temporary file, so a reader never sees half of it.
fn write_json(path: &Path, v: &Value) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("can't create {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, v.to_string()).map_err(|e| format!("can't write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("can't replace {}: {e}", path.display()))
}

/// The panel's settings. A key the writer never changed is absent: the companion's own options
/// (command line or default) stand for it.
pub fn read(dir: &Path) -> Map<String, Value> {
    read_object(&settings_path(dir))
}

/// Change one setting, keeping the others.
pub fn set(dir: &Path, key: &str, value: Value) -> Result<(), String> {
    let mut s = read(dir);
    s.insert(key.into(), value);
    write_json(&settings_path(dir), &Value::Object(s))
}

/// The documents (paths) not to learn from.
pub fn skipped(s: &Map<String, Value>) -> Vec<String> {
    s.get("skipLearning").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()
}

/// Learn from document `path`, or not.
pub fn skip_learning(dir: &Path, path: &str, skip: bool) -> Result<(), String> {
    let mut list = skipped(&read(dir));
    list.retain(|p| p != path);
    if skip {
        list.push(path.to_string());
    }
    set(dir, "skipLearning", json!(list))
}

/// The companion's last report, and how many seconds old it is.
pub fn status(dir: &Path) -> Option<(Map<String, Value>, u64)> {
    let s = read_object(&status_path(dir));
    let time = s.get("time").and_then(Value::as_u64)?;
    Some((s, unix_now().saturating_sub(time)))
}

/// Report the companion's state (`state`, `message`, `callsThisHour`…); the time is added.
pub fn write_status(dir: &Path, mut status: Map<String, Value>) -> Result<(), String> {
    status.insert("time".into(), json!(unix_now()));
    write_json(&status_path(dir), &Value::Object(status))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_merge_and_status_ages() {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("ghostwriter-settings-test-{}-{n}", std::process::id()));
        assert!(read(&dir).is_empty());
        set(&dir, "learn", json!(false)).unwrap();
        set(&dir, "max", json!(2)).unwrap();
        skip_learning(&dir, "/a.docx", true).unwrap();
        skip_learning(&dir, "/b.docx", true).unwrap();
        skip_learning(&dir, "/a.docx", false).unwrap();
        let s = read(&dir);
        assert_eq!((s["learn"].clone(), s["max"].clone()), (json!(false), json!(2)));
        assert_eq!(skipped(&s), vec!["/b.docx"]);
        assert!(status(&dir).is_none());
        let mut st = Map::new();
        st.insert("state".into(), json!("watching"));
        write_status(&dir, st).unwrap();
        let (st, age) = status(&dir).unwrap();
        assert_eq!(st["state"], "watching");
        assert!(age < STALE);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
