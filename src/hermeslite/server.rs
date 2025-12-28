// Hermes Lite Network Server
// Implements UDP-based Hermes Lite protocol with multi-channel support via VirtualRadio

use anyhow::{Context, Result};
use num_complex::Complex32;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::protocol::*;
use log::{debug, error, info, warn};

/// Callback type for frequency changes
pub type FrequencyChangeCallback = Arc<dyn Fn(usize, u64) + Send + Sync>;

const COMMAND_PORT: u16 = 1024;
const MAX_RECEIVERS: usize = 12; // Hermes Lite 2 supports up to 12 receivers

/// Channel state for sample buffering
#[derive(Clone)]
struct ChannelState {
    /// Buffered samples as (I, Q) 24-bit pairs
    buffer: Vec<(i32, i32)>,
}

impl ChannelState {
    fn new(_sample_rate: u32) -> Self {
        // Calculate samples per burst based on sample rate
        // Hermes Lite typically sends 126 samples per frame, 2 frames per packet
        let target_samples = 63; // per frame, we send 2 frames

        Self {
            buffer: Vec::with_capacity(target_samples * 4),
        }
    }

    fn add_sample(&mut self, i: i32, q: i32) {
        self.buffer.push((i, q));
    }

    fn get_samples(&mut self, count: usize) -> Vec<(i32, i32)> {
        if self.buffer.len() >= count {
            self.buffer.drain(0..count).collect()
        } else {
            // Return what we have and pad with zeros
            let mut result = self.buffer.drain(..).collect::<Vec<_>>();
            result.resize(count, (0, 0));
            result
        }
    }

    fn samples_available(&self) -> usize {
        self.buffer.len()
    }
}

/// Hermes Lite server state
pub struct HermesLiteServer {
    /// Command/control socket (UDP port 1024)
    command_socket: Arc<UdpSocket>,

    /// Current RX configuration
    rx_config: Arc<Mutex<RxConfig>>,

    /// Active client info
    client: Arc<Mutex<Option<ClientInfo>>>,

    /// Channel states for buffering
    channel_states: Arc<Mutex<Vec<ChannelState>>>,

    /// Packet builder for data transmission
    packet_builder: Arc<Mutex<DataPacketBuilder>>,

    /// Server running flag
    running: Arc<AtomicBool>,

    /// Command handler thread
    command_thread: Option<JoinHandle<()>>,

    /// Data transmission thread
    data_thread: Option<JoinHandle<()>>,

    /// MAC address for discovery replies
    mac_address: [u8; 6],

    /// Frequency change callback
    freq_change_callback: Option<FrequencyChangeCallback>,
}

impl HermesLiteServer {
    /// Create a new Hermes Lite server
    pub fn new(mac_address: Option<[u8; 6]>) -> Result<Self> {
        let command_socket = UdpSocket::bind(format!("0.0.0.0:{}", COMMAND_PORT))
            .context("Failed to bind command socket")?;

        command_socket
            .set_read_timeout(Some(Duration::from_millis(100)))
            .context("Failed to set socket timeout")?;

        // Generate or use provided MAC address
        let mac_address = mac_address.unwrap_or_else(|| {
            use std::time::{SystemTime, UNIX_EPOCH};
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            [
                0x02, // locally administered
                (timestamp >> 32) as u8,
                (timestamp >> 24) as u8,
                (timestamp >> 16) as u8,
                (timestamp >> 8) as u8,
                timestamp as u8,
            ]
        });

        Ok(Self {
            command_socket: Arc::new(command_socket),
            rx_config: Arc::new(Mutex::new(RxConfig::default())),
            client: Arc::new(Mutex::new(None)),
            channel_states: Arc::new(Mutex::new(vec![ChannelState::new(48_000); MAX_RECEIVERS])),
            packet_builder: Arc::new(Mutex::new(DataPacketBuilder::new())),
            running: Arc::new(AtomicBool::new(false)),
            command_thread: None,
            data_thread: None,
            mac_address,
            freq_change_callback: None,
        })
    }

    /// Constructor allowing custom port (used by tests)
    #[allow(dead_code)]
    pub fn new_with_port(mac_address: Option<[u8; 6]>, port: u16) -> Result<Self> {
        let command_socket =
            UdpSocket::bind(("127.0.0.1", port)).context("Failed to bind test command socket")?;
        command_socket
            .set_read_timeout(Some(Duration::from_millis(100)))
            .context("Failed to set socket timeout")?;
        let mac_address = mac_address.unwrap_or([0x02, 0, 0, 0, 0, 1]);
        Ok(Self {
            command_socket: Arc::new(command_socket),
            rx_config: Arc::new(Mutex::new(RxConfig::default())),
            client: Arc::new(Mutex::new(None)),
            channel_states: Arc::new(Mutex::new(vec![ChannelState::new(48_000); MAX_RECEIVERS])),
            packet_builder: Arc::new(Mutex::new(DataPacketBuilder::new())),
            running: Arc::new(AtomicBool::new(false)),
            command_thread: None,
            data_thread: None,
            mac_address,
            freq_change_callback: None,
        })
    }

