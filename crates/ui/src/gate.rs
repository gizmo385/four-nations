//! Placing and withdrawing the endeavor's gate (#643, `docs/design/endgame.md`): what the
//! Endeavor screen offers for it, the ghost the Gate tool draws, and the two actions behind it,
//! `Command::BeginEndeavor` and `Command::CancelEndeavor`, with their refusals worded for the
//! player.
//!
//! Every frontend goes through here: the TUI's Endeavor screen (`Enter` arms `Tool::Gate`, whose
//! `Enter` on the map places the site; `X` withdraws it), and through those same keys the agent
//! surface's `gate` and `cancel_gate` steps. Whether the gate fits, and whether the site may still
//! be withdrawn, are the sim's (`World::check_endeavor_site`, `World::check_cancel_endeavor`);
//! this only words the answer.

use fn_sim::Pos;
use fn_sim::building::BuildingId;
use fn_sim::world::{BuildError, Command, CommandError, EndeavorError, EndeavorState};

use crate::build::{self, Placement};
use crate::session::Session;

/// What the Endeavor screen offers for the gate right now, if anything.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GateAction {
    /// The endeavor is Ready: a site may be chosen.
    Place,
    /// The site is placed and no work on it has started: it may be withdrawn.
    Withdraw,
}

impl GateAction {
    /// The screen's own hint line for it: the whole line is the hint (docs/design/text-voice.md,
    /// rule 2).
    pub fn hint(self) -> &'static str {
        match self {
            Self::Place => "Enter: choose a site for the gate",
            Self::Withdraw => "X: withdraw the site",
        }
    }
}

/// What can be done with the gate now: place it once Ready, withdraw it until work starts, and
/// nothing otherwise (signs still to meet, work under way, or the gate finished).
pub fn action(session: &Session) -> Option<GateAction> {
    let (world, defs) = (&session.world, &session.defs);
    match world.endeavor() {
        EndeavorState::Ready { .. } if defs.endeavor.building.is_some() => Some(GateAction::Place),
        EndeavorState::Building(_) if world.check_cancel_endeavor(defs).is_ok() => {
            Some(GateAction::Withdraw)
        }
        _ => None,
    }
}

/// Why a site can't be chosen now, if it can't: "Can't place The Harmony Gate: 5 of 9 signs
/// met". `None` once the endeavor is Ready. What the Endeavor screen's `Enter` and the agent
/// surface's `gate` step say before either touches the map.
pub fn place_refusal(session: &Session) -> Option<String> {
    if action(session) == Some(GateAction::Place) {
        return None;
    }
    let why = match session.world.endeavor() {
        EndeavorState::Ready { .. } => EndeavorError::NoGate.to_string(),
        _ => describe_error(session, EndeavorError::NotReady),
    };
    Some(format!("Can't place {}: {why}", name(session)))
}

/// Why the site can't be withdrawn now, if it can't: "Can't withdraw the site: work has
/// started". `None` while it can.
pub fn withdraw_refusal(session: &Session) -> Option<String> {
    let e = session.world.check_cancel_endeavor(&session.defs).err()?;
    let why = match e {
        EndeavorError::NotBuilding => "no site to withdraw".to_owned(),
        other => describe_error(session, other),
    };
    Some(format!("Can't withdraw the site: {why}"))
}

/// The gate's building, if the data has one.
pub fn building(session: &Session) -> Option<BuildingId> {
    session.defs.endeavor.building
}

/// The gate's anchor (top-left tile) with the cursor on `cursor`: centred on it, as the Build
/// tool centres any footprint (`build::anchor_for`).
pub fn anchor_for(session: &Session, cursor: Pos) -> Option<Pos> {
    let gate = building(session)?;
    Some(build::anchor_for(cursor, session.defs.building(gate).footprint))
}

/// The cursor tile that puts the gate's anchor on `anchor`: the inverse of [`anchor_for`], for a
/// caller that names the anchor (the agent surface's `gate {x, y}`).
pub fn cursor_for(session: &Session, anchor: Pos) -> Option<Pos> {
    let gate = building(session)?;
    let (w, h) = session.defs.building(gate).footprint;
    Some(Pos::new(anchor.x + (i32::from(w) - 1) / 2, anchor.y + (i32::from(h) - 1) / 2))
}

