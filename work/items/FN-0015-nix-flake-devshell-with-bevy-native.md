+++
id = "FN-0015"
title = "Nix flake devShell with Bevy native libs"
status = "proposed"     # proposed | ready | in-progress | review | done | dropped
priority = "p2"         # p0 (blocking) .. p3 (someday)
area = "tooling"            # sim | client | data | art | tooling | design | docs
depends_on = []         # e.g. ["FN-0003"]
owner = ""              # set when claimed: branch name, e.g. "work/FN-0007-pathfinding"
created = 2026-10-21
+++

## Why
`scripts/run-client.sh` works around Nix binaries not finding Ubuntu's X11/Vulkan libs by
symlinking /usr/lib into target/syslibs. A flake devShell (libX11, libxkbcommon, vulkan-loader,
alsa, udev, wayland) makes this reproducible and lets us re-enable audio/gamepad/wayland features.

## Acceptance criteria
- [ ] `nix develop` then `cargo run -p fn_client` works on WSL2 and plain Linux.
- [ ] run-client.sh deleted or reduced to `nix develop -c`.

## Notes
- 2026-10-21: On this WSL2 box only llvmpipe (CPU) Vulkan is reachable from Nix binaries; fine
  for a 96x64 map, but investigate the d3d12/dzn driver for real GPU use.
