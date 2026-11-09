{
  # Dev environment for The Four Nations: the pinned Rust toolchain.
  #
  #   nix develop                    # interactive shell
  #   nix develop -c cargo xtask ci  # one command
  #
  # The Rust version, components and targets come from rust-toolchain.toml, the one source that CI
  # (`rustup toolchain install`) and plain rustup also read. Change the pin there, not here.
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
          # Extra tools for editing only; everything the gate needs is in the toolchain file.
          rust = (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override {
            extensions = [ "rust-src" "rust-analyzer" ];
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
