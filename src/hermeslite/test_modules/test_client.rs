// UDP Test Client for Hermes Lite Protocol
// This client discovers, connects, and receives data from the hermeslite server

use anyhow::{Context, Result};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

const MAGIC_DISCOVERY: u32 = 0x0002feef;
const MAGIC_START: u32 = 0x0104feef;
const MAGIC_STOP: u32 = 0x0004feef;
const MAGIC_DATA_RX: u32 = 0x0601feef;

pub struct HermesLiteClient {
    socket: UdpSocket,
    server_addr: Option<SocketAddr>,
    mac_address: Option<[u8; 6]>,
    is_running: bool,
}

impl HermesLiteClient {
    /// Create a new client
    pub fn new(bind_port: u16) -> Result<Self> {
        let socket = UdpSocket::bind(format!("0.0.0.0:{}", bind_port))
            .context("Failed to bind client socket")?;

        socket
            .set_read_timeout(Some(Duration::from_millis(1000)))
            .context("Failed to set socket timeout")?;

        Ok(Self {
            socket,
            server_addr: None,
            mac_address: None,
            is_running: false,
        })
    }

    /// Send discovery broadcast
    pub fn discover(&mut self, server_addr: &str) -> Result<()> {
        println!("Sending discovery to {}...", server_addr);

        let discovery_packet = MAGIC_DISCOVERY.to_le_bytes();

        let addr: SocketAddr = server_addr.parse().context("Invalid server address")?;

        self.socket
            .send_to(&discovery_packet, addr)
            .context("Failed to send discovery packet")?;

        // Wait for reply
        let mut buffer = vec![0u8; 1024];
        match self.socket.recv_from(&mut buffer) {
            Ok((size, from_addr)) => {
                println!(
                    "Received discovery reply from {} ({} bytes)",
                    from_addr, size
                );

                if size >= 60 {
                    let magic = [buffer[0], buffer[1], buffer[2], buffer[3]];
                    let status = buffer[3];

                    if magic[0] == 0xEF && magic[1] == 0xFE && magic[2] == 0x02 {
                        let mac = [
                            buffer[4], buffer[5], buffer[6], buffer[7], buffer[8], buffer[9],
                        ];
                        let board_id = buffer[10];
                        let protocol_version = buffer[11];
                        let num_receivers = buffer[19];

                        println!("  Status: 0x{:02X}", status);
                        println!(
                            "  MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                            mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
                        );
                        println!(
                            "  Board ID: {} ({})",
                            board_id,
                            if board_id == 73 {
                                "Hermes Lite 2"
                            } else {
                                "Unknown"
                            }
                        );
                        println!("  Protocol: 0x{:02X}", protocol_version);
                        println!("  Receivers: {}", num_receivers);

                        self.server_addr = Some(from_addr);
                        self.mac_address = Some(mac);

                        return Ok(());
                    }
                }

                anyhow::bail!("Invalid discovery reply");
            }
            Err(e) => {
                anyhow::bail!("No discovery reply received: {}", e);
            }
        }
    }

    /// Send start command
    pub fn start(&mut self) -> Result<()> {
        let addr = self
            .server_addr
            .ok_or_else(|| anyhow::anyhow!("Not discovered"))?;

        println!("Sending START command to {}...", addr);

        let start_packet = MAGIC_START.to_le_bytes();

        self.socket
            .send_to(&start_packet, addr)
            .context("Failed to send start command")?;

        self.is_running = true;

        println!("Started successfully");
        Ok(())
    }

    /// Send stop command
    pub fn stop(&mut self) -> Result<()> {
        let addr = self
            .server_addr
            .ok_or_else(|| anyhow::anyhow!("Not discovered"))?;

        println!("Sending STOP command to {}...", addr);

        let stop_packet = MAGIC_STOP.to_le_bytes();

        self.socket
            .send_to(&stop_packet, addr)
            .context("Failed to send stop command")?;

        self.is_running = false;

        println!("Stopped successfully");
        Ok(())
    }

