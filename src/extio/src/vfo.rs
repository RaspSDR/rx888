use crate::fir;
use num_complex::Complex32;
use realfft::{RealFftPlanner, RealToComplex};
use rustfft::{Fft, FftPlanner};
use std::sync::Mutex;
use wide::f32x4;

pub struct RxVFO {
    ctrl: Mutex<()>,

    half_fft: usize,

    // FFT plans
    r2c: std::sync::Arc<dyn RealToComplex<f32>>,
    c2c: std::sync::Arc<dyn Fft<f32>>,

    // Buffers
    adc_time: Vec<f32>,       // overlap-save time buffer
    adc_freq: Vec<Complex32>, // full FFT output
    freq_tmp: Vec<Complex32>, // decimated spectrum
    filter: Vec<Complex32>,   // frequency-domain filter

    // Parameters
    gain: f32,
    decim_idx: usize,
    tune_bin: usize,
    lsb: bool,

    in_sr: f64,
    out_sr: f64,

    // Fine frequency correction (NCO)
    fc: f32,
    phase: f32,
}

impl RxVFO {
    pub fn new_with_gain(in_sr: f64, gain: f32) -> Self {
        let fft_size = 8192;
        let half_fft = fft_size / 2;

        let mut rplanner = RealFftPlanner::<f32>::new();
        let r2c = rplanner.plan_fft_forward(2 * half_fft);

        let mut cplanner = FftPlanner::<f32>::new();
        let c2c = cplanner.plan_fft_inverse(half_fft);

        Self {
            ctrl: Mutex::new(()),

            half_fft,
            r2c,
            c2c,

            adc_time: vec![0.0; 2 * half_fft + half_fft],
            adc_freq: vec![Complex32::ZERO; half_fft + 1],
            freq_tmp: vec![Complex32::ZERO; half_fft],
            filter: vec![Complex32::ZERO; half_fft],

            gain,
            decim_idx: 1,
            tune_bin: half_fft / 4,
            lsb: false,

            in_sr: in_sr,
            out_sr: 32_000_000.0,

            fc: 0.0,
            phase: 0.0,
        }
    }

    fn shift_and_filter(dst: &mut [Complex32], src: &[Complex32], filt: &[Complex32]) {
        let count = src.len();
        assert!(dst.len() == count);
        assert!(filt.len() == count);

        for i in 0..count {
            dst[i] = src[i] * filt[i];
        }
    }

    #[inline]
    fn convert_i16_to_f32(src: &[i16], dst: &mut [f32]) {
        // Keep behavior identical: convert i16 -> f32 with scale 1.0
        let scale = 1.0f32;
        let len = std::cmp::min(src.len(), dst.len());

        let mut i = 0usize;
        // Process 4 samples at a time using wide::f32x4
        while i + 4 <= len {
            let a = src[i] as f32 * scale;
            let b = src[i + 1] as f32 * scale;
            let c = src[i + 2] as f32 * scale;
            let d = src[i + 3] as f32 * scale;

            let v = f32x4::from([a, b, c, d]);
            // Convert SIMD register to array and copy into dst
            let arr = v.to_array();
            dst[i..i + 4].copy_from_slice(&arr);

            i += 4;
        }

        // Handle remainder
        while i < len {
            dst[i] = src[i] as f32 * scale;
            i += 1;
        }
    }

    fn generate_freq_filter(&mut self, index: usize) {
        const Astop: f32 = 120.0;
        const relPass: f32 = 0.85; // 85% of Nyquist should be usable
        const relStop: f32 = 1.1; // 'some' alias back into transition band is OK

        let bw = 64.0 / (1 << index) as f32;

        let mut ht = vec![0.0f32; self.half_fft / 4 + 1];
        fir::kaiser_window(
            (self.half_fft / 4 + 1) as isize,
            Astop,
            relPass * bw / 128.0,
            relStop * bw / 128.0,
            Some(&mut ht),
        );

        let gain_adj = self.gain * 4096.0 / (self.half_fft * 2) as f32;
        self.filter.fill(Complex32::ZERO);

        for (i, &v) in ht.iter().enumerate() {
            self.filter[self.half_fft - 1 - i].re = v * gain_adj;
        }

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(self.half_fft);
        fft.process(&mut self.filter);

        self.c2c = planner.plan_fft_inverse(self.half_fft / (1 << index));
    }

    pub fn set_output_sample_rate(&mut self, out_sr: f64) {
        let _lock = self.ctrl.lock().unwrap();
        self.out_sr = out_sr;

        let decim = (self.in_sr / out_sr).round() as usize;
        self.decim_idx = decim.next_power_of_two().trailing_zeros() as usize;

        drop(_lock);
        self.generate_freq_filter(self.decim_idx);
    }

