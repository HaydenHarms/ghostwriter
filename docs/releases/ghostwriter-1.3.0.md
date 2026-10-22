Ghostwriter is a word processor where Claude suggests edits in the page when you ask. It's built on [WordCraft](https://github.com/storytold/wordcraft).

## What's new

- **Claude works only when you ask.** Before, Claude read along all the time: it asked for suggestions every time you paused and rewrote your style profile in the background, which used your Claude plan steadily. Now nothing goes to Claude while you just write.
  - **Suggest edits** (in the Claude panel, or **Alt+Shift+S** anywhere in the page) asks for a few edits to the paragraph you're in. They arrive in orange, as before.
  - **Update my style** (Claude panel › Your style) rewrites your style profile when you click it. Your accepts and rejects are still noted as you go, without calling Claude, and the next update folds them in.
  - Suggest a transition, Suggest a missing step and Review structure work as before.
  - Live suggestions are still there if you want them: turn on **Suggest edits as I write**. It starts off, even if you had it on before.
- The panel's status reads **"Claude is ready: it suggests only when you ask"** while it waits.

## Install (Windows)

1. Install [Claude Code](https://claude.com/claude-code) and sign in once.
2. Download `ghostwriter-1.3.0-windows-x64.msi` (or `-arm64` for ARM PCs) and run it. It upgrades an earlier Ghostwriter in place. The build isn't code-signed, so SmartScreen asks first: choose **More info → Run anyway**.
3. Open Ghostwriter, click **Claude** at the end of the Home tab, then **Start Claude**. Press **Alt+Shift+S** when you want edits.

Full guide: [`docs/companion.md`](https://github.com/HaydenHarms/ghostwriter/blob/main/docs/companion.md).

Licensed MIT OR Apache-2.0, like WordCraft. Copyright (c) 2026 ArtCraft Team and the WordCraft contributors. The thesaurus data is Open English WordNet 2024 (CC BY 4.0), derived from Princeton WordNet (WordNet License); see `NOTICE`.
