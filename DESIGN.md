# Rush Hour — design notes

A modern take on the Amiga freeware game *Rush Hour*: a top-down city of two-way streets where
the only thing the player controls is the traffic lights. This document records how the game is
built and every decision taken where the brief left room for interpretation. The guiding rule
for those decisions was: *pick what gives the most fun, readable gameplay.*

## Project setup

The brief asked for a new Cargo project in `./rush-hour`. The working directory was already an
empty, freshly initialised `rush-hour` repository with a matching `Cargo.toml`, so the game lives
there rather than in a nested `rush-hour/rush-hour`. `.gitignore` covers `target/` and
screenshot output (`screenshots/`, `*.png`).

## Architecture

```
src/
  main.rs        CLI dispatch: windowed game, --headless, --screenshot
  cli.rs         std::env::args parsing
  states.rs      AppState (Menu / InGame), GamePhase sub-state, Action messages
  levels.rs      the six hand-made ASCII levels and their parameters
  map/           grid parsing, lane graph, geometry, route finding   (plain Rust)
  lights.rs      signal state machine (plain Rust) + LightsPlugin
  vehicles/      kinds, spawning, driving model (plain Rust) + VehiclesPlugin
  scoring.rs     points, combo, lives (plain Rust) + ScoringPlugin
  autopilot.rs   built-in light controller
  sim.rs         SimPlugin: fixed timestep, system sets, seeded RNG, speed
  input.rs       mouse picking of junctions, keyboard shortcuts
  render/        camera, map meshes, lights, vehicles, effects
  ui/            HUD, menu, overlays
  headless.rs    --headless runner and report
  screenshot.rs  --screenshot runner
```

* **Simulation and presentation are separate plugins.** `SimPlugin` (map, lights, vehicles,
  scoring) plus `AutopilotPlugin` and `GameStatesPlugin` only need `MinimalPlugins` and
  `StatesPlugin`; the headless mode runs exactly that. Input, rendering and UI are added only for
  the windowed game.
* **The core logic is plain Rust.** Grid parsing, the lane graph, routing, the signal state
  machine, the driving model, spawning and scoring have no `App` dependency and are unit-tested
  directly. ECS systems are thin: collect, sort by id, call the core, write back.
* **Simulation runs in `FixedUpdate` at 60 Hz** in the ordered sets
  `Control → Lights → Spawn → Move → Score`, only while `GamePhase::Running`. All randomness
  comes from one seeded `fastrand::Rng`; vehicles are processed in id order, so a seed fully
  determines a headless run (there is a test for that).
* **Vehicles are entities** carrying the plain `Vehicle` struct as a component. Rendering adds
  a transform and child meshes when a vehicle appears, so headless runs never pay for visuals.
* Bevy is used with only its `2d` and `ui` feature sets. The single extra dependency is
  `fastrand`.

## World and map

* One ASCII cell is an 8 m tile. Roads fill their tile: two 4 m lanes, right-hand traffic.
  Legend: `.` city block, `T` park, `~` water, `-` east-west road, `|` north-south road,
  `+` junction. Roads crossing water are bridges.
* The parser rejects ragged rows, unknown characters, dead ends and touching junctions, so every
  level is guaranteed to produce a sane lane graph (tested for all levels).
* A `+` with 3–4 approaches is a **signalled junction**. A `+` with two perpendicular approaches
  is an unsignalled **bend**; with two opposite approaches it is just road.
* A `-`/`|` on the border pointing off the map is a **terminal**: an entry lane and an exit lane.
  Both lanes reach one tile beyond the map edge, where the road fades into the frame, so
  vehicles grow into view and shrink away instead of popping.

### Lane graph

* Each road between two graph nodes carries two directed road lanes. Inside a junction box,
  turn lanes connect every incoming lane to every outgoing lane except the U-turn: straight
  lines, right turns as 2 m-radius arcs, left turns as 6 m-radius arcs. Turn speed limits follow
  from a 4.5 m/s² lateral acceleration (≈3 m/s right, ≈5 m/s left).
* Two turn lanes of the same junction **conflict** when their centre lines come within 2.9 m
  (widest vehicle plus margin). This is computed from geometry, so crossing straights, left turns
  against oncoming traffic and merges into the same exit are all caught without hand-written
  tables; opposing straights and opposing right turns run side by side.
* Vehicles stop 2.5 m before a signalled junction box, which leaves room for zebra crossings.

### Routing

Vehicles spawn at a random free entry and drive to a random exit on **another side** of the map,
along the shortest route. Route cost is road length, but every turn lane costs one box length
plus a turn penalty (left 6 m, right 3 m). *Decision:* in a grid many routes are equally long,
and raw arc lengths would make right turns look like shortcuts, producing zig-zagging traffic;
the penalty makes drivers prefer the route with the fewest turns, which looks natural and keeps
flows readable.

## Driving model

