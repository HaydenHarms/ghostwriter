Ghostwriter is a fork of [WordCraft](https://github.com/storytold/wordcraft) that has Claude suggest edits as you write.

## What's new

- **Suggestions in the page.** When you pause, the companion reads the paragraph you're on, asks Claude for a few edits, and shows them inline in orange where they'd go. Hover one, or put the caret on it, for **✓** (accept) and **✗** (reject). Your caret never moves and nothing lands in what you're typing.
- **Runs on your Claude Code login.** Claude is called through `claude -p`, so you don't need an API key.
- **A brief and a style guide.** `--brief` describes the piece and `--style` describes your voice. Both are re-read on every call, so you can edit them as you go.
- **Six new commands** (`suggest.*`), available from the CLI, the control channel and MCP too.
- Everything else is WordCraft as of upstream commit `e6c5a70`.

## Install (Windows)

1. Install [Claude Code](https://claude.com/claude-code) and sign in once.
2. Download `ghostwriter-1.0.0-windows-x64.msi` (or `-arm64` for ARM PCs) and run it. The build isn't code-signed, so SmartScreen will ask first: choose **More info → Run anyway**.
3. Open Ghostwriter, then run `ghostwriter-companion --brief my-essay.brief.md` in a new terminal.

Full guide: [`docs/companion.md`](https://github.com/HaydenHarms/wordcraft/blob/main/docs/companion.md).

Ghostwriter installs beside WordCraft rather than replacing it. Both share settings, recent files and the control port 7981. The portable zips contain the same three programs, with no installer.

Licensed MIT OR Apache-2.0, like WordCraft. Copyright (c) 2026 ArtCraft Team and the WordCraft contributors.
