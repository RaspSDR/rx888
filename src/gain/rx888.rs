// Mapping logic for the original RX888 / BBRF103 model.

use crate::gain::find_closest_index;
use std::sync::OnceLock;

const VHF_IF_STEPS: [f32; 16] = [
    -4.7, -2.1, 0.5, 3.5, 7.7, 11.2, 13.6, 14.9, 16.3, 19.5, 23.1, 26.5, 30.0, 33.7, 37.2, 40.8,
];

const HF_STEPS: [f32; 3] = [-20.0, -10.0, 0.0];

static VHF_IF_USER_CACHE: OnceLock<Vec<f32>> = OnceLock::new();

fn vhf_if_user_steps_static() -> &'static [f32] {
    let v = VHF_IF_USER_CACHE.get_or_init(|| {
        // User-facing gain semantics: positive values mean more gain, so flip sign
        VHF_IF_STEPS.iter().map(|g| -*g).collect()
    });
    v.as_slice()
}

/// `direct_sampling` true means direct sample mode; VHF mode is !direct_sampling.
pub fn if_gain_to_index(direct_sampling: bool, gain_db: f32) -> u16 {
    // RX888 (BBRF103) has no IF gain in direct sampling (HF) mode.
    if !direct_sampling {
        // Hardware expects attenuation-like steps; user gain is opposite sign
        find_closest_index(&VHF_IF_STEPS, -gain_db)
    } else {
        // No IF in direct mode; return index 0 as a no-op
        0u16
    }
}

pub fn rf_gain_to_index(direct_sampling: bool, gain_db: f32) -> u16 {
    if direct_sampling {
        find_closest_index(&HF_STEPS, gain_db)
    } else {
        find_closest_index(&VHF_IF_STEPS, gain_db)
    }
}

pub fn get_if_gain_range(direct_sampling: bool) -> (f32, f32) {
    if !direct_sampling {
        let steps = vhf_if_user_steps_static();
        let min = steps.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = steps.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        (min, max)
    } else {
        // No IF in direct mode
        (0.0f32, 0.0f32)
    }
}

pub fn get_rf_gain_range(direct_sampling: bool) -> (f32, f32) {
    get_if_gain_range(direct_sampling)
}

pub fn get_if_gain_steps(direct_sampling: bool) -> &'static [f32] {
    if !direct_sampling {
        vhf_if_user_steps_static()
    } else {
        &[]
    }
}

pub fn get_rf_gain_steps(direct_sampling: bool) -> &'static [f32] {
    // In direct (HF) mode RX888 exposes RF attenuator steps (negative dB values)
    if direct_sampling {
        &HF_STEPS
    } else {
        // In VHF/tuner mode RF mapping mirrors IF steps for now
        &VHF_IF_STEPS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vhf_steps() {
        // VHF mode should map user-facing gain values to indices
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
    fn test_hf_steps() {
        // In direct (HF) mode RX888 has RF attenuator steps; verify RF mapping
        for (i, &v) in HF_STEPS.iter().enumerate() {
            let idx = rf_gain_to_index(true, v);
            assert_eq!(
                idx as usize, i,
                "HF step {} -> idx {} expected {}",
                v, idx, i
            );
        }
        // IF mapping in direct mode should be a no-op (index 0)
        assert_eq!(if_gain_to_index(true, HF_STEPS[1]), 0u16);
    }
}
