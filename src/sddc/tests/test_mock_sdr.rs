use sddc::SdrDevice;
/// Comprehensive tests for MockSDR signal generator
///
/// These tests verify that MockSDR correctly generates various signal patterns
use sddc::mock_sdr::{MockSDR, SignalPattern};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// No longer needed: buffers are now Vec<i16>

#[test]
fn test_mock_sdr_sine_wave() {
    // Test that MockSDR generates a proper sine wave
    let mut mock = MockSDR::new(
        64_000_000,
        SignalPattern::Sine {
            freq_hz: 1_000_000.0,
        },
        0.8,
    );

    let buffer = mock.generate_buffer(8192);
    assert_eq!(buffer.len(), 8192);

    let samples = buffer;

    // Check that signal has expected amplitude
    let max_sample = samples.iter().map(|&s| (s as i32).abs()).max().unwrap();
    assert!(
        max_sample > 20000,
        "Sine wave should have significant amplitude: {}",
        max_sample
    );

    // Check that signal oscillates (has zero crossings)
    let zero_crossings = samples
        .windows(2)
        .filter(|w| (w[0] > 0) != (w[1] > 0))
        .count();
    assert!(
        zero_crossings > 10,
        "Sine wave should have multiple zero crossings: {}",
        zero_crossings
    );
}

#[test]
fn test_mock_sdr_multi_tone() {
    const FREQS: &[f32] = &[1_000_000.0, 2_500_000.0, 5_000_000.0];
    let mut mock = MockSDR::new(64_000_000, SignalPattern::MultiTone { freqs: FREQS }, 0.7);

    let buffer = mock.generate_buffer(16384);

    let samples: Vec<f32> = buffer.into_iter().map(|s| s as f32).collect();

    // Multi-tone should have complex spectrum
    let mean: f32 = samples.iter().sum::<f32>() / samples.len() as f32;
    assert!(
        mean.abs() < 1000.0,
        "Multi-tone signal should have near-zero mean"
    );

    // Check dynamic range
    let max_val = samples.iter().map(|&s| s.abs()).fold(0.0_f32, f32::max);
    assert!(max_val > 5000.0, "Multi-tone should have sufficient power");
}

#[test]
fn test_mock_sdr_noise() {
    let mut mock = MockSDR::new(64_000_000, SignalPattern::Noise, 0.5);

    let buffer = mock.generate_buffer(8192);

    let samples: Vec<f32> = buffer.into_iter().map(|s| s as f32).collect();

    let mean: f32 = samples.iter().sum::<f32>() / samples.len() as f32;
    let variance: f32 = samples
        .iter()
        .map(|&s| {
            let diff = s - mean;
            diff * diff
        })
        .sum::<f32>()
        / samples.len() as f32;
    let stddev = variance.sqrt();

    // Noise should have near-zero mean
    assert!(
        mean.abs() < 500.0,
        "Noise mean should be near zero: {}",
        mean
    );

    // Noise should have significant variance
    assert!(
        stddev > 500.0,
        "Noise should have high standard deviation: {}",
        stddev
    );
}

#[test]
fn test_signal_quality_metrics() {
    // Test that we can measure signal quality with mock data
    let mut mock = MockSDR::new(
        64_000_000,
        SignalPattern::SignalPlusNoise {
            signal_freq: 5_000_000.0,
            snr_db: 20.0,
        },
        0.8,
    );

    let buffer = mock.generate_buffer(65536);

    let samples: Vec<f32> = buffer.into_iter().map(|s| s as f32).collect();

    // Compute power
    let power: f32 = samples.iter().map(|&s| s * s).sum::<f32>() / samples.len() as f32;
    let rms = power.sqrt();

    assert!(rms > 1000.0, "Signal should have measurable power");

    // Check for dynamic range
    let max_val = samples.iter().map(|&s| s.abs()).fold(0.0_f32, f32::max);
    let crest_factor = max_val / rms;

    assert!(
        crest_factor > 1.0 && crest_factor < 10.0,
        "Signal should have reasonable crest factor"
    );
}

