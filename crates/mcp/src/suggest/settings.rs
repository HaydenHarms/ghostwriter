//! The Claude panel's settings and the companion's status (fork addition, see
//! `docs/companion.md`): small JSON files in the profile folder. The panel writes
//! `settings.json` and the companion reads it about every second, so a change applies at once;
//! the companion writes `status.json` and the panel shows it. A button that asks Claude for
//! something (a transition, a missing step, a structure review) writes `request.json`; the
//! structure review's notes come back in `notes.json`.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

/// The models the panel offers: (Claude Code's name, what the panel calls it).
pub const MODELS: [(&str, &str); 3] = [("haiku", "Fast"), ("sonnet", "Balanced"), ("opus", "Thorough")];

/// What the panel's "Ask Claude" buttons ask for: (request kind, button label, tooltip).
pub const ASKS: [(&str, &str, &str); 4] = [
    ("edits", "Suggest edits", "A few edits to the paragraph you're in (Alt+Shift+S)"),
    ("transition", "Suggest a transition", "A sentence linking the paragraph you're in to the one before it"),
    ("gap", "Suggest a missing step", "A sentence where this paragraph's reasoning jumps"),
    ("structure", "Review structure", "Notes on the order and flow of the whole piece, and small fixes"),
];

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

fn request_path(dir: &Path) -> PathBuf {
    dir.join("request.json")
}

fn notes_path(dir: &Path) -> PathBuf {
    dir.join("notes.json")
}

/// One thing the writer asked Claude for from the panel.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    /// Increases with every request, so the companion answers each once.
    pub id: u64,
    /// One of [`ASKS`]' kinds.
    pub kind: String,
    /// The body paragraph the caret was in when the writer asked.
    pub block: Option<usize>,
    /// The document's file, if saved.
    pub document: Option<String>,
}

/// The latest request, if any.
pub fn request(dir: &Path) -> Option<Request> {
    let r = read_object(&request_path(dir));
    Some(Request {
        id: r.get("id").and_then(Value::as_u64)?,
        kind: r.get("kind").and_then(Value::as_str)?.to_string(),
        block: r.get("block").and_then(Value::as_u64).and_then(|b| usize::try_from(b).ok()),
        document: r.get("document").and_then(Value::as_str).map(str::to_string),
    })
}

/// Ask the companion for `kind` about paragraph `block` of `document`. Returns the request's id.
pub fn ask(dir: &Path, kind: &str, block: Option<usize>, document: Option<&str>) -> Result<u64, String> {
    let id = request(dir).map_or(1, |r| r.id.saturating_add(1));
    write_json(&request_path(dir), &json!({"id": id, "kind": kind, "block": block, "document": document, "time": unix_now()}))?;
    Ok(id)
}

/// Claude's notes from a structure review: `[{block, note}]`, for `document`.
pub fn notes(dir: &Path) -> (Option<String>, Vec<(usize, String)>) {
    let n = read_object(&notes_path(dir));
    let doc = n.get("document").and_then(Value::as_str).map(str::to_string);
    let list = n
        .get("notes")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| Some((usize::try_from(x.get("block")?.as_u64()?).ok()?, x.get("note")?.as_str()?.to_string()))).collect())
        .unwrap_or_default();
    (doc, list)
}

/// Replace the notes (an empty list clears them).
pub fn write_notes(dir: &Path, document: Option<&str>, notes: &[(usize, String)]) -> Result<(), String> {
    let list: Vec<Value> = notes.iter().map(|(b, n)| json!({"block": b, "note": n})).collect();
    write_json(&notes_path(dir), &json!({"document": document, "notes": list, "time": unix_now()}))
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
        assert!(request(&dir).is_none());
        assert_eq!(ask(&dir, "gap", Some(3), Some("/a.docx")).unwrap(), 1);
        assert_eq!(ask(&dir, "structure", None, None).unwrap(), 2);
        let r = request(&dir).unwrap();
        assert_eq!((r.id, r.kind.as_str(), r.block, r.document), (2, "structure", None, None));
        write_notes(&dir, Some("/a.docx"), &[(4, "Merge with ¶2.".into())]).unwrap();
        assert_eq!(notes(&dir), (Some("/a.docx".into()), vec![(4, "Merge with ¶2.".to_string())]));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
