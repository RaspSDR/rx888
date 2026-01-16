// Mapping logic for the original RX888 Plus Model.

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