/// The Gate tool's ghost with the cursor on `cursor`: the footprint, what blocks each tile, and
/// why placing it would be refused (`World::check_endeavor_site`; a refusal that isn't about a
/// tile, such as the endeavor not being Ready, reads as `BuildError::Endeavor`).
pub fn placement(session: &Session, cursor: Pos) -> Option<Placement> {
    let gate = building(session)?;
    let anchor = anchor_for(session, cursor)?;
    let mut p = build::placement_at(session, anchor, gate);
    p.refused = match session.world.check_endeavor_site(anchor, &session.defs) {
        Ok(()) => None,
        Err(EndeavorError::CannotBuild { at, why }) => Some(BuildError::CannotBuild { at, why }),
        Err(_) => Some(BuildError::Endeavor),
    };
    Some(p)
}

/// Why placing or withdrawing the gate was refused, for the notice: "an oak tree is in the way",
/// "5 of 9 signs met".
pub fn describe_error(session: &Session, e: EndeavorError) -> String {
    match e {
        EndeavorError::NotReady => match session.world.endeavor() {
            EndeavorState::Dormant => {
                let signs = session.world.endeavor_signs(&session.defs);
                let met = signs.iter().filter(|p| p.met).count();
                format!("{met} of {} signs met", signs.len())
            }
            EndeavorState::Building(_) => "the site is already placed".to_owned(),
            EndeavorState::Complete { .. } => "the gate already stands".to_owned(),
            EndeavorState::Ready { .. } => e.to_string(),
        },
        EndeavorError::CannotBuild { at, why } => build::describe_blocker(session, at, why),
        other => other.to_string(),
    }
}

fn name(session: &Session) -> &str {
    &session.defs.endeavor.name
}

/// Why a command came back refused, worded: only an endeavor refusal is expected here.
fn refusal(session: &Session, e: CommandError) -> String {
    match e {
        CommandError::Endeavor(e) => describe_error(session, e),
        other => other.to_string(),
    }
}

/// Places the gate with the cursor on `cursor` (`Command::BeginEndeavor`), centred as the ghost
/// shows it. The notice for either outcome: "The Harmony Gate: site placed", or "Can't place
/// the Harmony Gate: an oak tree is in the way".
pub fn place(session: &mut Session, cursor: Pos) -> Result<String, String> {
    let Some(anchor) = anchor_for(session, cursor) else {
        return Err(format!("Can't place {}: there is nothing to build", name(session)));
    };
    match session.apply(Command::BeginEndeavor { anchor }) {
        Ok(()) => Ok(format!("{}: site placed", name(session))),
        Err(e) => Err(format!("Can't place {}: {}", name(session), refusal(session, e))),
    }
}

/// Withdraws the gate's site (`Command::CancelEndeavor`) while no work on it has started:
/// "The Harmony Gate: site withdrawn", or "Can't withdraw the site: work has started".
pub fn withdraw(session: &mut Session) -> Result<String, String> {
    match session.apply(Command::CancelEndeavor) {
        Ok(()) => Ok(format!("{}: site withdrawn", name(session))),
        Err(CommandError::Endeavor(EndeavorError::NotBuilding)) => {
            Err("Can't withdraw the site: no site to withdraw".to_owned())
        }
        Err(e) => Err(format!("Can't withdraw the site: {}", refusal(session, e))),
    }
}

#[cfg(test)]
mod tests {
    use fn_sim::jobs::JobKind;
    use fn_sim::world::EndeavorState;
    use fn_sim::{Nation, World};

    use super::*;

    fn session() -> Session {
        Session::new(7, Nation::Earth, 64, 64)
    }

    /// `s` with the endeavor latched Ready, as if every sign had been met: a save round trip
    /// with the state spliced in (`World` serializes every field for saves), since meeting
    /// every sign takes a whole settlement's worth of play.
    pub(crate) fn ready(mut s: Session) -> Session {
        let text = s.world.save(&s.defs).unwrap();
        let text = text.replacen("Dormant", "Ready(tick:0)", 1);
        s.world = World::load(&text, &s.defs).expect("round trip");
        assert!(matches!(s.world.endeavor(), EndeavorState::Ready { .. }));
        s
    }

