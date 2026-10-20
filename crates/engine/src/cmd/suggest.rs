//! Companion suggestions (fork addition, see `docs/companion.md`): `suggest.add`, `suggest.list`,
//! `suggest.context`, `suggest.accept`, `suggest.reject` and `suggest.withdraw`.
//!
//! A suggestion is a tracked deletion of the words it replaces followed by a tracked insertion of
//! its own, both by [`AUTHOR`] (see `wordcraft_doc::suggest`). Adding or withdrawing one is not an
//! undo step and never moves the writer's caret: an assistant posts them while the writer types.
//! Accepting or rejecting one is an ordinary edit the writer can undo.
//!
//! Suggestions live in the top-level paragraphs of the body, one paragraph each.

use std::collections::VecDeque;
use std::sync::Mutex;

use serde_json::{Value, json};
use wordcraft_doc::para::OBJ;
use wordcraft_doc::suggest::{AUTHOR, is_suggestion};
use wordcraft_doc::{Block, Document, Path, Pos, Revision, RevisionKind, StoryRef};

use crate::{CmdError, CmdResult, CommandSpec, Session};

/// The writer's accepts and rejects in this process, oldest first (at most [`MAX_DECISIONS`]), for
/// `suggest.decisions`: an assistant learns from them. Withdrawals aren't the writer's, so they
/// aren't kept.
static DECISIONS: Mutex<Decisions> = Mutex::new(Decisions { next: 1, list: VecDeque::new() });
const MAX_DECISIONS: usize = 200;

struct Decisions {
    next: u64,
    list: VecDeque<Value>,
}

