# CLAUDE.md

Guidance for Claude Code (and other agents) working in this repository.

## What this is

`skiss` (Swedish for "sketch") is an infinite-canvas, Miro-style note-taking tool that stores
everything in Markdown, in the spirit of MDeck (`../mdeck/`). Usage: `skiss mynotes.md`.
Homepage: https://skiss.net. The design (rules, Markdown mapping, appendix format) is in
`docs/superpowers/specs/2026-10-20-skiss-design.md`; read it before changing behavior.

Non-negotiable rules from the design: any Markdown file must open; edits are minimal splices
(never re-render the whole file); plain Markdown and English, with the HTML-comment appendix as
the only skiss-specific syntax; autosave plus file watching; no Markdown syntax shown in the UI;
fully Rust (no web view); American English spelling everywhere.

## Template lineage

> **Note for agents working in a repository generated from this template** (i.e. the tool was renamed
> away from `rusty-tmpl`): the rest of this file describes the original scaffold. The references below
> intentionally still point at the upstream template, not at this tool.

- **Upstream template:** https://github.com/mklab-se/rusty-tmpl
- **Likely local clone:** `../rusty-tmpl/` (sibling directory under the same parent)

This repository was scaffolded from that template, which carries the shared MKLab CLI conventions
(workspace layout, clap CLI, Ailloy `ai` command, update checker, CI/release pipeline, `/release`
skill). Use the lineage in both directions:

- **Pulling improvements in:** when the template gains a fix or new convention, compare against
  `../rusty-tmpl/` and port the relevant change here, adapting the `rusty-tmpl` name to this tool.