    /// Set frequency for a receiver
    pub fn set_frequency(&mut self, rx_index: u8, freq_hz: u64) -> Result<()> {
        let addr = self
            .server_addr
            .ok_or_else(|| anyhow::anyhow!("Not discovered"))?;

        println!("Setting RX{} frequency to {} Hz", rx_index, freq_hz);

        // Build command packet
        // 4 bytes magic + 4 bytes sequence + (63 * 5 = 315 bytes commands)
        let mut packet = vec![0u8; 4 + 4 + 315];

        // Magic for data TX (commands from client)
        packet[0..4].copy_from_slice(&0x0201feefu32.to_le_bytes());

        // Sequence number (can be 0 for this test)
        packet[4..8].copy_from_slice(&0u32.to_be_bytes());

        // Command at offset 8
        // C0 byte determines command type
        // For RX frequencies: 0x02-0x08 for RX0-RX6, 0x12-0x16 for RX7-RX11
        let c0 = if rx_index < 7 {
            0x02 + rx_index
        } else if rx_index < 12 {
            0x12 + (rx_index - 7)
        } else {
            anyhow::bail!("Invalid RX index: {}", rx_index);
        };

        packet[8] = c0;

        // C1-C4 contain the 32-bit frequency in big-endian
        let freq_32 = (freq_hz as u32).to_be_bytes();
        packet[9..13].copy_from_slice(&freq_32);

        self.socket
            .send_to(&packet, addr)
            .context("Failed to send frequency command")?;

        Ok(())
    }

    /// Set sample rate and number of receivers
    pub fn configure(&mut self, sample_rate_index: u8, num_receivers: u8) -> Result<()> {
        let addr = self
            .server_addr
            .ok_or_else(|| anyhow::anyhow!("Not discovered"))?;

        println!(
            "Configuring: sample_rate_index={}, num_receivers={}",
            sample_rate_index, num_receivers
        );

        // Build command packet
        let mut packet = vec![0u8; 4 + 4 + 315];

        // Magic for data TX (commands from client)
        packet[0..4].copy_from_slice(&0x0201feefu32.to_le_bytes());

        // Sequence number
        packet[4..8].copy_from_slice(&0u32.to_be_bytes());

        // General command (C0 = 0x00)
        packet[8] = 0x00;

        // C1[7:3] = sample rate, C1[2:0] = 0 (speed)
        packet[9] = (sample_rate_index & 0x1F) << 3;

        // C2 bits for receivers
        packet[10] = num_receivers.saturating_sub(1); // 0-based in protocol

        self.socket
            .send_to(&packet, addr)
            .context("Failed to send config command")?;

        Ok(())
    }

    /// Receive data packets
    pub fn receive_data(&mut self, duration_secs: u32) -> Result<DataStats> {
        if !self.is_running {
            anyhow::bail!("Client not started");
        }

        println!("Receiving data for {} seconds...", duration_secs);

        let start_time = Instant::now();
        let mut stats = DataStats::default();
        let mut buffer = vec![0u8; 2048];

        while start_time.elapsed().as_secs() < duration_secs as u64 {
            match self.socket.recv(&mut buffer) {
                Ok(size) => {
                    stats.packet_count += 1;
                    stats.total_bytes += size;

                    if size >= 8 {
                        let magic =
                            u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);

                        if magic == MAGIC_DATA_RX {
                            // Valid data packet
                            let seq =
                                u32::from_be_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]);

                            if stats.packet_count == 1 {
                                stats.first_seq = seq;
                            }
                            stats.last_seq = seq;

                            // Check for dropped packets
                            if stats.packet_count > 1 {
                                let expected = stats.last_seq.wrapping_sub(1);
                                if expected != stats.prev_seq && stats.packet_count > 2 {
                                    stats.dropped_packets += 1;
                                }
                            }
                            stats.prev_seq = seq;
                        } else {
                            stats.invalid_packets += 1;
                        }
                    } else {
                        stats.invalid_packets += 1;
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Timeout, continue
                    continue;
                }
                Err(e) => {
                    println!("Error receiving: {}", e);
                    break;
                }
            }
        }

        println!("\nData Statistics:");
        println!("  Packets received: {}", stats.packet_count);
        println!("  Total bytes: {}", stats.total_bytes);
        println!("  Invalid packets: {}", stats.invalid_packets);
        println!("  Dropped packets: {}", stats.dropped_packets);
        println!("  Sequence: {} -> {}", stats.first_seq, stats.last_seq);

        if stats.packet_count > 0 {
            let avg_size = stats.total_bytes / stats.packet_count;
            let elapsed = start_time.elapsed().as_secs_f64();
            let rate_kbps = (stats.total_bytes as f64 * 8.0) / (elapsed * 1000.0);

            println!("  Average packet size: {} bytes", avg_size);
            println!("  Data rate: {:.2} kbps", rate_kbps);
            println!(
                "  Packet rate: {:.2} pps",
                stats.packet_count as f64 / elapsed
            );
        }

        Ok(stats)
    }
}

#[derive(Default, Debug)]
pub struct DataStats {
    pub packet_count: usize,
    pub total_bytes: usize,
    pub invalid_packets: usize,
    pub dropped_packets: usize,
    pub first_seq: u32,
    pub last_seq: u32,
    pub prev_seq: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = HermesLiteClient::new(0);
        assert!(client.is_ok());
    }
}
