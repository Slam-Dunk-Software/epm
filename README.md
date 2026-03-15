# epm

[![CI](https://github.com/Slam-Dunk-Software/epm/actions/workflows/ci.yml/badge.svg)](https://github.com/Slam-Dunk-Software/epm/actions/workflows/ci.yml)

The CLI for the [EPS](https://github.com/nickagliano/eps_mcp) ecosystem. Publish, search, and install personal software packages from an [EPM registry](https://github.com/Slam-Dunk-Software/epm_registry).

## Installation

```sh
cargo install --git https://github.com/Slam-Dunk-Software/epm
```

Or build from source:

```sh
git clone https://github.com/Slam-Dunk-Software/epm
cd epm
cargo build --release
# binary at target/release/epm
```

## Usage

```sh
# publish the package in the current directory
epm publish

# search the registry
epm search todo

# get info on a package
epm info todo

# install a package
epm install todo
epm install todo@0.1.0   # pin a version

# list installed packages
epm list

# upgrade a package
epm upgrade todo

# uninstall a package
epm uninstall todo
```

## Configuration

| Flag / Env Var | Description |
|---|---|
| `--registry <URL>` | Registry base URL (default: `http://localhost:3001`) |
| `--token <TOKEN>` | Auth token for publish (or set `EPM_PUBLISH_TOKEN`) |
| `EPM_PUBLISH_TOKEN` | Auth token for publish endpoint |

## Development

```sh
cargo test
```

## Registry

The registry server lives at [Slam-Dunk-Software/epm_registry](https://github.com/Slam-Dunk-Software/epm_registry).