- **Pushing improvements back:** if you discover a fix or better pattern here that is *generic*
  (not specific to this tool's domain), consider contributing it upstream to `mklab-se/rusty-tmpl`
  so every future tool benefits. Generalize it (strip tool-specific names/logic) before doing so.

When working from the local clone, prefer reading `../rusty-tmpl/` directly to diff conventions; fall
back to the GitHub URL if the sibling directory isn't present.

## Architecture

A two-crate Cargo workspace:

- `crates/skiss/`: the CLI binary.
  - `main.rs`: `#[tokio::main]`; sets up logging, dynamic-completion env, the `--no-color` override,
    and spawns the background update check, then calls `Cli::run`.
  - `cli.rs`: clap-derive `Cli`, `Commands`, `AiCommands`, `Shell`; `Cli::run` dispatches. With no
    subcommand it opens the canvas (`FILE` and `--theme` are optional).
  - `canvas/`: the eframe/egui window.
    - `app.rs`: start screen vs board, theme switching and persistence, dev hooks.
    - `view.rs`: the board view: painting, pointer/keyboard, editing, toolbar, prompts, overlays.
    - `editor.rs`: rich-text card editing on characters with styles (never on Markdown).
    - `content.rs`: text layout of a card (shared by measuring, drawing and caret hit-testing).
    - `layout.rs`: world layout (shelf packing in document order, around hand-placed items).
    - `motion.rs`: springs for every node, the camera; nothing repaints once settled.
    - `document.rs`: the open file: autosave (250 ms), file watching (notify), undo/redo.
    - `theme.rs`, `fonts.rs`, `draw.rs`, `images.rs`: looks, bundled OFL fonts, pencil lines, pictures.
    - `script.rs`: development aid. `SKISS_SCRIPT=steps.txt skiss file.md` drives the window with real
      input events (click, drag, type, key, shot); `SKISS_SCREENSHOT=out.png` saves the window and
      quits. Use these to verify UI changes end to end and to look at the result.
  - `commands/`: one module per command (`ai`, `completion`). Add new commands here.
  - `banner.rs`: ASCII block-letter banner + version line.
  - `update.rs`: polls crates.io, caches the result for 24h in `paths::cache_dir()`, notifies on a
    newer version.
- `crates/skiss-core/`: framework-agnostic library (no clap/tokio).
  - `board/`: the board model, with no UI dependencies.
    - `blocks.rs`: pulldown-cmark block structure with exact byte ranges.
    - `model.rs`: `Board::parse`: areas, cards, lines, connections, anchors (GitHub slugs).
    - `inline.rs`: spans with styles, and back to Markdown.
    - `appendix.rs`: the `<!-- skiss ... -->` decoration comment.
    - `edit.rs`: every change as a minimal splice of the source (text, cards, connections, moves,
      groups, tasks, appendix). Unchanged lines are copied verbatim; never write trailing spaces.
  - `config.rs`: YAML `Config` in `paths::config_dir()` (the chosen theme).
  - `paths.rs`: where the tool keeps files. Linux and macOS share the XDG layout, same as Ailloy:
    `$XDG_CONFIG_HOME/<tool>` or `~/.config/<tool>`, `$XDG_CACHE_HOME/<tool>` or
    `~/.cache/<tool>`. Windows uses its native `%APPDATA%` / `%LOCALAPPDATA%`. Every MKLab tool
    follows this; never call `dirs::config_dir()`/`dirs::cache_dir()` directly (on macOS those
    are `~/Library/...`).
  - `error.rs`: `thiserror` `Error` enum + `Result` alias.

## Adding a command

1. Add a variant to `Commands` in `cli.rs` (with a doc comment; it becomes the help text).
2. Add a `pub mod <name>;` in `commands/mod.rs` and implement `pub async fn run(...) -> anyhow::Result<()>`.
3. Add the dispatch arm in `Cli::run`.

## AI integration

`commands/ai.rs` wraps [Ailloy](https://crates.io/crates/ailloy) via its `config_tui` helpers and the
shared global config (`~/.config/ailloy/config.yaml`). To call a model from a command, use
`ailloy::Client`. The capability list is the `CAPABILITIES` const in `ai.rs` (`["chat"]`).

## Conventions

- Edition 2024, MSRV 1.95 (`[workspace.package]`; required by egui/eframe 0.37).
- All deps are declared in the root `[workspace.dependencies]` and inherited with `.workspace = true`.
  Current majors: clap 4.6, tokio 1.53, colored 3, dirs 7, thiserror 2, ailloy 3.0, reqwest 0.13.
  YAML is `serde_norway` 0.9, the maintained drop-in fork of the deprecated `serde_yaml`
  (chosen fleet-wide 2026-10-07).
- Building from source on Windows needs NASM and CMake on `PATH`, because `aws-lc-rs`
  (reqwest's TLS crypto backend) compiles optimized assembly routines at build time. macOS and Linux need nothing extra.
  The release workflow's Windows leg installs NASM via `ilammy/setup-nasm@v1`; CMake and MSVC are
  already on the `windows-latest` image.
- CI gates: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`. CI runs the latest stable toolchain, so run the gates on an up-to-date
  local toolchain (new clippy lints otherwise surface only in CI).
- `README.md` opens with the logo, the badges and then a centred "What's new" callout
  (`<strong>tool X.Y</strong> is here: ...` plus a `What's new` link to `CHANGELOG.md`). Refresh the
  callout on every minor/major release; generated tools inherit it.
- Releases go through the `/release` skill (`.claude/skills/release/`) → tag push → `release.yml`. The
  skill updates the toolchain and dependencies first, then watches the workflow. `release.yml` builds
  binaries with `cargo auditable` and attaches a per-target CycloneDX SBOM (`.cdx.json`) to the
  GitHub Release alongside the archives.

## Writing style

- **No em-dashes (U+2014), anywhere:** docs, CHANGELOG, code comments, doc comments, CLI help text,
  error messages, test strings, commit messages and PR descriptions. Use a comma where it works;
  otherwise a colon, parentheses, or a new sentence. If Rust code genuinely needs the character at
  runtime, write the escape `\u{2014}`, never the literal. CI enforces this (the "No em-dashes"
  step in `ci.yml`); check locally with `git grep -nIP '\x{2014}'`.

## Dependency Policy

We keep this tool's dependencies at their latest compatible versions, not just the versions that
happen to still compile. Staying current is the default, not something we get to eventually:
letting dependencies drift is how technical debt accumulates unnoticed until a security advisory or
a forced breaking upgrade makes it urgent. When a newer major is available and there's no concrete,
documented reason not to take it (see any `# Stays on ...` comments in `Cargo.toml` for the current
exceptions and why), take it during the next maintenance round rather than deferring it. This
applies to every tool scaffolded from this template too; the cross-repo `maintaining-rust-tools`
skill drives it for the whole fleet (ailloy + cosq + deemer + mdeck + pidge + rigg + rusty-tmpl).