thread_local! {
    /// Whether the window's Claude panel is open (`suggest.panel`). Interface state, so it lives
    /// with the thread that draws the window rather than in the document.
    static PANEL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether the Claude panel is open.
pub fn panel_open() -> bool {
    PANEL.with(std::cell::Cell::get)
}

/// Open or close the Claude panel.
pub fn set_panel(open: bool) {
    PANEL.with(|p| p.set(open));
}

fn panel(_: &mut Session, v: &Value) -> CmdResult {
    let open = v.get("value").and_then(Value::as_bool).unwrap_or(!panel_open());
    set_panel(open);
    Ok(json!({"open": open}))
}

/// The longest text a suggestion may quote or add, in bytes.
const MAX_TEXT: usize = 16 * 1024;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("suggest.add", "Add Suggestion", "Review › Companion", add)
            .params(
                r#"{"find": string, "replace": string} | {"after"|"before": string, "insert": string}, "occurrence"?: n (1-based, when the quote appears more than once)"#,
            )
            .pure(),
        CommandSpec::new("suggest.list", "List Suggestions", "Review › Companion", list).pure(),
        CommandSpec::new("suggest.context", "Suggestion Context", "Review › Companion", context).pure(),
        CommandSpec::new("suggest.accept", "Accept Suggestion", "Review › Companion", |s, v| resolve(s, v, Some(true)))
            .key("Alt+Shift+Enter")
            .params(r#"{"id"?: n, "all"?: bool} (without either: the suggestion at the caret)"#),
        CommandSpec::new("suggest.reject", "Reject Suggestion", "Review › Companion", |s, v| resolve(s, v, Some(false)))
            .key("Alt+Shift+Backspace")
            .params(r#"{"id"?: n, "all"?: bool} (without either: the suggestion at the caret)"#),
        CommandSpec::new("suggest.withdraw", "Withdraw Suggestion", "Review › Companion", |s, v| resolve(s, v, None))
            .params(r#"{"id"?: n, "all"?: bool}"#)
            .pure(),
        CommandSpec::new("suggest.panel", "Claude", "Review › Claude", panel).params(r#"{"value"?: bool} (without: toggle)"#).pure(),
        CommandSpec::new("suggest.decisions", "Suggestion Decisions", "Review › Companion", decisions)
            .params(r#"{"since"?: n} → {"next": n, "decisions": [{"seq", "verdict": "accepted"|"rejected", "text", "replaces", "context", "bulk"}]}"#)
            .pure(),
    ]
}

/// One suggestion as it stands in the text: body paragraph `block`, bytes `start..end`.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    /// The lowest revision index among its runs: stable until it is accepted or rejected.
    pub id: u32,
    pub block: usize,
    pub start: usize,
    pub end: usize,
    /// The words it adds.
    pub text: String,
    /// The words it replaces.
    pub replaces: String,
}

/// Every suggestion in the body, in reading order. Adjacent ones count as one.
pub fn spans(doc: &Document) -> Vec<Span> {
    let mut out = Vec::new();
    for (block, b) in doc.body.iter().enumerate() {
        let Block::Para(p) = &**b else { continue };
        let mut open: Option<Span> = None;
        for (r, c) in p.run_ranges() {
            let piece = p.text.get(r.clone()).unwrap_or_default();
            let ins = c.ins.filter(|_| is_suggestion(doc, c.ins));
            let del = c.del.filter(|_| is_suggestion(doc, c.del));
            let Some(rev) = ins.or(del) else {
                out.extend(open.take());
                continue;
            };
            let span =
                open.get_or_insert_with(|| Span { id: rev, block, start: r.start, end: r.start, text: String::new(), replaces: String::new() });
            span.id = span.id.min(rev);
            span.end = r.end;
            if del.is_some() {
                span.replaces.push_str(piece);
            } else {
                span.text.push_str(piece);
            }
        }
        out.extend(open.take());
    }
    out
}

/// Where byte `at` of body paragraph `block` falls in the paragraph as `suggest.context` reads it
/// (tracked deletions and objects left out).
fn reading_offset(doc: &Document, block: usize, at: usize) -> usize {
    let Some(Block::Para(p)) = doc.body.get(block).map(|b| &**b) else { return 0 };
    p.run_ranges()
        .filter(|(_, c)| c.del.is_none())
        .filter_map(|(r, _)| p.text.get(r.start.min(at)..r.end.min(at)))
        .flat_map(str::chars)
        .filter(|c| *c != OBJ)
        .map(char::len_utf8)
        .sum()
}

fn span_json(doc: &Document, sp: &Span) -> Value {
    json!({"id": sp.id, "block": sp.block, "start": sp.start, "end": sp.end, "text": sp.text, "replaces": sp.replaces,
        "at": reading_offset(doc, sp.block, sp.start)})
}

fn list(s: &mut Session, _: &Value) -> CmdResult {
    Ok(Value::Array(spans(&s.doc).iter().map(|sp| span_json(&s.doc, sp)).collect()))
}

/// The sentence or so around suggestion `sp`, as the paragraph reads with it in (at most about
/// 160 bytes each side).
fn around(doc: &Document, sp: &Span) -> String {
    let Some(Block::Para(p)) = doc.body.get(sp.block).map(|b| &**b) else { return String::new() };
    let reading: String =
        p.run_ranges().filter(|(_, c)| c.del.is_none()).filter_map(|(r, _)| p.text.get(r)).flat_map(str::chars).filter(|c| *c != OBJ).collect();
    let at = reading_offset(doc, sp.block, sp.start).min(reading.len());
    let end = at.saturating_add(sp.text.len()).min(reading.len());
    let mut lo = at.saturating_sub(160);
    while !reading.is_char_boundary(lo) {
        lo += 1;
    }
    let mut hi = end.saturating_add(160).min(reading.len());
    while !reading.is_char_boundary(hi) {
        hi -= 1;
    }
    reading.get(lo..hi).unwrap_or_default().to_string()
}

fn record(records: Vec<Value>) {
    let mut d = DECISIONS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    for mut r in records {
        let seq = d.next;
        d.next = d.next.saturating_add(1);
        if let Some(o) = r.as_object_mut() {
            o.insert("seq".into(), json!(seq));
        }
        d.list.push_back(r);
        while d.list.len() > MAX_DECISIONS {
            d.list.pop_front();
        }
    }
}

fn decisions(_: &mut Session, v: &Value) -> CmdResult {
    let since = v.get("since").and_then(Value::as_u64).unwrap_or(0);
    let d = DECISIONS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let list: Vec<Value> = d.list.iter().filter(|r| r.get("seq").and_then(Value::as_u64).is_some_and(|q| q >= since)).cloned().collect();
    Ok(json!({"next": d.next, "decisions": list}))
}

/// What an assistant reads: each top-level body paragraph as it shows without markup (tracked
/// deletions and objects left out, suggestions in), and where the caret is.
fn context(s: &mut Session, _: &Value) -> CmdResult {
    let doc = &s.doc;
    let counts = spans(doc);
    let blocks: Vec<Value> = doc
        .body
        .iter()
        .enumerate()
        .filter_map(|(i, b)| {
            let Block::Para(p) = &**b else { return None };
            let text: String = p
                .run_ranges()
                .filter(|(_, c)| c.del.is_none())
                .filter_map(|(r, _)| p.text.get(r))
                .flat_map(str::chars)
                .filter(|c| *c != OBJ)
                .collect();
            let n = counts.iter().filter(|sp| sp.block == i).count();
            Some(json!({"index": i, "text": text, "style": p.props.style, "suggestions": n}))
        })
        .collect();
    let f = &s.sel.focus;
    let caret = (f.story == StoryRef::Body && f.path.0.len() == 1).then(|| json!({"block": f.path.0.first(), "off": f.off}));
    let document = s.path.as_ref().map(|p| p.display().to_string());
    Ok(json!({"blocks": blocks, "caret": caret, "document": document}))
}

fn text_param<'a>(v: &'a Value, key: &str) -> Result<Option<&'a str>, CmdError> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(t)) if t.is_empty() => Err(CmdError::Params(format!("`{key}` is empty"))),
        Some(Value::String(t)) if t.len() > MAX_TEXT => Err(CmdError::Params(format!("`{key}` is longer than {MAX_TEXT} bytes"))),
        Some(Value::String(t)) if t.contains(OBJ) => Err(CmdError::Params(format!("`{key}` holds an object character"))),
        Some(Value::String(t)) => Ok(Some(t)),
        Some(_) => Err(CmdError::Params(format!("`{key}` must be a string"))),
    }
}

