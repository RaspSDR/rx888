pub mod rx888;
pub mod rx888plus;
pub mod rx888pro;
pub mod rx888r2;

use crate::interface::RadioModel;

/// Return IF gain steps slice for the given model and mode.
pub fn get_if_gain_steps(model: RadioModel, direct_sampling: bool) -> &'static [f32] {
    match model {
        RadioModel::RX888r2 => rx888r2::get_if_gain_steps(direct_sampling),
        RadioModel::RX888 => rx888::get_if_gain_steps(direct_sampling),
        RadioModel::RX888plus => rx888plus::get_if_gain_steps(direct_sampling),
        RadioModel::RX888pro => rx888pro::get_if_gain_steps(direct_sampling),
        _ => &[],
    }
}

/// Return RF gain steps slice for the given model and mode.
pub fn get_rf_gain_steps(model: RadioModel, direct_sampling: bool) -> &'static [f32] {
    match model {
        RadioModel::RX888r2 => rx888r2::get_rf_gain_steps(direct_sampling),
        RadioModel::RX888 => rx888::get_rf_gain_steps(direct_sampling),
        RadioModel::RX888plus => rx888plus::get_rf_gain_steps(direct_sampling),
        RadioModel::RX888pro => rx888pro::get_rf_gain_steps(direct_sampling),
        _ => &[],
    }
}

/// Map a requested IF gain (in dB) to a device-specific gain index.
/// - `model`: radio model
/// - `direct_sampling`: true = direct sampling mode (HF style path), false = tuner/VHF mode
/// - `gain_db`: requested gain in dB
pub fn if_gain_to_index(model: RadioModel, direct_sampling: bool, gain_db: f32) -> u16 {
    let steps = get_if_gain_steps(model, direct_sampling);
    find_closest_index(steps, gain_db)
}

/// Map a requested RF gain (in dB) to a device-specific gain index.
pub fn rf_gain_to_index(model: RadioModel, direct_sampling: bool, gain_db: f32) -> u16 {
    let steps = get_rf_gain_steps(model, direct_sampling);
    find_closest_index(steps, gain_db)
}

/// Return IF gain range (min, max) in dB for the given model.
pub fn get_if_gain_range(model: RadioModel, direct_sampling: bool) -> (f32, f32) {
    let steps = get_if_gain_steps(model, direct_sampling);
    let len = steps.len();
    if len == 0 {
        (0.0f32, 0.0f32)
    } else if len == 1 {
        (steps[0], steps[0])
    } else {
        let min = steps.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = steps.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        (min, max)
    }
}

/// Return RF gain range (min, max) in dB for the given model.
pub fn get_rf_gain_range(model: RadioModel, direct_sampling: bool) -> (f32, f32) {
    let steps = get_rf_gain_steps(model, direct_sampling);
    let len = steps.len();
    if len == 0 {
        (0.0f32, 0.0f32)
    } else if len == 1 {
        (steps[0], steps[0])
    } else {
        let min = steps.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = steps.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        (min, max)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vhf_if_step_match() {
        for model in [
            RadioModel::RX888,
            RadioModel::RX888plus,
            RadioModel::RX888pro,
            RadioModel::RX888r2,
        ] {
            let steps = get_if_gain_steps(model, false);
            for (i, &v) in steps.iter().enumerate() {
                let idx = if_gain_to_index(model, false, v);
                assert_eq!(
                    idx as usize, i,
                    "Model {:?} VHF user step {} -> idx {} expected {}",
                    model, v, idx, i
                );
            }

            // VHF mode (direct_sampling == false) should map user-facing gain values to indices
            let steps = get_if_gain_steps(model, false);
            for (i, &v) in steps.iter().enumerate() {
                let idx = if_gain_to_index(model, false, v);
                assert_eq!(
                    idx as usize, i,
                    "VHF user step {} -> idx {} expected {}",
                    v, idx, i
                );
            }
        }
    }

    #[test]
    fn test_hf_if_monotonic() {
        for model in [RadioModel::RX888pro, RadioModel::RX888r2, RadioModel::RX888] {
            // HF generated steps should be monotonic increasing; map a few values
            let hf_user = get_rf_gain_steps(model, true);
            assert!(hf_user.len() >= 3);
            let idx_mid = if_gain_to_index(model, true, hf_user[hf_user.len() / 2]);
            assert!(idx_mid as usize > 0 && (idx_mid as usize) < hf_user.len());
        }
    }
}
