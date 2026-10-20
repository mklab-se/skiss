# Installing skiss

## Homebrew (macOS / Linux)

```sh
brew install mklab-se/tap/skiss
```

Or add the tap once, then install:

```sh
brew tap mklab-se/tap
brew install skiss
```

Upgrade with `brew upgrade skiss`.

## Cargo (from crates.io)

```sh
cargo install skiss
```

On Windows, building from source needs [NASM](https://www.nasm.us/) and [CMake](https://cmake.org/)
on `PATH` (plus the Visual Studio Build Tools most Rust installs already have). They're needed to
compile [`aws-lc-rs`](https://github.com/aws/aws-lc-rs), the TLS crypto backend. macOS and Linux need
nothing extra. If you'd rather skip the build tools entirely, use `cargo binstall` or Homebrew below:
both fetch a pre-built binary.

## cargo-binstall (prebuilt binaries, no compilation)

```sh
cargo binstall skiss
```

## Prebuilt binaries (GitHub Releases)

Download the archive for your platform from the
[latest release](https://github.com/mklab-se/skiss/releases/latest), extract it, and put the
`skiss` binary somewhere on your `PATH`:

| Platform | Archive |
| --- | --- |
| Linux (x86-64) | `skiss-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` |
| macOS (Apple Silicon) | `skiss-vX.Y.Z-aarch64-apple-darwin.tar.gz` |
| macOS (Intel) | `skiss-vX.Y.Z-x86_64-apple-darwin.tar.gz` |
| Windows (x86-64) | `skiss-vX.Y.Z-x86_64-pc-windows-msvc.zip` |

## Software bill of materials (SBOM)

Every release asset above has a matching CycloneDX 1.5 SBOM listing the exact crate versions
compiled into that platform's binary:

```
skiss-vX.Y.Z-<target>.cdx.json
```

The binaries are also built with [`cargo auditable`](https://github.com/rust-secure-code/cargo-auditable),
so the dependency list travels inside the executable itself. Check a downloaded binary against the
RustSec advisory database with:

```sh
cargo install cargo-audit --features=fix
cargo audit bin ./skiss
```

`syft` and `trivy` also understand this format.

## From source

```sh
git clone https://github.com/mklab-se/skiss
cd skiss
cargo install --path crates/skiss
```

## Shell completions

```sh
# Static script (write it where your shell loads completions)
skiss completion zsh > ~/.zfunc/_skiss

# Or dynamic completions (re-evaluated on each tab)
source <(COMPLETE=zsh skiss)
```
