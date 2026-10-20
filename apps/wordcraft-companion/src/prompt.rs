//! What Claude is asked, and reading its answer.

use serde_json::Value;

use wordcraft_mcp::suggest::profile::Decision;

use crate::watch::{Block, Context};

/// The instructions; the brief, style guide and paragraphs follow.
const INSTRUCTIONS: &str = r#"You are an editor sitting beside the writer in their word processor. This is THEIR writing: your job is to help them say what they mean, better and more fully in their own voice, never to write it for them. The writer keeps typing while you work. Propose at most MAX small edits to the TARGET paragraph only. Each appears inline in the document, highlighted, and the writer accepts or rejects it with one click, so it must read naturally in place.

Good suggestions are small and sound like the writer: cutting a redundant word or phrase, a more precise word they would use, untangling a sentence, fixing grammar or punctuation, a few connecting words where the logic jumps. Don't add ideas, claims, examples or sentences of your own; don't make their prose more formal, more hedged or more generic; don't change a habit their style profile calls deliberate. If the paragraph works, return no suggestions: silence is better than noise.

Rules:
- "find" is copied exactly, character for character, from the TARGET paragraph: a phrase or one sentence, unique within it. "replace" is what it becomes.
- For a pure addition use "after" (exact text from the TARGET paragraph) and "insert" (the new text, with its leading space).
- A suggestion brings in at most about a dozen words that aren't already in the TARGET paragraph.
- No line breaks. Suggestions must not overlap.
- CARET_RULE
- "why" is a short reason (under 12 words).

Reply with JSON only, no prose and no code fence:
{"suggestions": [{"find": "...", "replace": "...", "why": "..."}, {"after": "...", "insert": "...", "why": "..."}]}"#;

pub struct Material<'a> {
    pub brief: Option<&'a str>,
    pub style: Option<&'a str>,
    /// What Ghostwriter has learned about the writer (`style.md`).
    pub profile: Option<&'a str>,
    /// Their decisions on earlier suggestions, oldest first.
    pub decisions: &'a [Decision],
    pub max: usize,
}

/// Recent rejections and acceptances shown to Claude.
const REJECTED: usize = 15;
const ACCEPTED: usize = 8;

/// The prompt for paragraph `target` of `ctx`.
pub fn build(ctx: &Context, target: usize, m: &Material) -> Option<String> {
    let t = ctx.block(target)?;
    let finished = t.text.trim_end().ends_with(['.', '!', '?', '"', '”', '’', ')', ':']);
    let caret_rule = if ctx.caret == Some(target) && !finished {
        "The writer's caret is in this paragraph and they may still be writing its last sentence: leave that sentence alone."
    } else {
        "The writer has moved on from this paragraph."
    };
    let mut p = INSTRUCTIONS.replace("MAX", &m.max.to_string()).replace("CARET_RULE", caret_rule);
    if let Some(b) = m.brief.filter(|b| !b.trim().is_empty()) {
        p.push_str("\n\n## The writer's brief for this piece\n");
        p.push_str(b.trim());
    }
    if let Some(s) = m.style.filter(|s| !s.trim().is_empty()) {
        p.push_str("\n\n## The writer's style guide\n");
        p.push_str(s.trim());
    }
    if let Some(s) = m.profile.filter(|s| !s.trim().is_empty()) {
        p.push_str("\n\n## The writer's style profile\n");
        p.push_str(s.trim());
    }
    let recent = |accepted: bool, n: usize| -> Vec<String> {
        let mut v: Vec<String> = m.decisions.iter().rev().filter(|d| d.accepted == accepted).take(n).map(Decision::line).collect();
        v.reverse();
        v
    };
    let rejected = recent(false, REJECTED);
    if !rejected.is_empty() {
        p.push_str("\n\n## Edits this writer rejected (don't suggest anything like these)\n");
        p.push_str(&rejected.join("\n"));
    }
    let accepted = recent(true, ACCEPTED);
    if !accepted.is_empty() {
        p.push_str("\n\n## Edits this writer accepted (this kind is welcome)\n");
        p.push_str(&accepted.join("\n"));
    }
    let outline: Vec<&str> = ctx
        .blocks
        .iter()
        .filter(|b| b.style.as_deref().is_some_and(|s| s.to_ascii_lowercase().starts_with("heading")))
        .map(|b| b.text.as_str())
        .collect();
    if !outline.is_empty() {
        p.push_str("\n\n## Outline so far\n");
        for h in outline {
            p.push_str("- ");
            p.push_str(h);
            p.push('\n');
        }
    }
    let pos = ctx.blocks.iter().position(|b| b.index == target)?;
    let before: Vec<&str> =
        ctx.blocks.get(pos.saturating_sub(3)..pos).unwrap_or_default().iter().map(|b| b.text.as_str()).filter(|t| !t.is_empty()).collect();
    if !before.is_empty() {
        p.push_str("\n\n## Paragraphs before (context only, don't edit)\n");
        p.push_str(&before.join("\n\n"));
    }
    p.push_str("\n\n## TARGET paragraph\n");
    p.push_str(&t.text);
    if let Some(next) = ctx.blocks.get(pos + 1).filter(|b| !b.text.is_empty()) {
        p.push_str("\n\n## Paragraph after (context only, don't edit)\n");
        p.push_str(&next.text);
    }
    Some(p)
}

