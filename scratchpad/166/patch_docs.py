def edit(p, pairs):
    s = open(p).read()
    for a, b in pairs:
        assert a in s, a
        s = s.replace(a, b, 1)
    open(p, "w").write(s)

edit("docs/design/pixel-art.md", [(
"""units stay in scale. Each character is one 64x64 sheet: the columns face down, up, left and
right, and the rows are 4 walk frames, with row 0 the idle pose. Idle-down is (0,0). The client
(`unit_anim.rs`, #165) picks the column from the way the unit last stepped and the row from how
far it has walked: row 0 standing, rows 0-3 in turn while walking. Clothes use palette colours chosen per nation, not the UI accent. Portraits
are 32x32 (head and shoulders, facing three-quarters), drawn at 2x (64px) in the 96px portrait
well.
""",
"""units stay in scale. Each character is one 64x96 sheet: the columns face down, up, left and
right, and rows 0-3 are the 4 walk frames, with row 0 the idle pose. Idle-down is (0,0). The client
(`unit_anim.rs`, #165) picks the column from the way the unit last stepped and the row from how
far it has walked: row 0 standing, rows 0-3 in turn while walking. Clothes use palette colours
chosen per nation, not the UI accent. Portraits are 32x32 (head and shoulders, facing
three-quarters), drawn at 2x (64px) in the 96px portrait well.

### Bend poses (#166)

Rows 4 and 5 of every character sheet are the **bend pose**, one cell per facing like the walk:
row 4 is the *stance* held while the unit channels a bend, row 5 the *release* as it lands (from
half a second before to 0.6 s after). `cast.rs` sets `UnitView::pose_override` to
`Pose::Sheet { row }` and turns the unit toward its target; rows 0-3 and every walk index are
unchanged.

*Why appended rows, not separate bend sheets:* a unit sprite is one image handle plus a rect, so
extra rows cost nothing in the client (no second image to load or swap per unit, no second
manifest entry and path table), the pose is drawn from the same clothes and palette as the walk
by construction (the generator below derives it from the character's own idle cells), and the
names stay uniform (`<facing>_<row>`, tests read them). The cost is that the sheet is 64x96, so
the sheet-size tests changed deliberately.

Each nation's stance reads as its bending style, in every facing and for all three of its
characters:

- **Earth** is low and grounded: a squat with the feet planted wide. The stance has both fists
  planted down at the sides, the release lifts them (raising the earth).
- **Fire** is a lunge into a punch: the stance guards with one fist raised, the release throws the
  other arm out (toward the viewer from the front, along the facing from the side, up and out
  from behind).
- **Water** is flowing arms: the stance holds the arms in a wave (one high, one low), the release
  sweeps both into a lean.
- **Air** is light and sweeping: the body lifts a pixel on stretched legs, one arm raised overhead
  in the stance, both arms swept wide in the release.

The right-facing cells mirror the left-facing ones, as in the walk. `scratchpad/166/gen_bend.py`
drew them from each character's idle cells and wrote them into the TOML as plain grids; the TOML
is the source of truth and can be edited by hand.

### Cast effects (#166)

The effects of the bend itself are on `art/fx.toml` (rows 2-8, three cells wide, below #154's
flame and wet rows) and played by `crates/client/src/cast.rs`: Raise Earth cracks, rock spikes
and stones with settling dust; Water the Land a stream of droplets from the bender to the target
and splashes over the patch; Hearthflame sparks gathering, a flare and rising embers; Wind Carry
curls of wind to the stack, a whirl, and the stack flying to the stockpile. Each effect's colours
are in its frames (stone, water, flame, wind), so a nation's look comes from the art, picked per
ability id. `FN_CAST_DEMO=T` stages a cast and pauses `T` seconds from completion for shots.
"""),
("| #154 | Item stacks and bending effects (after #130) | `jobview.rs`, `bending.rs` | none (placeholders today) |",
 "| #154 | Item stacks and bending effects (after #130) | `jobview.rs`, `bending.rs` | none (placeholders today) |\n| #166 | Bend poses (rows 4-5 of the unit sheets) and cast effects (`fx.toml` rows 2-8) | `cast.rs`, `unit_anim.rs`, `sprites/fx.rs` | none |"),
])

edit("art/README.md", [(
"""## Facesets: `art/faces_<nation>.toml`""",
"""## Character sheets: `art/<character>.toml`

One 64x96 sheet per character (`cell = [16, 16]`, `columns = 4`): the columns face down, up, left
and right, and sprite `n` sits at row `n / 4`. Rows 0-3 are the idle pose and the walk cycle
(#150/#151, `unit_anim.rs`); **rows 4 and 5 are the bend pose** (#166), the stance held while
channelling and the release as the bend lands, named `down_4`, `up_4`, `left_4`, `right_4`,
`down_5`, ... like the rest. They are rows appended to the same sheet rather than separate bend
sheets, so a unit stays one image (docs/design/pixel-art.md, "Bend poses"). Rows 0-3 never change
when a pose changes. Every character has all eight bend cells, the right-facing ones mirroring
the left-facing ones, and `sprites/characters.rs` tests it.

## Effects: `art/fx.toml`

Three columns of 16px cells; rows 0-1 are #154's lasting flame and wet marks, rows 2-8 #166's
cast animations (`earth`, `dust`, `drop`, `splash`, `spark`, `gust`, `swirl`, three frames each).
`sprites/fx.rs` names every cell and a test checks the names; `cast.rs` says when each plays.
Cast frames are centred on the cell so a particle can sit anywhere along a path.

## Facesets: `art/faces_<nation>.toml`"""),
])

edit("art/fx.toml", [(
"# Lasting bending effects on the map (crates/client/src/bending.rs), 16px, drawn at 1:1 over the\n# floor. Three columns, one family per row; #166's cast animations add rows below.",
"# Bending effects on the map, 16px, drawn at 1:1 over the floor. Three columns, one family per row.\n# Rows 0-1 are the lasting effects (crates/client/src/bending.rs); rows 2-8 are #166's cast\n# animations (crates/client/src/cast.rs), described below the lasting ones.")])