#[test]
fn test_streaming_callback() {
    // Test that streaming mode works correctly
    let mock = MockSDR::new(
        64_000_000,
        SignalPattern::Sine {
            freq_hz: 1_000_000.0,
        },
        0.8,
    );

    let buffer_count = Arc::new(Mutex::new(0));
    let count_clone = Arc::clone(&buffer_count);

    let mut mock_streaming = mock;
    mock_streaming
        .start_streaming(8192, move |buffer| {
            assert_eq!(buffer.len(), 8192);
            let mut count = count_clone.lock().unwrap();
            *count += 1;
        })
        .unwrap();

    // Let it run for a bit
    thread::sleep(Duration::from_millis(100));

    mock_streaming.stop_streaming();

    let final_count = *buffer_count.lock().unwrap();
    assert!(
        final_count >= 5,
        "Should have received at least 5 buffers: {}",
        final_count
    );
}

#[test]
fn test_frequency_sweep_detection() {
    // Test that we can detect a frequency sweep
    let mut mock = MockSDR::new(
        64_000_000,
        SignalPattern::Sweep {
            start_freq: 1_000_000.0,
            end_freq: 10_000_000.0,
            duration_secs: 0.5,
        },
        0.7,
    );

    // Generate 0.5 seconds worth of data
    let samples_needed = (64_000_000.0 * 0.5) as usize;
    let buffer = mock.generate_buffer(samples_needed);

    assert_eq!(buffer.len(), samples_needed);

    // Verify sweep characteristics by checking increasing zero-crossings per segment
    let samples = buffer;
    let segments = 4;
    let segment_size = samples.len() / segments;
    let mut crossings_vec = Vec::new();
    for i in 0..segments {
        let start = i * segment_size;
        let end = (i + 1) * segment_size;
        let segment = &samples[start..end];
        let crossings = segment
            .windows(2)
            .filter(|w| (w[0] > 0) != (w[1] > 0))
            .count();
        crossings_vec.push(crossings);
    }

    let avg_first = (crossings_vec[0] as f64 + crossings_vec[1] as f64) / 2.0;
    let avg_last = (crossings_vec[2] as f64 + crossings_vec[3] as f64) / 2.0;
    assert!(
        avg_last > avg_first,
        "avg zero crossings should increase from first half to last half: {:?}",
        crossings_vec
    );
}

#[test]
fn test_continuous_signal_generation() {
    // Test that multiple buffer generations maintain phase continuity
    let mut mock = MockSDR::new(
        64_000_000,
        SignalPattern::Sine {
            freq_hz: 1_000_000.0,
        },
        0.8,
    );

    // Generate multiple buffers
    let mut all_samples = Vec::new();
    for _ in 0..5 {
        let buffer = mock.generate_buffer(1024);
        all_samples.extend(buffer);
    }

    // Check that we have a continuous signal across buffer boundaries
    let zero_crossings = all_samples
        .windows(2)
        .filter(|w| (w[0] > 0) != (w[1] > 0))
        .count();

    // Just check that we have a reasonable number of zero crossings
    // indicating continuous signal generation
    assert!(
        zero_crossings > 100,
        "Should have many zero crossings indicating continuous signal: {}",
        zero_crossings
    );
}

#[test]
fn test_am_signal_envelope() {
    let mut mock = MockSDR::new(
        64_000_000,
        SignalPattern::AM {
            carrier_freq: 10_000_000.0,
            mod_freq: 1_000.0,
            mod_depth: 0.5,
        },
        0.7,
    );

    let buffer = mock.generate_buffer(64000); // a short burst
    let samples = buffer;

    // Compute coarse envelope by max per chunk
    let mut envelope = Vec::new();
    for chunk in samples.chunks(64) {
        let max_in_chunk = chunk.iter().map(|&s| (s as i32).abs()).max().unwrap();
        envelope.push(max_in_chunk as f32);
        if envelope.len() >= 500 {
            break;
        }
    }

    let env_max = *envelope
        .iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();
    let env_min = *envelope
        .iter()
        .min_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();
    let modulation_index = (env_max - env_min) / (env_max + env_min);

    // Modulation index should be within a reasonable range
    assert!(
        modulation_index > 0.15 && modulation_index < 0.85,
        "modulation_index out of expected range: {}",
        modulation_index
    );
}

