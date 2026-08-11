<div align="center">
  <img src="icons/icon.svg" alt="Paperless Scanner logo" width="96" height="96">
  <h1>Paperless Scanner</h1>
  <p>Scan documents and send them to Paperless-ngx.</p>
</div>

Paperless Scanner is a small, touch-friendly Linux desktop client for
SANE-compatible scanners and Paperless-ngx.

It exists to make ingesting stacks of documents into Paperless-ngx quick and
painless with a dedicated document scanner.

## Try it

The current build targets Linux. Windows and macOS support is planned.

Install the system dependencies on Debian or Ubuntu:

```sh
sudo apt update
sudo apt install build-essential pkg-config \
  libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev patchelf sane-utils \
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

Tagged releases publish x86_64 Debian (`.deb`) and Fedora (`.rpm`) packages on
the [release page](https://github.com/erikgoldenstein/simple-scan-paperless/releases).
Download both packages from the release assets, then install the one matching
your distribution:

```sh
# Debian or Ubuntu
sudo apt install './Paperless Scanner_<version>_amd64.deb'

# Fedora
sudo dnf install './Paperless Scanner-<version>-1.x86_64.rpm'
```

The packages install the desktop launcher and pull in the required Linux
runtime libraries. SANE still needs to be configured separately; verify that
your scanner is visible with `scanimage -L`.

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
