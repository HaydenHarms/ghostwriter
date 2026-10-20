Ghostwriter is a word processor where Claude suggests edits in the page as you write. It's built on [WordCraft](https://github.com/storytold/wordcraft).

## What's new

- **Ask Claude directly.** Claude Code, Claude Desktop and other MCP clients can now suggest edits in your open document. The server explains the `suggest.*` commands to Claude when it connects, so its edits arrive as orange ✓/✗ suggestions instead of changes to your text. To set it up, run this once with Ghostwriter open:

  ```powershell
  claude mcp add ghostwriter -- ghostwriter-cli mcp --connect 127.0.0.1:7981
  ```

- **A Claude Code skill.** The portable zips include `skill-ghostwriter\`, which adds worked examples and tells Claude when to use it. Copy it to `%USERPROFILE%\.claude\skills\ghostwriter\`.
- **A lighter orange.** Suggestions sit on a paler, more see-through orange wash, so they're easier to read through.
- **Companion messages say Ghostwriter.** `ghostwriter-companion --help` and its error messages use the Ghostwriter name.
- **Rewritten documentation.** The README, roadmap and guides now describe Ghostwriter's goal: live Claude suggestions, not a copy of Word.
- **Installer links** in Apps & features point to [HaydenHarms/ghostwriter](https://github.com/HaydenHarms/ghostwriter).

## Install (Windows)

1. Install [Claude Code](https://claude.com/claude-code) and sign in once.
2. Download `ghostwriter-1.0.1-windows-x64.msi` (or `-arm64` for ARM PCs) and run it. This upgrades 1.0.0 in place. The build isn't code-signed, so SmartScreen asks first: choose **More info → Run anyway**.
3. Open Ghostwriter, then in a new terminal run `ghostwriter-companion --brief my-essay.brief.md`. You can also add the MCP server above and ask Claude yourself.

Full guide: [`docs/companion.md`](https://github.com/HaydenHarms/ghostwriter/blob/main/docs/companion.md).

Licensed MIT OR Apache-2.0, like WordCraft. Copyright (c) 2026 ArtCraft Team and the WordCraft contributors.
