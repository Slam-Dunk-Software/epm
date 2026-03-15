# Customizing epm

epm is the CLI for the EPS ecosystem. It talks to an EPM registry to publish,
search, and install personal software packages.

---

## Ports

### `--registry` — registry URL
**Type:** CLI flag / environment variable
**Default:** `https://epm.dev`

Every command accepts `--registry <url>` to point at a different registry:

```sh
epm --registry http://localhost:3000 search
epm --registry https://my-registry.example.com install todo
```

Or set `EPM_REGISTRY` in your environment (not currently wired — override
`DEFAULT_REGISTRY` in `src/main.rs` to bake in a different default).

---

### `EPM_PUBLISH_TOKEN` — publish auth
**Type:** environment variable

Required when publishing packages to a registry that enforces authentication.
Can also be passed as `--token <value>` on any command.

```sh
EPM_PUBLISH_TOKEN=mytoken epm publish
```

---

### `EPM_INSTALL_DIR` — install location
**Type:** environment variable / source edit
**Default:** `~/.epm/packages/`

Where installed packages are unpacked. Change `install_dir()` in
`src/commands/install.rs` to relocate.

---

### Registry API shape
**Type:** source-level port

epm expects the registry to implement these endpoints (as defined by
`epm_registry`):

- `GET  /api/v1/packages` — list packages
- `GET  /api/v1/packages/:name` — package detail + versions
- `POST /api/v1/packages` — publish (requires auth token)

Implement these on any HTTP server to use epm against your own registry.
