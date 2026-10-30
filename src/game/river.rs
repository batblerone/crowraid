//! The meat and potatoes of the level design. Spawning a meandering
//! river or channel through the forest of trees. Havent, quite decided
//! what yet.
//!
//! No Bevy here, so it's deterministic and unit-testable.

use std::f32::consts::PI;

use noise::{NoiseFn, Perlin};

/// Half the width of the playfield in world pixels.
pub const PLAYFIELD_HALF_WIDTH: f32 = 600.0;
/// Land always left at the screen edges.
const MIN_BANK: f32 = 40.0;
/// Islands thinner than this aren't spawned. Just over two 32 px tiles, so every
/// island covers at least two grid corners: it's drawn AND gets a collider.
const MIN_ISLAND: f32 = 72.0;
/// River width at full boss pinch.
const PINCH_WIDTH: f32 = 340.0;
/// How fast (per row) level parameters and the pinch blend to their targets.
const PARAM_BLEND_PER_ROW: f32 = 0.01;
const PINCH_PER_ROW: f32 = 0.01;

// One Perlin, sampled in separate lanes so each feature moves on its own.
// Lanes are deliberately NOT whole numbers: on an integer lane half of each
// Perlin gradient cancels out and the output never gets past about ±0.57.
const CENTER: f64 = 0.37;
const WIDTH: f64 = 100.61;
const LEFT_BANK: f64 = 200.23;
const RIGHT_BANK: f64 = 300.89;

// Islands are decided per stretch of river with a seeded dice roll, so their
// `chance` is a real probability rather than "how far the noise happens to reach".
/// Length of one stretch for big splits: about 8 s of flying at 300 px/s.
const SPLIT_STRETCH: f64 = 2400.0;
/// Length of one stretch for small islets.
const ISLET_STRETCH: f64 = 900.0;
/// Salts so splits and islets roll different dice from the same seed.
const SPLIT_SALT: u64 = 0x51_17;
const ISLET_SALT: u64 = 0x15_1E7;

/// Open water in one row: sorted, non-overlapping (left, right) spans.
pub type Water = Vec<(f32, f32)>;

/// Every knob that shapes a level's river.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiverParams {
    pub base_width: f32,
    pub width_variation: f32,
    pub meander: f32,
    pub bank_roughness: f32,
    /// Chance (0..1) that a 2400 px stretch of river has a big island.
    pub split_chance: f32,
    pub max_split_width: f32,
    /// Chance (0..1) that a 900 px stretch of river has a small islet.
    pub islet_chance: f32,
    pub max_islet_width: f32,
    /// Narrowest gap the player is ever asked to fly through.
    pub min_passage: f32,
}

impl RiverParams {
    /// Level 1 is wide and calm; by level 7 it's twisty, rough and forked.
    pub fn for_level(level: u32) -> Self {
        let d = (level.saturating_sub(1) as f32 * 0.15).min(1.0);
        Self {
            base_width: lerp(760.0, 520.0, d),
            width_variation: lerp(80.0, 160.0, d),
            meander: lerp(120.0, 260.0, d),
            bank_roughness: lerp(12.0, 36.0, d),
            split_chance: lerp(0.35, 0.65, d),
            max_split_width: lerp(160.0, 280.0, d),
            islet_chance: lerp(0.15, 0.4, d),
            max_islet_width: 110.0,
            min_passage: lerp(220.0, 150.0, d),
        }
    }

    fn blend(self, to: Self, t: f32) -> Self {
        Self {
            base_width: lerp(self.base_width, to.base_width, t),
            width_variation: lerp(self.width_variation, to.width_variation, t),
            meander: lerp(self.meander, to.meander, t),
            bank_roughness: lerp(self.bank_roughness, to.bank_roughness, t),
            split_chance: lerp(self.split_chance, to.split_chance, t),
            max_split_width: lerp(self.max_split_width, to.max_split_width, t),
            islet_chance: lerp(self.islet_chance, to.islet_chance, t),
            max_islet_width: lerp(self.max_islet_width, to.max_islet_width, t),
            min_passage: lerp(self.min_passage, to.min_passage, t),
        }
    }
}

pub struct RiverGenerator {
    noise: Perlin,
    /// Also seeds the island dice, so the same seed gives the same river.
    seed: u32,
    params: RiverParams,
    target: RiverParams,
    pinch: f32,
    /// 1.0 narrows the river for a boss, 0.0 opens it back up.
    pub pinch_target: f32,
    /// World distance of the next row. Only ever increases.
    distance: f64,
}

impl RiverGenerator {
    pub fn new(seed: u32, level: u32) -> Self {
        let params = RiverParams::for_level(level);
        Self {
            noise: Perlin::new(seed),
            seed,
            params,
            target: params,
            pinch: 0.0,
            pinch_target: 0.0,
            distance: 0.0,
        }
    }

    /// Blend toward a new level's shape over the next ~100 rows.
    pub fn set_level(&mut self, level: u32) {
        self.target = RiverParams::for_level(level);
    }

    pub fn params(&self) -> &RiverParams {
        &self.params
    }

    /// The next row's water, then advance by one row.
    pub fn next_row(&mut self, row_height: f32) -> Water {
        self.params = self.params.blend(self.target, PARAM_BLEND_PER_ROW);
        self.pinch = step_toward(self.pinch, self.pinch_target, PINCH_PER_ROW);
        let water = self.sample(self.distance);
        self.distance += row_height as f64;
        water
    }

