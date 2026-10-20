//! What Ghostwriter learns about the writer (fork addition, see `docs/companion.md`): every
//! suggestion they accepted or rejected (`feedback.jsonl`) and a style profile (`style.md`) that
//! Claude distils from those and from the writer's own paragraphs. Both live in
//! `<settings>/ghostwriter/` and are plain files the writer can read; the top of `style.md`,
//! above [`MARKER`], is the writer's own and is never rewritten.

use std::io::Write;
use std::path::PathBuf;

use serde_json::{Value, json};

/// Everything below this line in `style.md` is Ghostwriter's and is rewritten when it learns.
pub const MARKER: &str = "<!-- Ghostwriter learns below this line and rewrites it from time to time. Write your own notes above it. -->";

const STARTER: &str = "# My writing style

Notes for Ghostwriter in your own words: what you're going for, habits you keep on purpose, edits
you never want. Everything above the line below is yours; Ghostwriter only reads it.

";

/// Feedback entries read back at most (the newest).
const MAX_READ: usize = 500;

/// What the writer did with one suggestion.
#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    pub accepted: bool,
    /// The words the suggestion replaced (empty for an addition).
    pub was: String,
    /// The words it suggested (empty for a cut).
    pub suggested: String,
    /// The sentence or so around it, as it read with the suggestion in.
    pub context: String,
    /// Claude's reason, when the companion made it.
    pub why: String,
}

impl Decision {
    /// From a `suggest.decisions` entry; `None` for accept/reject-all (not a verdict on this one).
    pub fn from_engine(v: &Value) -> Option<Decision> {
        if v.get("bulk").and_then(Value::as_bool) == Some(true) {
            return None;
        }
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        let accepted = match v.get("verdict").and_then(Value::as_str)? {
            "accepted" => true,
            "rejected" => false,
            _ => return None,
        };
        Some(Decision { accepted, was: s("replaces"), suggested: s("text"), context: s("context"), why: String::new() })
    }

    fn to_json(&self) -> Value {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        json!({"time": secs, "verdict": if self.accepted { "accepted" } else { "rejected" }, "was": self.was,
            "suggested": self.suggested, "context": self.context, "why": self.why})
    }

    fn from_json(v: &Value) -> Option<Decision> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        let accepted = match v.get("verdict").and_then(Value::as_str)? {
            "accepted" => true,
            "rejected" => false,
            _ => return None,
        };
        Some(Decision { accepted, was: s("was"), suggested: s("suggested"), context: s("context"), why: s("why") })
    }

    /// One line for a prompt: `“was” → “suggested”`, with its reason.
    pub fn line(&self) -> String {
        let edit = match (self.was.is_empty(), self.suggested.is_empty()) {
            (true, _) => format!("add “{}”", self.suggested),
            (false, true) => format!("cut “{}”", self.was),
            (false, false) => format!("“{}” → “{}”", self.was, self.suggested),
        };
        if self.why.is_empty() { format!("- {edit}") } else { format!("- {edit} ({})", self.why) }
    }
}

/// The writer's profile folder.
pub struct Profile {
    pub dir: PathBuf,
}

impl Profile {
    /// `<settings>/ghostwriter`, shared by every document.
    pub fn default_dir() -> Option<PathBuf> {
        wordcraft_control_key::settings_dir().map(|d| d.join("ghostwriter"))
    }

    pub fn new(dir: PathBuf) -> Profile {
        Profile { dir }
    }

    pub fn feedback_path(&self) -> PathBuf {
        self.dir.join("feedback.jsonl")
    }

    pub fn style_path(&self) -> PathBuf {
        self.dir.join("style.md")
    }

