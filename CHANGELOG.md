# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Config and cache live in the XDG layout on macOS as well as Linux (`~/.config/<tool>`,
  `~/.cache/<tool>`, or under `$XDG_CONFIG_HOME`/`$XDG_CACHE_HOME`), the same as Ailloy. The new
  `paths` module in the core crate is the single place that decides; Windows keeps `%APPDATA%`
  and `%LOCALAPPDATA%`. Tools generated from the template inherit it.

### Changed

- YAML: `serde_yaml` 0.9 (deprecated upstream) replaced by `serde_norway` 0.9, its maintained
  drop-in fork, chosen fleet-wide. No format change and no migration: a new test pins the exact
  bytes the config is written as, and it passes unchanged on both libraries. Ailloy 3.0.1 (which
  makes the same switch) leaves a single YAML stack in the build.
- Docs, code comments and CLI help text no longer use em-dashes; `CLAUDE.md` gains a "Writing
  style" section that scaffolded tools inherit.
- CI: the Check job now fails on any em-dash (U+2014) in tracked files. All workflow runners
  already use `ubuntu-latest`.
- Ailloy 2.2 to 3.0 (`ailloy = { version = "3.0", ... }`, same `config-tui` feature). The template
  passes an explicit capability list (`["chat"]`) and matches no Ailloy enums, so no code changes
  were needed; tools that do match `Capability`, `ProviderKind` or `Task` need `_ =>` arms now that
  they are `#[non_exhaustive]` (see Ailloy's `MIGRATION.md`). `cargo update` refreshes the lockfile
  (tokio 1.53.2, thiserror 2.0.21, hyper 1.12, and routine transitive bumps); every other manifest
  requirement already covers the latest release, MSRV stays 1.88, and all GitHub Action majors
  match Ailloy 3.0's workflows.
- `README.md` gains a "What's new" callout right after the badges (version, headline summary, link
  to `CHANGELOG.md`) so scaffolded tools inherit it; `CLAUDE.md` and the `/release` skill note
  that it must be refreshed on every minor/major release.
- `reqwest` upgraded 0.12 → 0.13 (feature `rustls-tls-native-roots` renamed `rustls`; ailloy bumped
  to 2.2 in lockstep so only one TLS stack, rustls + aws-lc-rs, gets compiled in). Building from
  source on Windows now needs NASM and CMake on `PATH`; `release.yml`'s Windows build installs NASM
  via `ilammy/setup-nasm@v1`. `cargo binstall` and Homebrew are unaffected (pre-built binaries).

### Added

- Supply-chain transparency for release builds: `release.yml` builds binaries with `cargo auditable`
  (dependency list embedded in the executable, readable with `cargo audit bin` or `syft`) and
  attaches a per-target CycloneDX 1.5 SBOM (`rusty-tmpl-vX.Y.Z-<target>.cdx.json`) to every
  GitHub Release. `INSTALL.md` documents how to read them.
- `/release` skill: updates the toolchain (`rustup update stable`) before the pre-flight checks,
  lints with `--all-targets` like CI, re-runs clippy after formatting, and ends with a
  "Watch and verify" step (`gh run watch`, then check the release assets, crates.io, and the
  Homebrew formula).

### Changed

- Dependency baseline raised to current majors: Ailloy 0.8 → 2.1, `colored` 2 → 3, `dirs` 6 → 7,
  `clap`/`clap_complete` 4.5 → 4.6, `tokio` 1.40 → 1.53, plus `cargo update` across the lockfile.
  `reqwest` stays on 0.12 to share a single TLS stack with Ailloy (0.13 needs cmake/NASM on Windows).
- MSRV 1.85 → 1.88 (required by Ailloy 2.x).
- 2026-09-22 maintenance round: `cargo update` refreshes the lockfile (Ailloy 2.1.1 → 2.1.2,
  plus routine transitive bumps); every manifest requirement already covers the latest crates.io
  release, MSRV stays 1.88, and all GitHub Action majors in `ci.yml`/`release.yml` are still current.
- 2026-09-16 maintenance round: `cargo update` refreshes the lockfile (Ailloy 2.1.0 → 2.1.1,
  clap 4.6.7, clap_complete 4.6.11, rustls 0.23.45, quinn 0.11.12, and friends); every manifest
  requirement already covers the latest crates.io release, MSRV stays 1.88, and all GitHub Action
  majors in `ci.yml`/`release.yml` are still current. `serde_yaml` pin reason recorded in `Cargo.toml`.
- GitHub Actions bumped to Node 24 majors: `actions/checkout@v7`, `actions/upload-artifact@v7`,
  `actions/download-artifact@v8`, `softprops/action-gh-release@v3`.
- Initial template: Cargo workspace (`rusty-tmpl` + `rusty-tmpl-core`), clap-derive CLI with
  `-v`/`-q`/`--no-color` global flags, `Hello world!` default command, `ai` subcommand backed by
  Ailloy, shell completions, version banner, and a background crates.io update checker.
- GitHub Actions CI (check / test / clippy / fmt) and a release pipeline that builds cross-platform
  binaries, publishes to crates.io, and updates the Homebrew tap.
- `/release` skill for cutting versioned releases.

[Unreleased]: https://github.com/mklab-se/rusty-tmpl/commits/main