/// Where in the body `quote` stands as plain, untracked text: (block, byte offset).
fn find_quote(doc: &Document, quote: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (i, b) in doc.body.iter().enumerate() {
        let Block::Para(p) = &**b else { continue };
        for (off, _) in p.text.match_indices(quote) {
            let end = off + quote.len();
            let tracked = p.run_ranges().any(|(r, c)| r.start < end && r.end > off && (c.ins.is_some() || c.del.is_some()));
            if !tracked {
                out.push((i, off));
            }
        }
    }
    out
}

/// Whether an end of the selection lies in body paragraph `block` strictly inside `lo..hi`.
fn caret_within(s: &Session, block: usize, lo: usize, hi: usize) -> bool {
    let path = Path::top(block);
    [&s.sel.anchor, &s.sel.focus].iter().any(|p| p.story == StoryRef::Body && p.path == path && lo < p.off && p.off < hi)
}

/// Shift the selection's ends in body paragraph `block` by an edit at `at` that removed `removed`
/// bytes and then added `added`. An end right at `at` stays before removed text but goes past
/// added text, so the writer types on after a suggestion that lands at the caret.
fn shift_selection(s: &mut Session, block: usize, at: usize, removed: usize, added: usize) {
    let path = Path::top(block);
    let fix = |p: &mut Pos| {
        if p.story == StoryRef::Body && p.path == path && (p.off > at || (p.off == at && added > 0)) {
            p.off = p.off.saturating_sub(removed).max(at).saturating_add(added);
        }
    };
    fix(&mut s.sel.anchor);
    fix(&mut s.sel.focus);
}

