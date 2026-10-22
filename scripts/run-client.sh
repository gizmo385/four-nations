#!/usr/bin/env bash
# Runs fn_client on a Nix-toolchain + Ubuntu (incl. WSL2/WSLg) machine.
#
# Nix-built binaries don't search /usr/lib, and pointing LD_LIBRARY_PATH at all of /usr/lib breaks
# Nix's glibc. So we symlink the system's graphics libs (minus glibc itself) into target/syslibs,
# and take libxkbcommon from nixpkgs (Ubuntu doesn't ship the -x11 variant by default).
# Replace with a flake devShell when issue #10 lands.
#
#   scripts/run-client.sh [seed] [nation] [width] [height]
#   FN_SCREENSHOT=/tmp/shot.png scripts/run-client.sh 42 earth   # headless: no window, save, exit
#
# Prefer `cargo xtask screenshot SEED NATION OUT.png [WIDTH HEIGHT]` for headless screenshots; it
# calls this script. Headless runs need no X11, but still load Vulkan (llvmpipe) from target/syslibs.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
libs="$root/target/syslibs"
say() { echo "run-client: $*" >&2; }

if [[ ! -d "$libs" ]]; then
  say "linking system graphics libraries into target/syslibs (first run only)"
  mkdir -p "$libs"
  for f in /usr/lib/x86_64-linux-gnu/lib*.so*; do
    case "$(basename "$f")" in
      libc.so*|libc-*|ld-linux*|libm.so*|libmvec*|libpthread*|libdl.so*|librt.so*|libresolv*|\
      libutil.so*|libanl*|libnss*|libthread_db*|libBrokenLocale*|libmemusage*|libpcprofile*|libc_malloc*) ;;
      *) ln -sf "$f" "$libs/" ;;
    esac
  done
fi
say "resolving libxkbcommon from nixpkgs"
xkb="$(nix build --no-link --print-out-paths nixpkgs#libxkbcommon)"
# Not -q: after pulling changes the rebuild can take a minute, and silence looks like a hang.
say "building fn_client"
cargo build -p fn_client --manifest-path "$root/Cargo.toml"
if [[ -n "${FN_SCREENSHOT:-}" ]]; then
  say "rendering headless screenshot to $FN_SCREENSHOT"
else
  say "starting (the window can take a few seconds to appear with software rendering)"
fi
LD_LIBRARY_PATH="$xkb/lib:$libs" exec "$root/target/debug/fn_client" "$@"
