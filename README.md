<p align="center"><img src="https://raw.githubusercontent.com/mklab-se/skiss/main/media/skiss-horizontal.png" width="600"></p>

<p align="center">
<a href="https://github.com/mklab-se/skiss/actions/workflows/ci.yml"><img src="https://github.com/mklab-se/skiss/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
<a href="https://crates.io/crates/skiss"><img src="https://img.shields.io/crates/v/skiss.svg" alt="crates.io"></a>
<a href="https://github.com/mklab-se/skiss/releases/latest"><img src="https://img.shields.io/github/v/release/mklab-se/skiss" alt="GitHub Release"></a>
<a href="https://github.com/mklab-se/homebrew-tap/blob/main/Formula/skiss.rb"><img src="https://img.shields.io/badge/dynamic/regex?url=https%3A%2F%2Fraw.githubusercontent.com%2Fmklab-se%2Fhomebrew-tap%2Fmain%2FFormula%2Fskiss.rb&search=%5Cd%2B%5C.%5Cd%2B%5C.%5Cd%2B&label=homebrew&prefix=v&color=orange" alt="Homebrew"></a>
<a href="https://github.com/mklab-se/skiss/blob/main/LICENSE"><img src="https://img.shields.io/crates/l/skiss.svg" alt="License"></a>
</p>

<!-- "What's new" callout: keep it right after the badges and refresh it on every minor/major
     release (version and a one-line summary of the headline changes). Only link CHANGELOG.md. -->
<p align="center">
  <strong>skiss</strong> now opens any Markdown file as a living board: cards, areas, arrows
  written as plain sentences, and every change saved straight back to your file.<br>
  <a href="CHANGELOG.md"><strong>What's new</strong></a>
</p>

# skiss

**skiss** (Swedish for "sketch") is an infinite canvas for your notes, stored as plain Markdown.
Think Miro or a whiteboard, but the board is a `.md` file on your own machine: you can read it in
any editor, diff it in Git, and hand it to a client.

```sh
skiss notes.md          # open (or create) a board; the prompt comes right back
skiss                   # start screen: name a file, pick a folder, or open one
skiss notes.md --theme ember
skiss notes.md --wait   # stay attached until the window closes
```

Esc twice closes the window (everything is already saved).

Try the demo board: `skiss examples/apollo.md`.

## Your Markdown is the board

skiss never shows Markdown syntax and never needs its own. The structure you already write becomes
the board:

| In the file | On the board |
| --- | --- |
| `# Title` (the only H1, first) | the board title |
| a heading with sub-headings | an **area** (a frame), nested as deep as you like |
| a heading without sub-headings | a **card**: the heading is its title, the text below its body |
| paragraphs, quotes, code, tables, images | one card each |
| a bullet list | one card per item, nested items as cards inside |
| a task list (`- [ ]`) | a checklist card with real checkboxes |

**Connections are sentences.** A list line with a link to another card's heading is drawn as an
arrow, labeled with the words in front of the link:

```markdown
### Anna Berg
Program sponsor.
- Manages [Erik Lund](#erik-lund) and [Sara Ek](#sara-ek)
- Told me to contact [Johan Holm](#johan-holm), 2026-10-14
```

Anyone reading the file sees ordinary notes. skiss sees who manages whom, who told you to contact
whom, and when. Drawing an arrow in skiss writes exactly that kind of sentence.

**Small diffs.** skiss rewrites only the bytes that changed: renaming a card changes one line,
ticking a box changes one character, a new arrow adds one line. Hand-written files survive being
opened and edited. Positions you set by dragging, and card colors, go into one HTML comment at the
end of the file (invisible in every Markdown viewer), one line per decorated card:

```markdown
<!-- skiss
Johan Holm: at 220, 120; blue
Colors: red means Blocked
-->
```

Pasted images are saved next to the file in `notes.assets/`.

## Working on the canvas

- **Autosave.** There is no save button. Changes are written a moment after you make them; if the
  file changes on disk (Git, another editor, a sync tool), the board animates to the new state.
- **Edit in place.** Double-click a card (or press Enter) and type. ⌘B, ⌘I, ⌘E, copy, paste,
  select all, undo and redo work as in any editor, and so do Markdown habits: typing `**bold**`
  turns bold. Enter in a task list adds a task; `[ ]` at the start of a line makes one.
- **Add and arrange.** The dock at the bottom has **+ Card**, **+ Area**, **Group** and **Tidy
  up**. Double-click empty space for a new card, Tab for the next one, drag cards to move them
  (into another area too; nothing else moves), ⌘G to group selected cards into a new area, 1 to 7
  for paper colors. Tidy up (⌘⇧L) lays the board out afresh.
- **Connect.** Drag from a card's border (or one of its dots) to another card, and say how they are
  connected. Click a line or its label to select it, double-click to rename it, Delete removes it.
- **Rename.** Double-click a card to edit it, an area's title or the board title to rename them.
- **Zoom.** Pinch, ⌘-scroll or a mouse wheel zooms, without limits. Far out, cards turn into a map
  of titles; close in, everything is readable. F fits everything. ⌘⇧+ makes text and lines bigger
  for sharing your screen.
- **Two looks.** Sketchbook (paper, handwriting) and Ember (MKLab's dark theme). ⌘T switches;
  skiss remembers your choice.

Press **?** in skiss for every shortcut.

## Install

See [INSTALL.md](INSTALL.md) for Homebrew, `cargo install`, `cargo binstall`, and from-source
instructions. The short version:

```sh
brew install mklab-se/tap/skiss
```

## Development

```sh
cargo fmt --all                                          # format
cargo clippy --workspace --all-targets -- -D warnings    # lint (matches CI)
cargo test --workspace                                   # test
```

`crates/skiss-core` holds the board model (Markdown in, board out, minimal-diff edits) with no UI
dependencies; `crates/skiss` is the CLI and the egui canvas. See [CLAUDE.md](CLAUDE.md) for the
architecture and [the design](docs/superpowers/specs/2026-10-20-skiss-design.md) for the reasoning.
Releases go through the [`/release`](.claude/skills/release/SKILL.md) skill: pushing a `vX.Y.Z` tag
builds auditable binaries with CycloneDX SBOMs, publishes to crates.io and updates the
[Homebrew tap](https://github.com/mklab-se/homebrew-tap).

## License

[MIT](LICENSE) © Kristofer Liljeblad. The bundled fonts (Caveat, Hanken Grotesk, Spectral,
JetBrains Mono) are under the SIL Open Font License; see `crates/skiss/fonts/`.
