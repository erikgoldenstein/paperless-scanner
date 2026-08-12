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
        in {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "paperless-scanner";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            cargoBuildFlags = [ "--features" "gui" ];
            nativeBuildInputs = with pkgs; [ pkg-config ];
            buildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux linuxInputs;
            doCheck = false;
            installPhase = ''
              install -Dm755 target/release/paperless-scanner $out/bin/paperless-scanner
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
