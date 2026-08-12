<div align="center">
  <img src="icons/icon.svg" alt="Paperless Scanner logo" width="96" height="96">
  <h1>Paperless Scanner</h1>
  <p>Scan documents and send them to Paperless-ngx.</p>
</div>

Paperless Scanner is a small, touch-friendly desktop client for scanners and
Paperless-ngx.

It exists to make ingesting stacks of documents into Paperless-ngx quick and
painless with a dedicated document scanner.

## Try it

The release builds currently target:

- Linux x86_64: `.deb`, `.rpm`, and `.AppImage`
- Linux ARM64: `.deb` and `.AppImage`
- Windows x86_64: `.msi` and NSIS `.exe`
- macOS Intel: `.dmg`
- macOS Apple Silicon: `.dmg`

These targets are available as builds, but only Linux is actively used and
tested at present. Contributions that fix or improve the other targets are
welcome.

An Android version is a potential future target for compact touchscreen
devices. It will require a separate Android scanner backend and USB-permission
integration; it is not supported yet.

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

Tagged releases publish all of the packages listed above on the
[release page](https://github.com/erikgoldenstein/simple-scan-paperless/releases).
Download the package matching your platform from the release assets.

```sh
# Debian or Ubuntu
sudo apt install './Paperless Scanner_<version>_amd64.deb'

# Fedora
sudo dnf install './Paperless Scanner-<version>-1.x86_64.rpm'

# Linux distributions supporting AppImage
chmod +x './Paperless Scanner_<version>_amd64.AppImage'
./'Paperless Scanner_<version>_amd64.AppImage'
```

Windows releases include an MSI and an NSIS installer. macOS releases are
provided separately for Intel and Apple Silicon.

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
