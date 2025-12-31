pub mod rx888;
pub mod rx888plus;
pub mod rx888r2;

use crate::interface::RadioModel;

/// Map a requested IF gain (in dB) to a device-specific gain index.
/// - `model`: radio model
/// - `direct_sampling`: true = direct sampling mode (HF style path), false = tuner/VHF mode
/// - `gain_db`: requested gain in dB
pub fn if_gain_to_index(model: RadioModel, direct_sampling: bool, gain_db: f32) -> u16 {
    match model {
        RadioModel::RX888r2 => rx888r2::if_gain_to_index(direct_sampling, gain_db),
        RadioModel::RX888 => rx888::if_gain_to_index(direct_sampling, gain_db),
        RadioModel::RX888plus => rx888plus::if_gain_to_index(direct_sampling, gain_db),
        _ => 0u16,
    }
}

/// Map a requested RF gain (in dB) to a device-specific gain index.
pub fn rf_gain_to_index(model: RadioModel, direct_sampling: bool, gain_db: f32) -> u16 {
    match model {
        RadioModel::RX888r2 => rx888r2::rf_gain_to_index(direct_sampling, gain_db),
        RadioModel::RX888 => rx888::rf_gain_to_index(direct_sampling, gain_db),
        RadioModel::RX888plus => rx888plus::rf_gain_to_index(direct_sampling, gain_db),
        _ => 0u16,
    }
}

/// Return IF gain range (min, max) in dB for the given model.
pub fn get_if_gain_range(model: RadioModel, direct_sampling: bool) -> (f32, f32) {
    match model {
        RadioModel::RX888r2 => rx888r2::get_if_gain_range(direct_sampling),
        RadioModel::RX888 => rx888::get_if_gain_range(direct_sampling),
        RadioModel::RX888plus => rx888plus::get_if_gain_range(direct_sampling),
        _ => (0.0f32, 0.0f32),
    }
}

/// Return RF gain range (min, max) in dB for the given model.
pub fn get_rf_gain_range(model: RadioModel, direct_sampling: bool) -> (f32, f32) {
    match model {
        RadioModel::RX888r2 => rx888r2::get_rf_gain_range(direct_sampling),
        RadioModel::RX888 => rx888::get_rf_gain_range(direct_sampling),
        RadioModel::RX888plus => rx888plus::get_rf_gain_range(direct_sampling),
        _ => (0.0f32, 0.0f32),
    }
}

/// Return IF gain steps slice for the given model and mode.
pub fn get_if_gain_steps(model: RadioModel, direct_sampling: bool) -> &'static [f32] {
    match model {
        RadioModel::RX888r2 => rx888r2::get_if_gain_steps(direct_sampling),
        RadioModel::RX888 => rx888::get_if_gain_steps(direct_sampling),
        RadioModel::RX888plus => rx888plus::get_if_gain_steps(direct_sampling),
        _ => &[],
    }
}

/// Return RF gain steps slice for the given model and mode.
pub fn get_rf_gain_steps(model: RadioModel, direct_sampling: bool) -> &'static [f32] {
    match model {
        RadioModel::RX888r2 => rx888r2::get_rf_gain_steps(direct_sampling),
        RadioModel::RX888 => rx888::get_rf_gain_steps(direct_sampling),
        RadioModel::RX888plus => rx888plus::get_rf_gain_steps(direct_sampling),
        _ => &[],
    }
}

pub fn find_closest_index(steps: &[f32], gain_db: f32) -> u16 {
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
