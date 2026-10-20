//! Learning the writer's style without being told: Claude reads the paragraphs they write and their
//! accepts and rejects (from `suggest.decisions`, kept in the feedback log), and every so often
//! rewrites the learned part of their style profile (`wordcraft_mcp::suggest::profile`). The first
//! profile comes as soon as there's a page or so of their writing.

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use wordcraft_mcp::suggest::profile::{Decision, Profile};
use wordcraft_mcp::{Backend, Remote};

use crate::claude::Claude;
use crate::prompt::{self, Proposal};
use crate::watch::{Block, Context, is_heading};

/// Proposals remembered for their reasons.
const POSTED: usize = 40;
/// The writer's own words sent when learning, at most.
const SAMPLE_WORDS: usize = 1500;
/// After a failed update, wait this long before trying again.
const RETRY: Duration = Duration::from_secs(600);
/// At least this long between updates.
const MIN_GAP: Duration = Duration::from_secs(600);
/// Words of new writing that make the first profile (later ones wait for `--learn-words`).
const FIRST_WORDS: usize = 150;
/// Paragraph fingerprints remembered as already learned from, at most.
const MAX_SEEN: usize = 5000;
/// A paragraph shorter than this (in words) says little about a voice.
const MIN_PARAGRAPH: usize = 12;

