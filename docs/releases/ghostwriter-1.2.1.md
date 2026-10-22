Ghostwriter is a word processor where Claude suggests edits in the page as you write. It's built on [WordCraft](https://github.com/storytold/wordcraft).

## What's new

- **A real thesaurus.** Review › **Thesaurus** (or **Shift+F7**) now opens a Thesaurus pane for the selected word or the word at the caret. Before, the button did nothing and only about 30 words had synonyms.
  - It covers about 150,000 English words from [Open English WordNet 2024](https://en-word.net). Each meaning comes with a short definition, its synonyms and its antonyms, the most common meaning first.
  - Click a synonym to swap it in. It arrives in the same form and capitalisation as your word: *Improving* becomes *Bettering*, *children* becomes *kids*. Click another to swap again, or press Ctrl+Z.
  - Right-click a synonym to look it up instead. You can also type any word in the pane's search box and use the arrow to go back.
  - Right-click a word in the page for a quick **Synonyms** list from the same thesaurus.
  - It runs entirely in the app: no internet connection and no Claude calls.

## Install (Windows)

1. Install [Claude Code](https://claude.com/claude-code) and sign in once.
2. Download `ghostwriter-1.2.1-windows-x64.msi` (or `-arm64` for ARM PCs) and run it. It upgrades an earlier Ghostwriter in place. The build isn't code-signed, so SmartScreen asks first: choose **More info → Run anyway**.
3. Open Ghostwriter, click **Claude** at the end of the Home tab, then **Start Claude**.

Full guide: [`docs/companion.md`](https://github.com/HaydenHarms/ghostwriter/blob/main/docs/companion.md).

Licensed MIT OR Apache-2.0, like WordCraft. Copyright (c) 2026 ArtCraft Team and the WordCraft contributors. The thesaurus data is Open English WordNet 2024 (CC BY 4.0), derived from Princeton WordNet (WordNet License); see `NOTICE`.
