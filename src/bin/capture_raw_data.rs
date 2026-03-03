// Capture raw ADC data from RX888 for testing and analysis
use anyhow::Result;
use sddc::Radio;
use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

fn main() -> Result<()> {
    println!("=== RX888 Raw Data Capture Tool ===");

    // Open device
    let mut radio = Radio::open(0)?;
    println!("Device model: {:?}", radio.get_model());
    println!(
        "Firmware version: {}.{}",
        radio.get_firmware_version() >> 8,
        radio.get_firmware_version() & 0xFF
    );

    // Configure for typical HF operation
    const SAMPLE_RATE: u32 = 64_000_000; // 64 MHz
    radio.set_xtal_freq(SAMPLE_RATE)?;
    radio.set_direct_sampling(true)?;
    radio.set_center_freq(7_100_000)?; // 7.1 MHz (40m band)
    radio.set_if_gain(30.0)?;
    radio.set_rf_gain(10.0)?;

    // Enable HF bias
    radio.enable_antenna_bias(0, true)?; // 0 = Direct/HF bias

    println!("\nConfiguration:");
    println!("  Sample rate: {} Hz", SAMPLE_RATE);
    println!("  Center freq: {} Hz", radio.get_center_freq());
    println!("  Direct sampling: {}", radio.get_direct_sampling());
    println!("  IF gain: {:.1} dB", radio.get_if_gain());
    println!("  RF gain: {:.1} dB", radio.get_rf_gain());
    println!("  HF antenna bias: enabled");

    // Capture parameters
    const CAPTURE_SECONDS: u64 = 2;
    const MAX_SAMPLES: usize = (SAMPLE_RATE as u64 * CAPTURE_SECONDS) as usize; // 2 seconds at 64 MHz

    let samples_captured = Arc::new(AtomicUsize::new(0));
    let running = Arc::new(AtomicBool::new(true));

    // Create output file
    let filename = format!(
        "rx888_capture_{}.raw",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );
    let file = Arc::new(std::sync::Mutex::new(File::create(&filename)?));

    println!("\nCapturing {} seconds to: {}", CAPTURE_SECONDS, filename);
    println!(
        "Expected file size: ~{:.1} MB",
        (MAX_SAMPLES * 2) as f64 / 1_048_576.0
    );

    let samples_cap_clone = samples_captured.clone();
    let running_clone = running.clone();
    let file_clone = file.clone();

    // Start capture
    let start_time = std::time::Instant::now();
    radio.read_async(Box::new(move |data: Option<&[i16]>| {
        if !running_clone.load(Ordering::Relaxed) {
            return;
        }

        if data.is_none() {
            // Error callback - stop the capture
            println!("\n⚠ USB transfer error detected, stopping capture");
            running_clone.store(false, Ordering::Relaxed);
            return;
        }

        let data = data.unwrap();

        let current = samples_cap_clone.fetch_add(std::mem::size_of_val(data), Ordering::Relaxed);

        // Write to file (convert i16 to bytes)
        if let Ok(mut f) = file_clone.lock() {
            let bytes: &[u8] =
                unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 2) };
            f.write_all(bytes).ok();
        }

        // Progress
        if current % 10_000_000 < data.len() {
            let mb = current as f64 / 1_048_576.0;
            println!("Captured: {:.1} MB ({} samples)", mb, current / 2);
        }

        // Stop after target
        if current >= MAX_SAMPLES * std::mem::size_of::<i16>() {
            running_clone.store(false, Ordering::Relaxed);
        }
    }))?;

    // Wait for capture to complete
    while running.load(Ordering::Relaxed) && start_time.elapsed().as_secs() < CAPTURE_SECONDS + 1 {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    running.store(false, Ordering::Relaxed);

    for _ in 0..5 {
        if !running.load(Ordering::Relaxed) {
            break;
        }
        println!("Waiting for capture to stop...");
        std::thread::sleep(std::time::Duration::from_millis(40));
    }

    radio.read_cancel()?;

    let total = samples_captured.load(Ordering::Relaxed);
    let duration = start_time.elapsed().as_secs_f64();

    println!("\n=== Capture Complete ===");
    println!(
        "Total bytes: {} ({:.1} MB)",
        total,
        total as f64 / 1_048_576.0
    );
    println!("Duration: {:.2} seconds", duration);
    println!(
        "Effective rate: {:.1} MSps",
        (total / 2) as f64 / duration / 1_000_000.0
    );
    println!("File: {}", filename);

    // Compute quick statistics
    drop(file);
    let data = std::fs::read(&filename)?;
    let mut sum: i64 = 0;
    let mut sum_sq: i64 = 0;
    let mut min_val: i16 = i16::MAX;
    let mut max_val: i16 = i16::MIN;
    let mut count: usize = 0;

    for chunk in data.chunks_exact(2) {
        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
        sum += sample as i64;
        sum_sq += (sample as i64) * (sample as i64);
        min_val = min_val.min(sample);
        max_val = max_val.max(sample);
        count += 1;
    }

    let mean = sum as f64 / count as f64;
    let variance = (sum_sq as f64 / count as f64) - (mean * mean);
    let std_dev = variance.sqrt();
    let rms = (sum_sq as f64 / count as f64).sqrt();

    // Normalize to [-1, 1] range
    let rms_norm = rms / 32768.0;
    let dbfs = 20.0 * rms_norm.log10();

    println!("\n=== Statistics ===");
    println!("Samples: {}", count);
    println!("Mean: {:.2} (DC offset: {:.2} LSB)", mean, mean);
    println!("Std dev: {:.2}", std_dev);
    println!("Min: {}, Max: {}", min_val, max_val);
    println!("Peak-to-peak: {} LSB", max_val - min_val);
    println!("RMS (raw): {:.2} LSB", rms);
    println!("RMS (normalized): {:.6}", rms_norm);
    println!("Power: {:.2} dBFS", dbfs);

    // CRITICAL ANALYSIS: The issue is DC offset!
    println!("\n=== ANALYSIS ===");
    if mean.abs() > 10.0 {
        println!("⚠ WARNING: Large DC offset detected: {:.2} LSB", mean);
        println!("  This will inflate RMS and power measurements!");
        println!("  For AC-coupled noise floor, we should remove DC offset.");

        // Recalculate without DC offset (AC-coupled)
        let mut sum_sq_ac: i64 = 0;
        for chunk in data.chunks_exact(2) {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            let ac_sample = sample as f64 - mean;
            sum_sq_ac += (ac_sample as i64) * (ac_sample as i64);
        }
        let rms_ac = (sum_sq_ac as f64 / count as f64).sqrt();
        let rms_ac_norm = rms_ac / 32768.0;
        let dbfs_ac = 20.0 * rms_ac_norm.log10();

        println!("\n=== AC-Coupled Statistics (DC removed) ===");
        println!("RMS (AC, raw): {:.2} LSB", rms_ac);
        println!("RMS (AC, normalized): {:.6}", rms_ac_norm);
        println!("Power (AC): {:.2} dBFS", dbfs_ac);
        println!(
            "DC component power: {:.2} dBFS",
            20.0 * (mean.abs() / 32768.0).log10()
        );
        println!("\n✓ AC-coupled measurement is the true noise floor!");

        // Histogram of sample distribution
        println!("\n=== Sample Distribution (first 500k samples) ===");
        let mut histogram = [0u32; 20];
        let hist_samples = 500_000.min(count);
        for i in 0..hist_samples {
            let chunk_start = i * 2;
            if chunk_start + 1 < data.len() {
                let sample = i16::from_le_bytes([data[chunk_start], data[chunk_start + 1]]);
                let bin =
                    ((sample - min_val) as usize * 20 / (max_val - min_val + 1) as usize).min(19);
                histogram[bin] += 1;
            }
        }

        for (i, &count_bin) in histogram.iter().enumerate() {
            let bin_min = min_val + ((max_val - min_val) as i32 * i as i32 / 20) as i16;
            let bin_max = min_val + ((max_val - min_val) as i32 * (i + 1) as i32 / 20) as i16;
            let bar_len = (count_bin as f64 / hist_samples as f64 * 75.0) as usize;
            let bar = "█".repeat(bar_len);
            println!("[{:4}..{:4}]: {} ({})", bin_min, bin_max, bar, count_bin);
        }
    } else {
        println!("✓ DC offset is minimal: {:.2} LSB", mean);
        println!("  RMS measurement is accurate for noise floor.");
    }

    Ok(())
}
