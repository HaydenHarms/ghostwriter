# Ghostwriter roadmap

**Stage: 1.0, Windows** · next: 1.1 (learns your style; start Claude from the window)

> **Last updated:** 2026-10-20 · **Change:** major (the goal restated as live Claude suggestions; WordCraft's Word-parity roadmap moved to upstream)

Ghostwriter's goal is **Claude suggesting edits in the page while you write**, with the writer
always in charge: suggestions, never silent rewrites. It isn't trying to copy Microsoft Word. The
word processor underneath is [WordCraft](https://github.com/storytold/wordcraft), and its Word
parity is tracked upstream, in WordCraft's own
[ROADMAP.md](https://github.com/storytold/wordcraft/blob/main/ROADMAP.md) and the parity documents
in [`docs/`](docs/). Ghostwriter takes those improvements by merging upstream (see
[docs/companion.md](docs/companion.md#keeping-up-with-upstream)).

## What works today (1.0.1)

| Area | State |
|---|---|
| Suggestions in the page | done: orange inline text, replaced words hidden, ✓/✗ card on hover or at the caret, keyboard accept/reject, undo |
| Live companion | done: watches the window, asks Claude after a pause, only paragraphs near the caret that changed, never at the caret |
| Brief and style guide | done: re-read on every call |
| Claude on your subscription | done: `claude -p` through Claude Code, no API key |
| Ask Claude directly | done: MCP server with built-in instructions, Claude Code skill |
| Saved files | done: suggestions are tracked changes by *Claude (suggestion)*; Word reads them |
| Windows | done: x64 and ARM64 installers and portable zips, unsigned |
| macOS, Linux | builds from source; no packages |
| Learning your style | done (unreleased): accepts and rejects noted in a feedback log; a style profile Claude rewrites from them and your paragraphs, with your own notes on top; rejected edits not repeated; MCP clients get the profile too |
| Interface languages | the suggestion commands are translated into all 22 of WordCraft's languages |

## Next

| # | Milestone | What | Estimate |
|---|---|---|---|
| G1 | Signed Windows builds | not planned: Ghostwriter is for the owner and friends, and "Run anyway" is fine | — |
| G2 | Start Claude from the window | a Companion toggle, brief and style pickers and status in the app instead of a terminal | 8–12 h |
| G3 | Own settings | separate settings folder and control port from WordCraft, so both can run at once | 3–5 h |
| G4 | macOS and Linux packages | DMG, AppImage/deb built by the release workflow | 6–10 h |
| G5 | Whole-document passes | on request: structure, argument and consistency across sections, not only the paragraph at the caret | 8–12 h |
| G6 | Learning from rejections | done: feedback log, style profile, no repeats | — |
| G7 | Explain a suggestion | a short reason on the card, from Claude's answer | 3–5 h |

## Progress log

- **2026-10-20 · learning.** The companion notes every accept and reject, keeps a style profile
  (`style.md`) Claude updates from them and your paragraphs, and stops repeating rejected edits;
  Claude is told to sharpen your prose, never write it. MCP clients get the profile too.
- **2026-10-20 · 1.0.1.** The MCP server tells every client how to suggest; Claude Code skill in
  `skills/ghostwriter/`; documentation rewritten around Ghostwriter's goal.
- **2026-10-20 · 1.0.0.** First release: inline suggestions, the companion, `suggest.*` commands,
  Windows installers.
