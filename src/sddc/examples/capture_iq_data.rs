// Capture IQ data from VirtualRadio single channel and analyze
// Similar to PScopeShot but for float Complex32 data from virtual_sdr

use anyhow::Result;
use num_complex::Complex32;
use sddc::{Radio, VirtualChannelConfig, VirtualRadio};
use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

fn main() -> Result<()> {
    // Enable debug logging
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,sddc::virtual_sdr=debug"),
    )
    .init();

    println!("=== VirtualRadio IQ Data Capture Tool ===");

    // Open device
    let radio = Radio::open(0)?;
    println!("Device model: {:?}", radio.get_model());
    println!(
        "Firmware version: {}.{}",
        radio.get_firmware_version() >> 8,
        radio.get_firmware_version() & 0xFF
    );

    // Configuration
    const SAMPLE_RATE: u32 = 64_000_000; // 64 MHz physical sample rate
    const CENTER_FREQ: u64 = 7_100_000; // 7.1 MHz (40m band)
    const CHANNEL_FREQ: u64 = 7_074_000; // FT8 frequency
    const DECIMATION: usize = 32; // Output rate = 64MHz / 32 = 2 MHz

    // Create VirtualRadio
    let mut vradio = VirtualRadio::new(radio, SAMPLE_RATE)?;

    // Configure radio
    vradio.set_direct_sampling(true)?;
    vradio.set_center_freq(CENTER_FREQ)?;
    vradio.set_if_gain(0.0)?;
    vradio.set_rf_gain(0.0)?;

    vradio.enable_antenna_bias(0, false)?;

    println!("\nConfiguration:");
    println!("  Physical sample rate: {} Hz", SAMPLE_RATE);
    println!("  Physical center freq: {} Hz", vradio.get_center_freq());
    println!("  Channel center freq: {} Hz", CHANNEL_FREQ);
    println!("  Decimation: {}", DECIMATION);
    println!(
        "  Output sample rate: {} Hz",
        SAMPLE_RATE / DECIMATION as u32
    );
    println!("  Direct sampling: {}", vradio.get_direct_sampling());

    // Capture parameters
    const CAPTURE_SECONDS: f64 = 2.0;
    let output_rate = SAMPLE_RATE / DECIMATION as u32;
    let max_samples = (output_rate as f64 * CAPTURE_SECONDS) as usize;

    let samples_captured = Arc::new(AtomicUsize::new(0));
    let running = Arc::new(AtomicBool::new(true));

    // Storage for captured IQ data
    let iq_data = Arc::new(Mutex::new(Vec::<Complex32>::new()));

    // Create output filenames
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let iq_filename = format!("vradio_iq_{}.dat", timestamp);
    let pscope_filename = format!("vradio_iq_{}.pscope", timestamp);
    let analysis_filename = format!("vradio_iq_{}_analysis.txt", timestamp);

    println!(
        "\nCapturing {:.1} seconds ({} samples)",
        CAPTURE_SECONDS, max_samples
    );
    println!("  IQ data file: {}", iq_filename);
    println!("  PScope file: {}", pscope_filename);
    println!("  Analysis file: {}", analysis_filename);

    let samples_cap_clone = samples_captured.clone();
    let running_clone = running.clone();
    let iq_data_clone = iq_data.clone();
    let callback_count = Arc::new(AtomicUsize::new(0));
    let callback_count_clone = callback_count.clone();

    // Create channel with callback
    let config = VirtualChannelConfig {
        center_freq: CHANNEL_FREQ,
        lsb: false,
        decimation: DECIMATION,
    };

    vradio.create_channel(config, move |_channel_idx, samples| {
        if !running_clone.load(Ordering::Relaxed) {
            return;
        }

        let cb_count = callback_count_clone.fetch_add(1, Ordering::Relaxed);
        let current = samples_cap_clone.fetch_add(samples.len(), Ordering::Relaxed);

        // Debug: print first few samples from first callback
        if cb_count == 0 {
            println!("First callback received! {} samples", samples.len());
            if !samples.is_empty() {
                println!("  First 5 samples:");
                for (i, s) in samples.iter().take(5).enumerate() {
                    println!(
                        "    [{}] I={:.6}, Q={:.6}, mag={:.6}",
                        i,
                        s.re,
                        s.im,
                        s.norm()
                    );
                }
            }
        }

        // Store samples
        if let Ok(mut data) = iq_data_clone.lock() {
            data.extend_from_slice(samples);
        }

        // Progress indicator
        if current > 0 && current.is_multiple_of(output_rate as usize) {
            println!(
                "Captured: {:.1} seconds ({} callbacks so far)",
                current as f64 / output_rate as f64,
                cb_count
            );
        }

        // Stop after target
        if current >= max_samples {
            running_clone.store(false, Ordering::Relaxed);
        }
    })?;

    // Start capture
    println!("\nStarting capture...");
    let start_time = std::time::Instant::now();
    vradio.start()?;

    // Wait for capture to complete
    while running.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(50)); // Check every 50ms
    }

    // Stop capture
    vradio.stop()?;
    let elapsed = start_time.elapsed();

    println!("\nCapture complete!");
    println!("  Duration: {:.2} seconds", elapsed.as_secs_f64());
    println!("  Samples: {}", samples_captured.load(Ordering::Relaxed));
    println!("  Callbacks: {}", callback_count.load(Ordering::Relaxed));

    // Get captured data
    let data = iq_data.lock().unwrap();
    println!("\nProcessing {} samples...", data.len());

    // Write raw IQ data (interleaved I/Q floats)
    write_iq_file(&iq_filename, &data)?;

    // Write PScope format
    write_pscope_file(&pscope_filename, &data, output_rate as f32)?;

    // Analyze data
    analyze_iq_data(&analysis_filename, &data, output_rate as f32)?;

    println!("\nDone! Files created:");
    println!("  {}", iq_filename);
    println!("  {}", pscope_filename);
    println!("  {}", analysis_filename);

    Ok(())
}

