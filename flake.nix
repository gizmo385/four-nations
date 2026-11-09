{
  # Dev environment for The Four Nations: the pinned Rust toolchain.
  #
  #   nix develop                    # interactive shell
  #   nix develop -c cargo xtask ci  # one command
  #
  # CI does not use this (it installs Rust with dtolnay/rust-toolchain). Keep `rustVersion` in sync
  # with .github/workflows/ci.yml so local clippy matches CI.
  description = "The Four Nations dev shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # rust-overlay over fenix: it pins an exact stable release by version string with no extra
    # hash to maintain, and offers the same component set rustup does.
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { nixpkgs, rust-overlay, ... }:
    let
      rustVersion = "1.98.1";
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems =
        f:
        nixpkgs.lib.genAttrs systems (
          system:
          f (
            import nixpkgs {
              inherit system;
              overlays = [ rust-overlay.overlays.default ];
            }
          )
        );
    in
    {
      devShells = forAllSystems (
        pkgs:
        let
          rust = pkgs.rust-bin.stable.${rustVersion}.default.override {
            extensions = [ "rust-src" "rust-analyzer" "clippy" "rustfmt" ];
            # `cargo xtask ci` lints the web replayer's crates for wasm32 (#594).
            targets = [ "wasm32-unknown-unknown" ];
          };
        in
        {
          default = pkgs.mkShell {
            packages = [
              rust
              pkgs.pkg-config
            ];
          };
        }
      );
    };
}
