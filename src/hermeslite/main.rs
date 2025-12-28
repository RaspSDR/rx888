// Hermes Lite Protocol Server with VirtualRadio Integration
// Provides multi-channel SDR streaming using Hermes Lite protocol

mod protocol;
mod server;

use anyhow::{Context, Result};
use clap::Parser;
use std::sync::Arc;

use log::{debug, info};
use sddc::{Radio, VirtualChannelConfig, VirtualRadio};
use server::HermesLiteServer;

const MAX_CHANNELS: u8 = 12; // Hermes Lite 2 protocol supports up to 12 receivers

/// Hermes Lite protocol server for RX888 SDR
#[derive(Parser, Debug)]
#[command(name = "hermeslite")]
#[command(about = "Hermes Lite protocol server using RX888 SDR", long_about = None)]
struct Args {
    /// Device index (default: 0)
    #[arg(short, long, default_value_t = 0)]
    device: u32,

    /// Enable direct sampling mode (for HF < 30 MHz)
    #[arg(short = 'D', long, default_value_t = true)]
    direct_sampling: bool,

    /// IF gain in dB
    #[arg(long, default_value_t = 0.0)]
    if_gain: f32,

    /// RF gain in dB
    #[arg(long, default_value_t = 0.0)]
    rf_gain: f32,

    /// MAC address for discovery (hex format: 00:11:22:33:44:55)
    #[arg(short, long)]
    mac: Option<String>,

    /// Initial center frequency in Hz (default: 7.1 MHz, can be changed from client)
    #[arg(short = 'f', long, default_value_t = 7_100_000)]
    initial_freq: u64,
}

fn parse_mac_address(mac_str: &str) -> Result<[u8; 6]> {
    let parts: Vec<&str> = mac_str.split(':').collect();
    if parts.len() != 6 {
        anyhow::bail!("MAC address must have 6 octets");
    }

    let mut mac = [0u8; 6];
    for (i, part) in parts.iter().enumerate() {
        mac[i] =
            u8::from_str_radix(part, 16).context(format!("Invalid MAC address octet: {}", part))?;
    }

    Ok(mac)
}