fn add(s: &mut Session, v: &Value) -> CmdResult {
    if s.doc.settings.protection.is_some() {
        return Err(CmdError::Disabled("suggest.add: the document is protected".into()));
    }
    let (quote, new, mode) = if let Some(find) = text_param(v, "find")? {
        let new = text_param(v, "replace")?.ok_or_else(|| CmdError::Params("`replace` is required with `find`".into()))?;
        (find, new, Mode::Replace)
    } else if let Some(after) = text_param(v, "after")? {
        (after, text_param(v, "insert")?.ok_or_else(|| CmdError::Params("`insert` is required with `after`".into()))?, Mode::After)
    } else if let Some(before) = text_param(v, "before")? {
        (before, text_param(v, "insert")?.ok_or_else(|| CmdError::Params("`insert` is required with `before`".into()))?, Mode::Before)
    } else {
        return Err(CmdError::Params("give `find` and `replace`, or `after` or `before` and `insert`".into()));
    };
    if new.contains(['\n', '\r', '\u{2029}', '\u{000B}']) {
        return Err(CmdError::Params("a suggestion stays within one paragraph: no line breaks".into()));
    }
    let found = find_quote(&s.doc, quote);
    let pick = match v.get("occurrence").and_then(Value::as_u64) {
        Some(n) if n >= 1 => found.get(usize::try_from(n - 1).unwrap_or(usize::MAX)).copied(),
        _ if found.len() > 1 => {
            return Err(CmdError::Failed(format!("the quote appears {} times; pass `occurrence` or quote more words", found.len())));
        }
        _ => found.first().copied(),
    };
    let Some((block, off)) = pick else {
        return Err(CmdError::Failed("the quote isn't in the body as plain untracked text (it may have changed)".into()));
    };
    let mut end = off + quote.len();
    let (mut off, mut new, mut mode) = (off, new, mode);
    if mode == Mode::Replace {
        // Only the words that change are marked: a whole sentence quoted to change one word
        // shows one orange word.
        let (p, q) = narrow(quote, new);
        new = new.get(p..new.len() - q).unwrap_or(new);
        off += p;
        end -= q;
        if off == end {
            mode = Mode::Before;
        }
    }
    let (lo, hi, at) = match mode {
        Mode::Replace => (off, end, end),
        Mode::After => (end, end, end),
        Mode::Before => (off, off, off),
    };
    if caret_within(s, block, lo, hi) {
        return Err(CmdError::Failed("the writer's caret is there".into()));
    }
    let date = super::now_iso();
    let before = s.doc.clone();
    let mut id = u32::try_from(s.doc.revisions.len()).map_err(|_| CmdError::Failed("too many revisions".into()))?;
    let del = if mode == Mode::Replace {
        s.doc.revisions.push(Revision::new(RevisionKind::Delete, AUTHOR, date.clone()));
        Some(id)
    } else {
        None
    };
    let ins = id + u32::from(del.is_some());
    s.doc.revisions.push(Revision::new(RevisionKind::Insert, AUTHOR, date));
    id = del.unwrap_or(ins);
    let edited = (|| -> Result<usize, CmdError> {
        let para = s.doc.para_mut(StoryRef::Body, &Path::top(block))?;
        let mut props = if mode == Mode::After { para.props_of_char(at.saturating_sub(1)).clone() } else { para.props_of_char(off).clone() };
        if let Some(d) = del {
            para.format(off, end, &|c| c.del = Some(d))?;
        }
        props.ins = Some(ins);
        props.del = None;
        props.fmt_change = None;
        Ok(para.insert_text(at, new, &props)?)
    })();
    let added = match edited {
        Ok(n) => n,
        Err(e) => {
            s.doc = before;
            return Err(e);
        }
    };
    shift_selection(s, block, at, 0, added);
    s.touch();
    Ok(json!({"id": id, "block": block, "start": lo, "end": at + added}))
}

