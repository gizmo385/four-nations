//! Picking the other end of a link (#683 for the chute's foot, #698 for a pair's partner): what
//! the Build tool and `O` on the map ask for, how a refusal reads, and the line drawn between the
//! two ends.
//!
//! A chute's foot is a tile (`chute`). A pair's partner is a second building or site of the same
//! id (docs/design/horizons-phase-2.md, "Shared sim work, 1. Paired links"): `Enter` on an
//! existing end links to it, and `Enter` anywhere else places a new site there and links it, so
//! both ends are placed in one go. `O` on an end later and `Enter` on another moves the partner:
//! `SetLink` clears any partner either end had. The head being linked is always an anchor,
//! whichever kind it is.
//!
//! Whether a partner is allowed is the sim's (`World::check_partner`, `Command::SetLink`); this
//! only words the answer. It checks reach and the new site's own footprint up front, so a refused
//! click doesn't leave a site behind.

use fn_sim::Pos;
use fn_sim::building::BuildingId;
use fn_sim::geom::Footprint;
use fn_sim::world::{Command, CommandError, LinkError};

use crate::build::anchor_for;
use crate::session::Session;

/// What the Tool section says while a pair's partner is being picked.
pub const ASKING_PARTNER: &str = "Pick the partner: Enter places or sets it, Esc leaves it unset";

/// The anchor of the chute or pair building (finished or a site) covering `pos`: the head a
/// foot or partner is picked for.
pub fn head_at(session: &Session, pos: Pos) -> Option<Pos> {
    if crate::chute::is_head(session, pos) {
        return Some(pos);
    }
    end_at(session, pos)
}

/// The anchor of the pair building or site covering `pos` (any tile of its footprint).
pub fn end_at(session: &Session, pos: Pos) -> Option<Pos> {
    let anchor = session.world.footprint_at(pos)?.anchor;
    session.world.pair_reach(anchor, &session.defs)?;
    Some(anchor)
}

/// The id of the pair building or site anchored on `end`.
pub fn pair_id(session: &Session, end: Pos) -> Option<BuildingId> {
    session.world.pair_reach(end, &session.defs)?;
    let world = &session.world;
    world.building_at(end).or_else(|| world.build_site(end).map(|(id, ..)| id))
}

/// Whether the head being linked is a pair end, not a chute.
pub fn is_pair(session: &Session, head: Pos) -> bool {
    session.world.pair_reach(head, &session.defs).is_some()
}

/// What the Tool section says while `head`'s other end is being picked.
pub fn asking(session: &Session, head: Pos) -> &'static str {
    if is_pair(session, head) { ASKING_PARTNER } else { crate::chute::ASKING }
}

/// What the notice says when picking is abandoned with `Esc`.
pub fn left_unset(session: &Session, head: Pos) -> &'static str {
    if is_pair(session, head) { "Partner left unset" } else { "Chute foot left unset" }
}

/// The first chute or pair site in the Build tool's rectangle that has no other end yet: the one
/// to ask about after placing `building` there.
pub fn unlinked_in(
    session: &Session,
    from: Pos,
    to: Pos,
    building: Option<BuildingId>,
) -> Option<Pos> {
    if let Some(id) = building.filter(|&id| session.defs.building(id).pair.is_some()) {
        let footprint = session.defs.building(id).footprint;
        return crate::tools::build_stamps(from, to, footprint)
            .into_iter()
            .find(|&p| end_at(session, p) == Some(p) && session.world.link_at(p).is_none());
    }
    crate::chute::unlinked_in(session, from, to)
}

/// The anchor of the end of `head`'s own id under the cursor, if the cursor is on one that isn't
/// `head` itself.
fn end_under(session: &Session, head: Pos, cursor: Pos) -> Option<Pos> {
    let id = pair_id(session, head)?;
    let at = end_at(session, cursor)?;
    (at != head && pair_id(session, at) == Some(id)).then_some(at)
}

/// Where a new partner for the pair end on `head` would be anchored with the cursor on
/// `cursor`.
fn new_anchor(session: &Session, head: Pos, cursor: Pos) -> Option<(BuildingId, Pos)> {
    let id = pair_id(session, head)?;
    Some((id, anchor_for(cursor, session.defs.building(id).footprint)))
}