    fn sample(&self, distance: f64) -> Water {
        let p = self.params;
        let t = distance * 0.001; // one noise unit per 1000 px
        let calm = 1.0 - self.pinch; // features fade out on the boss approach

        // Islands first, so the river can widen to make room for them.
        let split = self.island(
            SPLIT_SALT,
            SPLIT_STRETCH,
            distance,
            p.split_chance,
            p.max_split_width,
        );
        let islet = self.island(
            ISLET_SALT,
            ISLET_STRETCH,
            distance,
            p.islet_chance,
            p.max_islet_width,
        );
        let split_w = split.width * calm;
        let islet_w = islet.width * calm;

        let mut width = p.base_width
            + self.fbm(WIDTH, t * 0.7, 2) * p.width_variation
            + split_w
            + islet_w * 0.5;
        width = lerp(width, PINCH_WIDTH, self.pinch);
        let center = self.fbm(CENTER, t * 0.5, 3) * p.meander * calm;

        let edge = PLAYFIELD_HALF_WIDTH - MIN_BANK;
        let mut left =
            (center - width * 0.5 + self.fbm(LEFT_BANK, t * 6.0, 2) * p.bank_roughness).max(-edge);
        let mut right =
            (center + width * 0.5 + self.fbm(RIGHT_BANK, t * 6.0, 2) * p.bank_roughness).min(edge);
        if right - left < p.min_passage {
            let half = p.min_passage * 0.5;
            let mid = ((left + right) * 0.5).clamp(-edge + half, edge - half);
            left = mid - half;
            right = mid + half;
        }

        let mut water = vec![(left, right)];
        let split_at = center + split.offset * width * 0.15;
        carve_island(&mut water, split_at, split_w, p.min_passage);
        let islet_at = center + islet.offset * width * 0.35;
        carve_island(&mut water, islet_at, islet_w, p.min_passage);
        water
    }

    /// The island (if any) at `distance` for one kind of island. Each stretch
    /// rolls once: has an island or not, how long, how big, and which side.
    fn island(
        &self,
        salt: u64,
        stretch_len: f64,
        distance: f64,
        chance: f32,
        max_width: f32,
    ) -> Island {
        let along = distance / stretch_len;
        let index = along.floor();
        let u = (along - index) as f32; // 0..1 through this stretch

        let mut dice =
            fastrand::Rng::with_seed(mix(self.seed as u64 ^ mix(salt ^ mix(index as u64))));
        if dice.f32() >= chance {
            return Island::NONE;
        }
        // The island fills a random middle part of the stretch...
        let start = 0.05 + dice.f32() * 0.2;
        let end = 0.75 + dice.f32() * 0.2;
        let size = 0.6 + dice.f32() * 0.4;
        let offset = dice.f32() * 2.0 - 1.0;
        if u <= start || u >= end {
            return Island::NONE;
        }
        // ...and swells and tapers like a lens: rounded tips, full width mid-way.
        let s = (u - start) / (end - start);
        Island {
            width: max_width * size * (s * PI).sin().sqrt(),
            offset,
        }
    }

    /// Fractal noise in roughly -1..1: `octaves` layers, each double the
    /// frequency and half the strength of the last.
    fn fbm(&self, lane: f64, t: f64, octaves: u32) -> f32 {
        let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
        for _ in 0..octaves {
            sum += self.noise.get([lane, t * freq]) * amp;
            norm += amp;
            amp *= 0.5;
            freq *= 2.0;
        }
        (sum / norm) as f32
    }
}

/// One row's slice of an island.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Island {
    width: f32,
    /// Which side of the river's centre line, -1..1.
    offset: f32,
}

impl Island {
    const NONE: Self = Self {
        width: 0.0,
        offset: 0.0,
    };
}

/// Scrambles bits so neighbouring stretch numbers give unrelated dice seeds.
fn mix(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    x ^ (x >> 33)
}

/// Split whichever water span contains `center`, leaving at least
/// `min_passage` of water on both sides of the island.
fn carve_island(water: &mut Water, center: f32, width: f32, min_passage: f32) {
    let Some(i) = water.iter().position(|&(l, r)| center > l && center < r) else {
        return;
    };
    let (l, r) = water[i];
    let width = width.min(r - l - 2.0 * min_passage);
    if width < MIN_ISLAND {
        return;
    }
    let half = width * 0.5;
    let center = center.clamp(l + min_passage + half, r - min_passage - half);
    water[i] = (l, center - half);
    water.insert(i + 1, (center + half, r));
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn step_toward(value: f32, target: f32, step: f32) -> f32 {
    if value < target {
        (value + step).min(target)
    } else {
        (value - step).max(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn river_is_always_passable() {
        for level in 1..=8 {
            let mut river = RiverGenerator::new(1234 + level, level);
            for row in 0..20_000 {
                if row == 10_000 {
                    river.pinch_target = 1.0; // boss approach
                }
                let water = river.next_row(32.0);
                let min = river.params().min_passage;
                assert!(!water.is_empty(), "level {level} row {row}: no water");
                for &(l, r) in &water {
                    assert!(
                        r - l >= min - 0.5,
                        "level {level} row {row}: {l}..{r} too narrow"
                    );
                    assert!(l >= -PLAYFIELD_HALF_WIDTH && r <= PLAYFIELD_HALF_WIDTH);
                }
            }
        }
    }

    #[test]
    fn islands_actually_appear() {
        for level in [1, 4, 7] {
            let mut river = RiverGenerator::new(99, level);
            let rows = 20_000;
            let split_rows = (0..rows).filter(|_| river.next_row(32.0).len() > 1).count();
            let share = split_rows as f32 / rows as f32;
            assert!(
                share > 0.10,
                "level {level}: only {:.1}% of rows have an island",
                share * 100.0
            );
        }
    }

    #[test]
    fn same_seed_same_river() {
        let mut a = RiverGenerator::new(7, 3);
        let mut b = RiverGenerator::new(7, 3);
        for _ in 0..5_000 {
            assert_eq!(a.next_row(32.0), b.next_row(32.0));
        }
    }
}