/// The bytes `find` and `replace` share at their start and end, in whole words (cut after or
/// before white space), leaving at least one word of `replace` to show.
fn narrow(find: &str, replace: &str) -> (usize, usize) {
    let common_start = find.char_indices().zip(replace.chars()).take_while(|((_, a), b)| a == b).last().map_or(0, |((i, c), _)| i + c.len_utf8());
    // Cut points after white space, back from the common start.
    let starts: Vec<usize> =
        find.char_indices().filter(|(i, c)| c.is_whitespace() && i + c.len_utf8() <= common_start).map(|(i, c)| i + c.len_utf8()).collect();
    let common_end = find.chars().rev().zip(replace.chars().rev()).take_while(|(a, b)| a == b).map(|(a, _)| a.len_utf8()).sum::<usize>();
    // Cut points before white space, back from the common end (as lengths from the end).
    let ends: Vec<usize> =
        find.char_indices().filter(|(i, c)| c.is_whitespace() && find.len() - i <= common_end).map(|(i, _)| find.len() - i).collect();
    let shortest = find.len().min(replace.len());
    let mut p = starts.last().copied().unwrap_or(0);
    let mut s_list = ends; // longest first
    s_list.push(0);
    let mut k = 0;
    loop {
        let s = s_list.get(k).copied().unwrap_or(0);
        if p + s <= shortest && replace.len() - s > p {
            return (p, s);
        }
        if k + 1 < s_list.len() {
            k += 1;
        } else if p > 0 {
            // Give back one word at the start.
            p = starts.iter().rev().find(|&&x| x < p).copied().unwrap_or(0);
            k = 0;
        } else {
            return (0, 0);
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Replace,
    After,
    Before,
}

/// Accept (`Some(true)`), reject (`Some(false)`) or withdraw (`None`: reject, but as no undo step)
/// the suggestion `id`, every one (`all`), or the one at the caret.
fn resolve(s: &mut Session, v: &Value, accept: Option<bool>) -> CmdResult {
    let all = spans(&s.doc);
    let picked: Vec<Span> = if v.get("all").and_then(Value::as_bool) == Some(true) {
        all
    } else if let Some(id) = v.get("id").and_then(Value::as_u64) {
        all.into_iter().filter(|sp| u64::from(sp.id) == id).collect()
    } else {
        let f = &s.sel.focus;
        all.into_iter()
            .filter(|sp| f.story == StoryRef::Body && f.path == Path::top(sp.block) && (sp.start..=sp.end).contains(&f.off))
            .take(1)
            .collect()
    };
    if picked.is_empty() {
        return Err(CmdError::Failed("no such suggestion".into()));
    }
    let keep_new = accept == Some(true);
    let bulk = picked.len() > 1;
    let records: Vec<Value> = match accept {
        Some(a) => picked
            .iter()
            .map(|sp| {
                json!({"verdict": if a { "accepted" } else { "rejected" }, "text": sp.text, "replaces": sp.replaces,
                    "context": around(&s.doc, sp), "bulk": bulk})
            })
            .collect(),
        None => Vec::new(),
    };
    let before = s.doc.clone();
    let sel = s.sel.clone();
    // Last first, so the offsets of the earlier ones still hold.
    for sp in picked.iter().rev() {
        if let Err(e) = resolve_span(s, sp, keep_new) {
            s.doc = before;
            s.sel = sel;
            return Err(e);
        }
    }
    if accept.is_none() {
        s.touch();
    }
    record(records);
    Ok(json!({"resolved": picked.iter().map(|sp| sp.id).collect::<Vec<_>>()}))
}

fn resolve_span(s: &mut Session, sp: &Span, keep_new: bool) -> Result<(), CmdError> {
    let doc = &s.doc;
    let para = doc.para(StoryRef::Body, &Path::top(sp.block)).ok_or_else(|| CmdError::Failed("the suggestion's paragraph is gone".into()))?;
    let runs: Vec<(usize, usize, bool)> = para
        .run_ranges()
        .filter(|(r, _)| r.start >= sp.start && r.end <= sp.end)
        .filter_map(|(r, c)| {
            if is_suggestion(doc, c.ins) {
                Some((r.start, r.end, true))
            } else if is_suggestion(doc, c.del) {
                Some((r.start, r.end, false))
            } else {
                None
            }
        })
        .collect();
    for (a, b, inserted) in runs.into_iter().rev() {
        let para = s.doc.para_mut(StoryRef::Body, &Path::top(sp.block))?;
        if inserted != keep_new {
            para.delete(a, b)?;
            shift_selection(s, sp.block, a, b - a, 0);
        } else {
            para.format(a, b, &|c| {
                c.ins = None;
                c.del = None;
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Selection;

    fn session(text: &str) -> Session {
        let mut s = Session::new(Document::new());
        s.view.proofing = false;
        s.run("document.setText", &json!({"text": text})).unwrap();
        s
    }

    fn shown(s: &mut Session, block: usize) -> String {
        let c = s.run("suggest.context", &json!({})).unwrap();
        c["blocks"][block]["text"].as_str().unwrap().to_string()
    }

    fn plain(s: &Session, block: usize) -> String {
        s.doc.para(StoryRef::Body, &Path::top(block)).unwrap().text.clone()
    }

    #[test]
    fn a_replacement_reads_in_place_and_accepts() {
        let mut s = session("Markets are not natural.\nThe state builds them.");
        let r = s.run("suggest.add", &json!({"find": "builds", "replace": "constructs"})).unwrap();
        assert_eq!(r["block"], 1);
        assert_eq!(shown(&mut s, 1), "The state constructs them.");
        let list = s.run("suggest.list", &json!({})).unwrap();
        assert_eq!(list[0]["text"], "constructs");
        assert_eq!(list[0]["replaces"], "builds");
        s.run("suggest.accept", &json!({"id": r["id"]})).unwrap();
        assert_eq!(plain(&s, 1), "The state constructs them.");
        assert!(spans(&s.doc).is_empty());
        // Accepting is an undo step; undo brings the suggestion back.
        s.run("edit.undo", &json!({})).unwrap();
        assert_eq!(spans(&s.doc).len(), 1);
    }

    #[test]
    fn accepts_and_rejects_are_kept_for_assistants_withdrawals_are_not() {
        let mut s = session("The state builds them. Markets follow.");
        let start = s.run("suggest.decisions", &json!({})).unwrap()["next"].as_u64().unwrap();
        let a = s.run("suggest.add", &json!({"find": "builds", "replace": "constructs"})).unwrap();
        let b = s.run("suggest.add", &json!({"find": "follow", "replace": "obey"})).unwrap();
        s.run("suggest.reject", &json!({"id": b["id"]})).unwrap();
        s.run("suggest.accept", &json!({"id": a["id"]})).unwrap();
        let c = s.run("suggest.add", &json!({"find": "Markets", "replace": "Prices"})).unwrap();
        s.run("suggest.withdraw", &json!({"id": c["id"]})).unwrap();
        let d = s.run("suggest.decisions", &json!({"since": start})).unwrap();
        // Other tests in this process may record too: look only at this document's words.
        let mine: Vec<&Value> =
            d["decisions"].as_array().unwrap().iter().filter(|r| ["obey", "constructs", "Prices"].contains(&r["text"].as_str().unwrap())).collect();
        assert_eq!(mine.len(), 2, "{d}");
        assert_eq!(mine[0]["verdict"], "rejected");
        assert_eq!(mine[0]["replaces"], "follow");
        assert_eq!(mine[0]["context"], "The state constructs them. Markets obey.");
        assert_eq!(mine[1]["verdict"], "accepted");
        assert_eq!(mine[1]["bulk"], false);
        assert!(d["next"].as_u64().unwrap() > start);
    }

    #[test]
    fn reject_and_withdraw_restore_the_text() {
        for cmd in ["suggest.reject", "suggest.withdraw"] {
            let mut s = session("The state builds them.");
            let r = s.run("suggest.add", &json!({"find": "builds", "replace": "constructs"})).unwrap();
            s.run(cmd, &json!({"id": r["id"]})).unwrap();
            assert_eq!(plain(&s, 0), "The state builds them.", "{cmd}");
            assert!(s.doc.para(StoryRef::Body, &Path::top(0)).unwrap().runs.iter().all(|r| r.props.ins.is_none() && r.props.del.is_none()));
        }
    }

    #[test]
    fn insertions_go_after_or_before_the_quote() {
        let mut s = session("Rent rose. Wages fell.");
        s.run("caret.docEnd", &json!({})).unwrap();
        s.run("suggest.add", &json!({"after": "Rent rose.", "insert": " Meanwhile,"})).unwrap();
        s.run("suggest.add", &json!({"before": "Rent", "insert": "First, "})).unwrap();
        assert_eq!(shown(&mut s, 0), "First, Rent rose. Meanwhile, Wages fell.");
        s.run("suggest.accept", &json!({"all": true})).unwrap();
        assert_eq!(plain(&s, 0), "First, Rent rose. Meanwhile, Wages fell.");
    }

    #[test]
    fn adding_never_moves_the_caret_or_makes_an_undo_step() {
        let mut s = session("One two three. Four five.");
        s.run("caret.set", &json!({"pos": {"block": 0, "off": 25}})).unwrap();
        s.run("text.insert", &json!({"text": " Six"})).unwrap();
        s.run("suggest.add", &json!({"find": "two", "replace": "2"})).unwrap();
        // The caret is still after "Six", shifted by the added "2".
        assert_eq!(s.sel, Selection::caret(Pos::body(0, 29 + 1)));
        s.run("text.insert", &json!({"text": "!"})).unwrap();
        assert_eq!(shown(&mut s, 0), "One 2 three. Four five. Six!");
        s.run("suggest.reject", &json!({})).unwrap_err();
        s.run("suggest.withdraw", &json!({"all": true})).unwrap();
        assert_eq!(plain(&s, 0), "One two three. Four five. Six!");
        assert_eq!(s.sel.focus.off, 30);
    }

    #[test]
    fn refuses_where_the_writer_is_typing_and_ambiguous_quotes() {
        let mut s = session("the cat and the dog");
        s.run("caret.set", &json!({"pos": {"block": 0, "off": 5}})).unwrap();
        assert!(s.run("suggest.add", &json!({"find": "cat", "replace": "lynx"})).is_err());
        assert!(s.run("suggest.add", &json!({"find": "the", "replace": "a"})).is_err());
        s.run("suggest.add", &json!({"find": "the", "replace": "a", "occurrence": 2})).unwrap();
        assert_eq!(shown(&mut s, 0), "the cat and a dog");
        assert!(s.run("suggest.add", &json!({"find": "dog", "replace": "x\ny"})).is_err());
        assert!(s.run("suggest.add", &json!({"find": "absent", "replace": "y"})).is_err());
        // Already suggested text can't be quoted again.
        assert!(s.run("suggest.add", &json!({"find": "the dog", "replace": "y"})).is_err());
    }

    #[test]
    fn only_the_changed_words_are_marked() {
        let cut = |f: &str, r: &str| {
            let (p, s) = narrow(f, r);
            (f[p..f.len() - s].to_string(), r[p..r.len() - s].to_string())
        };
        assert_eq!(cut("blaming it is a bit like blaming a fish.", "blaming it is like blaming a fish."), ("a bit like".into(), "like".into()));
        assert_eq!(
            cut("say that the market cannot keep up.", "say the market can't keep up."),
            ("that the market cannot".into(), "the market can't".into())
        );
        assert_eq!(cut("builds", "constructs"), ("builds".into(), "constructs".into()));
        assert_eq!(cut("rents rose", "rents rose sharply"), ("rose".into(), "rose sharply".into()));
        assert_eq!(cut("très bien", "très très bien"), ("bien".into(), "très bien".into()));
        let words = ["", " ", "a", "é", "日本", "a b", " a ", "a  b", "é é", "x y z", "日本 語", "\t", "ab ab"];
        for f in words {
            for r in words {
                let (p, q) = narrow(f, r);
                assert!(f.get(p..f.len().saturating_sub(q)).is_some() && r.get(p..r.len().saturating_sub(q)).is_some(), "{f:?} {r:?}");
                assert!(p + q <= f.len().min(r.len()), "{f:?} {r:?}");
            }
        }
        let mut s = session("Blaming it is a bit like blaming a fish.");
        s.run("caret.docEnd", &json!({})).unwrap();
        s.run("suggest.add", &json!({"find": "Blaming it is a bit like blaming a fish.", "replace": "Blaming it is like blaming a fish."})).unwrap();
        let sp = spans(&s.doc);
        assert_eq!((sp[0].text.as_str(), sp[0].replaces.as_str()), ("like", "a bit like"));
        assert_eq!(shown(&mut s, 0), "Blaming it is like blaming a fish.");
    }

    #[test]
    fn a_suggestion_at_the_caret_puts_the_caret_after_it() {
        let mut s = session("Prices went up.");
        s.run("caret.docEnd", &json!({})).unwrap();
        s.run("suggest.add", &json!({"find": "went up.", "replace": "soared."})).unwrap();
        s.run("text.insert", &json!({"text": " Then"})).unwrap();
        assert_eq!(shown(&mut s, 0), "Prices soared. Then");
        s.run("suggest.reject", &json!({"all": true})).unwrap();
        assert_eq!(plain(&s, 0), "Prices went up. Then");
    }

    #[test]
    fn accept_at_the_caret() {
        let mut s = session("Prices went up a lot.");
        s.run("suggest.add", &json!({"find": "went up a lot", "replace": "soared"})).unwrap();
        let sp = spans(&s.doc).remove(0);
        s.run("caret.set", &json!({"pos": {"block": 0, "off": sp.end}})).unwrap();
        s.run("suggest.accept", &json!({})).unwrap();
        assert_eq!(plain(&s, 0), "Prices soared.");
        assert_eq!(s.sel.focus.off, "Prices soared".len());
    }

    #[test]
    fn survive_a_save_as_tracked_changes() {
        let mut s = session("The state builds them.");
        s.run("suggest.add", &json!({"find": "builds", "replace": "constructs"})).unwrap();
        let bytes = wordcraft_docx::write(&s.doc).unwrap();
        let back = wordcraft_docx::read(&bytes).unwrap();
        let sp = spans(&back);
        assert_eq!(sp.len(), 1);
        assert_eq!(sp[0].text, "constructs");
    }
}