/// The building to draw as a ghost at the cursor while `head`'s partner is being picked: the
/// pair's own, which `set` would place there, unless an end of it is under the cursor already.
/// `None` for a chute's foot.
pub fn partner_ghost(session: &Session, head: Pos, cursor: Pos) -> Option<BuildingId> {
    let id = pair_id(session, head)?;
    end_under(session, head, cursor).is_none().then_some(id)
}

/// Why the cursor can't be the other end of `head`, if it can't: for the notice as the cursor
/// moves. `None` when it would be accepted, or when only the same-region rule, which needs the
/// new end standing, could refuse it.
pub fn refusal(session: &Session, head: Pos, cursor: Pos) -> Option<String> {
    if !is_pair(session, head) {
        return crate::chute::refusal(session, head, cursor);
    }
    if let Some(at) = end_under(session, head, cursor) {
        let e = session.world.check_partner(head, at, &session.defs).err()?;
        return Some(format!("Can't pair with that: {e}"));
    }
    let (id, anchor) = new_anchor(session, head, cursor)?;
    let (reach, min) = session.world.pair_reach(head, &session.defs)?;
    let distance = head.chebyshev(anchor);
    if distance > reach as i32 {
        return Some(format!("Can't pair with that: {}", LinkError::TooFar));
    }
    if distance < min as i32 {
        return Some(format!("Can't pair with that: {}", LinkError::TooClose));
    }
    let e = crate::build::placement_at(session, anchor, id).refused?;
    Some(format!("Can't place the partner there: {e}"))
}

/// Sets the other end of `head` at the cursor: a chute's foot, or a pair's partner (placing a new
/// site there unless an end of the same id is under the cursor). The notice on success, or why
/// not.
pub fn set(session: &mut Session, head: Pos, cursor: Pos) -> Result<String, String> {
    if !is_pair(session, head) {
        return crate::chute::set_foot(session, head, cursor);
    }
    if let Some(at) = end_under(session, head, cursor) {
        let tiles = link(session, head, at)?;
        return Ok(format!("Partner set at ({}, {}), {tiles} tiles", at.x, at.y));
    }
    let Some((id, anchor)) = new_anchor(session, head, cursor) else {
        return Err("Can't set the partner: nothing to link".to_owned());
    };
    if let Some(why) = refusal(session, head, cursor) {
        return Err(why);
    }
    session
        .apply(Command::DesignateBuild { pos: anchor, building: id })
        .map_err(|e| format!("Can't place the partner there: {e}"))?;
    match link(session, head, anchor) {
        Ok(tiles) => Ok(format!("Partner placed at ({}, {}), {tiles} tiles", anchor.x, anchor.y)),
        Err(why) => {
            // The same-region rule, which only a standing end can be checked against: take the
            // new site back rather than leave a stray one.
            let _ = session.apply(Command::CancelBuild { pos: anchor });
            Err(why)
        }
    }
}

/// `SetLink` between two ends: how many tiles apart they are, or the refusal worded.
fn link(session: &mut Session, head: Pos, to: Pos) -> Result<i32, String> {
    match session.apply(Command::SetLink { pos: head, to }) {
        Ok(()) => Ok(head.chebyshev(to)),
        Err(CommandError::Link(e)) => Err(format!("Can't pair with that: {e}")),
        Err(e) => Err(format!("Can't set the partner: {e}")),
    }
}

/// The tiles of the `:` link line between `head` and `to`, leaving out the tiles of both ends'
/// footprints. For a chute `to` is the foot tile; for a pair it is where the partner's anchor is
/// or would be.
pub fn line(session: &Session, head: Pos, to: Pos) -> Vec<Pos> {
    let mut tiles = crate::chute::link_line(head, to);
    if is_pair(session, head) {
        let feet: Vec<Footprint> =
            [head, to].into_iter().filter_map(|p| session.world.footprint_at(p)).collect();
        tiles.retain(|&t| !feet.iter().any(|f| f.contains(t)));
    }
    tiles
}
