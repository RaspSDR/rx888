// Tuner Scanner - Scan frequency range and report PLL lock status
// This tool scans the tuner across frequencies from 50MHz to 5GHz in 1MHz increments
// and reports which frequencies can lock the PLL with harmonic mode info

use anyhow::Result;
use clap::Parser;
use sddc::Radio;
use std::time::Instant;

/// Tuner Scanner - Test PLL lock status across frequency range
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Start frequency in MHz (default: 50)
    #[arg(short, long, default_value_t = 50)]
    start: u32,

    /// End frequency in MHz (default: 5000)
    #[arg(short, long, default_value_t = 5000)]
    end: u32,

    /// Timeout in milliseconds to wait for PLL lock (default: 100)
    #[arg(short, long, default_value_t = 1000)]
    timeout: u64,

    /// Number of retries to detect lock (default: 5)
    #[arg(short, long, default_value_t = 5)]
    retries: u32,

    /// Delay in milliseconds between retries (default: 50)
    #[arg(short, long, default_value_t = 50)]
    delay: u64,
}

#[derive(Debug, Clone)]
struct FreqResult {
    freq_mhz: u32,
    #[allow(dead_code)]
    locked: bool,
    harmonic: bool,
    lock_time_ms: u64,
}

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .try_init()
        .ok();

    let args = Args::parse();

    println!("=== RX888 Tuner Scanner ===");
    println!(
        "Scanning frequency range from {} MHz to {} MHz",
        args.start, args.end
    );
    println!(
        "Lock timeout: {} ms, Retries: {}, Retry delay: {} ms\n",
        args.timeout, args.retries, args.delay
    );

    // Open device
    let mut radio = Radio::open(0)?;
    println!("Device model: {:?}", radio.get_model());
    println!(
        "Firmware version: {}.{}",
        radio.get_firmware_version() >> 8,
        radio.get_firmware_version() & 0xFF
    );

    // Switch to tuner mode
    radio.set_direct_sampling(false)?;
    println!("Switched to tuner mode\n");

    // Configure gains (reasonable defaults)
    radio.set_if_gain(0.0)?;
    radio.set_rf_gain(0.0)?;

    // read async
    radio.read_async(move |_data: &[i16]| {
        // No-op for scanning
    })?;

    // Scan frequencies
    let mut results = Vec::new();
    let total_freqs = args.end - args.start + 1;
    let mut count = 0;

    for freq_mhz in args.start..=args.end {
        count += 1;
        if count % 100 == 0 {
            print!("\rScanning... {}/{}", count, total_freqs);
            std::io::Write::flush(&mut std::io::stdout())?;
        }

        let freq_hz = (freq_mhz as u64) * 1_000_000;
        radio.set_center_freq(freq_hz + 4_570_000)?;

        // Try to detect lock with retries
        let start_time = Instant::now();
        let mut locked = false;
        let mut harmonic = false;
        let mut lock_time_ms = 0u64;

        for attempt in 0..args.retries {
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_millis(args.delay));
            }

            match radio.get_tuner_status() {
                Ok((pll_locked, harmonic0)) => {
                    if pll_locked {
                        lock_time_ms = start_time.elapsed().as_millis() as u64;
                        locked = true;
                        harmonic = harmonic0; // Use rf_overload as harmonic indicator
                        break;
                    }
                }
                Err(_) => {
                    // Continue retrying on error
                }
            }

            if start_time.elapsed().as_millis() as u64 > args.timeout {
                locked = false;
                break;
            }
        }

        if locked {
            results.push(FreqResult {
                freq_mhz,
                locked,
                harmonic,
                lock_time_ms,
            });
            log::info!(
                "Frequency {} MHz: PLL Locked (Harmonic: {}, Lock Time: {} ms)",
                freq_mhz,
                harmonic,
                lock_time_ms
            );
        } else {
            log::info!("Frequency {} MHz: PLL Not Locked till timeout", freq_mhz);
        }
    }

    radio.read_cancel()?;
    println!("\rScanning... {}/{}\n", total_freqs, total_freqs);

    // Process results and combine continuous ranges
    let ranges = combine_ranges(&results);

    // Display results
    println!("=== Results ===\n");
    println!(
        "{:<15} {:<15} {:<12} {:<15}",
        "Frequency Range", "Count", "Harmonic", "Avg Lock Time"
    );
    println!("{}", "=".repeat(60));

    for range in &ranges {
        let freq_range = if range.start_mhz == range.end_mhz {
            format!("{} MHz", range.start_mhz)
        } else {
            format!("{} - {} MHz", range.start_mhz, range.end_mhz)
        };

        let harmonic_str = if range.all_harmonic {
            "Yes"
        } else if range.any_harmonic {
            "Mixed"
        } else {
            "No"
        };

        let avg_lock_time = if range.lock_times.is_empty() {
            0u64
        } else {
            range.lock_times.iter().sum::<u64>() / range.lock_times.len() as u64
        };

        println!(
            "{:<15} {:<15} {:<12} {:<15}",
            freq_range,
            range.count,
            harmonic_str,
            format!("{}ms", avg_lock_time)
        );
    }

    println!("\n{:<15}", "=".repeat(60));
    let total_locked = results.len();
    let total_scanned = (args.end - args.start + 1) as usize;
    println!("\nSummary:");
    println!("  Total frequencies scanned: {}", total_scanned);
    println!("  Frequencies with PLL lock: {}", total_locked);
    println!(
        "  Lock percentage: {:.1}%",
        (total_locked as f32 / total_scanned as f32) * 100.0
    );

    Ok(())
}

#[derive(Debug)]
struct FreqRange {
    start_mhz: u32,
    end_mhz: u32,
    count: u32,
    all_harmonic: bool,
    any_harmonic: bool,
    lock_times: Vec<u64>,
}

fn combine_ranges(results: &[FreqResult]) -> Vec<FreqRange> {
    if results.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut current_start = results[0].freq_mhz;
    let mut current_end = results[0].freq_mhz;
    let mut harmonic_count = if results[0].harmonic { 1 } else { 0 };
    let mut lock_times = vec![results[0].lock_time_ms];

    for item in results.iter().skip(1) {
        // Check if frequencies are continuous (within 1 MHz)
        if item.freq_mhz - current_end == 1 {
            // Extend current range
            current_end = item.freq_mhz;
            if item.harmonic {
                harmonic_count += 1;
            }
            lock_times.push(item.lock_time_ms);
        } else {
            // Save current range and start new one
            let count = (current_end - current_start) + 1;
            ranges.push(FreqRange {
                start_mhz: current_start,
                end_mhz: current_end,
                count,
                all_harmonic: harmonic_count == count as usize,
                any_harmonic: harmonic_count > 0,
                lock_times: lock_times.clone(),
            });

            // Start new range
            current_start = item.freq_mhz;
            current_end = item.freq_mhz;
            harmonic_count = if item.harmonic { 1 } else { 0 };
            lock_times = vec![item.lock_time_ms];
        }
    }

    // Add last range
    let count = (current_end - current_start) + 1;
    ranges.push(FreqRange {
        start_mhz: current_start,
        end_mhz: current_end,
        count,
        all_harmonic: harmonic_count == count as usize,
        any_harmonic: harmonic_count > 0,
        lock_times,
    });

    ranges
}
