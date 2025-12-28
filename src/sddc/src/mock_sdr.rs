/// Mock SDR for testing virtual_sdr without physical hardware
/// Generates realistic int16_t signals with multiple test patterns
use std::f32::consts::PI;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Instant;

/// Signal generation patterns for testing
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SignalPattern {
    /// Pure sine wave at specified frequency
    Sine { freq_hz: f32 },
    /// Multiple sine waves combined
    MultiTone { freqs: &'static [f32] },
    /// Frequency sweep (chirp)
    Sweep {
        start_freq: f32,
        end_freq: f32,
        duration_secs: f32,
    },
    /// Amplitude modulated carrier
    AM {
        carrier_freq: f32,
        mod_freq: f32,
        mod_depth: f32,
    },
    /// White noise
    Noise,
    /// Combination of signal + noise
    SignalPlusNoise { signal_freq: f32, snr_db: f32 },
}

/// Mock SDR signal generator
pub struct MockSDR {
    pattern: SignalPattern,
    amplitude: f32,
    phase: f32,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    start_time: Option<Instant>,
    // Device-like state for trait compatibility with VirtualRadio
    center_freq: u64,
    xtal_freq: u32,
    if_gain: f32,
    rf_gain: f32,
    direct_sampling: bool,
}

impl MockSDR {
    /// Create a new mock SDR
    ///
    /// # Arguments
    /// * `xtal_freq` - Crystal oscillator frequency in Hz (for SDR device, xtal_freq = sample rate)
    /// * `pattern` - Signal generation pattern
    /// * `amplitude` - Signal amplitude (0.0 to 1.0)
    pub fn new(xtal_freq: u32, pattern: SignalPattern, amplitude: f32) -> Self {
        Self {
            pattern,
            amplitude: amplitude.clamp(0.0, 1.0),
            phase: 0.0,
            running: Arc::new(AtomicBool::new(false)),
            thread: None,
            start_time: None,
            center_freq: 0,
            xtal_freq,
            if_gain: 0.0,
            rf_gain: 0.0,
            direct_sampling: true,
        }
    }

    /// Generate a buffer of int16_t samples
    ///
    /// Returns Vec<i16> samples
    pub fn generate_buffer(&mut self, num_samples: usize) -> Vec<i16> {
        let mut buffer = vec![0i16; num_samples];
        let elapsed = self
            .start_time
            .map(|t| t.elapsed().as_secs_f32())
            .unwrap_or(0.0);

        for (i, sample) in buffer.iter_mut().enumerate().take(num_samples) {
            let t = i as f32 / self.xtal_freq as f32;
            let global_t = elapsed + t;

            let sample_f32 = self.generate_sample(t, global_t);

            // Scale to int16 range with clipping
            let scaled = (sample_f32 * self.amplitude * i16::MAX as f32)
                .clamp(i16::MIN as f32, i16::MAX as f32);
            *sample = scaled as i16;

            // Update phase for continuous signal
            self.update_phase(t);
        }

        buffer
    }

    /// Generate a single sample based on the pattern
    fn generate_sample(&self, t: f32, global_t: f32) -> f32 {
        match self.pattern {
            SignalPattern::Sine { freq_hz } => {
                let phase = 2.0 * PI * freq_hz * t + self.phase;
                phase.sin()
            }

            SignalPattern::MultiTone { freqs } => {
                let mut sum = 0.0;
                for &freq in freqs {
                    let phase = 2.0 * PI * freq * t + self.phase;
                    sum += phase.sin();
                }
                sum / freqs.len() as f32
            }

            SignalPattern::Sweep {
                start_freq,
                end_freq,
                duration_secs,
            } => {
                // Linear frequency sweep
                let progress = (global_t % duration_secs) / duration_secs;
                let instant_freq = start_freq + (end_freq - start_freq) * progress;
                let phase = 2.0 * PI * instant_freq * t + self.phase;
                phase.sin()
            }

            SignalPattern::AM {
                carrier_freq,
                mod_freq,
                mod_depth,
            } => {
                let carrier_phase = 2.0 * PI * carrier_freq * t + self.phase;
                let mod_phase = 2.0 * PI * mod_freq * t;
                let modulation = 1.0 + mod_depth * mod_phase.sin();
                carrier_phase.sin() * modulation
            }

            SignalPattern::Noise => {
                // Simple pseudo-random noise using phase as seed
                let x = (t * 12_345.679 + self.phase).sin() * 43_758.547;
                (x - x.floor()) * 2.0 - 1.0
            }

            SignalPattern::SignalPlusNoise {
                signal_freq,
                snr_db,
            } => {
                // Calculate signal and noise powers
                let signal = (2.0 * PI * signal_freq * t + self.phase).sin();
                let noise_amplitude = 10.0_f32.powf(-snr_db / 20.0);

                // Generate noise
                let x = (t * 12_345.679 + self.phase).sin() * 43_758.547;
                let noise = ((x - x.floor()) * 2.0 - 1.0) * noise_amplitude;

                signal + noise
            }
        }
    }

    /// Update phase accumulator for continuous signals
    fn update_phase(&mut self, dt: f32) {
        match self.pattern {
            SignalPattern::Sine { freq_hz }
            | SignalPattern::SignalPlusNoise {
                signal_freq: freq_hz,
                ..
            } => {
                self.phase += 2.0 * PI * freq_hz * dt;
                self.phase %= 2.0 * PI;
            }
            _ => {
                // For other patterns, phase tracking is less critical
                self.phase += dt;
                if self.phase > 100.0 {
                    self.phase -= 100.0;
                }
            }
        }
    }

    /// Start continuous streaming to a callback
    ///
    /// # Arguments
    /// * `buffer_size` - Number of samples per buffer
    /// * `callback` - Called with i16 samples when buffer is ready
    pub fn start_streaming<F>(&mut self, buffer_size: usize, callback: F) -> anyhow::Result<()>
    where
        F: Fn(&[i16]) + Send + 'static,
    {
        if self.running.load(Ordering::Relaxed) {
            anyhow::bail!("Already streaming");
        }

        self.running.store(true, Ordering::Relaxed);
        self.start_time = Some(Instant::now());

        let running = Arc::clone(&self.running);
        let xtal_freq = self.xtal_freq;
        let pattern = self.pattern;
        let amplitude = self.amplitude;

        let thread = thread::spawn(move || {
            let mut mock = MockSDR::new(xtal_freq, pattern, amplitude);
            mock.start_time = Some(Instant::now());

            while running.load(Ordering::Relaxed) {
                // Generate buffer
                let buffer = mock.generate_buffer(buffer_size);

                // Call user callback
                callback(&buffer);

                // Small yield to prevent busy-waiting and allow other threads to run
                thread::yield_now();
            }
        });

        self.thread = Some(thread);
        Ok(())
    }

    /// Stop streaming
    pub fn stop_streaming(&mut self) {
        if !self.running.load(Ordering::Relaxed) {
            return;
        }

        self.running.store(false, Ordering::Relaxed);

        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for MockSDR {
    fn drop(&mut self) {
        self.stop_streaming();
    }
}

// Implement SdrDevice for MockSDR so VirtualRadio can use it as backend
impl crate::device::SdrDevice for MockSDR {
    fn set_xtal_freq(&mut self, freq: u32) -> anyhow::Result<()> {
        self.xtal_freq = freq;
        Ok(())
    }

    fn get_xtal_freq(&self) -> u32 {
        self.xtal_freq
    }

    fn set_direct_sampling(&mut self, mode: bool) -> anyhow::Result<()> {
        self.direct_sampling = mode;
        Ok(())
    }

    fn get_direct_sampling(&self) -> bool {
        self.direct_sampling
    }

    fn set_center_freq(&mut self, freq: u64) -> anyhow::Result<()> {
        self.center_freq = freq;
        Ok(())
    }

    fn get_center_freq(&self) -> u64 {
        self.center_freq
    }

    fn set_if_gain(&mut self, gain: f32) -> anyhow::Result<()> {
        self.if_gain = gain;
        Ok(())
    }

    fn get_if_gain(&self) -> f32 {
        self.if_gain
    }

    fn set_rf_gain(&mut self, gain: f32) -> anyhow::Result<()> {
        self.rf_gain = gain;
        Ok(())
    }

    fn get_rf_gain(&self) -> f32 {
        self.rf_gain
    }

    fn get_if_gain_range(&self) -> (f32, f32) {
        // Reuse the library helper for ranges for consistency
        crate::gain::get_if_gain_range(crate::interface::RadioModel::RX888r2, self.direct_sampling)
    }

    fn get_if_gain_steps(&self) -> &'static [f32] {
        crate::gain::get_if_gain_steps(crate::interface::RadioModel::RX888r2, self.direct_sampling)
    }

    fn get_rf_gain_range(&self) -> (f32, f32) {
        crate::gain::get_rf_gain_range(crate::interface::RadioModel::RX888r2, self.direct_sampling)
    }

    fn get_rf_gain_steps(&self) -> &'static [f32] {
        crate::gain::get_rf_gain_steps(crate::interface::RadioModel::RX888r2, self.direct_sampling)
    }

    fn enable_adc_dither(&mut self, _enable: bool) -> anyhow::Result<()> {
        Ok(())
    }

    fn enable_adc_pga(&mut self, _enable: bool) -> anyhow::Result<()> {
        Ok(())
    }

    fn enable_antenna_bias(&mut self, _index: i32, _enable: bool) -> anyhow::Result<()> {
        Ok(())
    }

    fn read_async(
        &mut self,
        cb: Box<dyn Fn(&[i16]) + Send + Sync + 'static>,
    ) -> anyhow::Result<()> {
        // Use the streaming API to simulate async reads
        let cb_box = cb;
        // Increase buffer size to improve throughput for tests that expect higher sample counts.
        // Larger buffers reduce callback overhead and better approximate device burst transfers.
        const BUFFER_SIZE: usize = 65_536; // was 4_096
        self.start_streaming(BUFFER_SIZE, move |data| {
            (cb_box)(data);
        })?;
        Ok(())
    }

    fn read_cancel(&mut self) -> anyhow::Result<()> {
        self.stop_streaming();
        Ok(())
    }

    fn get_model(&self) -> crate::interface::RadioModel {
        crate::interface::RadioModel::RX888r2
    }

    fn get_firmware_version(&self) -> u16 {
        0x0300
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sine_generation() {
        let mut mock = MockSDR::new(
            64_000_000,
            SignalPattern::Sine {
                freq_hz: 1_000_000.0,
            },
            0.8,
        );

        let buffer = mock.generate_buffer(8192);
        assert_eq!(buffer.len(), 8192);

        // Check that we have non-zero data
        let has_nonzero = buffer.iter().any(|&b| b != 0);
        assert!(has_nonzero, "Buffer should contain non-zero samples");

        // Convert back to i16 and check range
        for sample in buffer.iter().take(8192) {
            assert!(
                sample.abs() < i16::MAX,
                "Sample should be within int16 range"
            );
        }
    }

    #[test]
    fn test_multi_tone() {
        const FREQS: &[f32] = &[1_000_000.0, 2_000_000.0, 3_000_000.0];
        let mut mock = MockSDR::new(64_000_000, SignalPattern::MultiTone { freqs: FREQS }, 0.5);

        let buffer = mock.generate_buffer(8192);
        assert_eq!(buffer.len(), 8192);

        // Should have complex waveform
        let has_variation = buffer.windows(4).any(|w| w[0] != w[2] || w[1] != w[3]);
        assert!(has_variation, "Multi-tone should produce varying samples");
    }

    #[test]
    fn test_noise() {
        let mut mock = MockSDR::new(64_000_000, SignalPattern::Noise, 0.3);

        let buffer = mock.generate_buffer(1024);

        // Noise should have high variance
        let samples: Vec<i16> = buffer.clone();

        let mean: f32 = samples.iter().map(|&s| s as f32).sum::<f32>() / samples.len() as f32;
        let variance: f32 = samples
            .iter()
            .map(|&s| {
                let diff = s as f32 - mean;
                diff * diff
            })
            .sum::<f32>()
            / samples.len() as f32;

        assert!(
            variance > 1000.0,
            "Noise should have significant variance, got {}",
            variance
        );
    }

    #[test]
    fn test_am_modulation() {
        let mut mock = MockSDR::new(
            64_000_000,
            SignalPattern::AM {
                carrier_freq: 10_000_000.0,
                mod_freq: 1_000.0,
                mod_depth: 0.5,
            },
            0.7,
        );

        let buffer = mock.generate_buffer(64000); // 1ms of data
        assert_eq!(buffer.len(), 64000);

        // Convert to samples and verify amplitude modulation exists
        let samples: Vec<i16> = buffer.clone();

        // Check for amplitude variation (envelope)
        let max_val = samples.iter().map(|&s| (s as i32).abs()).max().unwrap();
        let min_val = samples.iter().map(|&s| (s as i32).abs()).min().unwrap();

        assert!(
            max_val > min_val * 2,
            "AM signal should show amplitude modulation"
        );
    }

    #[test]
    fn test_sweep() {
        let mut mock = MockSDR::new(
            64_000_000,
            SignalPattern::Sweep {
                start_freq: 1_000_000.0,
                end_freq: 5_000_000.0,
                duration_secs: 1.0,
            },
            0.6,
        );

        let buffer = mock.generate_buffer(8192);
        assert_eq!(buffer.len(), 8192);

        // Should produce valid samples
        let has_data = buffer.iter().any(|&b| b != 0);
        assert!(has_data, "Sweep should produce non-zero samples");
    }

    #[test]
    fn test_signal_plus_noise() {
        let mut mock = MockSDR::new(
            64_000_000,
            SignalPattern::SignalPlusNoise {
                signal_freq: 5_000_000.0,
                snr_db: 10.0,
            },
            0.8,
        );

        let buffer = mock.generate_buffer(8192);
        assert_eq!(buffer.len(), 8192);

        // Convert to samples
        let samples: Vec<i16> = buffer.clone();

        // Should have both signal (periodic) and noise (random)
        let mean: f32 = samples.iter().map(|&s| s as f32).sum::<f32>() / samples.len() as f32;
        assert!(
            mean.abs() < 1000.0,
            "Mean should be near zero for AC signal"
        );
    }
}