/// One edit Claude proposes: `suggest.add` parameters, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Proposal {
    pub params: Value,
    pub why: String,
}

/// Read Claude's reply: the output of `claude -p --output-format json` (or its bare text).
pub fn parse(stdout: &str) -> Result<Vec<Proposal>, String> {
    let outer: Option<Value> = serde_json::from_str(stdout.trim()).ok();
    if let Some(o) = &outer
        && o.get("is_error").and_then(Value::as_bool) == Some(true)
    {
        return Err(format!("claude: {}", o.get("result").and_then(Value::as_str).unwrap_or("error")));
    }
    let text = outer.as_ref().and_then(|o| o.get("result")).and_then(Value::as_str).unwrap_or(stdout);
    let (Some(a), Some(b)) = (text.find('{'), text.rfind('}')) else { return Err(format!("no JSON in the reply: {}", short(text))) };
    let inner: Value =
        serde_json::from_str(text.get(a..=b).unwrap_or_default()).map_err(|e| format!("bad JSON in the reply ({e}): {}", short(text)))?;
    let list = inner.get("suggestions").and_then(Value::as_array).cloned().unwrap_or_default();
    Ok(list
        .into_iter()
        .filter_map(|s| {
            let str_of = |k: &str| s.get(k).and_then(Value::as_str).filter(|t| !t.is_empty()).map(str::to_string);
            let why = str_of("why").unwrap_or_default();
            let params = match (str_of("find"), str_of("replace"), str_of("after"), str_of("insert")) {
                (Some(f), Some(r), _, _) if f != r => serde_json::json!({"find": f, "replace": r}),
                (_, _, Some(a), Some(i)) => serde_json::json!({"after": a, "insert": i}),
                _ => return None,
            };
            Some(Proposal { params, why })
        })
        .collect())
}

fn short(t: &str) -> String {
    t.chars().take(200).collect()
}

/// Whether `pr` repeats an edit the writer rejected: it takes out the same words and puts in the
/// same words. Edits of a few letters ("the" → "a") don't count, so they don't block every "the".
pub fn was_rejected(pr: &Proposal, decisions: &[Decision]) -> bool {
    let get = |k: &str| pr.params.get(k).and_then(Value::as_str).unwrap_or_default();
    let (from, to) = (get("find"), if get("find").is_empty() { get("insert") } else { get("replace") });
    decisions.iter().filter(|d| !d.accepted).any(|d| {
        let (was, new) = (d.was.trim(), d.suggested.trim());
        if was.chars().count().max(new.chars().count()) < 4 {
            return false;
        }
        match (was.is_empty(), new.is_empty()) {
            (true, _) => from.is_empty() && to.contains(new),
            (false, true) => from.contains(was) && !to.contains(was),
            (false, false) => from.contains(was) && to.contains(new),
        }
    })
}

