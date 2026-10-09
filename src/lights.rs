//! Traffic lights.
//!
//! Every junction with three or more approaches has a two-phase signal: north-south green or
//! east-west green. Switching goes through 1.5 s of amber and 0.5 s of all-red before the other
//! axis turns green. Lights never switch by themselves; requests during a transition are ignored.

use bevy::prelude::*;

use crate::map::CityMap;
use crate::map::graph::NodeId;
use crate::map::grid::Axis;
use crate::states::{AppState, LevelLoad};

pub const AMBER_TIME: f32 = 1.5;
pub const ALL_RED_TIME: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Green(Axis),
    Amber { from: Axis, elapsed: f32 },
    AllRed { to: Axis, elapsed: f32 },
}

/// What a driver on a given axis sees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aspect {
    Green,
    Amber,
    Red,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Signal {
    pub phase: Phase,
    /// Seconds since the current axis turned green (0 while in transition).
    pub green_for: f32,
}

impl Signal {
    pub fn new(green: Axis) -> Signal {
        Signal {
            phase: Phase::Green(green),
            green_for: 0.0,
        }
    }

    /// Start switching to the other axis. Returns false (and does nothing) mid-transition.
    pub fn request_switch(&mut self) -> bool {
        match self.phase {
            Phase::Green(axis) => {
                self.phase = Phase::Amber {
                    from: axis,
                    elapsed: 0.0,
                };
                self.green_for = 0.0;
                true
            }
            Phase::Amber { .. } | Phase::AllRed { .. } => false,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        match &mut self.phase {
            Phase::Green(_) => self.green_for += dt,
            Phase::Amber { from, elapsed } => {
                *elapsed += dt;
                if *elapsed >= AMBER_TIME {
                    self.phase = Phase::AllRed {
                        to: from.other(),
                        elapsed: *elapsed - AMBER_TIME,
                    };
                    self.tick(0.0);
                }
            }
            Phase::AllRed { to, elapsed } => {
                *elapsed += dt;
                if *elapsed >= ALL_RED_TIME {
                    let over = *elapsed - ALL_RED_TIME;
                    self.phase = Phase::Green(*to);
                    self.green_for = over;
                }
            }
        }
    }

    pub fn aspect(&self, axis: Axis) -> Aspect {
        match self.phase {
            Phase::Green(g) if g == axis => Aspect::Green,
            Phase::Amber { from, .. } if from == axis => Aspect::Amber,
            _ => Aspect::Red,
        }
    }

    /// Progress through a switch, 0..1, or `None` when settled on green.
    pub fn transition_progress(&self) -> Option<f32> {
        let total = AMBER_TIME + ALL_RED_TIME;
        match self.phase {
            Phase::Green(_) => None,
            Phase::Amber { elapsed, .. } => Some(elapsed / total),
            Phase::AllRed { elapsed, .. } => Some((AMBER_TIME + elapsed) / total),
        }
    }
}

/// Signal state per graph node (`None` for unsignalled bends).
#[derive(Resource, Default, Clone, Debug)]
pub struct Signals(pub Vec<Option<Signal>>);

impl Signals {
    pub fn for_graph(graph: &crate::map::graph::LaneGraph) -> Signals {
        Signals(
            graph
                .nodes
                .iter()
                .map(|n| n.is_signal().then(|| Signal::new(Axis::EastWest)))
                .collect(),
        )
    }
}

/// Ask a junction to switch its lights (from a click or the autopilot).
#[derive(Message, Clone, Copy, Debug)]
pub struct SwitchRequest(pub NodeId);

pub struct LightsPlugin;

impl Plugin for LightsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Signals>()
            .add_message::<SwitchRequest>()
            .add_systems(
                OnEnter(AppState::InGame),
                init_signals.in_set(LevelLoad::Sim),
            )
            .add_systems(
                FixedUpdate,
                (apply_requests, tick_signals)
                    .chain()
                    .in_set(crate::sim::SimSet::Lights),
            );
    }
}

