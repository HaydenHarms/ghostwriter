# Ghostwriter roadmap: current focus

> **Last updated:** 2026-10-20 · **Change:** major (replaced WordCraft's Word-parity milestones with Ghostwriter's; those stay upstream)

The summary and milestone table are in [`ROADMAP.md`](../ROADMAP.md). WordCraft's own
milestones toward Word parity are in
[upstream's docs/roadmap.md](https://github.com/storytold/wordcraft/blob/main/docs/roadmap.md);
Ghostwriter picks them up by merging upstream, not by working on them here.

## Current focus (toward 1.1)

1. **Start Claude from the window** (G2). Writers shouldn't need a terminal: a Companion toggle,
   brief and style pickers and a status line (thinking, calls this hour, last error) in the app,
   driving the same watcher.
2. **Own settings and port** (G3), so Ghostwriter and WordCraft can be open at once.
3. **Signed Windows builds** (G1), once there's a certificate (an owner decision).

## Rules for this work

- Suggestions only: Claude never edits the writer's text unless they ask it to.
- Never at the caret, never moving it, never an undo step the writer didn't take.
- New code goes in new files; upstream files get only marked one-line hooks
  (`companion hook`, `ghostwriter brand`), so upstream merges stay easy.
