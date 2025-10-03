{
  description = "Rust development environment for vedit";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        crateName = "vedit";
      in {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = crateName;
          version = "0.1.0";
          src = ./.;
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          meta = with pkgs.lib; {
            description = "Vedit Rust application";
            mainProgram = crateName;
          };
        };

        apps.default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/${crateName}";
          meta = {
            description = "Run the vedit binary";
          };
        };

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            rustc
            cargo
            rustfmt
            clippy
            rust-analyzer
            pkg-config
          ];
          RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
        };

        formatter = pkgs.alejandra;
      });
}