const DISTILL: &str = r#"You keep a style profile for a writer who uses an AI editor that suggests small edits while they write. The editor's job is to help them write better in their own voice, never to write for them. The writer won't describe their style; you work it out. From their own paragraphs (the main evidence) and their decisions on the editor's suggestions (when there are any), write the profile the editor reads before every suggestion.

Describe, concretely and with short examples from their text:
- Voice: sentence length and rhythm, diction, register, punctuation habits, how they argue, cite and transition.
- Habits to keep: choices that look deliberate, which the editor must not "fix".
- Edits they welcome: the kinds of suggestions they accepted (or, before any, what their writing would gain from).
- Edits to avoid: the kinds they rejected, generalised (not just the instances), and changes that would flatten their voice.

Update the previous profile rather than starting over: keep what still holds, correct what the new evidence contradicts. Describe the writer, not the topic of these paragraphs. Don't repeat the writer's own notes, and never contradict them: they win. Under 350 words, Markdown with the four headings above, no preamble. Reply with the profile only."#;

/// The prompt that asks Claude to update the learned profile.
pub fn distill(own_notes: &str, previous: &str, decisions: &[Decision], samples: &[&Block]) -> String {
    let mut p = DISTILL.to_string();
    if !own_notes.trim().is_empty() {
        p.push_str("\n\n## The writer's own notes\n");
        p.push_str(own_notes.trim());
    }
    if !previous.trim().is_empty() {
        p.push_str("\n\n## Previous profile\n");
        p.push_str(previous.trim());
    }
    let take = |accepted: bool| -> Vec<String> {
        let mut v: Vec<String> = decisions
            .iter()
            .rev()
            .filter(|d| d.accepted == accepted)
            .take(40)
            .map(|d| {
                let ctx: String = d.context.chars().take(200).collect();
                if ctx.is_empty() { d.line() } else { format!("{} in: “{ctx}”", d.line()) }
            })
            .collect();
        v.reverse();
        v
    };
    for (accepted, title) in [(true, "Suggestions they accepted"), (false, "Suggestions they rejected")] {
        let lines = take(accepted);
        if !lines.is_empty() {
            p.push_str(&format!("\n\n## {title}\n"));
            p.push_str(&lines.join("\n"));
        }
    }
    if !samples.is_empty() {
        p.push_str("\n\n## Paragraphs the writer wrote\n");
        p.push_str(&samples.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("\n\n"));
    }
    p
}