* **Car following** is the Intelligent Driver Model (desired speed, 1.1 s headway, 2 m jam gap,
  per-type acceleration and comfortable braking), with the desired speed lowered in time for slow
  turns ahead. On top of IDM, every step clamps the distance a vehicle may move so its front
  never comes closer than 0.5 m to the body ahead (measured with the leader's *previous*
  position, which is conservative). Overlap along a lane is therefore impossible for any time
  step.
* **Junction reservations.** Before crossing its stop line a vehicle must *commit* to its turn
  lane. Committing is only possible while no conflicting turn lane is reserved or occupied, and a
  vehicle keeps its claim until its tail has left the box. An uncommitted vehicle treats the stop
  line as a wall. Decisions are taken one vehicle at a time, so claims are mutually exclusive:
  collisions inside junctions are impossible by construction, independently of the lights.
* Vehicles decide when they are within braking distance plus a margin of the stop line, and only
  the first vehicle waiting at a line may commit.
* Near the box entrance, vehicles on *sibling* turn lanes (same entry lane) still count as being
  ahead, because the paths have not diverged enough for two vehicles to pass side by side.

### Traffic rules

* **Lights.** Commit on green. On amber, go only if stopping would need more than 3.4 m/s² of
  braking (otherwise stop). Never commit on red. A vehicle that committed early backs out when the
  light changes and stopping is still comfortable.
* **Left turns yield** to oncoming straight and right-turning vehicles that would reach the
  junction within 4 s.
* *Decision — clearing on amber:* with only two phases and no protected left turn, a steady
  oncoming stream would starve left-turners forever. As real drivers do, the first left-turner
  waiting at the line reserves its turn when the light goes amber (blocking cross traffic from
  starting) and goes as soon as the last oncoming vehicles have passed. This keeps every switch
  meaningful for left-turners too.
* **Don't block the box.** A vehicle only enters when its exit lane has room for its full length
  plus jam gap, counting every vehicle on that lane, in the box before it, or committed to it.
  Lanes that leave the map never fill up.

