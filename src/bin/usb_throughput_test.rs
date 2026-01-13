// USB Throughput Test - Check for data loss
// This tool measures actual USB throughput and detects dropped samples

use anyhow::Result;
use clap::Parser;
use sddc::Radio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// USB Throughput Test - Check for data loss and measure actual USB performance
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Sample rate in MHz (e.g., 32, 64, 122.88)
    #[arg(short, long, default_value_t = 122.88f32)]
    samplerate: f32,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("=== RX888 USB Throughput Test ===");
    println!("This tool checks for USB data loss and throughput issues\n");

    // Open device
    let mut radio = Radio::open(0)?;
    println!("Device model: {:?}", radio.get_model());
    println!(
        "Firmware version: {}.{}",
        radio.get_firmware_version() >> 8,
        radio.get_firmware_version() & 0xFF
    );

    // Configure for specified data rate
    let samplerate = (args.samplerate * 1_000_000.0) as u32; // Convert MHz to Hz
    radio.set_xtal_freq(samplerate)?;
    radio.set_direct_sampling(true)?;
    radio.set_center_freq(7_100_000)?;
    radio.set_if_gain(0.0)?;
    radio.set_rf_gain(0.0)?;

    // Disable PGA to reduce noise
    radio.enable_adc_pga(false)?;
    radio.enable_adc_dither(true)?;

    println!("\nConfiguration:");
    println!("  Sample rate: {} MHz", samplerate / 1_000_000);
    println!(
        "  Expected throughput: {:.2} MB/s (16-bit samples)",
        samplerate as f64 * 2.0 / 1_000_000.0
    );

    // Test parameters
    const TEST_DURATION_SECS: u64 = 10;

    let bytes_received = Arc::new(AtomicU64::new(0));
    let packets_received = Arc::new(AtomicU64::new(0));
    let running = Arc::new(AtomicBool::new(true));
    let first_callback_time: Arc<std::sync::Mutex<Option<Instant>>> =
        Arc::new(std::sync::Mutex::new(None));

    let bytes_clone = bytes_received.clone();
    let packets_clone = packets_received.clone();
    let running_clone = running.clone();
    let first_callback_clone = first_callback_time.clone();

    println!(
        "\nStarting {} second USB throughput test...",
        TEST_DURATION_SECS
    );
    println!("Press Ctrl+C to stop early\n");

    radio.read_async(Box::new(move |data: &[i16]| {
        if !running_clone.load(Ordering::Relaxed) {
            return;
        }

        // Capture first callback time
        let mut first_time = first_callback_clone.lock().unwrap();
        if first_time.is_none() {
            *first_time = Some(Instant::now());
        }
        drop(first_time);

        let len = data.len() * 2; // i16 samples, 2 bytes each
        bytes_clone.fetch_add(len as u64, Ordering::Relaxed);
        packets_clone.fetch_add(1, Ordering::Relaxed);
    }))?;

    // Wait for first callback
    loop {
        std::thread::sleep(Duration::from_millis(10));
        let first_time = first_callback_time.lock().unwrap();
        if first_time.is_some() {
            break;
        }
    }

    let start_time = first_callback_time.lock().unwrap().unwrap();
    let mut last_report = start_time;
    let mut last_bytes = 0u64;
    let mut last_packets = 0u64;

    // Monitoring loop
    while running.load(Ordering::Relaxed) && start_time.elapsed().as_secs() < TEST_DURATION_SECS {
        std::thread::sleep(Duration::from_secs(1));

        if last_report.elapsed().as_secs() >= 1 {
            let elapsed = last_report.elapsed().as_secs_f64();
            let current_bytes = bytes_received.load(Ordering::Relaxed);
            let current_packets = packets_received.load(Ordering::Relaxed);

            let delta_bytes = current_bytes - last_bytes;
            let delta_packets = current_packets - last_packets;

            let throughput_mbps = delta_bytes as f64 / elapsed / 1_000_000.0;
            let expected_mbps = samplerate as f64 * 2.0 / 1_000_000.0;
            let efficiency = throughput_mbps / expected_mbps * 100.0;

            println!(
                "[{:3.0}s] Throughput: {:.2} MB/s ({:.1}% of expected), {} packets, avg {:.0} bytes/pkt",
                start_time.elapsed().as_secs_f64(),
                throughput_mbps,
                efficiency,
                delta_packets,
                if delta_packets > 0 {
                    delta_bytes as f64 / delta_packets as f64
                } else {
                    0.0
                }
            );

            if efficiency < 95.0 {
                println!("  ⚠ WARNING: Throughput below 95% - possible data loss!");
            }

            last_report = Instant::now();
            last_bytes = current_bytes;
            last_packets = current_packets;
        }
    }

    // Capture end time before stopping to get accurate duration
    let end_time = Instant::now();
    let elapsed = end_time.duration_since(start_time).as_secs_f64();

    running.store(false, Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(200));
    radio.read_cancel()?;

    // Final statistics - use values at the end_time snapshot
    let total_bytes = bytes_received.load(Ordering::Relaxed);
    let total_packets = packets_received.load(Ordering::Relaxed);

    let avg_throughput = total_bytes as f64 / elapsed / 1_000_000.0;
    let expected_throughput = samplerate as f64 * 2.0 / 1_000_000.0;
    let total_efficiency = avg_throughput / expected_throughput * 100.0;

    println!("\n=== Test Results ===");
    println!("Duration: {:.2} seconds", elapsed);
    println!(
        "Total bytes: {} ({:.2} MB)",
        total_bytes,
        total_bytes as f64 / 1_000_000.0
    );
    println!("Total packets: {}", total_packets);
    println!(
        "Average packet size: {:.0} bytes",
        if total_packets > 0 {
            total_bytes as f64 / total_packets as f64
        } else {
            0.0
        }
    );
    println!("\nAverage throughput: {:.2} MB/s", avg_throughput);
    println!(
        "Expected throughput: {:.2} MB/s ({} MHz * 2 bytes/sample)",
        expected_throughput,
        samplerate / 1_000_000
    );
    println!("Efficiency: {:.2}%", total_efficiency);

    // Calculate expected vs actual samples
    let expected_samples = (samplerate as f64 * elapsed) as u64;
    let actual_samples = total_bytes / 2; // 2 bytes per i16 sample
    let sample_loss = expected_samples as i64 - actual_samples as i64;
    let sample_loss_pct = (sample_loss as f64 / expected_samples as f64 * 100.0).abs();

    println!("\n=== Sample Loss Analysis ===");
    println!(
        "Expected samples: {} ({:.2} million)",
        expected_samples,
        expected_samples as f64 / 1_000_000.0
    );
    println!(
        "Actual samples: {} ({:.2} million)",
        actual_samples,
        actual_samples as f64 / 1_000_000.0
    );

    if sample_loss > 0 {
        println!("⚠ LOST SAMPLES: {} ({:.4}%)", sample_loss, sample_loss_pct);
        println!("\nPossible causes:");
        println!("  1. USB bus congestion (other USB devices active)");
        println!("  2. CPU too busy (high system load)");
        println!("  3. Insufficient USB buffer size (16 x 16KB = 256KB)");
        println!("  4. USB host controller issues");
        println!("\nRecommendations:");
        println!("  - Close other USB applications");
        println!("  - Reduce system load");
        println!("  - Use dedicated USB 3.0 controller");
        println!("  - Increase USB buffer count in code");
    } else if sample_loss < -1000 {
        println!(
            "⚠ MORE SAMPLES THAN EXPECTED: {} ({:.4}%)",
            -sample_loss, sample_loss_pct
        );
        println!("This indicates timing measurement error or duplicate packets");
    } else {
        println!("✓ No significant sample loss detected!");
        println!("  USB throughput is stable and consistent");
    }

    // Performance rating
    println!("\n=== Performance Rating ===");
    if total_efficiency >= 99.9 {
        println!("★★★★★ EXCELLENT - No measurable data loss");
    } else if total_efficiency >= 99.0 {
        println!("★★★★☆ VERY GOOD - Minimal data loss (<1%)");
    } else if total_efficiency >= 95.0 {
        println!("★★★☆☆ GOOD - Minor data loss acceptable for most applications");
    } else if total_efficiency >= 90.0 {
        println!("★★☆☆☆ FAIR - Noticeable data loss, may affect performance");
    } else {
        println!("★☆☆☆☆ POOR - Significant data loss, USB issues present");
    }

    Ok(())
}