/// The profile from Claude's reply (the output of `claude -p --output-format json`, or bare text).
pub fn parse_profile(stdout: &str) -> Result<String, String> {
    let outer: Option<Value> = serde_json::from_str(stdout.trim()).ok();
    if let Some(o) = &outer
        && o.get("is_error").and_then(Value::as_bool) == Some(true)
    {
        return Err(format!("claude: {}", o.get("result").and_then(Value::as_str).unwrap_or("error")));
    }
    let text = outer.as_ref().and_then(|o| o.get("result")).and_then(Value::as_str).unwrap_or(stdout).trim();
    let text = text.strip_prefix("```markdown").or_else(|| text.strip_prefix("```md")).or_else(|| text.strip_prefix("```")).unwrap_or(text);
    let text = text.strip_suffix("```").unwrap_or(text).trim();
    if text.is_empty() { Err("an empty profile".into()) } else { Ok(text.to_string()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_reply_inside_claude_json() {
        let reply = serde_json::json!({"type": "result", "is_error": false, "result":
            "```json\n{\"suggestions\": [{\"find\": \"builds\", \"replace\": \"constructs\", \"why\": \"precise\"}, {\"after\": \"them.\", \"insert\": \" Always.\"}, {\"find\": \"same\", \"replace\": \"same\"}, {\"bogus\": 1}]}\n```"});
        let got = parse(&reply.to_string()).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].params, serde_json::json!({"find": "builds", "replace": "constructs"}));
        assert_eq!(got[0].why, "precise");
        assert_eq!(got[1].params, serde_json::json!({"after": "them.", "insert": " Always."}));
        assert!(parse(r#"{"is_error": true, "result": "limit reached"}"#).is_err());
        assert!(parse("nothing here").is_err());
        assert_eq!(parse(r#"{"suggestions": []}"#).unwrap(), vec![]);
    }

    #[test]
    fn the_prompt_holds_the_target_brief_and_context() {
        let b = |i: usize, t: &str, s: Option<&str>| Block { index: i, text: t.into(), style: s.map(str::to_string), suggestions: 0 };
        let ctx = Context {
            blocks: vec![b(0, "Rents", Some("Heading1")), b(1, "First para.", None), b(2, "Target para.", None), b(3, "Next.", None)],
            caret: Some(2),
            document: None,
        };
        let p = build(&ctx, 2, &Material { brief: Some("Argue that rent is a tax."), style: None, profile: None, decisions: &[], max: 3 }).unwrap();
        assert!(p.contains("## TARGET paragraph\nTarget para."));
        assert!(p.contains("Argue that rent is a tax."));
        assert!(p.contains("- Rents"));
        assert!(p.contains("First para."));
        assert!(!p.contains("leave that sentence alone"), "the paragraph ends in a full stop");
        let mut unfinished = ctx.clone();
        if let Some(b) = unfinished.blocks.get_mut(2) {
            b.text = "Target para and".into();
        }
        assert!(
            build(&unfinished, 2, &Material { brief: None, style: None, profile: None, decisions: &[], max: 3 })
                .unwrap()
                .contains("leave that sentence alone")
        );
        assert!(p.contains("at most 3"));
        assert!(build(&ctx, 9, &Material { brief: None, style: None, profile: None, decisions: &[], max: 3 }).is_none());
    }

    fn dec(accepted: bool, was: &str, suggested: &str) -> Decision {
        Decision { accepted, was: was.into(), suggested: suggested.into(), context: format!("It {was} here."), why: String::new() }
    }

    #[test]
    fn the_prompt_holds_the_profile_and_decisions() {
        let ctx = Context { blocks: vec![Block { index: 0, text: "Target para.".into(), style: None, suggestions: 0 }], caret: None, document: None };
        let ds = [dec(false, "utilize", "use"), dec(true, "very big", "huge")];
        let p = build(&ctx, 0, &Material { brief: None, style: None, profile: Some("Writes in short bursts."), decisions: &ds, max: 3 }).unwrap();
        assert!(p.contains("## The writer's style profile\nWrites in short bursts."));
        assert!(p.contains("rejected (don't suggest anything like these)\n- “utilize” → “use”"));
        assert!(p.contains("accepted (this kind is welcome)\n- “very big” → “huge”"));
        let d = distill("No semicolons.", "Old profile.", &ds, &[&ctx.blocks[0]]);
        assert!(d.contains("## The writer's own notes\nNo semicolons."));
        assert!(d.contains("## Previous profile\nOld profile."));
        assert!(d.contains("“utilize” → “use” in: “It utilize here.”"));
        assert!(d.contains("## Paragraphs the writer wrote\nTarget para."));
    }

    #[test]
    fn rejected_edits_are_not_suggested_again() {
        let ds = [dec(false, "utilize", "use"), dec(false, "the", "a"), dec(false, "", " indeed")];
        let pr = |v: serde_json::Value| Proposal { params: v, why: String::new() };
        assert!(was_rejected(&pr(serde_json::json!({"find": "we utilize roads", "replace": "we use roads"})), &ds));
        assert!(!was_rejected(&pr(serde_json::json!({"find": "the road", "replace": "a road"})), &ds), "too short to block");
        assert!(was_rejected(&pr(serde_json::json!({"after": "roads.", "insert": " indeed"})), &ds));
        assert!(!was_rejected(&pr(serde_json::json!({"find": "roads", "replace": "streets"})), &ds));
    }

    #[test]
    fn reads_the_profile_reply() {
        let reply = serde_json::json!({"is_error": false, "result": "```markdown\n## Voice\nTerse.\n```"});
        assert_eq!(parse_profile(&reply.to_string()).unwrap(), "## Voice\nTerse.");
        assert!(parse_profile(r#"{"is_error": false, "result": "  "}"#).is_err());
    }
}