The rule set is covered by unit tests (stop at red, amber stop-or-go, yielding, clearing on
amber, don't-block-the-box) and by a stress test that runs every level with random light
switching and asserts that no two vehicle bodies (two oriented boxes each) ever overlap.

## Lights

* Two phases per junction: north–south green or east–west green. All junctions start east–west
  green, so the player has to act early.
* Clicking a junction (anywhere within 4 m of its box) starts a switch: 1.5 s amber, 0.5 s
  all-red, then the other axis turns green. Clicks during a switch are ignored; the hover frame
  turns amber and pulses to show it. Lights never switch on their own.
* T-junctions are signalled like crossroads (their stem simply has its own phase).

## Vehicles

| type      | length | speed    | accel    | patience | points |
|-----------|--------|----------|----------|----------|--------|
| car       | 4.4 m  | 12.5 m/s | 2.6 m/s² | 30 s     | 100    |
| truck     | 7.6 m  | 10.0 m/s | 1.5 m/s² | 34 s     | 150    |
| bus       | 9.5 m  | 9.5 m/s  | 1.4 m/s² | 34 s     | 200    |
| ambulance | 5.4 m  | 14.0 m/s | 3.0 m/s² | 12 s     | 150    |

Each vehicle also gets ±8 % personal speed variation and a random paint colour.

## Patience, lives and scoring

* **Patience** drains while a vehicle stands still (below 0.5 m/s) over the type's patience time,
  and *recovers at a quarter of that rate while driving*. *Decision:* without recovery, a car that
  waits at three reasonable red lights would eventually explode, punishing long routes rather
  than bad play. The body tints towards red as patience drops and a honk bubble appears below
  55 %, pulsing below 25 %. Ambulances drain 2.5× faster.
* When patience runs out the player loses a life and that driver calms down (patience resets).
  *Decision:* keeping the vehicle avoids teleporting cars out of a jam, which would hide the
  player's mistake; the reset avoids losing several lives to the same driver.
* A spawn point counts as blocked while a vehicle waits to enter there; after 8 s the player loses
  a life and that vehicle gives up. A countdown marker appears at the entry after 1 s.
* *Decision — grace period:* after losing a life, further losses are forgiven for 4 s (the lives
  in the HUD flash), so one jam cannot take all three lives within a second.
* **Points** per delivery: the type's points × (0.25 + 0.75 × speed factor), rounded to 5. The
  speed factor compares the trip with an empty-road trip (≥ 80 % of free-flow speed counts as
  full speed). Ambulances that keep a speed factor of 0.7 earn a 250-point "FAST!" bonus.
* **Combo:** each delivery within 6 s of the previous one extends the chain; the multiplier is
  ×2 from 5, ×3 from 12 and ×4 from 20 deliveries. Losing a life breaks the chain.
* Lives reset to three at each level; the score carries over. A retry restarts the level with
  the score it started with.

## Levels

| # | name         | size  | signals | target | spawns/min (start → end) |
|---|--------------|-------|---------|--------|--------------------------|
| 1 | Main Street  | 24×11 | 2       | 25     | 14 → 26                  |
| 2 | Twin Avenues | 28×14 | 4       | 35     | 20 → 34                  |
| 3 | Old Town     | 32×16 | 6       | 45     | 26 → 42                  |
| 4 | Riverside    | 34×17 | 9       | 55     | 30 → 48                  |
| 5 | Uptown       | 38×19 | 11      | 65     | 34 → 54                  |
| 6 | Rush Hour    | 40×21 | 14      | 80     | 38 → 60                  |

The vehicle mix shifts from 88 % cars to more buses, trucks and ambulances. Later levels add
T-junctions, a bend and a river with bridges.

* *Decision:* the spawn rate ramps with the fraction of the target delivered, not with time, so
  the pressure rises exactly as the player makes progress ("rush hour builds up") and a slow start
  is not punished twice.
* Spawn intervals are jittered (0.6–1.4× the mean) for an organic rhythm.

## States and flow

`AppState::Menu | InGame`, with the `GamePhase` sub-state `Running | Paused | LevelComplete |
GameOver` that only exists in game, so pausing or finishing never tears the level down. Buttons
and keys both send one `Action` message (`Play(n)`, `Next`, `Restart`, `Pause`, `Resume`,
`Menu`) handled in one system. Re-entering `InGame` rebuilds the level through `DespawnOnExit`
and ordered `OnEnter` sets (`Map → Sim → Visuals`).

All levels are unlocked in the menu: *decision*, since the game has no save data, forcing a
replay of earlier levels to reach a later one would only add friction.

## Presentation

* The map is a handful of vertex-coloured meshes (ground, markings, shadows, buildings) built
  from the grid, with deterministic per-cell variation.
* *Decision — perimeter blocks:* houses only stand on block cells that touch a road and face
  that road (row houses along a street, a corner house where two streets meet, back-to-back
  houses in a block one cell deep). Block cells that touch no road are the inside of the block:
  each connected inside area is either a garden (lawn, flower beds, bushes) or a small park
  (trees), and most get a playground (slide, swings, sandbox or roundabout). Houses in the middle
  of a block, with no street to reach them, looked wrong.
* The camera uses `Hdr` and a bloom pass with a threshold of 1.0: only HDR colours (lit lamps,
  brake lights, ambulance beacons, indicators) glow, the flat map stays crisp.
* Light heads sit on the corner to the right of each approach, red lamp nearest the junction.
* Vehicles: rounded body, glass and roof (buses get a window band, trucks a cargo box,
  ambulances a red cross and blue/red beacons), headlights, brake lights that glow while
  braking or stopped, indicators that blink from 30 m before a turn until it is done.
* The camera fits the whole map below the 60 px HUD and refits on every resize.
* Feedback: score pop-ups at exits, a red flash and "-1 LIFE" when a life is lost, flashing life
  dots during the grace period, countdown markers at blocked entries, and a title card per level.

## Headless mode, autopilot and screenshots

* `--headless` builds the simulation-only app, advances exactly one fixed tick per update with
  `TimeUpdateStrategy::FixedTimesteps(1)` and prints a report. Level completion and game over do
  not stop the run, so it doubles as a long stress test; the exit code is non-zero if vehicles
  ever overlapped.
* The autopilot switches each junction towards its longer queue: vehicles within 45 m of the
  stop line count, weighted by impatience, with a 5 s minimum and 30 s maximum green.
* `--screenshot` uses a fixed 1280×720, scale-factor-1, non-resizable window (so tiling window
  managers leave it alone) and a manual 1/60 s clock. Because shaders compile in the background,
  it first holds time still for a short warm-up, then simulates the requested frames.

## Verification results

`--headless --seconds 120 --seed 1 --autopilot`:

| level | spawned | delivered | lives lost | max wait | overlaps | target met |
|-------|---------|-----------|------------|----------|----------|------------|
| 1     | 37      | 29        | 0          | 12.6 s   | 0        | 101.5 s    |
| 2     | 50      | 35        | 0          | 14.1 s   | 0        | 116.6 s    |
| 3     | 66      | 46        | 0          | 10.5 s   | 0        | 116.5 s    |
| 4     | 70      | 48        | 0          | 13.8 s   | 0        | —          |
| 5     | 79      | 48        | 0          | 27.0 s   | 0        | —          |
| 6     | 86      | 52        | 0          | 14.0 s   | 0        | —          |

In 600 s runs over four seeds the autopilot met every level's target (after 100–170 s) before
losing its last life, with zero overlaps; without the autopilot a level is lost in under a
minute, so the player's switching clearly matters.

## Known limitations

* Vehicles never change their route once spawned, even when a junction ahead is jammed.
* There is no protected left-turn phase; left turns rely on gaps and on clearing at amber.
* Ambulances do not get priority from other traffic, only from the player.
* No sound and no persistent high scores.
* The default Bevy font has a limited glyph set, so the UI sticks to plain ASCII.
