# Vienna Rust migration

This workspace is the Rust rewrite entry point for the original Vienna Java services.

Currently migrated:

- `rust/apiserver`: HTTP service scaffold with matching CLI options
- `rust/buildplate-importer`: buildplate zip parser/import preview CLI
- `rust/buildplate-launcher`: buildplate launcher CLI scaffold
- `rust/cdn`: resource-pack CDN server
- `rust/db`: SQLite-backed object database helper
- `rust/eventbus-server`: TCP service scaffold with matching CLI option
- `rust/locator`: environment locator server
- `rust/objectstore-server`: object store server/client rewrite with tests
- `rust/staticdata`: reusable static data loader
- `rust/tappables`: tappables domain, static data loader, spawner, manager, and CLI generator

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