    /// Get the MAC address
    pub fn get_mac_address(&self) -> [u8; 6] {
        self.mac_address
    }

    /// Set the frequency change callback
    pub fn set_frequency_change_callback<F>(&mut self, callback: F)
    where
        F: Fn(usize, u64) + Send + Sync + 'static,
    {
        self.freq_change_callback = Some(Arc::new(callback));
    }

    /// Start the server
    pub fn start(&mut self) -> Result<()> {
        if self.running.load(Ordering::Relaxed) {
            return Ok(());
        }

        self.running.store(true, Ordering::Relaxed);

        // Start command handler thread
        let command_socket = self.command_socket.clone();
        let rx_config = self.rx_config.clone();
        let client = self.client.clone();
        let running = self.running.clone();
        let mac_address = self.mac_address;
        let freq_callback = self.freq_change_callback.clone();

        let command_thread = thread::spawn(move || {
            Self::command_handler(
                command_socket,
                rx_config,
                client,
                running,
                mac_address,
                freq_callback,
            );
        });

        self.command_thread = Some(command_thread);

        // Start data transmission thread
        let command_socket = self.command_socket.clone();
        let client = self.client.clone();
        let channel_states = self.channel_states.clone();
        let packet_builder = self.packet_builder.clone();
        let running = self.running.clone();
        let rx_config = self.rx_config.clone();

        let data_thread = thread::spawn(move || {
            Self::data_handler(
                command_socket,
                client,
                channel_states,
                packet_builder,
                running,
                rx_config,
            );
        });

        self.data_thread = Some(data_thread);

        info!(
            "Hermes Lite server started on port {} - waiting for client",
            COMMAND_PORT
        );

        Ok(())
    }

    /// Stop the server
    pub fn stop(&mut self) {
        if !self.running.load(Ordering::Relaxed) {
            return;
        }

        info!("Stopping Hermes Lite server...");
        self.running.store(false, Ordering::Relaxed);

        // Deactivate client
        if let Ok(mut client) = self.client.lock()
            && let Some(ref mut c) = *client
        {
            c.active = false;
        }

        // Wait for threads to finish
        if let Some(thread) = self.command_thread.take() {
            let _ = thread.join();
        }

        if let Some(thread) = self.data_thread.take() {
            let _ = thread.join();
        }

        info!("Hermes Lite server stopped");
    }

