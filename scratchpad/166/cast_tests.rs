#[cfg(test)]
mod tests {
    use fn_sim::world::Command;
    use fn_sim::{Defs, Nation};
    use fn_ui::{Clock, Session};

    use super::*;
    use crate::unit_anim::Facing;

    const ALL: [Look; 4] = [Look::Rise, Look::Stream, Look::Flare, Look::Gust];

    fn scene() -> Scene {
        Scene {
            stand: Vec2::new(2.0, 5.0),
            target: Vec2::new(5.0, 5.0),
            moved_to: Some(Vec2::new(11.0, 5.0)),
        }
    }

    #[test]
    fn every_ability_has_a_cast_effect_and_the_table_names_only_abilities() {
        let defs = Defs::bundled();
        for ability in &defs.abilities {
            assert!(look_of(&ability.id).is_some(), "{} has no cast effect", ability.id);
        }
        for (id, _) in LOOKS {
            assert!(defs.ability_id(id).is_some(), "{id} is not an ability in data/bending.toml");
        }
        assert_eq!(LOOKS.len(), defs.abilities.len());
        // And the abilities look different from one another.
        let looks: BTreeSet<String> = LOOKS.iter().map(|(_, l)| format!("{l:?}")).collect();
        assert_eq!(looks.len(), LOOKS.len());
    }

    #[test]
    fn time_counts_down_while_channelling_then_up_from_the_event() {
        assert_eq!(time_of(None, 1.5, 100), -1.5);
        assert_eq!(time_of(Some(100), 0.0, 100), 0.0);
        assert_eq!(time_of(Some(100), 0.0, 105), 0.5);
        assert_eq!(time_of(Some(100), 0.0, 130), 3.0);
    }

    #[test]
    fn the_pose_is_the_stance_then_the_release_then_gone() {
        assert_eq!(bend_row(-10.0), Some(BEND_STANCE_ROW));
        assert_eq!(bend_row(-RELEASE_LEAD - 0.01), Some(BEND_STANCE_ROW));
        assert_eq!(bend_row(-RELEASE_LEAD), Some(BEND_RELEASE_ROW));
        assert_eq!(bend_row(0.0), Some(BEND_RELEASE_ROW));
        assert_eq!(bend_row(POSE_TAIL), Some(BEND_RELEASE_ROW));
        assert_eq!(bend_row(POSE_TAIL + 0.01), None);
    }

    #[test]
    fn an_effect_draws_inside_its_span_and_nothing_outside_it() {
        let s = scene();
        for look in ALL {
            let (lead, tail) = look.span(s.flight());
            assert!(parts(look, -lead - 0.05, &s).is_empty(), "{look:?} before");
            assert!(parts(look, tail + 0.05, &s).is_empty(), "{look:?} after");
            let busy = (0..=40)
                .map(|i| -lead + (lead + tail) * i as f32 / 40.0)
                .filter(|t| !parts(look, *t, &s).is_empty())
                .count();
            assert!(busy >= 20, "{look:?} draws at {busy} of 41 moments");
            // About 0.5 to 1.5 seconds, as the issue asks.
            assert!((0.5..=2.0).contains(&(lead + tail)), "{look:?} lasts {}", lead + tail);
        }
    }

    #[test]
    fn raise_earth_cracks_then_spikes_then_stones_then_dust() {
        let s = scene();
        let cell = |t| match parts(Look::Rise, t, &s)[..] {
            [Part { visual: Visual::Fx(c), at, .. }] => {
                assert_eq!(at, s.target, "on the target tile");
                c
            }
            ref p => panic!("{t}: {p:?}"),
        };
        assert_eq!([cell(-0.8), cell(-0.4), cell(-0.1)], sprites::fx::EARTH);
        assert_eq!(cell(0.0), sprites::fx::DUST[0]);
        assert_eq!(cell(0.4), sprites::fx::DUST[2]);
    }

