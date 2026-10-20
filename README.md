<p align="center"><img src="https://raw.githubusercontent.com/mklab-se/rusty-tmpl/main/media/rusty-tmpl-horizontal.png" width="600"></p>

<p align="center">
<a href="https://github.com/mklab-se/rusty-tmpl/actions/workflows/ci.yml"><img src="https://github.com/mklab-se/rusty-tmpl/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
<a href="https://crates.io/crates/rusty-tmpl"><img src="https://img.shields.io/crates/v/rusty-tmpl.svg" alt="crates.io"></a>
<a href="https://github.com/mklab-se/rusty-tmpl/releases/latest"><img src="https://img.shields.io/github/v/release/mklab-se/rusty-tmpl" alt="GitHub Release"></a>
<a href="https://github.com/mklab-se/homebrew-tap/blob/main/Formula/rusty-tmpl.rb"><img src="https://img.shields.io/badge/dynamic/regex?url=https%3A%2F%2Fraw.githubusercontent.com%2Fmklab-se%2Fhomebrew-tap%2Fmain%2FFormula%2Frusty-tmpl.rb&search=%5Cd%2B%5C.%5Cd%2B%5C.%5Cd%2B&label=homebrew&prefix=v&color=orange" alt="Homebrew"></a>
<a href="https://github.com/mklab-se/rusty-tmpl/blob/main/LICENSE"><img src="https://img.shields.io/crates/l/rusty-tmpl.svg" alt="License"></a>
</p>

<!-- "What's new" callout: keep it right after the badges and refresh it on every minor/major
     release (version and a one-line summary of the headline changes). Only link CHANGELOG.md. -->
<p align="center">
  <strong>rusty-tmpl 0.1</strong> is here: a ready-to-scaffold Rust CLI workspace with Ailloy AI,
  shell completions, an update checker and auditable releases with SBOMs.<br>
  <a href="CHANGELOG.md"><strong>What's new</strong></a>
</p>

# rusty-tmpl