    /// The first cursor tile, scanning out from the settlement's first settler, where the gate
    /// fits.
    pub(crate) fn free_cursor(s: &Session) -> Pos {
        let home = s.world.units[0].pos;
        (1..30)
            .flat_map(|r| (-r..=r).flat_map(move |dy| (-r..=r).map(move |dx| Pos::new(dx, dy))))
            .map(|d| Pos::new(home.x + d.x, home.y + d.y))
            .find(|&c| placement(s, c).is_some_and(|p| p.ok()))
            .expect("somewhere near camp takes the gate")
    }

    #[test]
    fn before_ready_placing_is_refused_with_the_signs_met() {
        let mut s = session();
        assert_eq!(action(&s), None);
        let home = s.world.units[0].pos;
        let err = place(&mut s, home).unwrap_err();
        let total = s.defs.endeavor.signs.len();
        assert!(err.starts_with("Can't place The Harmony Gate: "), "{err}");
        assert!(err.ends_with(&format!(" of {total} signs met")), "{err}");
        assert_eq!(*s.world.endeavor(), EndeavorState::Dormant);
    }

    #[test]
    fn once_ready_the_screen_offers_a_site() {
        let s = ready(session());
        assert_eq!(action(&s), Some(GateAction::Place));
        assert_eq!(action(&s).unwrap().hint(), "Enter: choose a site for the gate");
    }

    /// The acceptance test of #643: a Ready settlement places the gate through this module, the
    /// endeavor is Building on the Foundation, and the settlers start on it.
    #[test]
    fn placing_the_gate_starts_the_foundation() {
        let mut s = ready(session());
        let cursor = free_cursor(&s);
        let notice = place(&mut s, cursor).expect("the ghost said it fits");
        assert_eq!(notice, "The Harmony Gate: site placed");
        let EndeavorState::Building(g) = s.world.endeavor() else {
            panic!("{:?}", s.world.endeavor())
        };
        assert_eq!(Some(g.anchor), anchor_for(&s, cursor));
        let (n, stage) = s.world.gate_stage(&s.defs).expect("a stage");
        assert_eq!((n, stage.id.as_str()), (0, "foundation"));
        assert_eq!(action(&s), Some(GateAction::Withdraw));
        // The Foundation is work for the settlers now: its construct job is open on the site,
        // where they haul its cost and then raise it (`fn_sim::world::gate`).
        let anchor = g.anchor;
        assert!(
            s.world.jobs().iter().any(|j| j.kind == JobKind::Build { pos: anchor }),
            "no construct job on the site: {:?}",
            s.world.jobs()
        );
    }

    #[test]
    fn a_blocked_site_is_refused_with_what_is_in_the_way() {
        let mut s = ready(session());
        // Somewhere the ghost says no for a tile reason.
        let (w, h) = (s.world.map.width(), s.world.map.height());
        let cursor = (0..h)
            .flat_map(|y| (0..w).map(move |x| Pos::new(x, y)))
            .find(|&c| {
                placement(&s, c)
                    .is_some_and(|p| matches!(p.refused, Some(BuildError::CannotBuild { .. })))
            })
            .unwrap();
        let err = place(&mut s, cursor).unwrap_err();
        assert!(err.starts_with("Can't place The Harmony Gate: "), "{err}");
        assert!(matches!(s.world.endeavor(), EndeavorState::Ready { .. }));
    }

    #[test]
    fn withdrawing_before_work_starts_makes_it_ready_again() {
        let mut s = ready(session());
        assert_eq!(
            withdraw(&mut s).unwrap_err(),
            "Can't withdraw the site: no site to withdraw".to_owned()
        );
        let cursor = free_cursor(&s);
        place(&mut s, cursor).unwrap();
        assert_eq!(withdraw(&mut s).unwrap(), "The Harmony Gate: site withdrawn");
        assert!(matches!(s.world.endeavor(), EndeavorState::Ready { .. }));
        assert_eq!(action(&s), Some(GateAction::Place));
    }

    #[test]
    fn cursor_for_inverts_anchor_for() {
        let s = session();
        let anchor = Pos::new(20, 30);
        let cursor = cursor_for(&s, anchor).unwrap();
        assert_eq!(anchor_for(&s, cursor), Some(anchor));
    }
}