/// Calculate optimal sample rate based on output rate
/// Returns (samplerate, decimation)
fn calculate_sample_rate(output_rate: u32) -> (u32, usize) {
    // Target sample rates between 32-64 MHz
    // Choose based on output rate to get nice decimation factors

    let (target_rate, decimation) = match output_rate {
        // 48 kHz: use 49.152 MHz (1024 decimation) or 48 MHz (1000)
        48_000 => (49_152_000, 1024),

        // 96 kHz: use 49.152 MHz (512 decimation)
        96_000 => (49_152_000, 512),

        // 192 kHz: use 49.152 MHz (256 decimation)
        192_000 => (49_152_000, 256),

        // 384 kHz: use 49.152 MHz (128 decimation)
        384_000 => (49_152_000, 128),

        // For other rates, calculate dynamically
        _ => {
            // Try to get a power-of-2 decimation in 32-64 MHz range
            let mut best_rate = 32_000_000;
            let mut best_decimation = 1;

            for power in 6..=12 {
                let decimation = 1 << power; // 64, 128, 256, ..., 4096
                let rate = output_rate * decimation;

                if (32_000_000..=64_000_000).contains(&rate) {
                    best_rate = rate;
                    best_decimation = decimation;
                    break;
                }
            }

            (best_rate, best_decimation)
        }
    };

    println!(
        "Calculated sample rate: {} MHz (decimation: {})",
        target_rate / 1_000_000,
        decimation
    );

    (target_rate, decimation as usize)
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging (user sets RUST_LOG externally if desired)
    env_logger::init();
    info!("Hermes Lite Protocol Server - RX888 SDR starting");

    // Start with default 96kHz output rate (client will configure via protocol)
    let default_output_rate = 96_000;
    let (samplerate, decimation) = calculate_sample_rate(default_output_rate);

    // Parse MAC address if provided
    let mac_address = if let Some(ref mac_str) = args.mac {
        Some(parse_mac_address(mac_str)?)
    } else {
        None
    };

    // Open and configure radio
    info!("Opening RX888 device {}...", args.device);
    let radio = Radio::open(args.device).context("Failed to open RX888 device")?;

    info!("Device model: {:?}", radio.get_model());
    info!(
        "Firmware version: {}.{}",
        radio.get_firmware_version() >> 8,
        radio.get_firmware_version() & 0xFF
    );

    // Create VirtualRadio
    info!(
        "Creating virtual radio with {} MHz sample rate...",
        samplerate / 1_000_000
    );
    let mut vradio = VirtualRadio::new(radio, samplerate)?;

    // Configure physical radio
    vradio.set_direct_sampling(args.direct_sampling)?;
    vradio.set_center_freq(args.initial_freq)?;
    vradio.set_if_gain(args.if_gain)?;
    vradio.set_rf_gain(args.rf_gain)?;

    // Disable PGA to reduce DC offset and input-referred noise
    info!("Disabling LT2208 ADC PGA to reduce DC offset");
    vradio.enable_adc_pga(false)?;

    // Disable HF antenna bias (not needed for general use)
    info!("Disabling HF antenna bias");
    vradio.enable_antenna_bias(0, true)?;

    // Enable LT2208 ADC features - randomizer is REQUIRED
    info!("Enabling LT2208 ADC dither and randomizer");
    vradio.enable_adc_dither(true)?;

    info!("Initial frequency: {} Hz", vradio.get_center_freq());
    info!("Direct sampling: {}", vradio.get_direct_sampling());
    info!("IF gain: {:.1} dB", vradio.get_if_gain());
    info!("RF gain: {:.1} dB", vradio.get_rf_gain());
    info!("Sample rate: {} MHz", samplerate / 1_000_000);
    info!(
        "Default output rate: {} kHz (decimation: {}) - client can reconfigure",
        default_output_rate / 1_000,
        decimation
    );

    // Create Hermes Lite server
    info!("Starting Hermes Lite server...");
    let mut server = HermesLiteServer::new(mac_address)?;
    let mac = server.get_mac_address();
    info!(
        "MAC Address: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    );

    // Wrap VirtualRadio in Arc<Mutex> for sharing with callback
    let vradio = Arc::new(std::sync::Mutex::new(vradio));
    let vradio_for_callback = vradio.clone();

    // Set frequency change callback
    server.set_frequency_change_callback(move |channel_idx, freq| {
        if channel_idx >= MAX_CHANNELS as usize {
            return;
        }
        debug!("Freq cmd RX{} -> {} Hz", channel_idx, freq);
        if let Ok(mut vr) = vradio_for_callback.lock() {
            if let Err(e) = vr.set_channel_center_freq(channel_idx, freq) {
                eprintln!("Failed to set channel {} frequency: {}", channel_idx, e);
            } else {
                info!("Updated channel {} to {} Hz", channel_idx, freq);
            }
        }
    });

    server.start()?;

    info!("Creating {} virtual receiver(s) at startup", MAX_CHANNELS);

    // Create virtual channels with default frequency (will be updated by client)
    let default_freq = args.initial_freq;

    // Pre-create maximum protocol-supported channels
    for i in 0..MAX_CHANNELS {
        let channel_freq = default_freq + (i as u64 * 25_000); // 25 kHz spacing as default
        let lsb = false; // Default to USB mode

        debug!("Creating RX{} at {} Hz", i, channel_freq);

        let callback = server.create_channel_callback(i as usize);

        let mut vr = vradio.lock().unwrap();
        vr.create_channel(
            VirtualChannelConfig {
                center_freq: channel_freq,
                lsb,
                decimation,
            },
            callback,
        )?;
    }

    info!("Server running: UDP port 1024, waiting for client discovery. Ctrl+C to stop.");
    info!("Client will configure sample rate and active receivers via protocol");

    // Start streaming
    {
        let mut vr = vradio.lock().unwrap();
        vr.start()?;
    }

    // Install Ctrl+C handler
    let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        info!("Received Ctrl+C, shutting down...");
        r.store(false, std::sync::atomic::Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    // Main loop
    while running.load(std::sync::atomic::Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // Cleanup
    info!("Stopping virtual radio...");
    {
        let mut vr = vradio.lock().unwrap();
        vr.stop()?;
    }

    info!("Stopping server...");
    server.stop();

    info!("Shutdown complete.");

    Ok(())
}