    #[test]
    fn the_stream_runs_from_the_bender_to_the_target_and_splashes_the_patch() {
        let s = scene();
        let mid = parts(Look::Stream, -0.5, &s);
        let drops: Vec<&Part> = mid.iter().filter(|p| p.lift > 0.0).collect();
        assert!(drops.len() >= 3, "a stream of droplets: {}", drops.len());
        for d in &drops {
            assert!((s.stand.x..=s.target.x).contains(&d.at.x), "{:?} between them", d.at);
            assert_eq!(d.at.y, 5.0);
        }
        // It reaches out before it flows: fewer droplets at the start than later.
        assert!(parts(Look::Stream, -0.95, &s).len() < mid.len());
        // The splash starts on the target and spreads to the ring round it, over the 3x3 patch.
        let splashed = |t| {
            parts(Look::Stream, t, &s)
                .iter()
                .filter(|p| p.lift == 0.0)
                .map(|p| p.at)
                .collect::<Vec<_>>()
        };
        assert_eq!(splashed(-0.35), vec![s.target]);
        assert_eq!(splashed(-0.1).len(), 9);
        assert!(splashed(0.2).len() >= 8);
    }

    #[test]
    fn a_bender_on_its_target_sprays_the_ring_around_it() {
        let on = Scene { stand: Vec2::new(5.0, 5.0), ..scene() };
        let drops: Vec<Part> =
            parts(Look::Stream, -0.4, &on).into_iter().filter(|p| p.lift > 0.0).collect();
        assert!(drops.iter().any(|d| (d.at - on.target).length() > 0.3), "{drops:?}");
    }

    #[test]
    fn the_flame_is_kindled_in_a_flare_between_sparks_and_embers() {
        let s = scene();
        let cell = |t| match parts(Look::Flare, t, &s)[..] {
            [Part { visual: Visual::Fx(c), lift, .. }] => (c, lift),
            ref p => panic!("{t}: {p:?}"),
        };
        let fx = sprites::fx::SPARK;
        assert_eq!(cell(-0.4).0, fx[0]);
        assert_eq!(cell(0.0).0, fx[1]);
        assert_eq!(cell(0.4).0, fx[2]);
        assert!(cell(0.5).1 > cell(0.2).1, "embers rise");
    }

    #[test]
    fn the_wind_carry_stack_travels_to_the_stockpile_instead_of_teleporting() {
        let s = scene();
        let item = |t: f32| {
            parts(Look::Gust, t, &s).into_iter().find(|p| p.visual == Visual::Item).map(|p| p.at)
        };
        assert_eq!(item(-0.1), None, "it sits on the ground until the bend lands");
        assert_eq!(item(0.0), Some(s.target));
        let flight = s.flight();
        assert!((0.5..=1.5).contains(&flight));
        let xs: Vec<f32> =
            (0..=10).map(|i| item(flight * 0.99 * i as f32 / 10.0).unwrap().x).collect();
        assert!(xs.windows(2).all(|w| w[1] > w[0]), "it moves steadily along: {xs:?}");
        assert!((xs[10] - 11.0).abs() < 0.2, "and arrives: {xs:?}");
        assert_eq!(item(flight + 0.01), None, "then it is the stockpile's to draw");
        // Wind trails it, and runs from the bender to the stack first.
        assert!(parts(Look::Gust, flight * 0.5, &s).len() >= 3);
        assert!(!parts(Look::Gust, -0.5, &s).is_empty());
        // No stockpile to land on: no flight.
        let none = Scene { moved_to: None, ..s };
        assert!(parts(Look::Gust, 0.2, &none).iter().all(|p| p.visual != Visual::Item));
    }

    #[test]
    fn a_cast_follows_the_session_clock_pausing_and_running_faster() {
        // How many wall seconds a cast needs to get 0.5 s past completion, as the clock turns
        // wall time into world ticks (60 frames a second).
        let wall_secs = |speed_ups: u32, paused: bool| {
            let mut clock = Clock::default();
            for _ in 0..speed_ups {
                clock.faster();
            }
            clock.paused = paused;
            let (mut tick, mut frames) = (0_u64, 0_u32);
            while time_of(Some(0), 0.0, tick) < 0.5 && frames < 60 * 30 {
                tick += u64::from(clock.advance(1.0 / 60.0));
                frames += 1;
            }
            (frames as f32 / 60.0, time_of(Some(0), 0.0, tick))
        };
        for (ups, speed) in [(0, 1.0), (1, 2.0), (2, 3.0), (3, 5.0)] {
            let (secs, _) = wall_secs(ups, false);
            assert!((secs - 0.5 / speed).abs() < 0.05, "x{speed}: {secs}s");
        }
        let (secs, t) = wall_secs(0, true);
        assert_eq!((secs, t), (30.0, 0.0), "paused: the cast never moves");
    }

    // ---- advance ----

    fn unit(n: u32) -> UnitId {
        UnitId(n)
    }