/// Write raw IQ data as interleaved float32 I/Q pairs
fn write_iq_file(filename: &str, data: &[Complex32]) -> Result<()> {
    let mut file = File::create(filename)?;

    // Write as binary float32 I/Q pairs
    for sample in data {
        file.write_all(&sample.re.to_le_bytes())?;
        file.write_all(&sample.im.to_le_bytes())?;
    }

    let filesize_mb = (data.len() * 2 * 4) as f64 / 1_048_576.0;
    println!("  Wrote IQ data: {:.2} MB", filesize_mb);

    Ok(())
}

/// Write PScope compatible format
/// Note: PScope expects i16 data, so we'll scale float32 to i16 range
fn write_pscope_file(filename: &str, data: &[Complex32], samplerate: f32) -> Result<()> {
    let mut file = File::create(filename)?;

    let len = if data.len() > 40960 {
        40960
    } else {
        data.len()
    };

    // PScope header
    write!(file, "Version,115\r\n")?;
    write!(
        file,
        "Retainers,0,1,{},1024,0,{:.15},1,1\r\n",
        len * 2,
        samplerate / 1_000_000.0
    )?;
    write!(file, "Placement,44,0,1,-1,-1,-1,-1,88,40,1116,879\r\n")?;
    write!(file, "WindMgr,7,2,0\r\n")?;
    write!(file, "Page,0,2\r\n")?;
    write!(file, "Col,3,1\r\n")?;
    write!(file, "Row,2,1\r\n")?;
    write!(file, "Row,3,146\r\n")?;
    write!(file, "Row,1,319\r\n")?;
    write!(file, "Col,2,1063\r\n")?;
    write!(file, "Row,4,1\r\n")?;
    write!(file, "Row,0,319\r\n")?;
    write!(file, "Page,1,2\r\n")?;
    write!(file, "Col,1,1\r\n")?;
    write!(file, "Row,1,1\r\n")?;
    write!(file, "Col,2,425\r\n")?;
    write!(file, "Row,4,1\r\n")?;
    write!(file, "Row,0,319\r\n")?;
    write!(
        file,
        "DemoID,VirtualRadio IQ Capture,I and Q channels,0\r\n"
    )?;
    write!(
        file,
        "RawData,1,{},16,-32768,32767,{:.15},-3.276800e+04,3.276800e+04\r\n",
        len * 2,
        samplerate / 1_000_000.0
    )?;

    // Write I/Q samples as i16 (scaled from float -1.0..1.0 to -32768..32767)
    let mut c = 0;
    for sample in data {
        let i_scaled = (sample.re * 32767.0).clamp(-32768.0, 32767.0) as i16;
        let q_scaled = (sample.im * 32767.0).clamp(-32768.0, 32767.0) as i16;
        write!(file, "{}\r\n", i_scaled)?;
        write!(file, "{}\r\n", q_scaled)?;
        c += 1;
        if c >= len {
            break;
        }
    }

    write!(file, "END\r\n")?;

    println!("  Wrote PScope file");

    Ok(())
}

