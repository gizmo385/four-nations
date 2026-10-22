{
  # Dev environment for The Four Nations: the pinned Rust toolchain plus every native library Bevy
  # needs to build and run, so Nix-built binaries never have to borrow Ubuntu's /usr/lib.
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
          inherit (pkgs) lib;
          rust = pkgs.rust-bin.stable.${rustVersion}.default.override {
            extensions = [ "rust-src" "rust-analyzer" "clippy" "rustfmt" ];
          };
          # Loaded with dlopen at runtime (winit, wgpu, cpal, gilrs), so the linker never records
          # them and they must be on LD_LIBRARY_PATH.
          runtimeLibs = with pkgs; [
            vulkan-loader
            libxkbcommon
            xorg.libX11
            xorg.libXcursor
            xorg.libXi
            xorg.libXrandr
            wayland
            alsa-lib
            udev
          ];
        in
        {
          default = pkgs.mkShell {
            packages = [
              rust
              pkgs.pkg-config
            ];
            # Headers for features we don't enable yet (bevy audio/gamepad/wayland) so turning
            # them on is a Cargo.toml change only.
            buildInputs = runtimeLibs;
            LD_LIBRARY_PATH = lib.makeLibraryPath runtimeLibs;
            shellHook = ''
              # Mesa's Vulkan drivers: lavapipe (CPU) everywhere, plus dozen (Vulkan on D3D12) for
              # WSL2 GPUs. Outside NixOS the loader would otherwise read /usr/share/vulkan, whose
              # drivers link Ubuntu's libc and can't load into a Nix-built process.
              VK_DRIVER_FILES="$(echo ${pkgs.mesa}/share/vulkan/icd.d/*.json | tr ' ' :)"
              export VK_DRIVER_FILES
              # WSL2: dozen needs Microsoft's libd3d12/libdxcore, which the host mounts here.
              if [ -d /usr/lib/wsl/lib ]; then
                export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:/usr/lib/wsl/lib"
              fi
            '';
          };
        }
      );
    };
}
