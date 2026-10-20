# CLAUDE.md

Guidance for Claude Code (and other agents) working in this repository.

## What this is

`rusty-tmpl` is a **template repository** for MKLab Rust CLI tools. New tools are created from it via
GitHub's "Use this template", then renamed (see the "Using this template" section of `README.md`).
Keep it generic: it should compile, run, and pass CI as-is, while carrying no tool-specific logic.

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

- `crates/rusty-tmpl/`: the CLI binary.
  - `main.rs`: `#[tokio::main]`; sets up logging, dynamic-completion env, the `--no-color` override,
    and spawns the background update check, then calls `Cli::run`.
  - `cli.rs`: clap-derive `Cli`, `Commands`, `AiCommands`, `Shell`; `Cli::run` dispatches. **The
    no-subcommand (`None`) arm intentionally prints `Hello world!`** instead of help (per the template
    brief). `--help`/`-h` still work via clap.
  - `commands/`: one module per command (`ai`, `completion`). Add new commands here.
  - `banner.rs`: ASCII block-letter banner + version line.
  - `update.rs`: polls crates.io, caches the result for 24h in `paths::cache_dir()`, notifies on a
    newer version.
- `crates/rusty-tmpl-core/`: framework-agnostic library (no clap/tokio).
  - `config.rs`: YAML `Config` in `paths::config_dir()`, a reusable starting point (unused so far).
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

- Edition 2024, MSRV 1.88 (`[workspace.package]`; set by Ailloy, 1.88 since 2.x and still in 3.0).
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
