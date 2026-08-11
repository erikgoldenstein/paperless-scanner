<div align="center">
  <img src="icons/icon.svg" alt="Paperless Scanner logo" width="96" height="96">
  <h1>Paperless Scanner</h1>
  <p>The quickest desktop scanner client for Paperless-ngx.</p>
</div>

Paperless Scanner is the fastest and least painful way to quickly ingest large numbers of documents into a paperless instance using a dedicated scanner.

It is a small, touch-friendly desktop application for scanning
multi-page documents, reviewing them, turning them into PDFs, and uploading
them directly to a Paperless-ngx instance.

It is built with Rust, Tauri, and a plain HTML/CSS/JavaScript frontend.
The current implementation is Linux-first and uses SANE's `scanimage`
command. The project is intentionally small and straightforward so that
platform-specific scanner backends and improvements to the document workflow
can be added without replacing the frontend.

## Application screenshot

<!-- TODO: add an application screenshot here. -->
tbd


## Getting Started

### Current support

The application currently supports Linux systems with a SANE-compatible
scanner. It has been tested through the frontend and Rust test suites, but it
does not yet provide native scanner backends for Windows or macOS.

### Dependencies

For the current Linux implementation you need:

- A recent stable [Rust toolchain](https://www.rust-lang.org/tools/install)
- Tauri 2's Linux WebKit/GTK development libraries
- SANE and `scanimage`
- Node.js and npm if you want to run the JavaScript and Playwright tests

On Debian or Ubuntu, the required native packages can be installed with:

```sh
sudo apt update
sudo apt install build-essential libwebkit2gtk-4.1-dev \
  libgtk-3-dev libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev patchelf sane-utils
```

Package names differ between Linux distributions. The equivalent WebKitGTK,
GTK, OpenSSL, AppIndicator, librsvg, patchelf, and SANE development packages
will be needed on other distributions.

Verify that SANE can see your scanner before launching the application:

```sh
scanimage -L
```

You may also need to configure your distribution's scanner permissions or
SANE backend configuration. That part is distribution- and device-specific.

### Install from source

Prebuilt release packages are not published yet, so the current installation
path is to build the application locally:

```sh
git clone <repository-url>
cd paperless-scanner
cargo install tauri-cli --version '^2'
cargo tauri build --features gui
```

The release executable is written to `target/release/paperless-scanner`.
Tauri bundling is currently disabled, so the repository does not yet produce
native `.deb`, `.rpm`, or other installer packages.

Start the development application with:

```sh
cargo tauri dev --features gui
```

On first launch, open Settings, select a scanner, enter the Paperless-ngx URL
and API token, and save. The application stores these settings locally.

### Currently implemented

- Scan pages from a SANE-compatible scanner
- Rescan and replace the selected page
- Large page preview with rotation, zoom, panning, and thumbnails
- Drag-and-drop page reordering
- Multi-page PDF creation and upload to Paperless-ngx
- Upload progress, retry handling, and optional filename/title prompts
- JPEG and JPEG 2000 PDF image compression
- A4, US Letter, Legal, and A5 paper formats
- Optional hash-based filenames
- Simple mode for one-document workflows
- Archived document groups with the newest archives at the right
- Local recovery of pages from an interrupted session
- Light, dark, and system-following themes
- Optional debug details and document state history

## Building from source

Install the dependencies above, then install the Rust and JavaScript project
dependencies:

```sh
npm ci
cargo fetch
```

Run the automated checks:

```sh
cargo test
npm test
npm run check
npm run test:e2e
```

Run the desktop application during development:

```sh
cargo tauri dev --features gui
```

Build an optimized release executable:

```sh
cargo tauri build --features gui
```

The optional image-encoder benchmarks are available with:

```sh
cargo bench --bench jpeg_encoding --features jpeg-turbo
cargo bench --bench jpeg2000_encoding --features openjph-experiment
```

The OpenJPH/HTJ2K path is experimental and is not used by the application.
OpenJPEG is currently the production JPEG 2000 encoder.

## Architecture

- `frontend/` contains the plain HTML, CSS, and JavaScript application
- `frontend/document-state.js` contains DOM-free document and tab-group state
  helpers
- `tests/` contains Playwright end-to-end coverage for the frontend flow
- `src/lib.rs` contains the testable document/session behavior and Tauri
  commands
- `src/main.rs` is the desktop entry point
- Linux scanning currently runs the installed `scanimage` command from SANE

## Contributing

This repository is the current implementation, not a claim that scanner
support is complete. Contributions are very welcome.

In particular, help with any of the following would be valuable:

- Scanner support for Windows, macOS, and less common Linux distributions
- Additional scanner backends, drivers, and device families
- Better handling of permissions and installation across distributions
- Bug fixes, accessibility improvements, tests, documentation, and UX work
- Paperless-ngx compatibility improvements and integrations

Please open an issue before large changes when possible, describe the platform
and scanner involved, and include reproducible steps for bugs. Small focused
pull requests are especially easy to review.

## License

Paperless Scanner is free software: you can redistribute it and/or modify it
under the terms of the [GNU General Public License, version 3 or later](LICENSE).

This project is copyleft software. Contributions are made under the same
license unless a different arrangement is agreed to explicitly.