    /// Add one decision to the feedback log.
    pub fn record(&self, d: &Decision) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("can't create {}: {e}", self.dir.display()))?;
        let path = self.feedback_path();
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path).map_err(|e| format!("can't open {}: {e}", path.display()))?;
        writeln!(f, "{}", d.to_json()).map_err(|e| format!("can't write {}: {e}", path.display()))
    }

    /// The feedback log, oldest first (the newest [`MAX_READ`]); unreadable lines are skipped.
    pub fn decisions(&self) -> Vec<Decision> {
        let text = std::fs::read_to_string(self.feedback_path()).unwrap_or_default();
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        let skip = lines.len().saturating_sub(MAX_READ);
        lines.iter().skip(skip).filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter_map(|v| Decision::from_json(&v)).collect()
    }

    /// How many decisions the feedback log holds in all.
    pub fn decision_count(&self) -> usize {
        std::fs::read_to_string(self.feedback_path()).unwrap_or_default().lines().filter(|l| !l.trim().is_empty()).count()
    }

    /// `style.md`: (the writer's part, Ghostwriter's part, decisions it was learned from).
    pub fn style(&self) -> (String, String, usize) {
        let text = std::fs::read_to_string(self.style_path()).unwrap_or_default();
        let Some((mine, learned)) = text.split_once(MARKER) else { return (text, String::new(), 0) };
        let learned = learned.trim();
        let (count, body) = match learned.strip_prefix("<!-- from ").and_then(|r| r.split_once(" decisions -->")) {
            Some((n, rest)) => (n.trim().parse().unwrap_or(0), rest.trim()),
            None => (0, learned),
        };
        (mine.to_string(), body.to_string(), count)
    }

    /// The profile as a prompt reads it: the writer's notes (without the starter text) and what
    /// was learned. `None` when there is neither.
    pub fn for_prompt(&self) -> Option<String> {
        let (mine, learned, _) = self.style();
        let mine = mine.replace(STARTER.trim_end(), "");
        let mut out = String::new();
        if !mine.trim().is_empty() {
            out.push_str("From the writer:\n");
            out.push_str(mine.trim());
        }
        if !learned.is_empty() {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str("Learned from their writing and their decisions:\n");
            out.push_str(&learned);
        }
        (!out.trim().is_empty()).then_some(out)
    }

    /// Replace Ghostwriter's part of `style.md`, keeping the writer's (a starter for a new file).
    pub fn save_learned(&self, learned: &str, from: usize) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("can't create {}: {e}", self.dir.display()))?;
        let (mine, _, _) = self.style();
        let mine = if mine.trim().is_empty() { STARTER.to_string() } else { format!("{}\n\n", mine.trim_end()) };
        let text = format!("{mine}{MARKER}\n<!-- from {from} decisions -->\n\n{}\n", learned.trim());
        let path = self.style_path();
        // Through a temporary file, so a crash never leaves half a profile.
        let tmp = self.dir.join("style.md.tmp");
        std::fs::write(&tmp, text).map_err(|e| format!("can't write {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &path).map_err(|e| format!("can't replace {}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> Profile {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        Profile::new(std::env::temp_dir().join(format!("ghostwriter-profile-test-{}-{n}", std::process::id())))
    }

    #[test]
    fn decisions_round_trip_and_skip_bulk() {
        let p = temp();
        let v = json!({"verdict": "rejected", "text": "obey", "replaces": "follow", "context": "Markets obey.", "bulk": false});
        let mut d = Decision::from_engine(&v).unwrap();
        d.why = "stronger verb".into();
        p.record(&d).unwrap();
        p.record(&Decision { accepted: true, was: String::new(), suggested: " too".into(), context: "x".into(), why: String::new() }).unwrap();
        assert!(Decision::from_engine(&json!({"verdict": "accepted", "text": "a", "replaces": "b", "bulk": true})).is_none());
        let got = p.decisions();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0], d);
        assert_eq!(got[0].line(), "- “follow” → “obey” (stronger verb)");
        assert_eq!(got[1].line(), "- add “ too”");
        assert_eq!(p.decision_count(), 2);
        let _ = std::fs::remove_dir_all(&p.dir);
    }

    #[test]
    fn learning_keeps_the_writers_notes() {
        let p = temp();
        assert_eq!(p.for_prompt(), None);
        p.save_learned("Short declarative sentences.", 4).unwrap();
        let (mine, learned, n) = p.style();
        assert!(mine.starts_with("# My writing style"));
        assert_eq!((learned.as_str(), n), ("Short declarative sentences.", 4));
        assert_eq!(p.for_prompt().as_deref(), Some("Learned from their writing and their decisions:\nShort declarative sentences."));
        // The writer adds a note above the line; learning again keeps it.
        let text = std::fs::read_to_string(p.style_path()).unwrap().replace(MARKER, &format!("I never use semicolons.\n\n{MARKER}"));
        std::fs::write(p.style_path(), text).unwrap();
        p.save_learned("Long sentences now.", 9).unwrap();
        let (mine, learned, n) = p.style();
        assert!(mine.contains("I never use semicolons."));
        assert_eq!((learned.as_str(), n), ("Long sentences now.", 9));
        assert!(p.for_prompt().unwrap().starts_with("From the writer:\nI never use semicolons."));
        let _ = std::fs::remove_dir_all(&p.dir);
    }
}
