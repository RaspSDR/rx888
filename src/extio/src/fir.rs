use std::f32::consts::PI as K_PI;
const K_2PI: f32 = 2.0 * K_PI;

fn izero(x: f32) -> f32 {
    let x2 = x / 2.0;
    let mut sum = 1.0_f32;
    let mut ds = 1.0_f32;
    let mut di = 1.0_f32;
    let errorlimit = 1e-9_f32;
    loop {
        let mut tmp = x2 / di;
        tmp *= tmp;
        ds *= tmp;
        sum += ds;
        di += 1.0;
        if ds < errorlimit * sum {
            break;
        }
    }
    sum
}

/// Ported from C: KaiserWindow
///
/// - `num_taps`: if >0 forces exact taps; if <0 limits to max (-num_taps);
/// - `astop`: stopband attenuation in dB
/// - `norm_fpass`, `norm_fstop`: normalized frequencies (relative to samplerate)
/// - `coef`: optional mutable slice to fill with coefficients. If `None`, function
///   returns the estimated number of taps (matching original behavior when Coef == nullptr)
///
/// Returns the used/estimated number of taps.
pub fn kaiser_window(
    num_taps: isize,
    astop: f32,
    norm_fpass: f32,
    norm_fstop: f32,
    coef: Option<&mut [f32]>,
) -> isize {
    let scale: f32 = 1.0; // kept from original

    let norm_fcut = (norm_fstop + norm_fpass) / 2.0;

    let beta = if astop < 20.96 {
        0.0_f32
    } else if astop >= 50.0 {
        0.1102_f32 * (astop - 8.71_f32)
    } else {
        0.5842_f32 * (astop - 20.96_f32).powf(0.4_f32) + 0.07886_f32 * (astop - 20.96_f32)
    };

    let mut m_num_taps =
        ((astop - 8.0) / (2.285_f32 * K_2PI * (norm_fstop - norm_fpass))).trunc() as isize + 1;

    if num_taps < 0 && m_num_taps > -num_taps {
        m_num_taps = -num_taps;
    }
    if m_num_taps < 3 {
        m_num_taps = 3;
    }

    if num_taps <= 0 && coef.is_none() {
        return m_num_taps;
    }

    if num_taps > 0 {
        m_num_taps = num_taps;
    }

    let f_center = 0.5_f32 * (m_num_taps as f32 - 1.0_f32);
    let izb = izero(beta);

    if let Some(buf) = coef {
        if buf.len() < m_num_taps as usize {
            // not enough space; behave as C would (UB in C), but we clip to available
            // to avoid panics: fill as much as possible
        }
        let m_minus1 = (m_num_taps as f32 - 1.0_f32) / 2.0_f32;
        let norm = 1.0_f32 / izb;
        for n in 0..(m_num_taps as usize) {
            let x = n as f32 - f_center;
            let c = if (n as f32) == f_center {
                2.0_f32 * norm_fcut
            } else {
                ((K_2PI * x * norm_fcut).sin()) / (K_PI * x)
            };
            let xx = ((n as f32) - ((m_num_taps as f32 - 1.0_f32) / 2.0_f32)) / m_minus1;
            let w = izero(beta * (1.0_f32 - (xx * xx)).sqrt());
            let coeff = scale * c * w * norm;
            if n < buf.len() {
                buf[n] = coeff;
            }
        }
    }

    m_num_taps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_and_fill_consistent() {
        let astop = 60.0_f32;
        let norm_fpass = 0.05_f32;
        let norm_fstop = 0.06_f32;

        let est = kaiser_window(0, astop, norm_fpass, norm_fstop, None);
        assert!(est >= 3, "estimated taps should be at least 3");

        let mut buf = vec![0.0_f32; est as usize];
        let filled = kaiser_window(0, astop, norm_fpass, norm_fstop, Some(&mut buf));
        assert_eq!(est, filled, "estimation and filled tap count should match");

        // coefficients should be symmetric
        let n = buf.len();
        for i in 0..(n / 2) {
            let a = buf[i];
            let b = buf[n - 1 - i];
            let diff = (a - b).abs();
            assert!(
                diff < 1e-5_f32,
                "coefficients not symmetric: idx {} diff {}",
                i,
                diff
            );
        }
    }

    #[test]
    fn fixed_num_taps_respected() {
        let astop = 80.0_f32;
        let norm_fpass = 0.02_f32;
        let norm_fstop = 0.03_f32;
        let num_taps = 11;
        let mut buf = vec![0.0_f32; num_taps as usize];
        let used = kaiser_window(num_taps, astop, norm_fpass, norm_fstop, Some(&mut buf));
        assert_eq!(used, num_taps);
        // basic sanity: center tap should be the largest magnitude
        let center = (num_taps as usize) / 2;
        let mut max_idx = 0usize;
        for (i, &v) in buf.iter().enumerate() {
            if v.abs() > buf[max_idx].abs() {
                max_idx = i;
            }
        }
        assert_eq!(max_idx, center, "center tap should be largest magnitude");
    }
}
