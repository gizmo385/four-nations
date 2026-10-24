p = "crates/sim/src/world/bending.rs"
s = open(p).read()


def rep(old, new, count=1):
    global s
    assert old in s, old
    s = s.replace(old, new, count)


rep(
    """    pub pos: Pos,
    pub outcome: BendOutcome,
}
""",
    """    pub pos: Pos,
    pub outcome: BendOutcome,
    /// Wind Carry: the stack that was blown away and where it landed (#166), for the client to
    /// fly it there. `None` for every other effect, and for a bend that failed.
    #[serde(default)]
    pub moved: Option<Moved>,
}

/// A stack Wind Carry moved: `count` of `item` from the bent tile to the stockpile tile `to`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Moved {
    pub to: Pos,
    pub item: ItemId,
    pub count: u32,
}
""",
)
rep(
    """    pub remaining_secs: f32,
    /// Wind Carry only: the stockpile tile the stack would land on if the bend finished now.
    pub carry_to: Option<Pos>,
}""",
    """    pub remaining_secs: f32,
}""",
)
rep(
    """            let carry_to = self.carry_destination(def, pos, defs);
            out.push(BendChannel { unit: u.id, ability, pos, remaining_secs, carry_to });""",
    """            out.push(BendChannel { unit: u.id, ability, pos, remaining_secs });""",
)
# remove carry_destination
a = s.index("    /// Wind Carry's destination if it finished now")
b = s.index("    fn apply_effect(&mut self,")
s = s[:a] + s[b:]
rep(
    """        let outcome = match self.validate_bend(def, pos, defs) {
            Ok(()) => {
                self.apply_bend(i, def, pos, defs);
                BendOutcome::Done
            }""",
    """        let mut moved = None;
        let outcome = match self.validate_bend(def, pos, defs) {
            Ok(()) => {
                moved = self.apply_bend(i, def, pos, defs);
                BendOutcome::Done
            }""",
)
rep(
    "self.bend_events.push(BendEvent { tick: self.tick, unit, ability, pos, outcome });",
    "self.bend_events.push(BendEvent { tick: self.tick, unit, ability, pos, outcome, moved });",
)
rep(
    """    /// Spends the chi and takes effect. The target was just validated.
    fn apply_bend(&mut self, i: usize, def: &AbilityDef, pos: Pos, defs: &Defs) {
        let u = &mut self.units[i];
        u.chi = (u.chi - def.chi).max(0.0);
        let magnitude = self.bend_magnitude(def);
        self.apply_effect(&def.effect, pos, magnitude, defs);""",
    """    /// Spends the chi and takes effect. The target was just validated. Returns the stack Wind
    /// Carry moved, if that is what it was.
    fn apply_bend(&mut self, i: usize, def: &AbilityDef, pos: Pos, defs: &Defs) -> Option<Moved> {
        let u = &mut self.units[i];
        u.chi = (u.chi - def.chi).max(0.0);
        let magnitude = self.bend_magnitude(def);
        self.apply_effect(&def.effect, pos, magnitude, defs)""",
)
rep(
    "    fn apply_effect(&mut self, effect: &Effect, pos: Pos, magnitude: f32, defs: &Defs) {\n        match effect {",
    "    fn apply_effect(\n        &mut self,\n        effect: &Effect,\n        pos: Pos,\n        magnitude: f32,\n        defs: &Defs,\n    ) -> Option<Moved> {\n        match effect {",
)
rep(
    """                self.map_changed();
                self.wake_idle();
            }""",
    """                self.map_changed();
                self.wake_idle();
                None
            }""",
)
rep(
    """                let Some(stack) = self.ground.get(&pos).copied() else { return };
                let reach = scaled_range(*range, magnitude);
                let Some(dest) = self.stockpile_in_reach(pos, stack.item, reach, defs) else {
                    return;
                };""",
    """                let stack = self.ground.get(&pos).copied()?;
                let reach = scaled_range(*range, magnitude);
                let dest = self.stockpile_in_reach(pos, stack.item, reach, defs)?;""",
)
rep(
    """                } else {
                    self.ground.remove(&pos);
                }
            }""",
    """                } else {
                    self.ground.remove(&pos);
                }
                Some(Moved { to: dest, item: stack.item, count: put })
            }""",
)
rep(
    """                    until: self.tick + minutes_to_ticks(duration_minutes * magnitude),
                });
            }""",
    """                    until: self.tick + minutes_to_ticks(duration_minutes * magnitude),
                });
                None
            }""",
)
rep(
    """                self.patches.insert(at, patch);
            }
            #[cfg(test)]
            Effect::Probe => {}""",
    """                self.patches.insert(at, patch);
                None
            }
            #[cfg(test)]
            Effect::Probe => None,""",
)
open(p, "w").write(s)

p = "crates/sim/src/world.rs"
s = open(p).read()
rep(
    "pub use bending::{BendChannel, BendError, BendEvent, BendOutcome};",
    "pub use bending::{BendChannel, BendError, BendEvent, BendOutcome, Moved};",
)
open(p, "w").write(s)