    /// Command handler thread
    fn command_handler(
        socket: Arc<UdpSocket>,
        rx_config: Arc<Mutex<RxConfig>>,
        client: Arc<Mutex<Option<ClientInfo>>>,
        running: Arc<AtomicBool>,
        mac_address: [u8; 6],
        freq_callback: Option<FrequencyChangeCallback>,
    ) {
        let mut buffer = vec![0u8; 2048];

        while running.load(Ordering::Relaxed) {
            match socket.recv_from(&mut buffer) {
                Ok((size, addr)) => {
                    if size < 4 {
                        continue;
                    }

                    // Parse magic number
                    let magic = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);

                    match magic {
                        MAGIC_DISCOVERY => {
                            info!("Discovery request from {}", addr);

                            // Check if we're active
                            let is_active = client
                                .lock()
                                .unwrap()
                                .as_ref()
                                .map(|c| c.active)
                                .unwrap_or(false);

                            let num_receivers = rx_config.lock().unwrap().num_receivers;

                            let reply = DiscoveryReply::new(mac_address, num_receivers, is_active);
                            let reply_bytes = reply.to_bytes();

                            if let Err(e) = socket.send_to(&reply_bytes, addr) {
                                error!("Failed to send discovery reply: {}", e);
                            }
                        }

                        MAGIC_START => {
                            info!("Start command from {}", addr);

                            let mut client_lock = client.lock().unwrap();
                            *client_lock = Some(ClientInfo::new(addr));
                            if let Some(ref mut c) = *client_lock {
                                c.active = true;
                            }

                            info!("Client activated: {} - data transmission will begin", addr);
                        }

                        MAGIC_STOP => {
                            info!("Stop command from {}", addr);

                            if let Ok(mut client_lock) = client.lock()
                                && let Some(ref mut c) = *client_lock
                            {
                                c.active = false;
                            }

                            info!("Client deactivated - data transmission stopped");
                        }

                        MAGIC_DATA_TX => {
                            // Control packets embedded in data stream
                            // This means client is ready to receive data - activate transmission
                            if let Ok(mut client_lock) = client.lock() {
                                if let Some(ref mut c) = *client_lock {
                                    if !c.active {
                                        c.active = true;
                                        log::info!(
                                            "Client activated - starting data transmission to {}",
                                            addr
                                        );
                                    }
                                } else {
                                    // First control packet from new client
                                    *client_lock = Some(ClientInfo { addr, active: true });
                                    log::info!("New client {} - starting data transmission", addr);
                                }
                            }

                            // Process control frames
                            if size >= 523 {
                                // Two control frames at offset 11 and 523
                                Self::process_control_frame(
                                    &buffer[11..16],
                                    &rx_config,
                                    &freq_callback,
                                );
                            }
                            if size >= 1032 {
                                Self::process_control_frame(
                                    &buffer[523..528],
                                    &rx_config,
                                    &freq_callback,
                                );
                            }
                        }

                        _ => {
                            // Unknown packet
                        }
                    }
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    // Socket timeout, no data available
                    continue;
                }
                Err(e) => {
                    error!("Socket error: {}", e);
                }
            }
        }
    }

    /// Process a control frame (5 bytes)
    fn process_control_frame(
        frame: &[u8],
        rx_config: &Arc<Mutex<RxConfig>>,
        freq_callback: &Option<FrequencyChangeCallback>,
    ) {
        if let Some(packet) = ControlPacket::parse(frame) {
            match packet.cmd_type {
                CommandType::General => {
                    if let Some(new_config) = RxConfig::parse_general_packet(&packet.data) {
                        let mut config = rx_config.lock().unwrap();
                        let prev = config.clone();
                        *config = new_config;
                        if prev.sample_rate_code != config.sample_rate_code {
                            info!(
                                "Sample rate code changed {} -> {} ({} Hz, decimation {})",
                                prev.sample_rate_code,
                                config.sample_rate_code,
                                config.get_sample_rate(),
                                config.get_decimation()
                            );
                        }
                        if prev.num_receivers != config.num_receivers {
                            info!(
                                "Receiver count changed {} -> {}",
                                prev.num_receivers, config.num_receivers
                            );
                        }
                        if prev.adc_config != config.adc_config {
                            info!(
                                "ADC config changed {} -> {}",
                                prev.adc_config, config.adc_config
                            );
                        }
                    }
                }
                CommandType::RxFreq(rx_idx) => {
                    let freq = u32::from_be_bytes(packet.data) as u64;
                    debug!("Set RX{} frequency to {} Hz", rx_idx, freq);
                    if let Some(callback) = freq_callback {
                        callback(rx_idx, freq);
                    }
                }
                CommandType::Attenuation => {
                    let att = if packet.data[3] & 0x40 != 0 {
                        packet.data[3] & 0x3f
                    } else if packet.data[3] & 0x20 != 0 {
                        (packet.data[3] & 0x1f) * 2
                    } else {
                        0
                    };
                    debug!("Set attenuation to {} dB", att);
                }
                CommandType::TxFreq => {
                    // TX frequency - not used for RX-only mode
                }
                CommandType::LpfSelect => {
                    // LPF/PA/VNA configuration - hardware dependent
                }
                CommandType::TxGainPtt
                | CommandType::LnaGainTx
                | CommandType::CwConfig
                | CommandType::CwHangTime
                | CommandType::PttLatency
                | CommandType::Predistortion
                | CommandType::MiscCommands
                | CommandType::ResetOnDisconnect
                | CommandType::Ad9866Spi
                | CommandType::I2c1
                | CommandType::I2c2
                | CommandType::ErrorResponse => {
                    // Advanced features - silently ignored for basic RX operation
                }
                CommandType::Unknown(code) => {
                    // Only log truly unknown codes that aren't in the protocol spec
                    if code > 0x3f {
                        warn!("Unknown command code: 0x{:02x}", code);
                    }
                }
            }
        }
    }

    /// Data handler thread - sends IQ data packets to client
    fn data_handler(
        socket: Arc<UdpSocket>,
        client: Arc<Mutex<Option<ClientInfo>>>,
        channel_states: Arc<Mutex<Vec<ChannelState>>>,
        packet_builder: Arc<Mutex<DataPacketBuilder>>,
        running: Arc<AtomicBool>,
        rx_config: Arc<Mutex<RxConfig>>,
    ) {
        let mut packet_count = 0u64;
        let mut last_log_time = std::time::Instant::now();

        while running.load(Ordering::Relaxed) {
            // Check if client is active
            let client_addr = {
                let client_lock = client.lock().unwrap();
                if let Some(ref c) = *client_lock {
                    if c.active { Some(c.addr) } else { None }
                } else {
                    None
                }
            };

            if let Some(addr) = client_addr {
                let num_receivers = rx_config.lock().unwrap().num_receivers as usize;

                // Check if we have enough samples
                let mut states = channel_states.lock().unwrap();
                let sample_counts: Vec<usize> = (0..num_receivers)
                    .map(|i| states[i].samples_available())
                    .collect();
                let min_samples = sample_counts.iter().copied().min().unwrap_or(0);

                if min_samples >= 126 {
                    // Enough samples to build a packet (63 per frame * 2 frames)
                    let samples: Vec<Vec<(i32, i32)>> = (0..num_receivers)
                        .map(|i| states[i].get_samples(126))
                        .collect();

                    drop(states); // Release lock before sending

                    let packet = packet_builder.lock().unwrap().build_packet(&samples);

                    match socket.send_to(&packet, addr) {
                        Ok(sent_bytes) => {
                            packet_count += 1;
                            if last_log_time.elapsed().as_secs() >= 5 {
                                info!(
                                    "Data TX: {} packets sent ({} bytes/pkt), samples available: {:?}",
                                    packet_count, sent_bytes, sample_counts
                                );
                                last_log_time = std::time::Instant::now();
                            }
                        }
                        Err(e) => {
                            warn!("Failed to send data packet to {}: {}", addr, e);
                        }
                    }
                } else {
                    // Log sample starvation periodically
                    if last_log_time.elapsed().as_secs() >= 10 && min_samples == 0 {
                        warn!(
                            "No samples available for transmission. Channel buffers: {:?}",
                            sample_counts
                        );
                        last_log_time = std::time::Instant::now();
                    }
                    drop(states);
                    thread::sleep(Duration::from_micros(100));
                }
            } else {
                // No active client, sleep
                thread::sleep(Duration::from_millis(10));
            }
        }
    }

    // Removed channel_callback; create_channel_callback covers usage.

    /// Create a callback closure for a specific channel
    pub fn create_channel_callback(
        &self,
        channel_idx: usize,
    ) -> impl Fn(usize, &[Complex32]) + Send + Sync + 'static {
        use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

        let channel_states = self.channel_states.clone();
        let sample_count = Arc::new(AtomicU64::new(0));
        let last_log = Arc::new(Mutex::new(std::time::Instant::now()));

        move |_idx: usize, samples: &[Complex32]| {
            if channel_idx >= MAX_RECEIVERS {
                return;
            }

            let mut states = channel_states.lock().unwrap();
            let samples_len = samples.len();

            for sample in samples {
                const SCALE: f32 = 8388607.0; // 2^23 - 1
                let i = (sample.re * SCALE).clamp(-8388608.0, 8388607.0) as i32;
                let q = (sample.im * SCALE).clamp(-8388608.0, 8388607.0) as i32;
                states[channel_idx].add_sample(i, q);
            }

            let total = sample_count.fetch_add(samples_len as u64, AtomicOrdering::Relaxed);
            let mut last_log_time = last_log.lock().unwrap();
            if last_log_time.elapsed().as_secs() >= 5 {
                let buffer_size = states[channel_idx].samples_available();
                debug!(
                    "Channel {} callback: received {} samples (total: {}), buffer: {}",
                    channel_idx,
                    samples_len,
                    total + samples_len as u64,
                    buffer_size
                );
                *last_log_time = std::time::Instant::now();
            }
        }
    }

    #[allow(dead_code)]
    /// Get current number of active receivers (used in tests / potential external API)
    pub fn get_num_receivers(&self) -> u8 {
        self.rx_config.lock().unwrap().num_receivers
    }

    #[allow(dead_code)]
    /// Get current sample rate (used in tests / potential external API)
    pub fn get_sample_rate(&self) -> u32 {
        self.rx_config.lock().unwrap().get_sample_rate()
    }

    /// Set number of receivers advertised/used (clamped to MAX_RECEIVERS)
    #[allow(dead_code)]
    pub fn set_num_receivers(&mut self, n: u8) {
        let mut cfg = self.rx_config.lock().unwrap();
        cfg.num_receivers = n.min(MAX_RECEIVERS as u8);
    }
}

impl Drop for HermesLiteServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_creation() {
        let server = HermesLiteServer::new_with_port(None, 0).unwrap();
        assert_eq!(server.get_num_receivers(), 1);
        assert_eq!(server.get_sample_rate(), 48_000);
    }

    #[test]
    fn test_channel_callback() {
        let server = HermesLiteServer::new_with_port(None, 0).unwrap();
        let callback = server.create_channel_callback(0);

        let samples = vec![Complex32::new(0.5, -0.5), Complex32::new(-0.8, 0.3)];

        callback(0, &samples);

        let states = server.channel_states.lock().unwrap();
        assert_eq!(states[0].samples_available(), 2);
    }
}
