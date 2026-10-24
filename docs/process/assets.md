# Assets and provenance

Every file under `assets/` must be listed in `assets/manifest.toml` with its source, license
and sha256. `cargo xtask assets` enforces this and runs as part of `cargo xtask ci`.

## Adding an asset

1. Prefer **CC0** (Kenney, OpenGameArt CC0), then **CC-BY**, then anything else. Because the
   game is personal-only, "free for personal use" packs and fan art are acceptable. They just
   need `release_ok = false` and an honest `license` field (`UNKNOWN` is fine).
2. Put it under `assets/third_party/<source-id>/` (or `assets/original/` for our own art) and
   keep the upstream license file next to it.
3. Add a `[[source]]` entry (url, download_url, author, license, `release_ok`, retrieved date,
   who retrieved it) and a `[[file]]` entry per file. Note any modifications: crops,
   re-encodes, recolors.
4. Run `cargo xtask assets`.

## `release_ok`

The game is personal and never distributed (see docs/design/vision.md), so assets with unknown
or non-commercial licenses are allowed. We still record provenance. `release_ok = false` flags
sources that would have to be replaced *if* that ever changed. `cargo xtask assets --release`
lists them as errors, and `cargo xtask assets` only warns. Today the flagged source is
`thefournations-2013` (unknown artists, canon-character fan art).

## Original art

Art we make ourselves lives in `assets/original/` (source `original`) and is never edited by
hand. It is written as palette-indexed text sheets in `art/*.toml` (format: `art/README.md`;
design: `docs/design/pixel-art.md`) and rendered by `cargo xtask art`, which also writes the
`[[file]]` entries (with `generated_from = "art/<sheet>.toml"`) in `assets/manifest.toml`.
`cargo xtask assets` re-renders each generated file in memory and fails with "stale: run cargo
xtask art" if the PNG differs, and fails if an `original` PNG has a pixel that isn't a palette
colour. To change original art, edit the sheet and rerun `cargo xtask art`.
