{
  description = "PolyGlid cross-platform development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { nixpkgs, rust-overlay, ... }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in
    {
      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          rustToolchain = pkgs.rust-bin.stable."1.96.0".minimal.override {
            extensions = [ "rustfmt" ];
            targets = [ "wasm32-unknown-unknown" ];
          };
          moon = assert pkgs.moon.version == "2.4.6"; pkgs.moon;
          dioxusCli = assert pkgs.dioxus-cli.version == "0.7.9"; pkgs.dioxus-cli;
          linuxLibraries = with pkgs; [
            fontconfig
            freetype
            libxcb
            libxkbcommon
            mesa
            vulkan-loader
            wayland
            xdotool
          ];
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cacert
              cargo-watch
              curl
              dioxusCli
              git
              go
              jq
              moon
              pkg-config
              rustToolchain
            ];

            buildInputs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux linuxLibraries;

            LD_LIBRARY_PATH = pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux (
              pkgs.lib.makeLibraryPath linuxLibraries
            );
            LIBRARY_PATH = pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux (
              pkgs.lib.makeLibraryPath linuxLibraries
            );
          };
        }
      );
    };
}
