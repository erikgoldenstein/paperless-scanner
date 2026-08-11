# Development

Install the system dependencies from the main [README](../README.md), then
install the project dependencies:

```sh
npm ci
cargo fetch
```

Run the application:

```sh
cargo tauri dev --features gui
```

Run the checks:

```sh
cargo test --locked --all-features
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
npm test
npm run check
npm run test:e2e
```

The main code is in `src/`. The frontend is in `frontend/`, its unit tests are
next to the source files, and Playwright tests are in `tests/`.

Build a local executable without packaging:

```sh
cargo tauri build --features gui --no-bundle
```

Tagged CI releases build Debian and Fedora packages. A Linux Nix build is
available with:

```sh
nix build
```

Image encoder benchmarks:

```sh
cargo bench --bench jpeg_encoding --features jpeg-turbo
cargo bench --bench jpeg2000_encoding --features openjph-experiment
```
