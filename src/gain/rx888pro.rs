// Mapping logic for the original RX888 mk2 Model.

use std::f32;
use std::sync::OnceLock;

const VHF_IF_STEPS: [f32; 16] = [
    -4.7, -2.1, 0.5, 3.5, 7.7, 11.2, 13.6, 14.9, 16.3, 19.5, 23.1, 26.5, 30.0, 33.7, 37.2, 40.8,
];

const VHF_RF_STEPS: [f32; 45] = [
    0.0, 0.9, 1.4, 2.7, 3.7, 7.7, 8.7, 12.5, 14.4, 15.7, 16.6, 19.7, 20.7, 22.9, 25.4, 28.0, 29.7,
    32.8, 33.8, 36.4, 37.2, 38.6, 40.2, 42.1, 43.4, 43.9, 44.5, 48.0, 49.6, 51.0, 52.0, 53.0, 54.0, 55.0, 56.0,
    57.0, 58.0, 59.0, 60.0, 61.0, 62.0, 63.0, 64.0, 65.0, 66.0
];

const HF_RF_STEP_SIZE: usize = 64;

fn build_hf_rf_steps() -> Vec<f32> {
    let mut v = Vec::with_capacity(HF_RF_STEP_SIZE);
    // Build HF RF steps using bit-manipulation algorithm, reversed order
    for i in 0..HF_RF_STEP_SIZE {
        let idx = HF_RF_STEP_SIZE - i - 1;
        let mut val = 0.0f32;
        if (idx & 0x01) != 0 {
            val -= 0.5f32;
        }
        if (idx & 0x02) != 0 {
            val -= 1.0f32;
        }
        if (idx & 0x04) != 0 {
            val -= 2.0f32;
        }
        if (idx & 0x08) != 0 {
            val -= 4.0f32;
        }
        if (idx & 0x10) != 0 {
            val -= 8.0f32;
        }
        if (idx & 0x20) != 0 {
            val -= 16.0f32;
        }
        v.push(val);
    }
    v
}

static HF_RF_CACHE: OnceLock<Vec<f32>> = OnceLock::new();
static VHF_IF_USER_CACHE: OnceLock<Vec<f32>> = OnceLock::new();

fn hf_rf_steps_static() -> &'static [f32] {
    let v = HF_RF_CACHE.get_or_init(build_hf_rf_steps);
    v.as_slice()
}

fn vhf_if_user_steps_static() -> &'static [f32] {
    let v = VHF_IF_USER_CACHE.get_or_init(|| VHF_IF_STEPS.to_vec());
    v.as_slice()
}

pub fn get_if_gain_steps(direct_sampling: bool) -> &'static [f32] {
    if !direct_sampling {
        vhf_if_user_steps_static()
    } else {
        &[0.0f32, 18.0f32]
    }
}

pub fn get_rf_gain_steps(direct_sampling: bool) -> &'static [f32] {
    if !direct_sampling {
        &VHF_RF_STEPS
    } else {
        hf_rf_steps_static()
    }
}
