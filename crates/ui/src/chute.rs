//! Picking a chute's foot (#683, docs/design/horizons-phase-1.md "`omashu_chutes`"): the tiles a
//! foot may go on, the line drawn from head to foot, and `Command::SetLink` with its refusals
//! worded for the player.
//!
//! The TUI asks for the foot right after a chute site is placed (the Build tool) and again when
//! `O` is pressed on a chute in Select mode; either way `Enter` on the map sets it here. Whether a
//! tile may be the foot is the sim's (`World::check_foot`); this only lists and words the answer.

use fn_sim::Pos;
use fn_sim::world::{Command, CommandError};

use crate::session::Session;

/// What the Tool section says while a foot is being picked.
pub const ASKING: &str = "Pick the chute's foot: Enter sets it, Esc leaves it unset";

/// Whether a chute, finished or still a site, is anchored on `pos`.
pub fn is_head(session: &Session, pos: Pos) -> bool {
    session.world.chute_reach(pos, &session.defs).is_some()
}

/// The first chute site in the rectangle from `from` to `to` (corners in any order) that has no
/// foot yet: the one the Build tool asks about after placing it.
pub fn unlinked_in(session: &Session, from: Pos, to: Pos) -> Option<Pos> {
    let (x0, x1) = (from.x.min(to.x), from.x.max(to.x));
    let (y0, y1) = (from.y.min(to.y), from.y.max(to.y));
    (y0..=y1)
        .flat_map(|y| (x0..=x1).map(move |x| Pos::new(x, y)))
        .find(|&p| is_head(session, p) && session.world.link_at(p).is_none())
}

/// Every tile that may be the foot of the chute anchored on `head`: the picker's reach tint.
pub fn foot_tiles(session: &Session, head: Pos) -> Vec<Pos> {
    let Some(reach) = session.world.chute_reach(head, &session.defs) else { return Vec::new() };
    let r = reach as i32;
    (head.y - r..=head.y + r)
        .flat_map(|y| (head.x - r..=head.x + r).map(move |x| Pos::new(x, y)))
        .filter(|&p| session.world.check_foot(head, p, &session.defs).is_ok())
        .collect()
}

/// Why `to` can't be the foot of the chute on `head`, if it can't: "Can't put the foot there:
/// foot out of reach". For the notice as the cursor moves.
pub fn refusal(session: &Session, head: Pos, to: Pos) -> Option<String> {
    let e = session.world.check_foot(head, to, &session.defs).err()?;
    Some(format!("Can't put the foot there: {e}"))
}

/// Points the chute on `head` at `to`: "Chute foot set at (31, 18)", or the refusal.
pub fn set_foot(session: &mut Session, head: Pos, to: Pos) -> Result<String, String> {
    match session.apply(Command::SetLink { pos: head, to }) {
        Ok(()) => Ok(format!("Chute foot set at ({}, {})", to.x, to.y)),
        Err(CommandError::Link(e)) => Err(format!("Can't put the foot there: {e}")),
        Err(e) => Err(format!("Can't set the foot: {e}")),
    }
}

/// The tiles between `head` and `foot` (both left out) on a straight line: where the `:` link
/// line is drawn.
pub fn link_line(head: Pos, foot: Pos) -> Vec<Pos> {
    let (dx, dy) = ((foot.x - head.x).abs(), -(foot.y - head.y).abs());
    let (sx, sy) = ((foot.x - head.x).signum(), (foot.y - head.y).signum());
    let (mut x, mut y, mut err) = (head.x, head.y, dx + dy);
    let mut out = Vec::new();
    while (x, y) != (foot.x, foot.y) {
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
        if (x, y) != (foot.x, foot.y) {
            out.push(Pos::new(x, y));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_link_line_leaves_out_both_ends() {
        let line = link_line(Pos::new(0, 0), Pos::new(4, 2));
        assert_eq!(line.len(), 3);
        assert!(!line.contains(&Pos::new(0, 0)) && !line.contains(&Pos::new(4, 2)));
        assert!(link_line(Pos::new(0, 0), Pos::new(1, 1)).is_empty());
        assert_eq!(link_line(Pos::new(3, 0), Pos::new(0, 0)), vec![Pos::new(2, 0), Pos::new(1, 0)]);
    }
}
