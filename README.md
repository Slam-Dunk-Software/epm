# epm

[![CI](https://github.com/Slam-Dunk-Software/epm/actions/workflows/ci.yml/badge.svg)](https://github.com/Slam-Dunk-Software/epm/actions/workflows/ci.yml)

The CLI for the [EPS](https://github.com/Slam-Dunk-Software/eps_docs) ecosystem — publish, search, install, and run personal software packages.

## Installation

```sh
curl -fsSL https://raw.githubusercontent.com/Slam-Dunk-Software/epm/main/install.sh | sh
```

Installs a pre-built binary to `~/.local/bin`. Verify:

```sh
epm --version
```

## Commands

### Packages

```sh
epm search                  # browse all packages
epm search todo             # filter by name / description
epm info todo               # details + versions
epm install todo            # install a package
epm install todo@0.1.0      # pin a specific version
epm upgrade todo            # upgrade to latest
epm uninstall todo          # remove a package
epm list                    # list installed packages
```

### Harnesses

```sh
epm new todo                # scaffold a harness into ./todo/ (yours to customize)
epm new todo my-tasks       # custom directory name
```

### Services

```sh
epm services serve                  # deploy current directory as a service
epm services serve <name>           # deploy an installed package
epm services serve --local <path>   # deploy from an explicit path
epm services ps                     # list running services
epm services logs <name>            # tail logs for a service
epm services stop <name>            # stop a service
epm services restart <name>         # restart a service (picks up source changes)
epm services remove <name>          # stop + remove a service entirely
epm services prune                  # remove services whose directories no longer exist
epm services sync                   # repair services.toml from the persistent registry
epm services startup                # restart all registered services (used at login)
epm services install-startup        # install a LaunchAgent/systemd unit for auto-start
epm services audit                  # check services for insecure network bindings
epm services observatory rm <name>  # remove a stale entry from the Observatory database
```

### MCP servers

```sh
epm mcp install eps_mcp     # install + register an MCP server with Claude
epm mcp list                # list registered MCP servers
epm mcp remove eps_mcp      # unregister + uninstall
```

### Skills (Claude Code slash commands)

```sh
epm skills install eps_skills   # install skill packages
epm skills list                 # list installed skills
epm skills remove eps_skills    # remove skills
```

### Publishing

```sh
epm init my-package         # scaffold a new EPS package
epm publish                 # publish current package to the registry
epm adopt todo              # pull a package into vendor/ as first-class source
epm sync todo               # check for upstream changes on an adopted package
```

### Self

```sh
epm self-update             # update epm to the latest release
epm self-uninstall          # remove epm and everything it installed
```

## Configuration

| Env var | Description |
|---|---|
| `EPM_REGISTRY` | Registry base URL (default: `https://epm.dev`) |
| `EPM_PUBLISH_TOKEN` | Auth token for publishing packages |

## Development

```sh
cargo test
```

## Related

| | |
|---|---|
| [epm_registry](https://github.com/Slam-Dunk-Software/epm_registry) | Registry server |
| [eps_docs](https://github.com/Slam-Dunk-Software/eps_docs) | ADRs, concepts, and guides |