    pub fn set_offset(&mut self, freq_offset: f64) {
        let _lock = self.ctrl.lock().unwrap();
        let original_tubebin = self.tune_bin;
        if (freq_offset > self.in_sr) {
            return;
        }

        self.lsb = freq_offset < 0.0;
        let freq_offset = freq_offset.abs();

        self.tune_bin = (freq_offset * self.half_fft as f64 / 4.0) as usize * 4; // mtunebin step 4 bin  ?
        let delta = (self.tune_bin as f64 / self.half_fft as f64) - freq_offset;

        self.fc = delta as f32 * (1 << self.decim_idx) as f32;
        if self.lsb {
            self.fc = -self.fc;
        }

        // TODO
    }

    pub fn process(&mut self, input: &[i16], output: &mut Vec<Complex32>) -> usize {
        let (mfft, hop) = {
            let _lock = self.ctrl.lock().unwrap();
            (self.half_fft >> self.decim_idx, 3 * self.half_fft / 2)
        };

        // Convert input → time buffer
        Self::convert_i16_to_f32(
            input,
            &mut self.adc_time[self.half_fft..self.half_fft + input.len()],
        );

        let mut real_pos = 0;
        let mut produced = 0;

        while real_pos + 2 * self.half_fft <= self.adc_time.len() {
            // R2C FFT
            self.r2c
                .process(
                    &mut self.adc_time[real_pos..real_pos + 2 * self.half_fft],
                    &mut self.adc_freq,
                )
                .unwrap();

            self.adc_freq[0] = Complex32::ZERO;

            // Frequency shift + filter
            let src = &self.adc_freq[self.tune_bin..self.tune_bin + mfft / 2];
            let filt = &self.filter[self.half_fft - mfft / 2..];
            Self::shift_and_filter(&mut self.freq_tmp[..mfft / 2], src, filt);

            // IFFT (decimated)
            self.c2c.process(&mut self.freq_tmp[..mfft]);

            // Output overlap-save section
            let start = mfft / 4;
            let count = mfft / 2;

            for i in 0..count {
                let mut v = self.freq_tmp[start + i];
                if self.lsb {
                    v.im = -v.im;
                }
                output.push(v);
            }

            produced += count;
            real_pos += hop;
        }

        // Save overlap
        self.adc_time
            .copy_within(input.len()..input.len() + self.half_fft, 0);

        // Fine frequency correction (portable NCO)
        // if self.fc != 0.0 {
        //     let step = self.fc;
        //     let start_idx = output.len().saturating_sub(produced);
        //     for v in &mut output[start_idx..] {
        //         let (s, c) = self.phase.sin_cos();
        //         let rot = Complex32::new(c, s);
        //         *v *= rot;
        //         self.phase += step;
        //     }
        // }

        produced
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_i16_to_f32() {
        let src: Vec<i16> = vec![-32768, -16384, 0, 16384, 32767];
        let mut dst: Vec<f32> = vec![0.0; src.len()];

        RxVFO::convert_i16_to_f32(&src, &mut dst);

        let expected: Vec<f32> = vec![-32768.0, -16384.0, 0.0, 16384.0, 32767.0];
        for (d, e) in dst.iter().zip(expected.iter()) {
            assert!((d - e).abs() < 1e-5);
        }
    }

    #[test]
    fn test_shift_and_filter() {
        let src: Vec<Complex32> = vec![
            Complex32::new(1.0, 2.0),
            Complex32::new(3.0, 4.0),
            Complex32::new(5.0, 6.0),
            Complex32::new(7.0, 8.0),
        ];
        let filt: Vec<Complex32> = vec![
            Complex32::new(0.5, 0.5),
            Complex32::new(1.0, 0.0),
            Complex32::new(0.0, 1.0),
            Complex32::new(0.5, -0.5),
        ];
        let mut dst: Vec<Complex32> = vec![Complex32::ZERO; src.len()];
        RxVFO::shift_and_filter(&mut dst, &src, &filt);
        let expected: Vec<Complex32> = vec![
            Complex32::new(-0.5, 1.5),
            Complex32::new(3.0, 4.0),
            Complex32::new(-6.0, 5.0),
            Complex32::new(7.5, 0.5),
        ];
        for (d, e) in dst.iter().zip(expected.iter()) {
            println!("d: {:?}, e: {:?}", d, e);
            assert!((d.re - e.re).abs() < 1e-5);
            assert!((d.im - e.im).abs() < 1e-5);
        }
    }
}
