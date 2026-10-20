# Ghostwriter roadmap: current focus

> **Last updated:** 2026-10-20 · **Change:** major (replaced WordCraft's Word-parity milestones with Ghostwriter's; those stay upstream)

The summary and milestone table are in [`ROADMAP.md`](../ROADMAP.md). WordCraft's own
milestones toward Word parity are in
[upstream's docs/roadmap.md](https://github.com/storytold/wordcraft/blob/main/docs/roadmap.md);
Ghostwriter picks them up by merging upstream, not by working on them here.

## Current focus (toward 1.1)

1. **Learning your style** (G6) and **the Claude panel** (G2): landed. Next is trying them on real
   writing and tuning the prompts, the learning thresholds and the profile's length.
2. **Bigger suggestions on request** (G8): buttons in the Claude panel for a transition between
   two sections, a paragraph that skips a step, and a structure pass, delivered as suggestions.
3. **Own settings and port** (G3), so Ghostwriter and WordCraft can be open at once.

Signed builds (G1) are off the list: Ghostwriter is for its owner and friends.

## Rules for this work

- Suggestions only: Claude never edits the writer's text unless they ask it to.
- Enhance, don't write: small edits in the writer's own words; no new sentences or ideas.
- Never at the caret, never moving it, never an undo step the writer didn't take.
- New code goes in new files; upstream files get only marked one-line hooks
  (`companion hook`, `ghostwriter brand`), so upstream merges stay easy.
