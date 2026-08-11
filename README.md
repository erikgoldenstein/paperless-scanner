# Paperless Scanner

A small desktop scanner client for Paperless-ngx. The first version targets Linux and SANE.

## Features

- Add pages from a SANE scanner
- Rescan and replace the selected page
- Review pages with thumbnails and a large preview
- Combine pages into a PDF
- Upload the PDF to Paperless-ngx
- Organize pages as document groups with tab-like previews
- Reorder page tabs by dragging them
- Choose A4, US Letter, Legal, or A5 PDF paper size
- Optional Simple mode for one-document workflows
- Save scanner and Paperless settings locally

## Linux prerequisites

Install the system libraries required by Tauri's Linux webview, plus SANE:

```sh
sudo apt install build-essential libwebkit2gtk-4.1-dev \
  libgtk-3-dev libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev patchelf sane-utils
```

Check that the scanner is visible before starting the app:

```sh
scanimage -L
```

## Development

Run the behavior tests without desktop system dependencies:

```sh
cargo test
npm test
npm run test:e2e
```

Install the Tauri CLI once, then run the desktop app:

```sh
cargo install tauri-cli --version '^2'
cargo tauri dev --features gui
```

Build a release bundle:

```sh
cargo tauri build --features gui
```

Benchmark the image encoders:

```sh
cargo bench --bench jpeg_encoding --features jpeg-turbo
cargo bench --bench jpeg2000_encoding --features openjph-experiment
```

The OpenJPH/HTJ2K path is experimental and is not used by the app. OpenJPEG is
the production JPEG 2000 encoder because it is currently faster and produces
smaller PDF image streams in the included benchmark.

The first run opens Settings. Select a scanner, enter the Paperless URL and API token, then save.

## Architecture

- `frontend/`: deliberately plain HTML, CSS, and JavaScript
- `frontend/document-state.js`: small, DOM-free document/tab-group state helpers
- `tests/`: Playwright end-to-end tests for the real frontend flow
- `src/lib.rs`: testable document/session behavior plus Tauri commands
- `src/main.rs`: desktop entry point
- Linux scanning uses the installed `scanimage` command from SANE

Scanner support for Windows and macOS should be added as platform-specific backends while keeping the frontend and document workflow unchanged.