fn init_signals(mut commands: Commands, map: Res<CityMap>) {
    commands.insert_resource(Signals::for_graph(&map.graph));
}

fn apply_requests(mut requests: MessageReader<SwitchRequest>, mut signals: ResMut<Signals>) {
    for &SwitchRequest(node) in requests.read() {
        if let Some(Some(signal)) = signals.0.get_mut(node) {
            signal.request_switch();
        }
    }
}

fn tick_signals(time: Res<Time>, mut signals: ResMut<Signals>) {
    let dt = time.delta_secs();
    for signal in signals.0.iter_mut().flatten() {
        signal.tick(dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(signal: &mut Signal, seconds: f32) {
        let dt = 1.0 / 60.0;
        let steps = (seconds / dt).round() as usize;
        for _ in 0..steps {
            signal.tick(dt);
        }
    }

    #[test]
    fn starts_green_for_one_axis() {
        let s = Signal::new(Axis::EastWest);
        assert_eq!(s.aspect(Axis::EastWest), Aspect::Green);
        assert_eq!(s.aspect(Axis::NorthSouth), Aspect::Red);
    }

    #[test]
    fn never_switches_on_its_own() {
        let mut s = Signal::new(Axis::NorthSouth);
        run(&mut s, 600.0);
        assert_eq!(s.phase, Phase::Green(Axis::NorthSouth));
        assert!(s.green_for > 599.0);
    }

    #[test]
    fn switch_goes_amber_then_all_red_then_green() {
        let mut s = Signal::new(Axis::EastWest);
        assert!(s.request_switch());
        assert_eq!(s.aspect(Axis::EastWest), Aspect::Amber);
        assert_eq!(s.aspect(Axis::NorthSouth), Aspect::Red);

        run(&mut s, 1.4);
        assert_eq!(s.aspect(Axis::EastWest), Aspect::Amber);

        run(&mut s, 0.2); // 1.6 s: amber over, all red
        assert!(matches!(s.phase, Phase::AllRed { .. }));
        assert_eq!(s.aspect(Axis::EastWest), Aspect::Red);
        assert_eq!(s.aspect(Axis::NorthSouth), Aspect::Red);

        run(&mut s, 0.3); // 1.9 s: still all red
        assert_eq!(s.aspect(Axis::NorthSouth), Aspect::Red);

        run(&mut s, 0.2); // 2.1 s: the other axis is green
        assert_eq!(s.phase, Phase::Green(Axis::NorthSouth));
        assert_eq!(s.aspect(Axis::EastWest), Aspect::Red);
        assert!(s.green_for < 0.2);
    }

    #[test]
    fn clicks_during_a_transition_are_ignored() {
        let mut s = Signal::new(Axis::EastWest);
        assert!(s.request_switch());
        run(&mut s, 0.5);
        assert!(!s.request_switch());
        run(&mut s, 1.2); // all red
        assert!(!s.request_switch());
        run(&mut s, 0.4); // green north-south
        assert_eq!(s.phase, Phase::Green(Axis::NorthSouth));
        // Settled again: a new request is accepted and switches back.
        assert!(s.request_switch());
        run(&mut s, 2.1);
        assert_eq!(s.phase, Phase::Green(Axis::EastWest));
    }

    #[test]
    fn large_steps_carry_over() {
        let mut s = Signal::new(Axis::EastWest);
        s.request_switch();
        s.tick(5.0);
        assert_eq!(s.phase, Phase::Green(Axis::NorthSouth));
        assert!((s.green_for - 3.0).abs() < 1e-4);
    }

    #[test]
    fn transition_progress_runs_from_zero_to_one() {
        let mut s = Signal::new(Axis::EastWest);
        assert_eq!(s.transition_progress(), None);
        s.request_switch();
        assert_eq!(s.transition_progress(), Some(0.0));
        s.tick(1.0);
        assert!((s.transition_progress().unwrap() - 0.5).abs() < 1e-4);
    }
}
