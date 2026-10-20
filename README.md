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
  <strong>skiss 0.0.1</strong> is here: the name is claimed and the release pipeline works. The
  canvas itself is still on the drawing board.<br>
  <a href="CHANGELOG.md"><strong>What's new</strong></a>
</p>

# skiss

**skiss** (Swedish for "sketch") is an infinite canvas for your notes, stored as plain Markdown.
Think Miro or a whiteboard app, but your board is a `.md` file you own, can diff, and can read in
any editor:

```sh
skiss mynotes.md
```

It is a sibling of [MDeck](https://github.com/mklab-se/mdeck), which turns Markdown into
presentations, and part of MKLab's family of Rust CLIs ([ailloy](https://github.com/mklab-se/ailloy),
[cosq](https://github.com/mklab-se/cosq), [pidge](https://github.com/mklab-se/pidge),
[rigg](https://github.com/mklab-se/rigg), and more).

> **Status: early.** Version 0.0.1 is a placeholder release that claims the name and proves the
> build and release pipeline. It does not open a canvas yet. Follow along at
> [skiss.net](https://skiss.net).

## What works today

```sh
skiss --help          # show the CLI help
skiss version         # banner + version
skiss ai              # AI status (via Ailloy, MKLab's shared AI config)
skiss completion zsh  # generate a shell completion script
```

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

The CLI lives in `crates/skiss` and reusable logic in `crates/skiss-core`. See
[CLAUDE.md](CLAUDE.md) for the architecture. Releases go through the
[`/release`](.claude/skills/release/SKILL.md) skill: pushing a `vX.Y.Z` tag builds auditable
binaries with CycloneDX SBOMs, publishes to crates.io and updates the
[Homebrew tap](https://github.com/mklab-se/homebrew-tap).

## License

[MIT](LICENSE) © Kristofer Liljeblad
