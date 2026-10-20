//! Learning the writer's taste: their accepts and rejects (from `suggest.decisions`) go to the
//! feedback log, and every so often Claude rewrites the learned part of their style profile from
//! those and their own paragraphs (`wordcraft_mcp::suggest::profile`).

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use wordcraft_mcp::suggest::profile::{Decision, Profile};
use wordcraft_mcp::{Backend, Remote};

use crate::claude::Claude;
use crate::prompt::{self, Proposal};
use crate::watch::{Context, is_heading};

/// Proposals remembered for their reasons.
const POSTED: usize = 40;
/// The writer's own words sent when learning, at most.
const SAMPLE_WORDS: usize = 1500;
/// After a failed update, wait this long before trying again.
const RETRY: Duration = Duration::from_secs(600);

pub struct Learner {
    pub profile: Profile,
    /// The next `suggest.decisions` sequence number to read.
    since: u64,
    /// Decisions since the profile was last learned.
    pending: usize,
    /// Learn again after this many new decisions.
    every: usize,
    posted: VecDeque<Proposal>,
    not_before: Option<Instant>,
}

fn quote(s: &str) -> String {
    let t: String = s.chars().take(60).collect();
    if s.chars().count() > 60 { format!("{t}…") } else { t }
}

impl Learner {
    /// Start learning into `dir` (default: the profile folder), from decisions made from now on.
    pub fn start(app: &mut Remote, dir: Option<PathBuf>, every: usize) -> Result<Learner, String> {
        let dir = dir.or_else(Profile::default_dir).ok_or("no settings folder for the style profile; pass --profile DIR")?;
        let profile = Profile::new(dir);
        let (_, _, from) = profile.style();
        let pending = profile.decision_count().saturating_sub(from);
        let next = app
            .call("engine.execute", json!({"command": "suggest.decisions", "params": {}}))
            .map_err(|e| format!("this Ghostwriter window can't report decisions ({e}); update Ghostwriter"))?
            .get("next")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        Ok(Learner { profile, since: next, pending, every: every.max(1), posted: VecDeque::new(), not_before: None })
    }

    /// Remember a proposal that was posted, for its reason.
    pub fn posted(&mut self, pr: &Proposal) {
        self.posted.push_back(pr.clone());
        while self.posted.len() > POSTED {
            self.posted.pop_front();
        }
    }

    fn why(&self, d: &Decision) -> String {
        self.posted
            .iter()
            .rev()
            .find(|pr| {
                let get = |k: &str| pr.params.get(k).and_then(Value::as_str).unwrap_or_default();
                let (from, to) = if get("find").is_empty() { ("", get("insert")) } else { (get("find"), get("replace")) };
                (!d.suggested.is_empty() || !d.was.is_empty()) && to.contains(d.suggested.as_str()) && from.contains(d.was.as_str())
            })
            .map(|pr| pr.why.clone())
            .unwrap_or_default()
    }

    /// Read the writer's new decisions and add them to the feedback log. Returns how many.
    pub fn poll(&mut self, app: &mut Remote, log: &dyn Fn(&str)) -> usize {
        let Ok(v) = app.call("engine.execute", json!({"command": "suggest.decisions", "params": {"since": self.since}})) else { return 0 };
        let mut n = 0;
        for r in v.get("decisions").and_then(Value::as_array).into_iter().flatten() {
            let Some(mut d) = Decision::from_engine(r) else { continue };
            d.why = self.why(&d);
            match self.profile.record(&d) {
                Ok(()) => {
                    n += 1;
                    let verb = if d.accepted { "accepted" } else { "rejected" };
                    log(&format!("noted: you {verb} “{}” → “{}”", quote(&d.was), quote(&d.suggested)));
                }
                Err(e) => log(&format!("can't save that decision: {e}")),
            }
        }
        if let Some(next) = v.get("next").and_then(Value::as_u64) {
            self.since = next;
        }
        self.pending += n;
        n
    }

    /// Whether enough has happened to learn again.
    pub fn due(&self, now: Instant) -> bool {
        self.pending >= self.every && self.not_before.is_none_or(|t| now >= t)
    }

    /// Ask Claude to update the learned profile from the feedback log and the writer's paragraphs
    /// in `ctx`.
    pub fn learn(&mut self, ctx: &Context, claude: &Claude, verbose: bool, log: &dyn Fn(&str)) -> Result<(), String> {
        let decisions = self.profile.decisions();
        let mut words = 0;
        let samples: Vec<_> = ctx
            .blocks
            .iter()
            .filter(|b| b.suggestions == 0 && !is_heading(b) && b.text.split_whitespace().count() >= 12)
            .take_while(|b| {
                words += b.text.split_whitespace().count();
                words <= SAMPLE_WORDS
            })
            .collect();
        if decisions.is_empty() && samples.is_empty() {
            return Err("nothing to learn from yet: write a few paragraphs, or accept and reject some suggestions".into());
        }
        let (mine, previous, _) = self.profile.style();
        let p = prompt::distill(&mine, &previous, &decisions, &samples);
        log(&format!("learning your style from {} decision(s) and {} paragraph(s)…", decisions.len(), samples.len()));
        let result = claude.ask(&p).and_then(|reply| {
            if verbose {
                log(&format!("reply: {}", reply.trim()));
            }
            prompt::parse_profile(&reply)
        });
        let learned = match result {
            Ok(l) => l,
            Err(e) => {
                self.not_before = Some(Instant::now() + RETRY);
                return Err(e);
            }
        };
        self.profile.save_learned(&learned, self.profile.decision_count())?;
        self.pending = 0;
        log(&format!("updated your style profile: {}", self.profile.style_path().display()));
        Ok(())
    }
}
