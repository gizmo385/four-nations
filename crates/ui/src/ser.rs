//! Serializers for the few `fn_sim` types a view model embeds that don't implement `Serialize`
//! themselves (#858), so the sim stays free of a serialization format for UI-only shapes. Each
//! turns the value into something a reader of the JSON can use without the sim's types: a plain
//! tuple for a rectangle, the sim's own `Display` wording for an error or a blocker. Only built
//! with the `serde` feature.

use fn_sim::Pos;
use fn_sim::geom::Footprint;
use fn_sim::world::{Blocker, BuildError};
use serde::{Serialize, Serializer};

/// A footprint as `[anchor, w, h]`.
pub fn footprint<S: Serializer>(f: &Footprint, s: S) -> Result<S::Ok, S::Error> {
    (f.anchor, f.w, f.h).serialize(s)
}

/// A refusal as its `Display` text ("no such building"), or `null`.
pub fn refusal<S: Serializer>(e: &Option<BuildError>, s: S) -> Result<S::Ok, S::Error> {
    e.as_ref().map(ToString::to_string).serialize(s)
}

/// Each tile with what blocks it, as `Display` text, or `null` where nothing does.
pub fn blocked_tiles<S: Serializer>(
    tiles: &[(Pos, Option<Blocker>)],
    s: S,
) -> Result<S::Ok, S::Error> {
    s.collect_seq(tiles.iter().map(|(pos, why)| (pos, why.as_ref().map(ToString::to_string))))
}
