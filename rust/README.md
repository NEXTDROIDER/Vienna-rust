# Vienna Rust migration

Current version: 0.0.4

This workspace is the Rust rewrite entry point for the original Vienna Java services.

Currently migrated:

- `rust/apiserver`: HTTP service scaffold with matching CLI options
- `rust/buildplate-importer`: buildplate zip parser/import preview CLI
- `rust/buildplate-launcher`: buildplate launcher CLI scaffold
- `rust/cdn`: resource-pack CDN server
- `rust/db`: SQLite-backed object database helper
- `rust/eventbus-server`: TCP service scaffold with matching CLI option
- `rust/example-hello-mcemod`: sample `.mcemod` compiled from Rust through VMA
- `rust/locator`: environment locator server
- `rust/mcemod-packager`: packages Rust cdylibs into `.mcemod` files
- `rust/modloader`: Windows-native `.mcemod` loader for VMA mods
- `rust/objectstore-server`: object store server/client rewrite with tests
- `rust/staticdata`: reusable static data loader
- `rust/tappables`: tappables domain, static data loader, spawner, manager, and CLI generator
- `rust/vma`: Vienna Modding API for Rust-based mods

The original Java code remains in place as the source of truth for behavior while the migration continues module by module.

Run helpers from the workspace root:

- `cargo run` starts the API server
- `cargo buildplate-importer -- --help` targets the buildplate importer binary
- `cargo buildplate-launcher -- --help` targets the buildplate launcher binary
- `cargo cdn -- --help` targets the CDN binary
- `cargo eventbus -- --help` targets the event bus binary
- `cargo locator -- --help` targets the locator binary
- `cargo objectstore -- --help` targets the object store binary
- `cargo tappables -- --help` targets the tappables generator binary

VMA mods:

- `apiserver` now scans `./mods` by default and loads `.mcemod` files on startup
- supported hooks: `on_load`, `on_unload`, `on_server_start`, `on_server_stop`, `on_player_join`, `on_player_leave`, `on_command`
- build and package the sample mod with `cargo mcemod-pack -- --package hello-mcemod`
- run `cargo run -- --mods-dir ./mods` and inspect `/mods` or `/health`
- debug hook routes:
- `GET /debug/hooks/player-join/<player>`
- `GET /debug/hooks/player-leave/<player>`
- `GET /debug/hooks/command/<player>/<command>`
