//! How an MCP client should use Ghostwriter's suggestions (fork addition, see
//! `docs/companion.md`): appended to the server's `instructions`, with what Ghostwriter has learned
//! about the writer ([`profile`]).

pub mod profile;
pub mod settings;

use profile::Profile;

/// Recent rejections sent to a client.
const REJECTIONS: usize = 12;
/// The most of the profile sent to a client, in bytes.
const MAX_PROFILE: usize = 6 * 1024;

/// Sent to every client in `initialize`.
pub const INSTRUCTIONS: &str = "\
Ghostwriter suggestions: while the writer has the document open, propose edits as suggestions \
they accept (✓) or reject (✗) instead of changing the text yourself, unless they ask you to edit \
directly. It's their writing: help them say what they mean in their own voice (a tighter phrase, a \
more precise word they would use, grammar, a short connective where the logic jumps), and don't \
write it for them: no new sentences, ideas or arguments unless they ask. Flow: (1) execute suggest.context → {blocks:[{index,text,style,suggestions}], \
caret:{block,off}}: the body paragraphs as they read with pending suggestions in, and where the \
writer is. (2) execute suggest.add {find:\"exact words\", replace:\"new words\"}, or \
{after|before:\"exact words\", insert:\" added words\"} (mind the spaces). Quote the writer's own \
words exactly as they stand, within one paragraph, never words that are already a pending \
suggestion; quote a whole phrase or sentence when that reads better: only the words that change \
are marked. No line breaks. (3) On an error: \"appears N times\" → quote more words or pass \
occurrence (1-based); \"isn't in the body\" → the writer changed it, run suggest.context again; \
\"the writer's caret is there\" → skip it, they're typing there. (4) suggest.list → \
[{id,block,start,end,text,replaces}]; suggest.withdraw {id} or {all:true} takes back yours that \
no longer fit. Leave suggest.accept and suggest.reject to the writer unless asked. Keep it to a \
few suggestions per paragraph, each one worth the writer's attention, and don't redo one the \
writer rejected.";

/// [`INSTRUCTIONS`], then the writer's style profile and recent rejections when Ghostwriter has
/// learned any (`style.md` and `feedback.jsonl` in the profile folder).
pub fn instructions() -> String {
    let mut out = INSTRUCTIONS.to_string();
    let Some(p) = Profile::default_dir().map(Profile::new) else { return out };
    if let Some(style) = p.for_prompt() {
        let mut cut = style.len().min(MAX_PROFILE);
        while !style.is_char_boundary(cut) {
            cut -= 1;
        }
        out.push_str(" The writer's style profile; hold every suggestion to it: <<<");
        out.push_str(style.get(..cut).unwrap_or_default());
        out.push_str(">>>");
    }
    let rejected: Vec<String> = p.decisions().iter().rev().filter(|d| !d.accepted).take(REJECTIONS).map(|d| d.line()).collect();
    if !rejected.is_empty() {
        out.push_str(" Edits this writer recently rejected; don't suggest anything like them: ");
        out.push_str(&rejected.join(" "));
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn names_only_real_commands() {
        let session = wordcraft_engine::Session::new(wordcraft_doc::Document::new());
        let ids: Vec<&str> = session.registry.all().iter().map(|c| c.id).collect();
        let named = super::INSTRUCTIONS.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.')).filter(|w| w.starts_with("suggest."));
        let mut n = 0;
        for id in named {
            assert!(ids.contains(&id), "{id} is not a command");
            n += 1;
        }
        assert!(n >= 6, "the instructions name the suggest commands");
    }
}
