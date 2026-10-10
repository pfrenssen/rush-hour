# Rush Hour

A modern clone of the classic Amiga freeware game *Rush Hour*, written in Rust with
[Bevy](https://bevyengine.org) 0.20.

Cars, buses, trucks and ambulances pour into a small city from every road at the map edge and
head for an exit on another side. Every junction has traffic lights, and the lights are the only
thing you control. Keep the traffic moving: drivers lose patience while they wait, and when one
gives up, or an entry road stays blocked, you lose a life. Deliver enough vehicles to clear the
level; six levels grow from two crossings to fourteen.

Everything is drawn and synthesised procedurally; there are no asset files. Ambulances announce
themselves with a siren and impatient drivers honk.

## Running

```sh
cargo run --release
```

Rust stable (edition 2024) is required. On Linux, Bevy needs the usual system libraries
(`libudev`, `libxkbcommon`, ALSA, X11 or Wayland and Vulkan drivers); see Bevy's
[Linux dependencies](https://github.com/bevyengine/bevy/blob/main/docs/linux_dependencies.md).

## How to play

* **Click a junction** to switch its lights. The green direction gets 1.5 s of amber, then all
  lights are red for 0.5 s, then the other direction turns green. Clicks during a switch are
  ignored (the hover frame pulses amber). Lights never switch on their own.
* Drivers stop for amber and red when they can, left-turners wait for gaps in oncoming traffic
  (the first one waiting may clear the junction at amber), and nobody enters a junction without
  room on the far side.
* Waiting drivers turn red, show a honk bubble and honk (waiting ambulances sound their siren). If a driver's patience runs out, or a spawn point
  stays blocked for 8 s (watch for the red countdown at the map edge), you lose one of three
  lives.
* Faster trips score more; a steady flow of deliveries builds a combo up to x4. Ambulances are
  very impatient but pay a bonus when they get through quickly.

### Controls

| input              | action                                   |
|--------------------|------------------------------------------|
| left click         | switch the lights of a junction          |
| `P` / `Esc`        | pause / resume                           |
| `F` / `Tab`        | toggle 1x / 2x speed (also the HUD button) |
| `R`                | restart the level (paused, game over, level complete) |
| `M`                | back to the main menu (paused, game over, level complete) |
| `Enter`            | play / resume / next level / try again   |
| `1`–`6`            | start a level from the main menu         |

## Command-line flags

| flag                 | meaning |
|----------------------|---------|
| `--level N`          | start directly in level N (1-based) |
| `--seed X`           | seed for the traffic generator (default: random in the windowed game, 1 for headless and screenshots) |
| `--autopilot`        | let the built-in controller switch the lights (switches towards the longer queue) |
| `--headless`         | run the simulation without a window and print a report |
| `--seconds S`        | simulated seconds for `--headless` (default 120) |
| `--screenshot PATH`  | run for `--frames` frames, save a PNG screenshot and exit; captures the main menu unless `--level` is given |
| `--frames F`         | frames to simulate before the screenshot (default 600, at a fixed 60 Hz) |
| `-h`, `--help`       | show the help |

Examples:

```sh
# Ten simulated minutes of level 3 with the autopilot, no window
cargo run --release -- --headless --level 3 --seconds 600 --seed 7 --autopilot

# Screenshot of level 4 after 80 simulated seconds
cargo run --release -- --screenshot screenshots/level4.png --level 4 --frames 4800 --autopilot
```

A headless run prints spawned and delivered vehicles, lives lost, the longest wait of a single
vehicle, the number of overlapping vehicle pairs (always 0; the exit code is non-zero
otherwise), the score, and when the target was met or the last life lost. Reaching the target
or losing all lives does not stop a headless run.

## Development

```sh
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

The tests cover level parsing and validation, the lane graph and route finding, light
transitions, car-following, every traffic rule, an overlap stress test on all levels, scoring,
the state flow, CLI parsing and reproducible headless runs.

See [DESIGN.md](DESIGN.md) for the architecture, the traffic model and the gameplay decisions.