/// Analyze IQ data and write statistics
fn analyze_iq_data(filename: &str, data: &[Complex32], samplerate: f32) -> Result<()> {
    let mut file = File::create(filename)?;

    writeln!(file, "=== VirtualRadio IQ Data Analysis ===")?;
    writeln!(file)?;
    writeln!(file, "Sample Count: {}", data.len())?;
    writeln!(file, "Sample Rate: {} Hz", samplerate)?;
    writeln!(
        file,
        "Duration: {:.6} seconds",
        data.len() as f32 / samplerate
    )?;
    writeln!(file)?;

    // Calculate statistics
    let mut i_sum = 0.0_f64;
    let mut q_sum = 0.0_f64;
    let mut i_sum_sq = 0.0_f64;
    let mut q_sum_sq = 0.0_f64;
    let mut mag_sum = 0.0_f64;
    let mut mag_sum_sq = 0.0_f64;
    let mut i_min = f32::MAX;
    let mut i_max = f32::MIN;
    let mut q_min = f32::MAX;
    let mut q_max = f32::MIN;
    let mut mag_max = 0.0_f32;

    for sample in data {
        let i = sample.re as f64;
        let q = sample.im as f64;
        let mag = sample.norm() as f64;

        i_sum += i;
        q_sum += q;
        i_sum_sq += i * i;
        q_sum_sq += q * q;
        mag_sum += mag;
        mag_sum_sq += mag * mag;

        i_min = i_min.min(sample.re);
        i_max = i_max.max(sample.re);
        q_min = q_min.min(sample.im);
        q_max = q_max.max(sample.im);
        mag_max = mag_max.max(sample.norm());
    }

    let n = data.len() as f64;
    let i_mean = i_sum / n;
    let q_mean = q_sum / n;
    let i_rms = (i_sum_sq / n).sqrt();
    let q_rms = (q_sum_sq / n).sqrt();
    let mag_mean = mag_sum / n;
    let mag_rms = (mag_sum_sq / n).sqrt();

    let i_variance = (i_sum_sq / n) - (i_mean * i_mean);
    let q_variance = (q_sum_sq / n) - (q_mean * q_mean);
    let i_stddev = i_variance.sqrt();
    let q_stddev = q_variance.sqrt();

    writeln!(file, "I Channel Statistics:")?;
    writeln!(file, "  Mean:   {:.6}", i_mean)?;
    writeln!(file, "  RMS:    {:.6}", i_rms)?;
    writeln!(file, "  StdDev: {:.6}", i_stddev)?;
    writeln!(file, "  Min:    {:.6}", i_min)?;
    writeln!(file, "  Max:    {:.6}", i_max)?;
    writeln!(file, "  Peak-to-Peak: {:.6}", i_max - i_min)?;
    writeln!(file)?;

    writeln!(file, "Q Channel Statistics:")?;
    writeln!(file, "  Mean:   {:.6}", q_mean)?;
    writeln!(file, "  RMS:    {:.6}", q_rms)?;
    writeln!(file, "  StdDev: {:.6}", q_stddev)?;
    writeln!(file, "  Min:    {:.6}", q_min)?;
    writeln!(file, "  Max:    {:.6}", q_max)?;
    writeln!(file, "  Peak-to-Peak: {:.6}", q_max - q_min)?;
    writeln!(file)?;

    writeln!(file, "Magnitude Statistics:")?;
    writeln!(file, "  Mean: {:.6}", mag_mean)?;
    writeln!(file, "  RMS:  {:.6}", mag_rms)?;
    writeln!(file, "  Max:  {:.6}", mag_max)?;
    writeln!(file)?;

    // DC offset analysis
    writeln!(file, "DC Offset Analysis:")?;
    writeln!(
        file,
        "  I DC offset: {:.6} ({:.2}%)",
        i_mean,
        (i_mean / i_rms) * 100.0
    )?;
    writeln!(
        file,
        "  Q DC offset: {:.6} ({:.2}%)",
        q_mean,
        (q_mean / q_rms) * 100.0
    )?;
    writeln!(file)?;

    // IQ balance
    let iq_gain_imbalance = (i_rms / q_rms - 1.0) * 100.0;
    writeln!(file, "IQ Balance:")?;
    writeln!(file, "  I/Q Gain Imbalance: {:.3}%", iq_gain_imbalance)?;
    writeln!(file)?;

    // Power analysis
    let power_dbfs = 20.0 * mag_rms.log10();
    let peak_dbfs = 20.0 * (mag_max as f64).log10();
    writeln!(file, "Power Analysis:")?;
    writeln!(file, "  RMS Power: {:.2} dBFS", power_dbfs)?;
    writeln!(file, "  Peak Power: {:.2} dBFS", peak_dbfs)?;
    writeln!(file, "  Crest Factor: {:.2} dB", peak_dbfs - power_dbfs)?;
    writeln!(file)?;

    // Sample distribution (histogram)
    writeln!(file, "Sample Distribution (magnitude):")?;
    let mut histogram = [0usize; 10];
    for sample in data {
        let mag = sample.norm();
        let bin = ((mag / mag_max) * 9.99).floor() as usize;
        histogram[bin.min(9)] += 1;
    }
    for (i, count) in histogram.iter().enumerate() {
        let range_start = (i as f32 / 10.0) * mag_max;
        let range_end = ((i + 1) as f32 / 10.0) * mag_max;
        let percentage = (*count as f64 / n) * 100.0;
        writeln!(
            file,
            "  {:.3} - {:.3}: {:6} samples ({:5.2}%) {}",
            range_start,
            range_end,
            count,
            percentage,
            "#".repeat((percentage / 2.0) as usize)
        )?;
    }
    writeln!(file)?;

    println!("  Wrote analysis file");
    println!("\n--- Quick Analysis Summary ---");
    println!("  I RMS: {:.6}, Q RMS: {:.6}", i_rms, q_rms);
    println!("  DC Offset: I={:.6}, Q={:.6}", i_mean, q_mean);
    println!("  IQ Gain Imbalance: {:.3}%", iq_gain_imbalance);
    println!("  RMS Power: {:.2} dBFS", power_dbfs);

    Ok(())
}
