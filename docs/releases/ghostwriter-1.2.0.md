Ghostwriter is a word processor where Claude suggests edits in the page as you write. It's built on [WordCraft](https://github.com/storytold/wordcraft).

## What's new

- **Ask Claude.** The Claude panel (Home › **Claude**) has three new buttons for bigger help, used only when you click them:
  - **Suggest a transition:** one sentence linking the paragraph you're in to the one before it.
  - **Suggest a missing step:** up to two sentences where your paragraph's reasoning jumps.
  - **Review structure:** notes on the whole piece's order, flow and repetition, plus a few small fixes. Each note shows the paragraph it's about, and clicking it takes you there. The notes are kept with the document.

  What Claude adds still arrives as orange suggestions you accept or reject. It's written from what your document already says, in your voice, with no new facts or claims.
- **Steadier panel.** If Claude restarts, the panel no longer waits on an old request.

## Install (Windows)

1. Install [Claude Code](https://claude.com/claude-code) and sign in once.
2. Download `ghostwriter-1.2.0-windows-x64.msi` (or `-arm64` for ARM PCs) and run it. It upgrades an earlier Ghostwriter in place. The build isn't code-signed, so SmartScreen asks first: choose **More info → Run anyway**.
3. Open Ghostwriter, click **Claude** at the end of the Home tab, then **Start Claude**.

Full guide: [`docs/companion.md`](https://github.com/HaydenHarms/ghostwriter/blob/main/docs/companion.md).

Licensed MIT OR Apache-2.0, like WordCraft. Copyright (c) 2026 ArtCraft Team and the WordCraft contributors.
