# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The canvas. `skiss <file>` opens any Markdown file as a board (and creates the file if it does
  not exist); `skiss` alone opens a start screen to name one. Headings with sub-headings become
  areas, other headings become cards, loose paragraphs, lists, task lists, quotes, code, tables,
  HTML and images become cards too.
- Connections written as plain sentences: a list line such as
  `- Manages [Erik Lund](#erik-lund)` is drawn as a labeled arrow. Drawing an arrow writes one.
- Editing in place with rich text (bold, italic, code, strikethrough, links), the usual editor
  shortcuts, Markdown habits while typing, task lists, and pasted images (saved in
  `<file>.assets/`).
- Autosave a moment after every change, live reload when the file changes on disk, and session
  undo/redo (⌘Z, ⌘Y or ⌘⇧Z).
- Minimal diffs: edits splice only the changed bytes. Hand-placed positions and card colors live in
  one HTML comment appendix at the end of the file, one line per decorated card.
- New cards (double-click, Tab), moving and reparenting by drag, grouping into areas (⌘G),
  colors (toolbar or 1 to 7, with names you choose), deleting cards and connections.
- Infinite zoom with levels of detail, a readable start view for big boards, every card showing
  all of its text (nothing is ever cut off or hidden behind a click), presenter text size (⌘⇧+ and ⌘⇧−), and springy animations that stop when nothing moves.
- Two themes: Sketchbook (default) and Ember, switched with ⌘T or `--theme`.
- A dock with + Card, + Area, Group, Tidy up (⌘⇧L) and help, on a pane of frosted glass
  (cream in Sketchbook, smoky in Ember), with buttons that press like buttons and hints that
  are easy to read. The card toolbar is glass too. Moving a card keeps
  everything else where it is; Tidy up lays the board out afresh.
- Connections can be selected by clicking the line, renamed by double-clicking it or its label,
  and drawn by dragging from anywhere on a card's border. Double-click the board title to rename it.
- The prompt comes back right away (the window runs on its own; `--wait` stays attached). Esc
  twice closes the window. The start screen picks a folder or opens an existing file.

- Search (⌘F) with color filters, and the related panel (R) for everything about a card.
- AI help through Ailloy: fix spelling, make clearer, shorten, translate, picture from a card,
  redraw a picture.
- Arrows route around cards instead of under them.
- Hard line breaks (for poems and addresses): ⇧Enter inserts one, saved as a trailing `\`.
- A board opens zoomed out to show all of it at once; zoom in from there.
- Boards look good as they open. The layout packs cards like a masonry wall and picks the shape
  that fills the window. The board title grows with the board. Square and portrait pictures get
  narrower cards, and long writing flows into two or three newspaper columns, so lines stay
  a comfortable length and a page of prose is a spread, not a scroll. Tables stay tables when zoomed
  out. Area names show once at any zoom.
- Arrows keep their shape at every zoom. They avoid passing under cards (so they never seem to
  point at the wrong one), avoid loops, and stay inside the board. The opening view includes
  them.
- Typing in a card only nudges the cards it covers; the rest of the board stays put.
- Hierarchies lay out as trees, with nothing to configure. When one kind of connection (such
  as "Reports to", "Part of" or "Manages") gives every card at most one parent, has no cycles
  and links at least three cards, parents go on top with their children centered below. This
  works inside an area and between areas, so an org chart reads like one.
- Chains ("Followed by", "Then": one before and one after each card) are timelines: they lay out
  in a row from left to right, so time flows to the right.
- Resizing cards and areas from a corner grip; pasting copied cards between boards keeps them
  cards.

### Changed

- MSRV 1.88 to 1.95, required by egui/eframe 0.37.

## [0.0.1] - 2026-10-20

### Added

- First placeholder release of skiss, an infinite canvas for notes stored as Markdown. It claims
  the name on crates.io and Homebrew and exercises the release pipeline; the canvas itself is not
  built yet. Scaffolded from MKLab's Rust CLI template: `ai` subcommand (Ailloy), shell
  completions, version banner, background update checker, and auditable release builds with
  CycloneDX SBOMs.

[Unreleased]: https://github.com/mklab-se/skiss/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/mklab-se/skiss/releases/tag/v0.0.1