/// A fingerprint of a paragraph's text that stays the same from run to run (FNV-1a).
fn fingerprint(text: &str) -> u64 {
    text.trim().bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

pub struct Learner {
    pub profile: Profile,
    /// The next `suggest.decisions` sequence number to read.
    since: u64,
    /// Decisions since the profile was last learned.
    pending: usize,
    /// Learn again after this many new decisions.
    every: usize,
    /// … or after this many words of new writing.
    words: usize,
    /// Paragraphs already learned from (fingerprints), kept in `learned-paragraphs.txt`.
    seen: HashSet<u64>,
    /// The feedback log, as last read or written.
    decisions: Vec<Decision>,
    /// Whether the profile has a learned part yet.
    learned: bool,
    posted: VecDeque<Proposal>,
    not_before: Option<Instant>,
}

fn quote(s: &str) -> String {
    let t: String = s.chars().take(60).collect();
    if s.chars().count() > 60 { format!("{t}…") } else { t }
}

impl Learner {
    /// Start learning into `dir` (default: the profile folder), from decisions made from now on
    /// and the writer's paragraphs.
    pub fn start(app: &mut Remote, dir: Option<PathBuf>, every: usize, words: usize) -> Result<Learner, String> {
        let dir = dir.or_else(Profile::default_dir).ok_or("no settings folder for the style profile; pass --profile DIR")?;
        let profile = Profile::new(dir);
        let next = app
            .call("engine.execute", json!({"command": "suggest.decisions", "params": {}}))
            .map_err(|e| format!("this Ghostwriter window can't report decisions ({e}); update Ghostwriter"))?
            .get("next")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let mut l = Learner {
            profile,
            since: next,
            pending: 0,
            every: every.max(1),
            words: words.max(1),
            seen: HashSet::new(),
            decisions: Vec::new(),
            learned: false,
            posted: VecDeque::new(),
            not_before: None,
        };
        l.reload();
        Ok(l)
    }

    /// Read what's been learned from the profile folder again (after the writer told the Claude
    /// panel to forget it, say).
    pub fn reload(&mut self) {
        let (_, learned, from) = self.profile.style();
        self.pending = self.profile.decision_count().saturating_sub(from);
        self.seen = std::fs::read_to_string(self.profile.dir.join("learned-paragraphs.txt"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| u64::from_str_radix(l.trim(), 16).ok())
            .collect();
        self.decisions = self.profile.decisions();
        self.learned = !learned.is_empty();
        self.not_before = None;
    }

    /// The paragraphs in `ctx` that are the writer's own writing: body text long enough to show a
    /// voice, with no suggestion waiting in it and none of Claude's accepted words.
    fn own<'a>(ctx: &'a Context, decisions: &[Decision]) -> Vec<&'a Block> {
        let claude: Vec<&str> =
            decisions.iter().filter(|d| d.accepted).map(|d| d.suggested.trim()).filter(|t| t.split_whitespace().count() >= 2).collect();
        ctx.blocks
            .iter()
            .filter(|b| b.suggestions == 0 && !is_heading(b) && b.text.split_whitespace().count() >= MIN_PARAGRAPH)
            .filter(|b| !claude.iter().any(|c| b.text.contains(c)))
            .collect()
    }

    /// Count the writer's paragraphs in `ctx` as already learned from, without learning from them
    /// (after "Forget my style": only what they write next should shape the new profile).
    pub fn pass_over(&mut self, ctx: &Context) {
        let decisions = self.decisions.clone();
        let own = Self::own(ctx, &decisions);
        self.remember(&own);
    }

    /// Words of the writer's own writing in `ctx` not learned from yet.
    fn fresh_words(&self, ctx: &Context, decisions: &[Decision]) -> usize {
        Self::own(ctx, decisions).iter().filter(|b| !self.seen.contains(&fingerprint(&b.text))).map(|b| b.text.split_whitespace().count()).sum()
    }

    fn remember(&mut self, blocks: &[&Block]) {
        let mut new: Vec<u64> = blocks.iter().map(|b| fingerprint(&b.text)).filter(|h| self.seen.insert(*h)).collect();
        if new.is_empty() {
            return;
        }
        let path = self.profile.dir.join("learned-paragraphs.txt");
        let mut all: Vec<String> = std::fs::read_to_string(&path).unwrap_or_default().lines().map(str::to_string).collect();
        all.extend(new.drain(..).map(|h| format!("{h:016x}")));
        let skip = all.len().saturating_sub(MAX_SEEN);
        let _ = std::fs::write(&path, all.get(skip..).unwrap_or_default().join("\n") + "\n");
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

    /// Read the writer's new decisions and, when `record`, add them to the feedback log. Returns
    /// how many were added.
    pub fn poll(&mut self, app: &mut Remote, record: bool, log: &dyn Fn(&str)) -> usize {
        let Ok(v) = app.call("engine.execute", json!({"command": "suggest.decisions", "params": {"since": self.since}})) else { return 0 };
        let mut n = 0;
        for r in v.get("decisions").and_then(Value::as_array).into_iter().flatten() {
            let Some(mut d) = Decision::from_engine(r).filter(|_| record) else { continue };
            d.why = self.why(&d);
            match self.profile.record(&d) {
                Ok(()) => {
                    n += 1;
                    self.decisions.push(d.clone());
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

    /// Whether enough has happened to learn again: enough new decisions, or enough new writing
    /// (a page or so for the first profile).
    pub fn due(&self, ctx: &Context, now: Instant) -> bool {
        if self.not_before.is_some_and(|t| now < t) {
            return false;
        }
        if self.pending >= self.every {
            return true;
        }
        let enough = if self.learned { self.words } else { FIRST_WORDS };
        self.fresh_words(ctx, &self.decisions) >= enough
    }

    /// Ask Claude to update the learned profile from the feedback log and the writer's paragraphs
    /// in `ctx`.
    pub fn learn(&mut self, ctx: &Context, claude: &Claude, verbose: bool, log: &dyn Fn(&str)) -> Result<(), String> {
        let decisions = self.profile.decisions();
        self.decisions.clone_from(&decisions);
        // New writing first, then what was already seen, up to the word budget.
        let mut own = Self::own(ctx, &decisions);
        own.sort_by_key(|b| self.seen.contains(&fingerprint(&b.text)));
        let mut words = 0;
        let samples: Vec<&Block> = own
            .into_iter()
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
        self.remember(&samples);
        self.learned = true;
        self.pending = 0;
        self.not_before = Some(Instant::now() + MIN_GAP);
        log(&format!("updated your style profile: {}", self.profile.style_path().display()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn para(i: usize, t: &str) -> Block {
        Block { index: i, text: t.into(), style: None, suggestions: 0 }
    }

    const MINE: &str = "I walked to the corner shop at dawn because the bread there is still warm then.";

    #[test]
    fn the_writers_own_paragraphs_leave_out_claudes_words() {
        let mut waiting = para(2, MINE);
        waiting.suggestions = 1;
        let mut heading = para(3, MINE);
        heading.style = Some("Heading1".into());
        let ctx = Context {
            blocks: vec![
                para(0, MINE),
                para(1, "Too short to tell."),
                waiting,
                heading,
                para(4, "The baker, who has run the place for thirty years, never once raised the price."),
            ],
            caret: None,
            document: None,
        };
        let accepted = Decision { accepted: true, was: "never".into(), suggested: "never once".into(), context: String::new(), why: String::new() };
        let own = Learner::own(&ctx, &[accepted]);
        assert_eq!(own.iter().map(|b| b.index).collect::<Vec<_>>(), vec![0], "short, waiting, heading and Claude's words left out");
        assert_eq!(Learner::own(&ctx, &[]).len(), 2);
    }

    #[test]
    fn fingerprints_are_stable() {
        assert_eq!(fingerprint(" abc "), fingerprint("abc"));
        assert_eq!(fingerprint("abc"), 0xe71f_a219_0541_574b);
        assert_ne!(fingerprint("abc"), fingerprint("abd"));
    }
}
