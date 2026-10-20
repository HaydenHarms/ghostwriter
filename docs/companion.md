# Ghostwriter: Claude's suggestions in the page

*This is Ghostwriter's main guide. Ghostwriter's goal is live Claude suggestions in the page; the
word processor underneath is [WordCraft](https://github.com/storytold/wordcraft), which Ghostwriter
follows from upstream. Everything here is Ghostwriter's addition, not part of upstream WordCraft.*

You write in Ghostwriter as you would in any word processor. When you pause, a small program, `wordcraft-companion`, reads the
paragraph you're on, asks Claude for a few edits, and puts them **in the page, in orange**, where
they would go. The words they replace are hidden, so you read the paragraph as it would be once
accepted. Hover a suggestion (or put the caret on it) for a card with **✓** (accept), **✗**
(reject) and the words it replaces. Keep typing the whole time; suggestions never move your caret
and never land in what you're typing.

Claude is called through Claude Code's print mode (`claude -p`), so it runs on your Claude Code
login (your subscription). No API key.

## Install (Windows)

1. **Claude Code**: install it and sign in once (`claude`, then `/login`). Check that
   `claude -p "say hi"` answers in a terminal.
2. From the repository's **Releases** page, download `ghostwriter-<version>-windows-x64.msi`
   (`-arm64` for ARM PCs) and run it. The build isn't code-signed, so SmartScreen asks first:
   **More info → Run anyway**. Ghostwriter installs beside WordCraft, not over it.
3. Start **Ghostwriter** from the Start menu or by opening a document. The installer sets
   `WORDCRAFT_CONTROL_PORT=7981`, so every window opens the port the companion uses.
4. In a terminal, start the companion with a brief for the piece:

   ```powershell
   ghostwriter-companion --brief my-essay.brief.md
   ```

   If the command isn't found, open a new terminal, or run it from
   `C:\Program Files\Ghostwriter\`. (The portable zip has the same three programs, unzipped
   anywhere. There, set the port yourself, as in step 3 below.)

## Build from source (Windows)

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

## Ask Claude Code directly (MCP)

Besides the companion watching as you type, you can ask Claude for suggestions when you want them
("tighten paragraph 3", "check my citations against the brief"). They land in the page the same
way, in orange with ✓/✗.

1. With Ghostwriter open, add its MCP server to Claude Code once:

   ```powershell
   claude mcp add ghostwriter -- ghostwriter-cli mcp --connect 127.0.0.1:7981
   ```

   (From a source build: `.\target\release\wordcraft-cli.exe` in place of `ghostwriter-cli`.)
2. Optionally, install the skill so Claude knows when to reach for it: copy
   [`skills/ghostwriter/`](../skills/ghostwriter/SKILL.md) to `%USERPROFILE%\.claude\skills\ghostwriter\`.
3. Start `claude` and ask, for example: *"Read my open Ghostwriter document and suggest edits to
   the third section against my brief in essay.brief.md."*

You don't need the skill: the server sends every client short instructions on the `suggest.*`
commands when it connects (`crates/mcp/src/suggest.rs`). The skill adds worked examples and
when to use them. Claude Desktop and other MCP clients use the same server command.

### The commands

Any client can run these (the companion uses only these):

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
- `crates/ui-egui/src/i18n/companion.tsv` (+ `companion.rs`, two lines in `i18n/mod.rs`): the
  command labels in every interface language, apart from the upstream catalogs.
- `crates/mcp/src/suggest.rs` (+ one line each in `lib.rs` and `server.rs`): the instructions
  every MCP client gets on the `suggest.*` commands.
- `apps/wordcraft-companion/`: the watcher. `Cargo.toml` (`default-members`) and
  `xtask/src/layers.rs` list it.

### Keeping up with upstream

```sh
git remote add upstream https://github.com/storytold/wordcraft.git   # once
git fetch upstream
git merge upstream/main
cargo xtask ci
```

A code conflict can only be in one of the hook lines above; keep upstream's version of the line
and put the hook back. Documentation conflicts: keep Ghostwriter's `README.md`, `ROADMAP.md`,
`docs/roadmap.md`, `AGENTS.md` and `CLAUDE.md` (`git checkout --ours <file>`, then re-apply any
upstream rule change to the agent files by hand). For the other files in `docs/`, take upstream's
version and put back the one-paragraph `**Ghostwriter:**` note under the title (and the
"Ghostwriter additions" section at the end of `docs/architecture.md`). `git grep -n "companion hook" -- '*.rs' '*.toml'` should list 14 lines afterwards.

The Ghostwriter name works the same way: the 13 lines marked `ghostwriter brand`
(`git grep -n "ghostwriter brand" -- '*.rs'`) set the window title, the About box, the
`ProductName` in the Windows program files and the "created by" field in saved DOCX and PDF
files. Program files, settings and the control-key folder keep WordCraft's names, so Ghostwriter
and WordCraft share settings. Upstream's sync also bumps the version back to its own: set it
again with `cargo xtask version set <version>` before tagging a release.

## Releasing

1. `cargo xtask version set 1.1.0`, write `docs/releases/ghostwriter-1.1.0.md`, and add the
   release to the progress log in `ROADMAP.md`.
2. Commit and push, then either `git tag v1.1.0 && git push origin v1.1.0`, or run the workflow by
   hand on `main` (Actions › Ghostwriter release › Run workflow); it creates the tag.
3. `.github/workflows/ghostwriter-release.yml` builds the Windows x64 and ARM64 installers
   (`packaging/windows/ghostwriter-package.ps1`, `ghostwriter.wxs`) and publishes
   "Ghostwriter 1.1.0" with them. Upstream's `release.yml` stays as it is and isn't used.