    fn channel(n: u32, remaining: f32) -> BendChannel {
        BendChannel {
            unit: unit(n),
            ability: AbilityId(0),
            pos: Pos::new(5, 5),
            remaining_secs: remaining,
        }
    }

    fn event(n: u32, tick: u64, outcome: BendOutcome) -> BendEvent {
        BendEvent {
            tick,
            unit: unit(n),
            ability: AbilityId(0),
            pos: Pos::new(5, 5),
            outcome,
            moved: None,
        }
    }

    fn step(
        casts: &mut BTreeMap<UnitId, Cast>,
        channels: &[BendChannel],
        events: &[BendEvent],
        now: u64,
    ) -> Vec<UnitId> {
        advance(casts, channels, events, now, |_| Some(Pos::new(4, 5)), |_| Some(Look::Rise))
    }

    #[test]
    fn a_cast_starts_with_the_channel_counts_down_and_ends_with_its_event() {
        let mut casts = BTreeMap::new();
        step(&mut casts, &[channel(1, 2.0)], &[], 10);
        let c = casts[&unit(1)];
        assert_eq!((c.stand, c.target, c.finished), (Pos::new(4, 5), Pos::new(5, 5), None));
        assert_eq!(c.time(10), -2.0);
        step(&mut casts, &[channel(1, 0.4)], &[], 26);
        assert_eq!(casts[&unit(1)].time(26), -0.4);

        // The bend lands: the channel is gone and the event is there, on tick 27.
        let events = [event(1, 27, BendOutcome::Done)];
        step(&mut casts, &[], &events, 27);
        assert_eq!(casts[&unit(1)].finished, Some(27));
        assert_eq!(casts[&unit(1)].time(30), 0.3);

        // It plays out, and is then dropped; the old event doesn't start it again.
        assert!(step(&mut casts, &[], &events, 30).is_empty());
        let ended = step(&mut casts, &[], &events, 27 + 12);
        assert_eq!(ended, vec![unit(1)]);
        assert!(casts.is_empty());
        step(&mut casts, &[], &events, 27 + 15);
        assert!(casts.is_empty(), "no second life from the same event");
    }

    #[test]
    fn a_failed_bend_or_a_need_that_interrupts_ends_the_cast_at_once() {
        let mut casts = BTreeMap::new();
        step(&mut casts, &[channel(1, 1.0), channel(2, 1.0)], &[], 5);
        assert_eq!(casts.len(), 2);
        // Unit 2 left to eat: no channel any more, no event.
        let ended = step(&mut casts, &[channel(1, 0.9)], &[], 6);
        assert_eq!(ended, vec![unit(2)]);
        // Unit 1's bend failed.
        let fail = [event(1, 7, BendOutcome::Failed("gone"))];
        assert_eq!(step(&mut casts, &[], &fail, 7), vec![unit(1)]);
        assert!(casts.is_empty());
    }

    #[test]
    fn a_bend_seen_only_by_its_event_still_plays() {
        // A loaded game, or a channel shorter than a frame.
        let mut casts = BTreeMap::new();
        step(&mut casts, &[], &[event(3, 50, BendOutcome::Done)], 52);
        assert_eq!(casts[&unit(3)].time(52), 0.2);
        // But an old event does not.
        let mut casts = BTreeMap::new();
        step(&mut casts, &[], &[event(3, 50, BendOutcome::Done)], 50 + RECENT_TICKS);
        assert!(casts.is_empty());
    }

    #[test]
    fn the_events_stack_is_flown_to_where_it_landed() {
        let mut casts = BTreeMap::new();
        let moved = Moved { to: Pos::new(9, 5), item: fn_sim::jobs::ItemId(0), count: 4 };
        let events = [BendEvent { moved: Some(moved), ..event(1, 20, BendOutcome::Done) }];
        step(&mut casts, &[channel(1, 0.1)], &[], 19);
        step(&mut casts, &[], &events, 20);
        let scene = casts[&unit(1)].scene();
        assert_eq!(scene.moved_to, Some(Vec2::new(9.0, 5.0)));
    }

    // ---- in an app, on a real world ----

