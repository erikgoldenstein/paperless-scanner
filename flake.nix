{
  description = "Paperless Scanner desktop client";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in {
      packages = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          linuxInputs = with pkgs; [
            gtk3
            libayatana-appindicator
            librsvg
            xdotool
            openssl
            sane-backends
            webkitgtk_4_1
          ];
          desktopItem = pkgs.makeDesktopItem {
            name = "paperless-scanner";
            desktopName = "Paperless Scanner";
            comment = "Scan documents and send them to Paperless-ngx";
            exec = "paperless-scanner";
            icon = "paperless-scanner";
            categories = [ "Office" "Scanning" ];
          };
        in {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "paperless-scanner";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            cargoBuildFlags = [ "--features" "gui" ];
            # cmake is only needed by a crate build script, not as the build system.
            nativeBuildInputs = with pkgs; [ pkg-config cmake ]
              ++ pkgs.lib.optionals pkgs.stdenv.isLinux [ wrapGAppsHook3 ];
            dontUseCmakeConfigure = true;
            buildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux linuxInputs;
            doCheck = false;
            # cargoBuildHook writes to target/<triple>/release.
            installPhase = ''
              runHook preInstall
              install -Dm755 target/*/release/paperless-scanner $out/bin/paperless-scanner
              install -Dm644 icons/icon.svg $out/share/icons/hicolor/scalable/apps/paperless-scanner.svg
              install -Dm644 ${desktopItem}/share/applications/* -t $out/share/applications
              runHook postInstall
            '';
          };
        });

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/paperless-scanner";
        };
      });

      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
        in {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo
              clippy
              cmake
              nodejs_22
              python3
              rustc
              rustfmt
            ];
            nativeBuildInputs = with pkgs; [ pkg-config ];
            buildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux [
              pkgs.gtk3
              pkgs.libayatana-appindicator
              pkgs.librsvg
              pkgs.xdotool
              pkgs.openssl
              pkgs.sane-backends
              pkgs.webkitgtk_4_1
            ];
          };
        });
    };
}
