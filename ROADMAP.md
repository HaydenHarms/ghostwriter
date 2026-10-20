# Ghostwriter roadmap

**Stage: 1.2, Windows** · next: tuning on real writing; own settings (G3)

> **Last updated:** 2026-10-20 · **Change:** major (the goal restated as live Claude suggestions; WordCraft's Word-parity roadmap moved to upstream)

Ghostwriter's goal is **Claude suggesting edits in the page while you write**, with the writer
always in charge: suggestions, never silent rewrites. It isn't trying to copy Microsoft Word. The
word processor underneath is [WordCraft](https://github.com/storytold/wordcraft), and its Word
parity is tracked upstream, in WordCraft's own
[ROADMAP.md](https://github.com/storytold/wordcraft/blob/main/ROADMAP.md) and the parity documents
in [`docs/`](docs/). Ghostwriter takes those improvements by merging upstream (see
[docs/companion.md](docs/companion.md#keeping-up-with-upstream)).

## What works today (1.2.0)

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
| Learning your style | done: accepts and rejects noted in a feedback log; a style profile Claude builds from your writing by itself (first after ~150 words) and refines from your decisions; rejected edits not repeated; MCP clients get the profile too |
| Claude panel | done: Home › Claude; start Claude from the window, suggestions and learning on/off, per-document learning, model, profile preview, forget; live, no restart |
| Ask Claude | done: panel buttons for a transition, a missing step and a structure review; notes in the panel, click to jump |
| Interface languages | the suggestion commands are translated into all 22 of WordCraft's languages |

## Next

| # | Milestone | What | Estimate |
|---|---|---|---|
| G1 | Signed Windows builds | not planned: Ghostwriter is for the owner and friends, and "Run anyway" is fine | — |
| G2 | Start Claude from the window | done: the Claude panel | — |
| G3 | Own settings | separate settings folder and control port from WordCraft, so both can run at once | 3–5 h |
| G4 | macOS and Linux packages | DMG, AppImage/deb built by the release workflow | 6–10 h |
| G5 | Whole-document passes | the structure review (G8) covers order and flow; still to come: consistency of terms, facts and claims across sections | 6–10 h |
| G6 | Learning from rejections | done: feedback log, style profile, no repeats | — |
| G7 | Explain a suggestion | a short reason on the card, from Claude's answer | 3–5 h |
| G8 | Bigger suggestions on request | done: Ask Claude in the panel (transition, missing step, structure review with notes) | — |

## Progress log

- **2026-10-20 · 1.2.0.** Ask Claude (transition, missing step, structure review), released.
- **2026-10-20 · Ask Claude.** Claude panel buttons for a transition, a missing step and a
  structure review; the review's notes sit in the panel and jump to their paragraph.
- **2026-10-20 · 1.1.0.** The Claude panel, style learning and the latest WordCraft, released.
- **2026-10-20 · WordCraft merged.** 899 upstream commits (to `d1b0bfc`) with no code conflicts;
  `scripts/update-from-wordcraft.sh` now does the whole update.
- **2026-10-20 · Claude panel.** Home › Claude: start Claude from the window, switch suggestions
  and learning on or off (also per document), pick the model, see and forget what it has learned.
  The companion follows the panel live and stops with the window.
- **2026-10-20 · learning.** Claude works out your style from your own paragraphs (the first profile
  after about 150 words, refreshed as you write) and from every accept and reject, keeps it in
  `style.md`, and stops repeating rejected edits; no brief or notes needed;
  Claude is told to sharpen your prose, never write it. MCP clients get the profile too.
- **2026-10-20 · 1.0.1.** The MCP server tells every client how to suggest; Claude Code skill in
  `skills/ghostwriter/`; documentation rewritten around Ghostwriter's goal.
- **2026-10-20 · 1.0.0.** First release: inline suggestions, the companion, `suggest.*` commands,
  Windows installers.