    fn app_with(nation: Nation) -> (App, UnitId) {
        let mut app = App::new();
        crate::insert_test_art(&mut app);
        let sim = Sim(Session::new(1, nation, 48, 32));
        let id = sim.world.units[0].id;
        let pos = sim.world.units[0].pos;
        let at = crate::unit_translation(pos);
        app.insert_resource(sim)
            .init_resource::<Casts>()
            .add_systems(Update, (track_casts, pose_casters, sync_parts).chain());
        app.world_mut().spawn((
            Sprite::default(),
            Transform::from_translation(at),
            UnitSprite(id),
            UnitView::new(pos, at.truncate()),
        ));
        (app, id)
    }

    fn view(app: &mut App) -> (Option<Pose>, Facing) {
        let mut q = app.world_mut().query::<&UnitView>();
        let v = q.single(app.world()).unwrap();
        (v.pose_override, v.facing)
    }

    fn parts_drawn(app: &mut App) -> usize {
        app.world_mut().query::<&CastPart>().iter(app.world()).count()
    }

    fn tick_until(app: &mut App, limit: u32, mut done: impl FnMut(&Sim) -> bool) {
        for _ in 0..limit {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            if done(&sim) {
                return;
            }
            sim.tick();
        }
        panic!("not reached in {limit} ticks");
    }

    fn cast_time(app: &App, id: UnitId) -> Option<f32> {
        let tick = app.world().resource::<Sim>().world.tick;
        app.world().resource::<Casts>().of(id).map(|c| c.time(tick))
    }

    #[test]
    fn a_bender_takes_the_stance_while_channelling_the_release_as_it_lands_then_goes_idle() {
        let (mut app, id) = app_with(Nation::Fire);
        let pos = {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            let ability = fn_ui::tools::ability_for_nation(&sim.defs, Nation::Fire).unwrap();
            let pos = sim.world.units[0].pos;
            sim.apply(Command::Bend { ability, pos }).unwrap();
            pos
        };
        app.update();
        assert_eq!(view(&mut app).0, None, "not yet: the unit is walking to the tile");

        tick_until(&mut app, 3000, |s| !s.world.bend_channels(&s.defs).is_empty());
        app.update();
        let row = |r| Some(Pose::Sheet { row: r });
        assert_eq!(view(&mut app).0, row(BEND_STANCE_ROW));
        assert_eq!(parts_drawn(&mut app), 0, "sparks only gather in the last half second");

        // Half a second before it lands, the sparks gather and the release begins.
        tick_until(&mut app, 3000, |s| {
            s.world.bend_channels(&s.defs).first().is_some_and(|c| c.remaining_secs <= 0.4)
        });
        app.update();
        assert_eq!(view(&mut app).0, row(BEND_RELEASE_ROW));
        assert_eq!(parts_drawn(&mut app), 1);

        // It lands: still the release, and the flame is lit.
        tick_until(&mut app, 3000, |s| !s.world.bend_events().is_empty());
        app.update();
        assert_eq!(view(&mut app).0, row(BEND_RELEASE_ROW));
        assert!(app.world().resource::<Sim>().world.is_lit(pos));
        assert_eq!(parts_drawn(&mut app), 1);

        // Held still while the world doesn't tick: no frame changes however long it waits.
        let before = cast_time(&app, id);
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(cast_time(&app, id), before);
        assert!(before.is_some());

        // A second later it is over: the pose and the effect are gone.
        app.world_mut().resource_mut::<Sim>().tick_n(11);
        app.update();
        assert_eq!(view(&mut app).0, None);
        assert!(cast_time(&app, id).is_none());
        app.update();
        assert_eq!(parts_drawn(&mut app), 0);
    }

    #[test]
    fn the_bender_turns_to_face_its_target() {
        let (mut app, _) = app_with(Nation::Fire);
        let target = {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            let ability = fn_ui::tools::ability_for_nation(&sim.defs, Nation::Fire).unwrap();
            let pos = sim.world.units[0].pos;
            // A few tiles to the right of where it stands, whichever of them takes the bend.
            let target = (2..6)
                .map(|dx| Pos::new(pos.x + dx, pos.y))
                .find(|&p| sim.apply(Command::Bend { ability, pos: p }).is_ok())
                .expect("a tile to the right takes a flame");
            target
        };
        tick_until(&mut app, 3000, |s| !s.world.bend_channels(&s.defs).is_empty());
        app.update();
        let sim = app.world().resource::<Sim>();
        let from = sim.world.units[0].pos;
        let want = facing_after(Facing::Down, target.x - from.x, target.y - from.y);
        assert_eq!(view(&mut app).1, want);
    }
}
