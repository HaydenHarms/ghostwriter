---
name: ghostwriter
description: Suggest edits in the writer's open Ghostwriter document as inline suggestions they accept or reject, through the Ghostwriter MCP server. Use when the writer asks for feedback, line edits, a tighter paragraph, a fact or citation fix, or "suggestions" on what they're writing in Ghostwriter, or asks you to review the open document.
---

# Suggesting edits in Ghostwriter

Ghostwriter shows your suggestions **in the page, in orange**, where they would go. The writer
hovers one and clicks ✓ or ✗. You use the `ghostwriter` MCP server's `execute` tool to run the
`suggest.*` commands. Unless the writer asks you to edit directly, suggest instead of editing:
they stay the author.

**It's their writing.** Help them say what they mean, better and in their own voice: a tighter
phrase, a more precise word they would use, a grammar or punctuation fix, a few connecting words
where the logic jumps. Don't write it for them: no new sentences, ideas, examples or arguments
unless they ask, and don't make their prose more formal, more hedged or more generic.

If the `ghostwriter` tools are missing, tell the writer to run
`claude mcp add ghostwriter -- ghostwriter-cli mcp --connect 127.0.0.1:7981` with Ghostwriter
open, then restart Claude Code.

## 1. Read the document

`execute {command: "suggest.context"}` returns:

```json
{"blocks": [{"index": 0, "text": "…", "style": "Heading 1", "suggestions": 0}, …],
 "caret": {"block": 4, "off": 120}}
```

`text` is each body paragraph as it reads, with your pending suggestions already in. `caret` is
where the writer is typing. For a review of the whole piece, read every block. For "this
paragraph", use the caret's block.

If the writer gave you a brief or a style guide (thesis, audience, voice), hold every suggestion
to it. The server's instructions also carry their **style profile** (what Ghostwriter has learned
about their voice and the edits they welcome or reject) and the edits they rejected recently.
Follow the profile; the writer's own notes in it win over everything else. Its file is
`style.md` in the `ghostwriter` folder of Ghostwriter's settings (`%APPDATA%\WordCraft\ghostwriter\`
on Windows); read it there if the instructions didn't include it.

## 2. Suggest

Replace words:

```json
{"command": "suggest.add", "params": {"find": "a policy failure, not a market one", "replace": "a policy choice, not a market failure"}}
```

Add words after or before a quote (include the space you need):

```json
{"command": "suggest.add", "params": {"after": "induced demand", "insert": " (the claim that new roads only fill with new traffic)"}}
```

Rules:

- Copy `find`/`after`/`before` **exactly** from the writer's own words, within one paragraph.
  Don't quote words that are already a pending suggestion.
- You may quote a whole sentence to change one word. Only the words that differ turn orange.
- No line breaks in a suggestion. For a new paragraph, suggest the sentence at the end of the one
  before.
- Be selective: a few suggestions per paragraph, each one worth a click. Fix what's wrong or
  unclear (logic, facts, clarity, flow, the brief) before touching taste.
- A suggestion brings in at most about a dozen words of your own.

## 3. When `suggest.add` fails

| Error | Do |
|---|---|
| `the quote appears N times` | quote more words, or pass `"occurrence": n` (1-based, reading order) |
| `the quote isn't in the body as plain untracked text` | the writer changed it: run `suggest.context` again |
| `the writer's caret is there` | skip it: they're typing in those words |
| `the document is protected` | tell the writer |

## 4. Tidy up

`suggest.list` returns `[{id, block, start, end, text, replaces}]`, every pending suggestion.
After the writer rewrites a passage, `suggest.withdraw {"id": n}` takes back yours that no longer
fit. `{"all": true}` takes back all of them. Leave `suggest.accept` / `suggest.reject` to the
writer unless they ask you to, for example "accept all of them". Don't suggest again what the
writer just rejected.

Finish with one line on what you suggested and why. Don't repeat the suggestions back in chat:
the writer reads them in the page.
