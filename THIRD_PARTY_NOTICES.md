# Third-party notices

Paperless Scanner is distributed under the GNU General Public License v3 or
later. Its source and build dependencies remain under their own licenses.

The primary runtime dependencies include:

- Tauri and Tauri Build: Apache-2.0 or MIT
- Rustls-enabled Reqwest: MIT or Apache-2.0
- image: MIT or Apache-2.0
- lopdf: MIT
- OpenJPEG/OpenJPEG-sys: BSD-2-Clause
- OpenJPH: BSD-2-Clause when the experimental feature is enabled
- keyring: MIT or Apache-2.0
- TurboJPEG bindings: Unlicense or MIT; the bundled libjpeg-turbo code is
  distributed under its upstream BSD-style license

The JavaScript test dependency, Playwright, is Apache-2.0 licensed.

Cargo.lock and package-lock.json identify the exact dependency versions and
integrity data used by this repository. Before publishing a binary release,
regenerate a complete dependency-license report for the locked dependency
graph and include any required upstream license texts in the release source
archive.
