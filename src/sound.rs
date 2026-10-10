//! Sound effects, synthesised at startup (no audio files).
//!
//! An ambulance entering the city announces itself with a short two-tone siren. A driver who
//! starts to lose patience honks (and honks again when nearly fed up); waiting ambulances sound
//! their siren instead. A limiter keeps a jam from turning into a wall of noise.

use std::f32::consts::TAU;
use std::sync::Arc;
use std::time::Duration;

use bevy::audio::{AddAudioSource, ChannelCount, SampleRate, Source, Volume};
use bevy::prelude::*;

use crate::states::GamePhase;
use crate::vehicles::kind::VehicleKind;
use crate::vehicles::traffic::Vehicle;

pub const SAMPLE_RATE: u32 = 44_100;
/// Patience below which a driver honks (the same point where the honk bubble appears).
pub const HONK_AT: f32 = 0.55;
/// Patience below which a driver honks once more.
pub const HONK_AGAIN_AT: f32 = 0.25;
/// At most one horn per this many seconds, and only a few at the same time.
const HORN_GAP: f32 = 0.3;
const MAX_HORNS: usize = 3;
const SIREN_GAP: f32 = 1.5;

// ---- Synthesis --------------------------------------------------------------------------

/// A tone rich in harmonics (horns are buzzy), `harmonics` partials with 1/k amplitudes.
fn buzzy(freq: f32, t: f32, harmonics: u32) -> f32 {
    (1..=harmonics)
        .map(|k| (TAU * freq * k as f32 * t).sin() / k as f32)
        .sum()
}

/// Linear attack and release envelope for a note of `len` seconds.
fn envelope(t: f32, len: f32, attack: f32, release: f32) -> f32 {
    (t / attack).min(1.0).min(((len - t) / release).max(0.0))
}

/// Soft clipping followed by normalising the peak to `peak`.
fn finish(mut samples: Vec<f32>, peak: f32) -> Vec<f32> {
    for s in &mut samples {
        *s = (1.6 * *s).tanh();
    }
    let max = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if max > 0.0 {
        for s in &mut samples {
            *s *= peak / max;
        }
    }
    samples
}

/// Sequence of `(on, off)` blasts of a two-tone horn.
fn horn(freqs: [f32; 2], harmonics: u32, pattern: &[(f32, f32)]) -> Vec<f32> {
    let mut out = Vec::new();
    for &(on, off) in pattern {
        let n = (on * SAMPLE_RATE as f32) as usize;
        for i in 0..n {
            let t = i as f32 / SAMPLE_RATE as f32;
            let tone = buzzy(freqs[0], t, harmonics) + buzzy(freqs[1], t, harmonics);
            out.push(tone * envelope(t, on, 0.01, 0.04));
        }
        out.extend(std::iter::repeat_n(
            0.0,
            (off * SAMPLE_RATE as f32) as usize,
        ));
    }
    finish(out, 0.8)
}

/// A car's "honk honk".
pub fn car_horn() -> Vec<f32> {
    horn([415.0, 523.0], 6, &[(0.15, 0.07), (0.24, 0.0)])
}

/// A bus or truck's deeper, longer blast.
pub fn heavy_horn() -> Vec<f32> {
    horn([196.0, 247.0], 9, &[(0.55, 0.0)])
}

/// Two-tone ambulance siren ("nee-naw"), four notes.
pub fn siren() -> Vec<f32> {
    let note = 0.38;
    let tones = [587.0, 440.0, 587.0, 440.0];
    let len = note * tones.len() as f32;
    let n = (len * SAMPLE_RATE as f32) as usize;
    let mut phase = 0.0f32;
    let samples = (0..n)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE as f32;
            let freq = tones[((t / note) as usize).min(tones.len() - 1)];
            // Integrate the frequency so note changes do not click.
            phase = (phase + freq / SAMPLE_RATE as f32).fract();
            // Triangle-ish tone: odd harmonics falling off with 1/k².
            let tone: f32 = [1.0f32, 3.0, 5.0]
                .iter()
                .map(|&k| (TAU * phase * k).sin() / (k * k))
                .sum();
            tone * envelope(t, len, 0.05, 0.25)
        })
        .collect();
    finish(samples, 0.75)
}

/// How worked up a driver is: 0 calm, 1 honking, 2 about to give up.
pub fn honk_level(patience: f32) -> u8 {
    if patience < HONK_AGAIN_AT {
        2
    } else if patience < HONK_AT {
        1
    } else {
        0
    }
}

// ---- Playback ---------------------------------------------------------------------------

/// A mono clip of synthesised samples.
#[derive(Asset, TypePath)]
pub struct Clip {
    samples: Arc<[f32]>,
}

pub struct ClipDecoder {
    samples: Arc<[f32]>,
    pos: usize,
}

impl Iterator for ClipDecoder {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let s = self.samples.get(self.pos).copied();
        self.pos += 1;
        s
    }
}

impl Source for ClipDecoder {
    fn current_span_len(&self) -> Option<usize> {
        Some(self.samples.len().saturating_sub(self.pos))
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(1).expect("one channel")
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(SAMPLE_RATE).expect("non-zero rate")
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(
            self.samples.len() as f32 / SAMPLE_RATE as f32,
        ))
    }
}

impl bevy::audio::Decodable for Clip {
    type Decoder = ClipDecoder;

    fn decoder(&self) -> ClipDecoder {
        ClipDecoder {
            samples: self.samples.clone(),
            pos: 0,
        }
    }
}

#[derive(Resource)]
pub struct Sounds {
    car_horn: Handle<Clip>,
    heavy_horn: Handle<Clip>,
    siren: Handle<Clip>,
}

