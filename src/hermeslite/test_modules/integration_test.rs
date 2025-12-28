// Integration test: Hermes Lite Server with MockSDR and UDP Client
// Tests the complete data flow from MockSDR -> VirtualRadio -> HermesLite Server -> UDP Client

use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use super::test_client::HermesLiteClient;
use crate::server::HermesLiteServer;
use sddc::{MockSDR, SignalPattern, VirtualChannelConfig, VirtualRadio};

const TEST_SAMPLE_RATE: u32 = 64_000_000; // 64 MSPS
const TEST_DECIMATION: usize = 512; // 64M / 512 = 125k (power of 2)
const TEST_CENTER_FREQ: u64 = 14_200_000; // 14.2 MHz
const TEST_CHANNEL_FREQ: u64 = 14_070_000; // 14.07 MHz (FT8 frequency)

#[test]
fn test_hermeslite_with_mock_sdr() -> Result<()> {
    println!("\n=== Testing Hermes Lite Server with MockSDR ===\n");

    // Step 1: Create MockSDR with a test signal
    println!("Creating MockSDR with 5 MHz sine wave...");
    let mock = MockSDR::new(
        TEST_SAMPLE_RATE,
        SignalPattern::Sine {
            freq_hz: 5_000_000.0, // 5 MHz signal
        },
        0.7, // 70% amplitude
    );

    // Step 2: Create VirtualRadio with MockSDR
    println!("Creating VirtualRadio...");
    let mut vradio = VirtualRadio::new(mock, TEST_SAMPLE_RATE)?;

    vradio.set_direct_sampling(true)?;
    vradio.set_center_freq(TEST_CENTER_FREQ)?;

    println!("VirtualRadio configured:");
    println!("  Center frequency: {} Hz", vradio.get_center_freq());
    println!("  Sample rate: {} MSPS", TEST_SAMPLE_RATE / 1_000_000);

    // Step 3: Start Hermes Lite server with test port
    println!("\nStarting Hermes Lite server on test port 10024...");
    let mut server = HermesLiteServer::new_with_port(None, 10024)?;
    let mac = server.get_mac_address();
    println!(
        "Server MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    );

    // Wrap VirtualRadio for sharing
    let vradio = Arc::new(Mutex::new(vradio));
    let vradio_for_callback = vradio.clone();

    // Set frequency change callback
    server.set_frequency_change_callback(move |channel_idx, freq| {
        println!("Freq change callback: RX{} -> {} Hz", channel_idx, freq);
        if let Ok(mut vr) = vradio_for_callback.lock()
            && let Err(e) = vr.set_channel_center_freq(channel_idx, freq)
        {
            eprintln!("Failed to set channel {} frequency: {}", channel_idx, e);
        }
    });

    server.start()?;

    // Step 4: Create virtual channels
    println!("\nCreating virtual channels...");
    for i in 0..4 {
        let channel_freq = TEST_CHANNEL_FREQ + (i as u64 * 25_000);
        let callback = server.create_channel_callback(i);

        println!("  RX{}: {} Hz", i, channel_freq);

        let mut vr = vradio.lock().unwrap();
        vr.create_channel(
            VirtualChannelConfig {
                center_freq: channel_freq,
                lsb: false,
                decimation: TEST_DECIMATION,
            },
            callback,
        )?;
    }

    // Step 5: Start streaming
    println!("\nStarting VirtualRadio streaming...");
    {
        let mut vr = vradio.lock().unwrap();
        vr.start()?;
    }

    // Give it time to start generating samples
    thread::sleep(Duration::from_millis(500));

    // Step 6: Create UDP client and connect
    println!("\n=== Testing UDP Client Connection ===\n");
    let mut client = HermesLiteClient::new(10000)?;

    // Discovery
    println!("Discovering server...");
    match client.discover("127.0.0.1:10024") {
        Ok(_) => println!("✓ Discovery successful"),
        Err(e) => {
            eprintln!("✗ Discovery failed: {}", e);
            return Err(e);
        }
    }

    thread::sleep(Duration::from_millis(100));

    // Configure
    println!("\nConfiguring receiver...");
    match client.configure(0x00, 4) {
        Ok(_) => println!("✓ Configuration sent"),
        Err(e) => {
            eprintln!("✗ Configuration failed: {}", e);
            return Err(e);
        }
    }

    thread::sleep(Duration::from_millis(100));

    // Set frequencies
    println!("\nSetting frequencies...");
    for i in 0..4 {
        let freq = TEST_CHANNEL_FREQ + (i as u64 * 25_000);
        match client.set_frequency(i, freq) {
            Ok(_) => println!("✓ RX{} frequency set to {} Hz", i, freq),
            Err(e) => {
                eprintln!("✗ Failed to set RX{} frequency: {}", i, e);
            }
        }
    }

    thread::sleep(Duration::from_millis(100));

    // Start data streaming
    println!("\nStarting data reception...");
    match client.start() {
        Ok(_) => println!("✓ Started successfully"),
        Err(e) => {
            eprintln!("✗ Start failed: {}", e);
            return Err(e);
        }
    }

    // Step 7: Receive data and check
    println!("\n=== Receiving Data ===\n");
    let stats = client.receive_data(5)?;

    // Step 8: Stop and cleanup
    println!("\n=== Cleanup ===\n");
    client.stop()?;

    thread::sleep(Duration::from_millis(100));

    {
        let mut vr = vradio.lock().unwrap();
        vr.stop()?;
    }

    server.stop();

    // Step 9: Analyze results and report bugs
    println!("\n=== Test Results ===\n");

    let mut bugs_found = Vec::new();

    // Check if we received any data
    if stats.packet_count == 0 {
        bugs_found.push("BUG: No data packets received from server".to_string());
    } else {
        println!("✓ Data packets received: {}", stats.packet_count);
    }

    // Check for invalid packets
    if stats.invalid_packets > 0 {
        bugs_found.push(format!(
            "BUG: {} invalid packets received",
            stats.invalid_packets
        ));
    } else if stats.packet_count > 0 {
        println!("✓ All packets valid");
    }

    // Check for dropped packets (some drops acceptable in test environment)
    if stats.dropped_packets > stats.packet_count / 10 {
        bugs_found.push(format!(
            "BUG: High packet drop rate: {}/{}",
            stats.dropped_packets, stats.packet_count
        ));
    } else if stats.packet_count > 0 {
        println!(
            "✓ Packet drop rate acceptable: {}/{}",
            stats.dropped_packets, stats.packet_count
        );
    }

    // Report bugs
    if !bugs_found.is_empty() {
        println!("\n=== BUGS FOUND ===\n");
        for bug in &bugs_found {
            println!("  ⚠ {}", bug);
        }
        println!();
    } else if stats.packet_count > 0 {
        println!("\n✓ All tests passed! No bugs detected.");
    }

    Ok(())
}

#[test]
fn test_hermeslite_multi_tone() -> Result<()> {
    println!("\n=== Testing Hermes Lite with Multi-Tone Signal ===\n");

    // Create MockSDR with multi-tone pattern
    const FREQS: &[f32] = &[1_000_000.0, 5_000_000.0, 10_000_000.0];
    let mock = MockSDR::new(
        TEST_SAMPLE_RATE,
        SignalPattern::MultiTone { freqs: FREQS },
        0.6,
    );

    let mut vradio = VirtualRadio::new(mock, TEST_SAMPLE_RATE)?;
    vradio.set_direct_sampling(true)?;
    vradio.set_center_freq(TEST_CENTER_FREQ)?;

    let mut server = HermesLiteServer::new_with_port(None, 10025)?;

    let vradio = Arc::new(Mutex::new(vradio));
    let vradio_for_callback = vradio.clone();

    server.set_frequency_change_callback(move |channel_idx, freq| {
        if let Ok(mut vr) = vradio_for_callback.lock() {
            let _ = vr.set_channel_center_freq(channel_idx, freq);
        }
    });

    server.start()?;

    // Create channels
    for i in 0..3 {
        let callback = server.create_channel_callback(i);
        let mut vr = vradio.lock().unwrap();
        vr.create_channel(
            VirtualChannelConfig {
                center_freq: TEST_CHANNEL_FREQ + (i as u64 * 100_000),
                lsb: false,
                decimation: TEST_DECIMATION,
            },
            callback,
        )?;
    }

    {
        let mut vr = vradio.lock().unwrap();
        vr.start()?;
    }

    thread::sleep(Duration::from_millis(500));

    // Test with client
    let mut client = HermesLiteClient::new(10002)?;
    client.discover("127.0.0.1:10025")?;
    client.configure(0x00, 3)?;
    client.start()?;

    let stats = client.receive_data(3)?;

    client.stop()?;
    {
        let mut vr = vradio.lock().unwrap();
        vr.stop()?;
    }
    server.stop();

    println!("\n=== Multi-Tone Test Results ===");
    println!("Packets received: {}", stats.packet_count);

    assert!(
        stats.packet_count > 0,
        "Should receive packets with multi-tone signal"
    );

    Ok(())
}
