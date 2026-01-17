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
        runtimeLibs = with pkgs;
          lib.optionals stdenv.isLinux [
            vulkan-loader
            libxkbcommon
            wayland
            wayland-protocols
            libGL
            mesa
            libinput
            fontconfig
            xorg.libX11
            xorg.libXcursor
            xorg.libXi
            xorg.libXrandr
            wl-clipboard # For clipboard support on Wayland
          ];
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
          nativeBuildInputs = with pkgs; [
            pkg-config
            makeWrapper
          ];
          buildInputs = runtimeLibs;
          postInstall = pkgs.lib.optionalString pkgs.stdenv.isLinux ''
            wrapProgram $out/bin/${crateName} \
              --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath runtimeLibs}
          '';
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
          ] ++ runtimeLibs;
          shellHook = pkgs.lib.optionalString pkgs.stdenv.isLinux ''
            export LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath runtimeLibs}:$LD_LIBRARY_PATH
          '';
          RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
        };

        formatter = pkgs.alejandra;
      });
}
