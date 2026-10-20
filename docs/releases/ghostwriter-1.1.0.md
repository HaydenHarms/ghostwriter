Ghostwriter is a word processor where Claude suggests edits in the page as you write. It's built on [WordCraft](https://github.com/storytold/wordcraft).

## What's new

- **The Claude button and panel.** Home › **Claude** (the little reaper ghost at the end of the Home tab) opens a panel on the right. Click **Start Claude** and Claude reads along as you write, with no terminal needed. It stops when you close the window. From the panel you can also:
  - turn suggestions on or off;
  - choose how many suggestions per paragraph and how long a pause before Claude looks;
  - pick the model: Fast, Balanced or Thorough;
  - turn learning on or off, for everything or just the document you have open;
  - see what Claude has learned about your writing, open the profile, or forget it.

  Changes apply within a second.
- **Claude learns your style by itself.** You don't describe your voice. Once you've written about a page, Claude builds a style profile from your own paragraphs: your rhythm, word choice, punctuation habits, and the habits it shouldn't "fix". It keeps refining the profile as you write and as you accept or reject its suggestions. Edits you reject don't come back.
- **It sharpens your writing; it doesn't write for you.** Claude keeps to small edits in your own words: a tighter phrase, a more precise word, grammar, a few connecting words. It never adds sentences or ideas of its own.
- **Claude Code gets your profile too.** When you ask Claude Code for suggestions over MCP, it receives your style profile and your recent rejections.
- **The latest WordCraft.** This release brings in 899 upstream improvements.

## Install (Windows)

1. Install [Claude Code](https://claude.com/claude-code) and sign in once.
2. Download `ghostwriter-1.1.0-windows-x64.msi` (or `-arm64` for ARM PCs) and run it. It upgrades an earlier Ghostwriter in place. The build isn't code-signed, so SmartScreen asks first: choose **More info → Run anyway**.
3. Open Ghostwriter, click **Claude** at the end of the Home tab, then **Start Claude**.

Your style profile is kept in `%APPDATA%\WordCraft\ghostwriter\`. Full guide: [`docs/companion.md`](https://github.com/HaydenHarms/ghostwriter/blob/main/docs/companion.md).

Licensed MIT OR Apache-2.0, like WordCraft. Copyright (c) 2026 ArtCraft Team and the WordCraft contributors.
