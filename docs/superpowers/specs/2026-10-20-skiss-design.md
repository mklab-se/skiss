# skiss 0.1 design

Status: approved in conversation with Kristofer on 2026-10-20. Spelling: American English everywhere.

## Intent

skiss (Swedish for "sketch") is an infinite canvas for notes, like Miro or a whiteboard, where the
board is a plain Markdown file on your own machine. Kristofer needs it to:

- take notes on the projects and programs he leads,
- capture who belongs to whom, who manages whom, who told him to contact whom and what came of it,
- replace pen-and-paper mind maps without a hosted service,
- show a client that everything is under control, in one file on their own computer.

What decides whether skiss lives on: **the wow, and the ease of use**, the first time someone opens
their own file and starts editing.

## Rules (from Kristofer)

1. skiss works with **any** Markdown file. The board may not be the best possible, but it always opens.
2. skiss is a visual Markdown editor, but it behaves like Miro, not like a text editor with a preview.
3. **Minimal diffs.** When skiss changes the file it keeps format and order as close to the original
   as possible, so files version-control well. Restructuring is allowed when needed, avoided when not.
4. **Autosave.** No save button. Every change is written to the file shortly after it happens. The
   file is watched: when it changes on disk, the board updates (animated). Undo (Cmd/Ctrl+Z) and redo
   (Cmd/Ctrl+Y, also Cmd/Ctrl+Shift+Z) cover changes made in the current session.
5. **Plain Markdown, plain English.** A file edited by skiss reads like ordinary notes. Custom syntax
   is the exception. The one exception in 0.1 is the layout appendix (below).
6. skiss never exposes Markdown syntax. Users edit text in place, with normal editor shortcuts.
7. It is not Word or PowerPoint. skiss looks the way skiss looks, but it has themes.
8. Fully Rust. No JavaScript, no web view.

## Command line

- `skiss notes.md` opens the board. If the file does not exist, it is created (with a `# Notes`
  title derived from the file name) and opened.
- `skiss` with no file opens the app on a small start screen asking for a file name (in the window,
  not the terminal). Enter creates or opens that file in the current directory.
- Existing subcommands (`ai`, `completion`, `version`) stay.

## Storage model

A canvas is **one Markdown file plus an optional assets folder** next to it
(`notes.assets/`) holding pasted images. Nothing else.

### How Markdown maps onto the board

The document is a tree; the tree is the board.

| Markdown | Board |
| --- | --- |
| First `# H1` (if the file starts with one) | Board title |
| A heading that has sub-headings | **Area** (a frame), nested to any depth |
| A heading without sub-headings | **Card**: heading is the title, its content is the body |
| Loose content directly under an area heading (paragraphs, quotes, code, tables, images) | One card per block |
| A plain bullet list loose in an area | One card per top-level item; nested items become cards inside that card |
| A task list (`- [ ]`) loose in an area | One checklist card |
| Content before the first heading | Cards in a top-level "loose" area with no frame |

A file with no headings at all is still a valid board: every block becomes a card.

### Connections as sentences

A list item inside a card whose text links to another card's heading anchor is a **connection**:

```markdown
### Anna Berg
Program sponsor.
- Manages [Erik Lund](#erik-lund) and [Sara Ek](#sara-ek)
- Told me to contact [Johan Holm](#johan-holm), 2026-10-14
```

- Each internal link (`#anchor` matching a heading slug, GitHub slug rules) becomes an arrow from
  the card that contains the item to the target card (or area).
- The label is the item's text before the first link ("Manages", "Told me to contact"), shown in
  lowercase on the arrow. Text after the links ("2026-10-14") is shown as a small note on the arrow.
- Connection items are drawn as arrows, not as body text.
- Drawing a connection in the UI appends `- <Label> [<Target title>](#<slug>)` to the source card.
  An empty label writes `- Related to [...]`.

### The layout appendix

