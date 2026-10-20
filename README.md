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

<h3 align="center">A whiteboard for your thinking. A Markdown file on your disk.</h3>

<p align="center">
<img src="https://raw.githubusercontent.com/mklab-se/skiss/main/media/screenshots/studio.jpg" alt="A painter's notebook in skiss: areas for ideas, works, poems, materials and shows, with photos of paintings, checklists and a few arrows">
</p>

**skiss** (Swedish for "sketch") turns any Markdown file into an infinite canvas you can see,
arrange and connect, like Miro or a whiteboard. The difference: there is no account, no cloud and
no export. The board *is* your `.md` file. Open it in any editor, diff it in Git, send it to a
colleague, and it reads like ordinary notes.

```sh
skiss notes.md
```

That's it: the window opens, the board appears, and your terminal is free again. Everything you do
is saved to the file as you go.

## Why skiss

- **Your notes stay yours.** Plain Markdown in a folder you own, readable in 50 years. skiss adds
  nothing you can't read without it.
- **See the shape of your thinking.** Headings become cards and areas, lists become checklists,
  links become arrows. A file you have been growing for months suddenly has a map.
- **Write like you talk.** An arrow is just a sentence: `- Reports to [Amara Okafor](#amara-okafor)`.
  Anyone reading the file understands it; skiss draws it.
- **Git loves it.** skiss changes only the bytes you changed. Tick a box and the diff is one
  character. Your hand-written files survive being opened.
- **Beautiful and calm.** Smooth, springy animation when something moves, complete stillness when
  nothing does, and two themes: a warm paper sketchbook and a dark, glowing Ember.
- **Fast and native.** One binary, written in Rust, for macOS, Linux and Windows. No browser,
  no Electron.

## See it in action

### A notebook that grew for two years

The painter's notebook above holds ideas, poems, finished paintings, suppliers, shows and a journal.
Zoomed out it is a map; zoom in and every card is there in full, photos included.

<p align="center">
<img src="https://raw.githubusercontent.com/mklab-se/skiss/main/media/screenshots/studio-closeup.jpg" alt="A close-up of the Works area: cards with photos of abstract paintings, sizes and notes, arrows from the ideas they came from">
</p>

### An org chart that draws itself

Write who reports to whom as sentences, and skiss recognizes the hierarchy and lays it out as a tree.
No diagram tool, no boxes to drag: add a person and the chart rearranges.

<p align="center">
<img src="https://raw.githubusercontent.com/mklab-se/skiss/main/media/screenshots/orgchart.jpg" alt="An org chart of a fictional company with portraits: the CEO at the top, the CTO below, and four teams arranged as trees">
</p>

```markdown
### Mei Lin Chen
![Portrait of Mei Lin Chen](orgchart.assets/mei-lin-chen.jpg)

**Engineering Manager**. Runs planning, hiring and the weekly demo.
- Reports to [Lukas Brandt](#lukas-brandt)
```

### Long writing reads like a book

A card full of prose flows into columns at a comfortable line length, so a two-page chapter is a
spread you can read, not a narrow scroll. Nothing is ever cut off or hidden behind a click.

<p align="center">
<img src="https://raw.githubusercontent.com/mklab-se/skiss/main/media/screenshots/stockholm-closeup.jpg" alt="A chapter about the history of Stockholm laid out in three columns of handwritten text">
</p>

### A project board with structure

The Apollo program as a working board: people and centers, the Saturn V, test flights, risks with
colors that mean something, and the decisions that connect it all.

<p align="center">
<img src="https://raw.githubusercontent.com/mklab-se/skiss/main/media/screenshots/apollo.jpg" alt="A board about the Apollo program with areas for people, the Saturn V, spacecraft, the test program, Apollo 11 and risks">
</p>

### Two moods

Sketchbook for paper and handwriting, Ember for a dark room. ⌘T switches.

<p align="center">
<img src="https://raw.githubusercontent.com/mklab-se/skiss/main/media/screenshots/ember.jpg" alt="The painter's notebook in the dark Ember theme">
</p>

## Get started

**1. Install** (macOS and Linux):

```sh
brew install mklab-se/tap/skiss
```

Or with Rust: `cargo install skiss` (or `cargo binstall skiss` for a prebuilt binary). Windows,
prebuilt downloads and SBOMs are in [INSTALL.md](INSTALL.md).

**2. Open the samples.** Clone the repository and open any of them:

```sh
git clone https://github.com/mklab-se/skiss && cd skiss
skiss examples/studio.md       # a painter's notebook, two years of it
skiss examples/orgchart.md     # an org chart that lays itself out
skiss examples/apollo.md       # a project board
skiss examples/stockholm.md    # long writing in a few big cards
```

**3. Make your own.** Point skiss at any Markdown file, new or old:

```sh
skiss ideas.md        # a new file is created; an existing one opens as a board
skiss                 # a start screen to name a file or pick a folder
```

Double-click anywhere to add a card, drag cards around, drag from a card's edge to another card to
connect them. Press **?** for every shortcut. Press Esc twice to close; everything is already saved.

## How your Markdown becomes a board

skiss never shows Markdown syntax, and never needs its own. The structure you already write is the
board:

| In the file | On the board |
| --- | --- |
| `# Title` (the first and only H1) | the board title |
| a heading with sub-headings | an **area**, a frame around its cards, nested as deep as you like |
| a heading without sub-headings | a **card**: the heading is its title, the text below its body |
| paragraphs, quotes, code, tables, images | one card each |
| a bullet list | one card per item, nested items as cards inside |
| a task list (`- [ ]`) | a checklist card with real checkboxes |
| `- Manages [Erik Lund](#erik-lund)` | an **arrow** labeled "manages" |

Positions you set by dragging, and card colors, are kept in one HTML comment at the end of the file.
It is invisible in every Markdown viewer and easy to read:

```markdown
<!-- skiss
Johan Holm: at 220, 120; blue
Colors: red means Blocked, green means Done
-->
```

Pasted and generated pictures are saved next to the file, in `notes.assets/`.

## Everything it does

- **Autosave and live reload.** No save button. If the file changes on disk (Git, another editor, a
  sync tool), the board animates to the new state.
- **Edit in place.** Bold, italic, code, links, tasks, line breaks, undo and redo, and your Markdown
  habits: typing `**bold**` turns bold.
- **Arrange.** Add cards and areas, drag cards between areas, group a selection (⌘G), resize, pick
  paper colors and give them meanings. Tidy up lays everything out afresh.
- **Layout that thinks.** Boards open fitted to the window. Cards pack tightly, pictures get the
  right shape, hierarchies become trees, and arrows find their way around cards.
- **Find.** ⌘F searches the whole board and filters by color. R shows everything about a card: its
  arrows both ways and every card that mentions it.
- **Copy between boards.** Selected cards copy as Markdown and paste into any board as cards again.
- **AI help, with your own models.** Fix spelling, make text clearer or shorter, translate, or draw
  a picture from a card, through [Ailloy](https://crates.io/crates/ailloy) and the providers you
  already use (`skiss ai config`). Every change is one undo step.
- **Present it.** Infinite zoom with a readable map when zoomed out, and ⌘⇧+ for bigger text and
  lines when you share your screen.

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