A **template repository** for building Rust command-line tools, pre-wired with the conventions used
across MKLab's CLIs ([cosq](https://github.com/mklab-se/cosq), [honk](https://github.com/mklab-se/honk),
[pidge](https://github.com/mklab-se/pidge), [rigg](https://github.com/mklab-se/rigg), …):

- 📦 A Cargo **workspace** (`rusty-tmpl` binary + `rusty-tmpl-core` library)
- 🧰 A [clap](https://docs.rs/clap)-derive CLI with global flags (`-v`, `-q`, `--no-color`) and `--help`
- 🤖 An `ai` subcommand backed by [**Ailloy**](https://crates.io/crates/ailloy), MKLab's shared AI config
- 🐚 Static **and** dynamic shell completions
- 🔔 A background **crates.io update checker**
- ⚙️ **GitHub Actions** CI (check / test / clippy / fmt) and a release pipeline
- 🍺 Automated publishing to **crates.io** and **Homebrew**
- 🪄 A `/release` skill that drives the whole release flow

Run with no subcommand, it just prints `Hello world!`. Everything else is plumbing waiting for your logic.

## Using this template

1. Click **“Use this template”** on GitHub to create a new repository.
2. Pick a name for your tool and rename everything in one pass. From the repo root:

   ```sh
   NEW=mytool   # your new tool name (kebab-case)

   # Rename crate directories
   git mv crates/rusty-tmpl       "crates/$NEW"
   git mv crates/rusty-tmpl-core  "crates/$NEW-core"
   git mv media/rusty-tmpl-horizontal.png "media/$NEW-horizontal.png"

   # Replace the name in every file (macOS sed shown; on Linux use `sed -i`).
   # CLAUDE.md is excluded on purpose: its "Template lineage" section must keep
   # pointing at the upstream rusty-tmpl template (see step 4).
   grep -rl --exclude-dir=.git --exclude-dir=target --exclude=CLAUDE.md 'rusty-tmpl' . \
     | xargs sed -i '' "s/rusty-tmpl/$NEW/g; s/rusty_tmpl/${NEW//-/_}/g"

   # The block-letter banner in src/banner.rs and the ASCII art are template-specific;
   # regenerate or edit them for your tool.
   cargo build && cargo test
   ```

3. Replace `media/<tool>-horizontal.png` with your own artwork, and rewrite this README. Keep the
   "What's new" callout right after the badges (rewrite its text for your tool) and refresh it on
   every minor/major release.
4. Edit `CLAUDE.md` by hand: rename the architecture/path references to your tool, but **leave the
   "Template lineage" section pointing at `mklab-se/rusty-tmpl`** so future agents know where the
   scaffold came from.
5. Set up the release secrets (see [Releasing](#releasing)).

## Build & run

```sh
cargo run                    # prints "Hello world!"
cargo run -- --help          # show the CLI help
cargo run -- version         # banner + version
cargo run -- ai              # AI status (via Ailloy)
cargo run -- completion zsh  # generate a zsh completion script
cargo test --workspace       # run the unit tests
```

### Install

See [INSTALL.md](INSTALL.md) for Homebrew, `cargo install`, `cargo binstall`, and from-source instructions.

## AI integration (Ailloy)

The `ai` subcommand reuses MKLab's shared [Ailloy](https://crates.io/crates/ailloy) configuration
(`~/.config/ailloy/config.yaml`), so every tool shares the same providers and API keys.

```sh
rusty-tmpl ai          # show status
rusty-tmpl ai config   # interactively configure a provider/model
rusty-tmpl ai test     # send a test message
rusty-tmpl ai enable   # / disable: toggle AI for this tool
```

To call a model from your own commands, use `ailloy::Client`; see `crates/rusty-tmpl/src/commands/ai.rs`
for where the integration lives.

## Releasing

Releases are driven by the [`/release`](.claude/skills/release/SKILL.md) skill (run it in Claude
Code with `major`, `minor`, or `patch`). It updates the toolchain and dependencies, runs the CI
gates, bumps the version, updates the changelog, then commits, pushes, and tags `vX.Y.Z`. Pushing
the tag triggers `.github/workflows/release.yml`, which:

1. Re-runs the full CI suite
2. Builds [auditable](https://github.com/rust-secure-code/cargo-auditable) binaries for Linux, macOS
   (Intel + ARM), and Windows, with a CycloneDX SBOM per target
3. Creates a GitHub Release with the archives and SBOMs (see
   [INSTALL.md](INSTALL.md#software-bill-of-materials-sbom) for how to read them)
4. Publishes `rusty-tmpl-core` then `rusty-tmpl` to crates.io
5. Updates the Homebrew formula in [`mklab-se/homebrew-tap`](https://github.com/mklab-se/homebrew-tap)

### Required secrets

Configure these once on the GitHub repository (these are the same secrets used by the other MKLab tools):

| Secret | Where | Purpose | How to create |
| --- | --- | --- | --- |
| `CARGO_REGISTRY_TOKEN` | Environment **`crates-io`** | Publish to crates.io | [crates.io/settings/tokens](https://crates.io/settings/tokens) → new token with publish scope |
| `HOMEBREW_TAP_TOKEN` | Repository secret | Push the formula to the tap | A GitHub PAT with `repo` scope for `mklab-se/homebrew-tap` |

If `HOMEBREW_TAP_TOKEN` is missing, the release still succeeds; the Homebrew step just logs a warning.

## Development

```sh
cargo fmt --all              # format
cargo clippy --workspace --all-targets -- -D warnings   # lint (matches CI)
cargo test --workspace       # test
```

The CLI lives in `crates/rusty-tmpl` and reusable logic in `crates/rusty-tmpl-core`. To add a
command: declare it in `cli.rs` (`Commands` enum), add a module under `commands/`, and wire the
dispatch arm in `Cli::run`. See [CLAUDE.md](CLAUDE.md) for the architecture in more detail.

## License

[MIT](LICENSE) © Kristofer Liljeblad
