//! The meat and potatoes of the level design. Spawning a meandering
//! river or channel through the forest of trees. Havent, quite decided
//! what yet.
//!
//! No Bevy here, so it's deterministic and unit-testable.

use noise::{NoiseFn, Perlin};

/// Half the width of the playfield in world pixels.
pub const PLAYFIELD_HALF_WIDTH: f32 = 600.0;
/// Land always left at the screen edges.
const MIN_BANK: f32 = 40.0;
/// Islands thinner than this aren't worth spawning.
const MIN_ISLAND: f32 = 24.0;
/// River width at full boss pinch.
const PINCH_WIDTH: f32 = 340.0;
/// How fast (per row) level parameters and the pinch blend to their targets.
const PARAM_BLEND_PER_ROW: f32 = 0.01;
const PINCH_PER_ROW: f32 = 0.01;

// One Perlin, sampled in separate lanes so each feature moves on its own.
const CENTER: f64 = 0.0;
const WIDTH: f64 = 100.0;
const LEFT_BANK: f64 = 200.0;
const RIGHT_BANK: f64 = 300.0;
const SPLIT: f64 = 400.0;
const SPLIT_OFFSET: f64 = 500.0;
const ISLET: f64 = 600.0;
const ISLET_OFFSET: f64 = 700.0;

/// Open water in one row: sorted, non-overlapping (left, right) spans.
pub type Water = Vec<(f32, f32)>;

/// Every knob that shapes a level's river.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiverParams {
    pub base_width: f32,
    pub width_variation: f32,
    pub meander: f32,
    pub bank_roughness: f32,
    /// Knobs, not probabilities: Perlin values cluster near zero.
    pub split_chance: f32,
    pub max_split_width: f32,
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
            split_chance: lerp(0.15, 0.35, d),
            max_split_width: lerp(160.0, 280.0, d),
            islet_chance: lerp(0.05, 0.25, d),
            max_islet_width: 70.0,
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
        let split_w = island_width(self.fbm(SPLIT, t * 0.5, 2), p.split_chance, p.max_split_width) * calm;
        let islet_w = island_width(self.fbm(ISLET, t * 2.0, 1), p.islet_chance, p.max_islet_width) * calm;

        let mut width = p.base_width + self.fbm(WIDTH, t * 0.7, 2) * p.width_variation + split_w + islet_w * 0.5;
        width = lerp(width, PINCH_WIDTH, self.pinch);
        let center = self.fbm(CENTER, t * 0.5, 3) * p.meander * calm;

        let edge = PLAYFIELD_HALF_WIDTH - MIN_BANK;
        let mut left = (center - width * 0.5 + self.fbm(LEFT_BANK, t * 6.0, 2) * p.bank_roughness).max(-edge);
        let mut right = (center + width * 0.5 + self.fbm(RIGHT_BANK, t * 6.0, 2) * p.bank_roughness).min(edge);
        if right - left < p.min_passage {
            let half = p.min_passage * 0.5;
            let mid = ((left + right) * 0.5).clamp(-edge + half, edge - half);
            left = mid - half;
            right = mid + half;
        }

        let mut water = vec![(left, right)];
        let split_at = center + self.fbm(SPLIT_OFFSET, t * 0.4, 1) * width * 0.2;
        carve_island(&mut water, split_at, split_w, p.min_passage);
        let islet_at = center + self.fbm(ISLET_OFFSET, t * 1.5, 1) * width * 0.4;
        carve_island(&mut water, islet_at, islet_w, p.min_passage);
        water
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

/// Turn a noise value into an island width that grows and shrinks smoothly.
fn island_width(field: f32, chance: f32, max_width: f32) -> f32 {
    let n = field * 0.5 + 0.5; // 0..1
    let threshold = 1.0 - chance;
    let s = ((n - threshold) / chance.max(0.01)).clamp(0.0, 1.0);
    s * s * (3.0 - 2.0 * s) * max_width // smoothstep
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
    if value < target { (value + step).min(target) } else { (value - step).max(target) }
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
                let water = river.next_row(10.0);
                let min = river.params().min_passage;
                assert!(!water.is_empty(), "level {level} row {row}: no water");
                for &(l, r) in &water {
                    assert!(r - l >= min - 0.5, "level {level} row {row}: {l}..{r} too narrow");
                    assert!(l >= -PLAYFIELD_HALF_WIDTH && r <= PLAYFIELD_HALF_WIDTH);
                }
            }
        }
    }
}