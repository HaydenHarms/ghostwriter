# Companion: Claude's suggestions in the page

*Fork addition, not part of upstream WordCraft.*

You write in WordCraft as usual. When you pause, a small program, `wordcraft-companion`, reads the
paragraph you're on, asks Claude for a few edits, and puts them **in the page, in orange**, where
they would go. The words they replace are hidden, so you read the paragraph as it would be once
accepted. Hover a suggestion (or put the caret on it) for a card with **✓** (accept), **✗**
(reject) and the words it replaces. Keep typing the whole time; suggestions never move your caret
and never land in what you're typing.

Claude is called through Claude Code's print mode (`claude -p`), so it runs on your Claude Code
login (your subscription). No API key.

## Set up (Windows)

1. **Claude Code**: install it and sign in once (`claude`, then `/login`). Check that
   `claude -p "say hi"` answers in a terminal.
2. **Build this fork** (once, and again after pulling changes). Install
   [Rust](https://rustup.rs) with the default MSVC toolchain (it offers the Visual Studio C++
   build tools if they're missing), then in the repository folder:

   ```powershell
   cargo build --release -p wordcraft -p wordcraft-cli -p wordcraft-companion
   ```

   The programs are in `target\release\`: `wordcraft.exe`, `wordcraft-cli.exe`,
   `wordcraft-companion.exe`.
3. **Start WordCraft with its control port.** Either every time:

   ```powershell
   .\target\release\wordcraft.exe --control 7981 my-essay.docx
   ```

   or once for good, so double-clicking a document works too:

   ```powershell
   setx WORDCRAFT_CONTROL_PORT 7981
   ```

   (Only one WordCraft window at a time can use port 7981.)
4. **Start the companion** in a terminal, with a brief for the piece:

   ```powershell
   .\target\release\wordcraft-companion.exe --brief my-essay.brief.md
   ```

   It prints what it asks and what it posts; Ctrl+C stops it. It finds the window's key by
   itself (`%APPDATA%\WordCraft\control-key.local-7981`).

## The brief and your style

`--brief FILE` is a short note on the piece: thesis, audience, tone, what each section must do.
`--style FILE` is a style guide in your voice (sentence habits, words you avoid, how you cite).
Both are read again on every call, so edit them while you write.

```markdown
Thesis: housing scarcity in American cities is a policy choice, not a market failure.
Audience: Substack readers who follow political economy. Tone: plain-spoken, a little polemical.
Section 3 must answer the "induced demand" objection before the conclusion.
```

## Using it

| | |
|---|---|
| Accept | ✓ on the card, or **Alt+Shift+Enter** with the caret in the suggestion |
| Reject | ✗ on the card, or **Alt+Shift+Backspace** |
| Undo an accept or reject | Ctrl+Z, as for any edit |
| Accept or reject them all | `suggest.accept {"all": true}` / `suggest.reject {"all": true}` (command search, CLI or MCP) |

- Suggestions arrive a few seconds after you stop typing (`--idle`, default 4 s, plus Claude's
  answer time, typically 5–10 s). Only paragraphs near the caret (`--reach`, default 2) that
  changed since Claude last saw them are looked at; headings and short paragraphs are skipped.
- A paragraph gets at most `--max` (3) suggestions per round and no new round while it has
  suggestions waiting, so accept or reject them to get more.
- Ctrl+Z can take back a suggestion that arrived after the step you undo. The companion doesn't
  ask again about text it has seen; type a little more to trigger a new round.
- Suggestions are tracked changes by the author **Claude (suggestion)**. Saving keeps them, so
  they come back when you reopen the file; Word shows them as ordinary tracked changes. Accept or
  reject them before you export or paste into Substack. A PDF export shows them as accepted, in
  orange.
- Review › Accept All / Reject All also act on them.

## Options

| Option | Default | |
|---|---|---|
| `--connect HOST:PORT` | `127.0.0.1:7981` | the window to watch |
| `--brief FILE`, `--style FILE` | | see above |
| `--model NAME` | `sonnet` | `haiku` is faster and lighter on your plan's limits, `opus` reads closer |
| `--idle SECONDS` | 4 | pause before asking |
| `--max N` | 3 | suggestions per paragraph |
| `--reach N` | 2 | paragraphs around the caret to consider |
| `--review-existing` | off | also review the text that was there at start (any paragraph) |
| `--max-per-hour N` | 60 | a ceiling on Claude calls |
| `--claude PATH` | `claude` | the Claude Code program |
| `--claude-arg ARG` | | more `claude -p` arguments, e.g. `--claude-arg=--effort=low` |
| `--once` | | review the caret's paragraph now and exit |
| `--dry-run`, `--verbose` | | print instead of posting; print Claude's whole reply |

Every call counts toward your Claude plan's usage limits. Writing steadily, expect about one call
per paragraph.

## For other clients (Claude Code over MCP, scripts)

The companion only uses commands any client can run (`wordcraft-cli mcp --connect 127.0.0.1:7981`
gives them to an interactive Claude Code session):

| Command | Params | |
|---|---|---|
| `suggest.add` | `{"find", "replace"}` or `{"after"\|"before", "insert"}`, `"occurrence"?` | quote exact body text; only the words that differ are marked; refused if the caret is inside the quoted words, if the quote isn't plain untracked text, or appears more than once without `occurrence` |
| `suggest.list` | | `[{id, block, start, end, text, replaces}]` |
| `suggest.context` | | the body paragraphs as they read (suggestions in), and the caret's paragraph |
| `suggest.accept` / `suggest.reject` | `{"id"?, "all"?}` | without either: the one at the caret; an undo step |
| `suggest.withdraw` | `{"id"?, "all"?}` | reject without an undo step (for clients) |

## How it fits in WordCraft

All code is in new files, except a few hooks, each marked `companion hook`
(`git grep -n "companion hook" -- '*.rs' '*.toml'`):

- `crates/doc/src/suggest.rs`: the author name and the orange.
- `crates/engine/src/cmd/suggest.rs`: the commands. Adding a suggestion is not an undo step and
  keeps the caret on the same character.
- `crates/layout/src/lib.rs` (`HideDeleted::hides`): replaced words are left out of the layout.
- `crates/layout/src/display.rs` (two lines): suggested words on orange, drawn as plain text.
- `crates/ui-egui/src/suggest.rs` + one call in `crates/ui-egui/src/lib.rs`: the ✓/✗ card.
- `apps/wordcraft-companion/`: the watcher. `Cargo.toml` (`default-members`) and
  `xtask/src/layers.rs` list it.

### Keeping up with upstream

```sh
git remote add upstream https://github.com/storytold/wordcraft.git   # once
git fetch upstream
git merge upstream/main
cargo xtask ci
```

A conflict can only be in one of the hook lines above; keep upstream's version of the line and
put the hook back. `git grep -n "companion hook" -- '*.rs' '*.toml'` should list 10 lines afterwards.