#[test]
fn test_different_amplitudes() {
    // Test signals at different amplitude settings
    for amplitude in [0.1, 0.5, 0.9] {
        let mut mock = MockSDR::new(
            64_000_000,
            SignalPattern::Sine {
                freq_hz: 5_000_000.0,
            },
            amplitude,
        );

        let buffer = mock.generate_buffer(1024);
        let samples = buffer;

        let max_sample = samples.iter().map(|&s| (s as i32).abs()).max().unwrap();
        let expected_max = (amplitude * i16::MAX as f32) as i32;

        // Should be within 10% of expected
        let tolerance = (expected_max as f32 * 0.1) as i32;
        assert!(
            (max_sample - expected_max).abs() < tolerance,
            "Amplitude {} should produce max sample near {}",
            amplitude,
            expected_max
        );
    }
}

#[test]
fn test_amplitude_clamp_and_zero_length() {
    // Amplitude > 1.0 clamps to 1.0
    let mut mock = MockSDR::new(
        64_000_000,
        SignalPattern::Sine {
            freq_hz: 1_000_000.0,
        },
        1.5,
    );
    let buf_high = mock.generate_buffer(1024);
    let max_sample = buf_high
        .iter()
        .map(|s| s.unsigned_abs() as i32)
        .max()
        .unwrap();
    assert!(max_sample <= i16::MAX as i32);

    // Amplitude < 0 clamps to 0.0 -> near zero samples
    let mut mock_zero = MockSDR::new(
        64_000_000,
        SignalPattern::Sine {
            freq_hz: 1_000_000.0,
        },
        -0.3,
    );
    let buf_zero = mock_zero.generate_buffer(256);
    let all_zero = buf_zero.iter().all(|&s| s == 0);
    assert!(all_zero, "Amplitude clamp to 0 should yield all zeros");
    assert_eq!(buf_zero.len(), 256);

    // Zero-length buffer request returns empty
    let empty = mock.generate_buffer(0);
    assert_eq!(empty.len(), 0);
}

#[test]
fn test_streaming_double_start_and_stop_idempotent() {
    let mut mock = MockSDR::new(
        64_000_000,
        SignalPattern::Sine {
            freq_hz: 2_000_000.0,
        },
        0.5,
    );

    mock.start_streaming(2048, |_b| {}).unwrap();

    // Starting again should error
    let err = mock.start_streaming(2048, |_b| {});
    assert!(err.is_err(), "second start_streaming should error");

    // Stop is idempotent
    mock.stop_streaming();
    mock.stop_streaming();
}

#[test]
fn test_read_async_and_cancel() {
    let mut mock = MockSDR::new(
        64_000_000,
        SignalPattern::Sine {
            freq_hz: 1_000_000.0,
        },
        0.8,
    );

    let count = Arc::new(Mutex::new(0usize));
    let count_clone = Arc::clone(&count);

    mock.read_async(Box::new(move |_data| {
        let mut c = count_clone.lock().unwrap();
        *c += 1;
    }))
    .unwrap();

    thread::sleep(Duration::from_millis(50));
    mock.read_cancel().unwrap();

    let before = *count.lock().unwrap();
    thread::sleep(Duration::from_millis(20));
    let after = *count.lock().unwrap();

    // After cancel, callbacks should not increase significantly
    assert!(after <= before + 1);
}

#[test]
fn test_trait_setters_and_getters() {
    let mut mock = MockSDR::new(64_000_000, SignalPattern::Noise, 0.3);

    // xtal
    mock.set_xtal_freq(100_000_000).unwrap();
    assert_eq!(mock.get_xtal_freq(), 100_000_000);

    // direct sampling
    mock.set_direct_sampling(false).unwrap();
    assert!(!mock.get_direct_sampling());

    // center freq
    mock.set_center_freq(28_200_000).unwrap();
    assert_eq!(mock.get_center_freq(), 28_200_000);

    // gains
    mock.set_if_gain(12.5).unwrap();
    assert_eq!(mock.get_if_gain(), 12.5);

    mock.set_rf_gain(7.0).unwrap();
    assert_eq!(mock.get_rf_gain(), 7.0);

    // Ranges and steps should be non-empty / sensible
    let (if_min, if_max) = mock.get_if_gain_range();
    assert!(if_max >= if_min);
    assert!(!mock.get_if_gain_steps().is_empty());

    let (rf_min, rf_max) = mock.get_rf_gain_range();
    assert!(rf_max >= rf_min);
    assert!(!mock.get_rf_gain_steps().is_empty());
}