Decoration (hand-placed positions, sizes, colors and color names) lives in a single HTML comment at
the very end of the file, invisible in every Markdown renderer and readable in the raw file:

```markdown
<!-- skiss
Johan Holm: at 220, 120; red
Core team: at 150, 100; size 290 x 128
Colors: red means Blocked, yellow means Waiting
-->
```

- One line per decorated item, keyed by the item's title, in document order, coordinates rounded to
  whole numbers and relative to the parent area. Only items the user moved, resized or colored get a
  line, so a file nobody arranges by hand has no appendix.
- Moving one card changes exactly one line.
- Lines whose key no longer matches anything are ignored (the item falls back to automatic layout)
  and dropped the next time skiss rewrites the appendix.
- Renaming a card in skiss renames its line too.
- Deleting the appendix loses decoration only, never content.

### Minimal-diff writing

skiss keeps the original source text. Every parsed element remembers its byte range. An edit is a
**splice**: replace exactly the range that changed with newly serialized text, leave every other byte
alone. Examples: editing a card body rewrites that body's range; a new card inserts a heading after the
last card of its area; a connection inserts one list line; checking a task flips one character;
grouping cards into a sub-area inserts a heading and shifts the grouped headings one level down.

Line endings and the trailing newline of the original file are preserved.

## The editor

- Click a card to select it. Double-click (or Enter) to edit its text in place. Escape or a click
  outside ends editing.
- In-card editing is rich text: bold (Cmd/Ctrl+B), italic (Cmd/Ctrl+I), inline code (Cmd/Ctrl+E),
  copy, cut, paste, select all, word and line navigation, Shift selection, mouse selection.
- Enter in a task list adds the next task; typing `[ ] ` at the start of a line makes it a task.
  Clicking a checkbox toggles it (one-character diff).
- Paste an image: it is saved as `notes.assets/<name>.png` and becomes an image card.
- Double-click empty canvas: a new card at that spot, already in typing mode (title first).
  Tab with a card selected: new sibling card. Delete/Backspace on a selected card removes it.
- Drag cards to move them (into another area moves them in the Markdown too). Drag area titles to
  move areas.
- Select cards (Shift-click or drag a lasso) and press Cmd/Ctrl+G: they are grouped in a new sub-area
  with its title ready to type.
- Hover a card: connection handles appear. Drag from a handle to another card, drop, type or pick a
  label, Enter.
- Selected card: a small floating toolbar with bold, italic, the color swatches and delete.
- Undo/redo is a per-session stack of file snapshots plus selection.

## Look and motion

- Built on egui/eframe (as MDeck), custom-painted canvas, own rich-text editing.
- Two themes in 0.1: **Sketchbook** (default: warm paper, handwritten Caveat font, slightly tilted
  cards, pencil arrows) and **Ember** (MKLab's dark brand: near-black, hairlines, Spectral and Hanken
  Grotesk, an ember glow on the selection). Toggle with Cmd/Ctrl+T. The choice is per-user config,
  not stored in the file.
- Everything that changes animates with springs: cards and areas move and resize, new cards pop in,
  removed cards fade, the camera glides. When nothing changes, skiss requests no frames.
- Infinite zoom (mouse wheel / trackpad pinch, Cmd/Ctrl +/-/0, double-click an area title to fly to
  it, `F` to fit everything) with **levels of detail**: far out, only areas and arrows; closer, card
  titles; closer still, full text.
- **Presenter size** (Cmd/Ctrl+Shift +/-): scales text and lines without moving the camera, for
  sharing a screen.
- Selecting a card highlights its connections.

## Out of scope for 0.1

Ailloy features (spell check, writing help, translation, generated images), link previews, MDeck
diagrams, shapes, freehand sketches (later as SVG files in the assets folder), templates, live
collaboration (never; Git covers it).

## Left to the builder

Exact auto-layout algorithm, animation constants, palette values, keyboard details beyond the list
above, how much Markdown inline syntax the rich-text model covers (bold, italic, code, links, strike
are required), test strategy.
