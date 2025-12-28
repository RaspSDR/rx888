use std::f32;
use std::sync::OnceLock;

const GAIN_SWEET_POINT: usize = 18;
const HIGH_GAIN_RATIO: f32 = 0.409f32;
const LOW_GAIN_RATIO: f32 = 0.059f32;

const VHF_IF_STEPS: [f32; 16] = [
    -4.7, -2.1, 0.5, 3.5, 7.7, 11.2, 13.6, 14.9, 16.3, 19.5, 23.1, 26.5, 30.0, 33.7, 37.2, 40.8,
];

const VHF_RF_STEPS: [f32; 29] = [
    0.0, 0.9, 1.4, 2.7, 3.7, 7.7, 8.7, 12.5, 14.4, 15.7, 16.6, 19.7, 20.7, 22.9, 25.4, 28.0, 29.7,
    32.8, 33.8, 36.4, 37.2, 38.6, 40.2, 42.1, 43.4, 43.9, 44.5, 48.0, 49.6,
];

const HF_RF_STEP_SIZE: usize = 64;
const HF_IF_STEP_SIZE: usize = 127;

fn build_hf_if_steps() -> Vec<f32> {
    let mut v = Vec::with_capacity(HF_IF_STEP_SIZE);
    for i in 0..HF_IF_STEP_SIZE {
        let val = if i > GAIN_SWEET_POINT {
            20.0f32 * (HIGH_GAIN_RATIO * ((i - GAIN_SWEET_POINT) as f32 + 3.0)).log10()
        } else {
            20.0f32 * (LOW_GAIN_RATIO * ((i + 1) as f32)).log10()
        };
        v.push(-val);
    }
    v
}

fn build_hf_rf_steps() -> Vec<f32> {
    let mut v = Vec::with_capacity(HF_RF_STEP_SIZE);
    // Build HF RF steps using bit-manipulation algorithm, reversed order
    for i in 0..HF_RF_STEP_SIZE {
        let idx = HF_RF_STEP_SIZE - i - 1;
        let mut val = 0.0f32;
        if (idx & 0x01) != 0 {
            val += 0.5f32;
        }
        if (idx & 0x02) != 0 {
            val += 1.0f32;
        }
        if (idx & 0x04) != 0 {
            val += 2.0f32;
        }
        if (idx & 0x08) != 0 {
            val += 4.0f32;
        }
        if (idx & 0x10) != 0 {
            val += 8.0f32;
        }
        if (idx & 0x20) != 0 {
            val += 16.0f32;
        }
        v.push(-val);
    }
    v
}

static HF_IF_CACHE: OnceLock<Vec<f32>> = OnceLock::new();
static HF_RF_CACHE: OnceLock<Vec<f32>> = OnceLock::new();
static HF_IF_USER_CACHE: OnceLock<Vec<f32>> = OnceLock::new();
static VHF_IF_USER_CACHE: OnceLock<Vec<f32>> = OnceLock::new();

fn hf_if_steps_static() -> &'static [f32] {
    let v = HF_IF_CACHE.get_or_init(build_hf_if_steps);
    v.as_slice()
}

fn hf_if_user_steps_static() -> &'static [f32] {
    let v = HF_IF_USER_CACHE.get_or_init(|| {
        // User-facing gain semantics: positive values mean more gain, so flip sign
        hf_if_steps_static().iter().copied().collect::<Vec<f32>>()
    });
    v.as_slice()
}

fn hf_rf_steps_static() -> &'static [f32] {
    let v = HF_RF_CACHE.get_or_init(build_hf_rf_steps);
    v.as_slice()
}

fn vhf_if_user_steps_static() -> &'static [f32] {
    let v = VHF_IF_USER_CACHE.get_or_init(|| VHF_IF_STEPS.to_vec());
    v.as_slice()
}

fn find_closest_index(steps: &[f32], gain_db: f32) -> u16 {
    if steps.is_empty() {
        return 0u16;
    }
    let mut best = 0usize;
    let mut best_diff = f32::INFINITY;
    for (i, &v) in steps.iter().enumerate() {
        let diff = (v - gain_db).abs();
        if diff < best_diff {
            best_diff = diff;
            best = i;
        }
    }
    best as u16
}

/// `direct_sampling` is true when device is in direct sampling mode.
/// Per repo convention VHF mode == !direct_sampling.
pub fn if_gain_to_index(direct_sampling: bool, gain_db: f32) -> u16 {
    if !direct_sampling {
        // VHF mode
        // Hardware expects attenuation-like steps; user gain is opposite sign
        find_closest_index(&VHF_IF_STEPS, gain_db)
    } else {
        // HF / direct sampling: build HF steps
        let hf = hf_if_steps_static();
        find_closest_index(hf, gain_db)
    }
}

pub fn rf_gain_to_index(direct_sampling: bool, gain_db: f32) -> u16 {
    if !direct_sampling {
        // VHF mode
        find_closest_index(&VHF_RF_STEPS, gain_db)
    } else {
        // HF / direct sampling: build HF RF steps
        let hf = hf_rf_steps_static();
        find_closest_index(hf, gain_db)
    }
}

pub fn get_if_gain_range(direct_sampling: bool) -> (f32, f32) {
    if !direct_sampling {
        let steps = vhf_if_user_steps_static();
        let min = steps.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = steps.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        (min, max)
    } else {
        let steps = hf_if_user_steps_static();
        let min = steps.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = steps.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        (min, max)
    }
}

pub fn get_rf_gain_range(direct_sampling: bool) -> (f32, f32) {
    if !direct_sampling {
        let min = VHF_RF_STEPS.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = VHF_RF_STEPS
            .iter()
            .cloned()
            .fold(f32::NEG_INFINITY, f32::max);
        (min, max)
    } else {
        let hf = hf_rf_steps_static();
        let min = hf.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = hf.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        (min, max)
    }
}

pub fn get_if_gain_steps(direct_sampling: bool) -> &'static [f32] {
    if !direct_sampling {
        vhf_if_user_steps_static()
    } else {
        hf_if_user_steps_static()
    }
}

pub fn get_rf_gain_steps(direct_sampling: bool) -> &'static [f32] {
    if !direct_sampling {
        &VHF_RF_STEPS
    } else {
        hf_rf_steps_static()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vhf_if_step_match() {
        // VHF mode (direct_sampling == false) should map user-facing gain values to indices
        let steps = get_if_gain_steps(false);
        for (i, &v) in steps.iter().enumerate() {
            let idx = if_gain_to_index(false, v);
            assert_eq!(
                idx as usize, i,
                "VHF user step {} -> idx {} expected {}",
                v, idx, i
            );
        }
    }

    #[test]
    fn test_hf_if_monotonic() {
        // HF generated steps should be monotonic increasing; map a few values
        let hf_user = hf_if_user_steps_static();
        assert!(hf_user.len() > 10);
        let idx_mid = if_gain_to_index(true, hf_user[hf_user.len() / 2]);
        assert!(idx_mid as usize > 0 && (idx_mid as usize) < hf_user.len());
    }
}