impl FromWorld for Sounds {
    fn from_world(world: &mut World) -> Self {
        let mut clips = world.resource_mut::<Assets<Clip>>();
        let mut add = |samples: Vec<f32>| {
            clips.add(Clip {
                samples: samples.into(),
            })
        };
        Sounds {
            car_horn: add(car_horn()),
            heavy_horn: add(heavy_horn()),
            siren: add(siren()),
        }
    }
}

/// How worked up a vehicle was at the last check.
#[derive(Component, Default)]
pub struct Mood(u8);

#[derive(Component)]
pub struct Horn;

#[derive(Resource, Default)]
struct Limiter {
    last_horn: f32,
    last_siren: f32,
}

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<Clip>()
            .init_resource::<Sounds>()
            .insert_resource(Limiter {
                last_horn: f32::NEG_INFINITY,
                last_siren: f32::NEG_INFINITY,
            })
            .add_systems(
                Update,
                (announce_ambulances, honk).run_if(in_state(GamePhase::Running)),
            );
    }
}

fn play(commands: &mut Commands, clip: &Handle<Clip>, volume: f32, horn: bool) {
    let mut e = commands.spawn((
        AudioPlayer(clip.clone()),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)),
    ));
    if horn {
        e.insert(Horn);
    }
}

fn announce_ambulances(
    mut commands: Commands,
    new: Query<&Vehicle, Added<Vehicle>>,
    sounds: Res<Sounds>,
    time: Res<Time<Real>>,
    mut limiter: ResMut<Limiter>,
) {
    let now = time.elapsed_secs();
    if new.iter().any(|v| v.kind == VehicleKind::Ambulance) && now - limiter.last_siren > SIREN_GAP
    {
        limiter.last_siren = now;
        play(&mut commands, &sounds.siren, 0.45, false);
    }
}

fn honk(
    mut commands: Commands,
    mut vehicles: Query<(Entity, &Vehicle, Option<&mut Mood>)>,
    horns: Query<(), With<Horn>>,
    sounds: Res<Sounds>,
    time: Res<Time<Real>>,
    mut limiter: ResMut<Limiter>,
) {
    let now = time.elapsed_secs();
    let mut playing = horns.iter().count();
    for (entity, v, mood) in &mut vehicles {
        let level = honk_level(v.patience);
        let Some(mut mood) = mood else {
            commands.entity(entity).insert(Mood(level));
            continue;
        };
        if level <= mood.0 {
            // Calmed down (or unchanged): remember it so the next wait honks again.
            mood.0 = level;
            continue;
        }
        let sound = match v.kind {
            VehicleKind::Ambulance => {
                if now - limiter.last_siren < SIREN_GAP {
                    continue;
                }
                limiter.last_siren = now;
                Some((&sounds.siren, 0.4, false))
            }
            _ if now - limiter.last_horn < HORN_GAP || playing >= MAX_HORNS => None,
            VehicleKind::Bus | VehicleKind::Truck => Some((&sounds.heavy_horn, 0.35, true)),
            VehicleKind::Car => Some((&sounds.car_horn, 0.3, true)),
        };
        // A driver whose honk was suppressed tries again next frame.
        if let Some((clip, volume, is_horn)) = sound {
            mood.0 = level;
            if is_horn {
                limiter.last_horn = now;
                playing += 1;
            }
            play(&mut commands, clip, volume, is_horn);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seconds(samples: &[f32]) -> f32 {
        samples.len() as f32 / SAMPLE_RATE as f32
    }

    fn zero_crossings(samples: &[f32]) -> usize {
        samples
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count()
    }

    #[test]
    fn clips_are_short_loud_enough_and_never_clip() {
        for (clip, min, max) in [
            (car_horn(), 0.3, 0.6),
            (heavy_horn(), 0.4, 0.8),
            (siren(), 1.0, 2.0),
        ] {
            let len = seconds(&clip);
            assert!((min..max).contains(&len), "length {len}");
            let peak = clip.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!((0.5..=1.0).contains(&peak), "peak {peak}");
        }
    }

    #[test]
    fn car_horn_honks_twice() {
        let clip = car_horn();
        // The short gap between the two blasts is silent.
        let gap_start = (0.16 * SAMPLE_RATE as f32) as usize;
        let gap = &clip[gap_start..gap_start + 1000];
        assert!(gap.iter().all(|s| s.abs() < 1e-3));
    }

    #[test]
    fn siren_alternates_between_two_pitches() {
        let clip = siren();
        let note = (0.38 * SAMPLE_RATE as f32) as usize;
        let window = |i: usize| &clip[i * note + note / 4..i * note + 3 * note / 4];
        let high = zero_crossings(window(0));
        let low = zero_crossings(window(1));
        assert!(high as f32 > low as f32 * 1.2, "{high} vs {low}");
        assert!(zero_crossings(window(2)) as f32 > low as f32 * 1.2);
    }

    #[test]
    fn drivers_honk_as_patience_runs_low() {
        assert_eq!(honk_level(1.0), 0);
        assert_eq!(honk_level(0.5), 1);
        assert_eq!(honk_level(0.1), 2);
    }

    #[test]
    fn decoder_plays_the_clip_once() {
        let clip = Clip {
            samples: vec![0.1, 0.2, 0.3].into(),
        };
        let d = bevy::audio::Decodable::decoder(&clip);
        let frames = d.total_duration().unwrap().as_secs_f32() * SAMPLE_RATE as f32;
        assert!((frames - 3.0).abs() < 0.01);
        assert_eq!(d.current_span_len(), Some(3));
        assert_eq!(d.collect::<Vec<_>>(), vec![0.1, 0.2, 0.3]);
    }
}
