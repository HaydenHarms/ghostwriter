# Ghostwriter roadmap: current focus

> **Last updated:** 2026-10-20 · **Change:** major (replaced WordCraft's Word-parity milestones with Ghostwriter's; those stay upstream)

The summary and milestone table are in [`ROADMAP.md`](../ROADMAP.md). WordCraft's own
milestones toward Word parity are in
[upstream's docs/roadmap.md](https://github.com/storytold/wordcraft/blob/main/docs/roadmap.md);
Ghostwriter picks them up by merging upstream, not by working on them here.

## Current focus (toward 1.1)

1. **Learning your style** (G6): landed. Next is trying it on real writing and tuning the
   prompts, the `--learn-every` default and the profile's length.
2. **Start Claude from the window** (G2). Writers shouldn't need a terminal: a Companion toggle,
   brief and style pickers, a status line, and the style profile one click away.
3. **Own settings and port** (G3), so Ghostwriter and WordCraft can be open at once.

Signed builds (G1) are off the list: Ghostwriter is for its owner and friends.

## Rules for this work

- Suggestions only: Claude never edits the writer's text unless they ask it to.
- Enhance, don't write: small edits in the writer's own words; no new sentences or ideas.
- Never at the caret, never moving it, never an undo step the writer didn't take.
- New code goes in new files; upstream files get only marked one-line hooks
  (`companion hook`, `ghostwriter brand`), so upstream merges stay easy.
