<div align="center">
  <img src="icons/icon.svg" alt="Paperless Scanner logo" width="96" height="96">
  <h1>Paperless Scanner</h1>
  <p>Scan documents and send them to Paperless-ngx.</p>
</div>

Paperless Scanner is a small, touch-friendly desktop client for scanners and
Paperless-ngx.

> **Alpha software:** v0.1.0 is an early Linux release. Use it for testing and
> expect rough edges; Windows, macOS, Android, and other platform targets are
> still in development.

It exists to make ingesting stacks of documents into Paperless-ngx quick and
painless with a dedicated document scanner.

## Try it

The v0.1.0 release currently publishes only Linux packages:

- Linux x86_64: `.deb` and `.rpm`
- Linux ARM64: `.deb` and `.rpm`

Windows and macOS support is currently in development and no Windows or macOS
installers are included in v0.1.0. AppImage and Android support are also not
part of this alpha release.

An Android version is a potential future target for compact touchscreen
devices. It will require a separate Android scanner backend and USB-permission
integration; it is not supported yet.

Install the system dependencies on Debian or Ubuntu:

```sh
sudo apt update
sudo apt install build-essential pkg-config \
  libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev patchelf sane-utils libsane-dev \
  libopenjp2-7-dev
```

Check that SANE can see your scanner:

```sh
scanimage -L
```

Build and run the application:

```sh
git clone https://github.com/erikgoldenstein/simple-scan-paperless.git
cd simple-scan-paperless
cargo install tauri-cli --version '^2'
cargo tauri dev --features gui
```

On first launch, choose a scanner and enter your Paperless-ngx URL and API
token. HTTPS is recommended. The token is stored in the operating system's
credential store.

## Install a release

Tagged releases publish the Linux `.deb` and `.rpm` packages listed above on the
[release page](https://github.com/erikgoldenstein/simple-scan-paperless/releases).
Download the package matching your Linux architecture from the release assets.

```sh
# Debian or Ubuntu
sudo apt install './Paperless Scanner_<version>_amd64.deb'

# Fedora
sudo dnf install './Paperless Scanner-<version>-1.x86_64.rpm'

```

The packages install the desktop launcher and pull in the required Linux
runtime libraries. On Linux, the app delegates SANE discovery and scanning to
the `scanimage` command, so the SANE driver packages still need to be installed
and configured separately; verify that your scanner is visible with
`scanimage -L`. The Linux Settings dialog also provides a separate
`Linux SANE (legacy external scanimage)` backend for installations that worked
with the older external `scanimage` workflow; it performs the historical
preflight and option fallback sequence. Windows uses the WIA service built into Windows, macOS uses
ImageCaptureCore, and the eSCL backend talks directly to compatible network
scanners. These non-Linux backends are development-only and are not included
in the v0.1.0 release.

To use an eSCL scanner, select the eSCL backend in Settings and enter its base
HTTP URL (for example, `http://scanner.local`). The legacy
`PAPERLESS_SCANNER_ESCL_URL` environment variable remains available as a
fallback for scripted deployments.

With Nix, run it directly with:

```sh
nix shell github:erikgoldenstein/simple-scan-paperless -c paperless-scanner
```

## Development

See [docs/development.md](docs/development.md) for the project layout, test
commands, builds, and benchmarks.

## More

- [Security policy](SECURITY.md)
- [Third-party notices](THIRD_PARTY_NOTICES.md)
- [Asset provenance](ASSET_LICENSES.md)

## License

Paperless Scanner is licensed under the
[GNU General Public License, version 3 or later](LICENSE).
