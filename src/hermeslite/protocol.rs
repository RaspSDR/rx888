// Hermes Lite Protocol Implementation
// Based on: https://github.com/softerhardware/Hermes-Lite2/wiki/Protocol

use std::net::SocketAddr;

/// Magic packet prefixes
pub const MAGIC_DISCOVERY: u32 = 0x0002feef;
pub const MAGIC_START: u32 = 0x0104feef;
pub const MAGIC_STOP: u32 = 0x0004feef;
pub const MAGIC_DATA_TX: u32 = 0x0201feef;
pub const MAGIC_DATA_RX: u32 = 0x0601feef;

/// Discovery reply packet
#[repr(C)]
pub struct DiscoveryReply {
    pub magic: [u8; 4],       // 0xEF, 0xFE, 0x02, status
    pub mac_address: [u8; 6], // MAC address
    pub board_id: u8,         // Board ID (73 = HL2)
    pub protocol_version: u8, // Protocol version
    pub _reserved: [u8; 9],   // Reserved bytes
    pub num_receivers: u8,    // Number of receivers
    pub _padding: [u8; 2],    // Padding
}

impl DiscoveryReply {
    pub fn new(mac_address: [u8; 6], num_receivers: u8, active: bool) -> Self {
        Self {
            magic: [0xEF, 0xFE, 0x02, if active { 0x03 } else { 0x02 }],
            mac_address,
            board_id: 73, // Hermes Lite 2 identifier
            protocol_version: 0x06,
            _reserved: [0; 9],
            num_receivers,
            _padding: [0; 2],
        }
    }

    pub fn to_bytes(&self) -> [u8; 60] {
        let mut buf = [0u8; 60];
        buf[0..4].copy_from_slice(&self.magic);
        // Place MAC immediately after magic/status without overwriting status byte
        buf[4..10].copy_from_slice(&self.mac_address);
        buf[10] = self.board_id;
        buf[11] = self.protocol_version;
        buf[19] = self.num_receivers;
        buf
    }
}

/// Command packet type (C0 byte parsing)
/// Based on Hermes Lite 2 Protocol: <https://github.com/softerhardware/Hermes-Lite2/wiki/Protocol>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandType {
    General,           // 0x00: Sample rate, receivers, ADC config
    TxFreq,            // 0x01: TX NCO frequency
    RxFreq(usize),     // 0x02-0x08, 0x12-0x16: RX1-RX12 NCO frequencies
    LpfSelect,         // 0x09: TX drive, VNA, PA, filters
    Attenuation,       // 0x0a: LNA gain/attenuation
    TxGainPtt,         // 0x0b-0x0d: Reserved/future use
    LnaGainTx,         // 0x0e: LNA gain during TX
    CwConfig,          // 0x0f: CWX enable
    CwHangTime,        // 0x10: CW hang time
    PttLatency,        // 0x17: PTT hang time and TX buffer latency
    Predistortion,     // 0x2b: Predistortion settings
    MiscCommands,      // 0x39: Watchdog, receiver locking, sync, clock
    ResetOnDisconnect, // 0x3a: Reset HL2 on disconnect
    Ad9866Spi,         // 0x3b: AD9866 SPI access
    I2c1,              // 0x3c: I2C bus 1 access
    I2c2,              // 0x3d: I2C bus 2 access (filter board)
    ErrorResponse,     // 0x3f: Error responses
    Unknown(u8),
}

impl CommandType {
    pub fn from_raw(raw: u8) -> Self {
        match raw {
            0x00 => CommandType::General,
            0x01 => CommandType::TxFreq,
            0x02..=0x08 => CommandType::RxFreq((raw - 0x02) as usize),
            0x09 => CommandType::LpfSelect,
            0x0a => CommandType::Attenuation,
            0x0b..=0x0d => CommandType::TxGainPtt,
            0x0e => CommandType::LnaGainTx,
            0x0f => CommandType::CwConfig,
            0x10 => CommandType::CwHangTime,
            0x11 => CommandType::Unknown(raw), // Reserved
            0x12 => CommandType::RxFreq(7),    // RX8
            0x13 => CommandType::RxFreq(8),    // RX9
            0x14 => CommandType::RxFreq(9),    // RX10
            0x15 => CommandType::RxFreq(10),   // RX11
            0x16 => CommandType::RxFreq(11),   // RX12
            0x17 => CommandType::PttLatency,
            0x18..=0x2a => CommandType::Unknown(raw), // Reserved
            0x2b => CommandType::Predistortion,
            0x2c..=0x38 => CommandType::Unknown(raw), // Reserved
            0x39 => CommandType::MiscCommands,
            0x3a => CommandType::ResetOnDisconnect,
            0x3b => CommandType::Ad9866Spi,
            0x3c => CommandType::I2c1,
            0x3d => CommandType::I2c2,
            0x3e => CommandType::Unknown(raw), // Reserved
            0x3f => CommandType::ErrorResponse,
            other => CommandType::Unknown(other),
        }
    }
}

/// Parsed control packet
#[derive(Debug)]
pub struct ControlPacket {
    pub cmd_type: CommandType,
    pub data: [u8; 4],
}

impl ControlPacket {
    pub fn parse(frame: &[u8]) -> Option<Self> {
        if frame.len() < 5 {
            return None;
        }

        let raw = frame[0] >> 1;
        Some(ControlPacket {
            cmd_type: CommandType::from_raw(raw),
            data: [frame[1], frame[2], frame[3], frame[4]],
        })
    }
}

/// RX configuration from control packets
#[derive(Debug, Clone)]
pub struct RxConfig {
    pub num_receivers: u8,
    pub sample_rate_code: u8, // 0=48k, 1=96k, 2=192k, 3=384k
    pub adc_config: u8,
    #[allow(dead_code)]
    pub attenuation_db: u8, // last received attenuation value (0..=127 dB scaled)
    #[allow(dead_code)]
    pub lpf_select: u8, // last received LPF selection raw code
}

impl Default for RxConfig {
    fn default() -> Self {
        Self {
            num_receivers: 1,
            sample_rate_code: 0, // 48 kHz
            adc_config: 0,
            attenuation_db: 0,
            lpf_select: 0,
        }
    }
}

impl RxConfig {
    pub fn parse_general_packet(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }

        Some(RxConfig {
            num_receivers: ((data[3] >> 3) & 0x7) + 1,
            sample_rate_code: data[0] & 0x3,
            adc_config: (data[2] >> 2) & 0x3,
            attenuation_db: 0,
            lpf_select: 0,
        })
    }

    /// Get decimation factor for given sample rate code
    pub fn get_decimation(&self) -> u16 {
        match self.sample_rate_code {
            0 => 1280, // 48 kHz (from 61.44 MHz)
            1 => 640,  // 96 kHz
            2 => 320,  // 192 kHz
            3 => 160,  // 384 kHz
            _ => 1280,
        }
    }

    /// Get output sample rate in Hz
    pub fn get_sample_rate(&self) -> u32 {
        match self.sample_rate_code {
            0 => 48_000,
            1 => 96_000,
            2 => 192_000,
            3 => 384_000,
            _ => 48_000,
        }
    }
}

/// Data packet headers (sent in rotation)
pub const HEADERS: [[u8; 8]; 5] = [
    [127, 127, 127, 0, 0, 33, 17, 25],
    [127, 127, 127, 8, 0, 0, 0, 0],
    [127, 127, 127, 16, 0, 0, 0, 0],
    [127, 127, 127, 24, 0, 0, 0, 0],
    [127, 127, 127, 32, 66, 66, 66, 66],
];

/// Data packet builder for Hermes Lite protocol
pub struct DataPacketBuilder {
    sequence: u32,
    header_idx: usize,
}

impl Default for DataPacketBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DataPacketBuilder {
    pub fn new() -> Self {
        Self {
            sequence: 0,
            header_idx: 0,
        }
    }

    /// Build a data packet with IQ samples from multiple receivers
    /// Each receiver contributes 3 bytes I + 3 bytes Q = 6 bytes per sample
    /// Returns a 1032-byte UDP packet
    pub fn build_packet(&mut self, samples: &[Vec<(i32, i32)>]) -> Vec<u8> {
        let mut packet = vec![0u8; 1032];

        // Magic number
        packet[0..4].copy_from_slice(&MAGIC_DATA_RX.to_le_bytes());

        // Sequence number
        packet[4..8].copy_from_slice(&self.sequence.to_be_bytes());
        self.sequence = self.sequence.wrapping_add(1);

        let num_receivers = samples.len();
        let samples_per_receiver = if !samples.is_empty() {
            samples[0].len()
        } else {
            0
        };

        // Each UDP packet contains 2 frames of 512 bytes
        for frame_idx in 0..2 {
            let frame_offset = 8 + frame_idx * 512;

            // Write header
            let header = &HEADERS[self.header_idx];
            packet[frame_offset..frame_offset + 8].copy_from_slice(header);
            self.header_idx = (self.header_idx + 1) % HEADERS.len();

            // Write IQ samples
            let mut write_offset = frame_offset + 8;
            let samples_this_frame = samples_per_receiver / 2; // Split across 2 frames

            for sample_idx in 0..samples_this_frame {
                let global_sample_idx = frame_idx * samples_this_frame + sample_idx;

                if global_sample_idx >= samples_per_receiver {
                    break;
                }

                // Interleave receivers
                for rx_idx in 0..num_receivers {
                    if rx_idx < samples.len() && global_sample_idx < samples[rx_idx].len() {
                        let (i, q) = samples[rx_idx][global_sample_idx];

                        // Write 24-bit I value (big-endian, left-aligned in 32-bit)
                        let i_bytes = i.to_be_bytes();
                        packet[write_offset] = i_bytes[1];
                        packet[write_offset + 1] = i_bytes[2];
                        packet[write_offset + 2] = i_bytes[3];

                        // Write 24-bit Q value
                        let q_bytes = q.to_be_bytes();
                        packet[write_offset + 3] = q_bytes[1];
                        packet[write_offset + 4] = q_bytes[2];
                        packet[write_offset + 5] = q_bytes[3];

                        write_offset += 6;
                    }
                }
            }
        }

        packet
    }
}

/// Client connection info
#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub addr: SocketAddr,
    pub active: bool,
}

impl ClientInfo {
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            active: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovery_reply() {
        let mac = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let reply = DiscoveryReply::new(mac, 4, false);
        let bytes = reply.to_bytes();

        assert_eq!(bytes[0], 0xEF);
        assert_eq!(bytes[1], 0xFE);
        assert_eq!(bytes[2], 0x02);
        assert_eq!(bytes[3], 0x02); // inactive
        assert_eq!(&bytes[4..10], &mac);
        assert_eq!(bytes[10], 73);
        assert_eq!(bytes[19], 4);
    }

    #[test]
    fn test_rx_config_parse() {
        let data = [0x00, 0x00, 0x00, 0x18]; // 4 receivers (3 << 3 + 1 = 4)
        let config = RxConfig::parse_general_packet(&data).unwrap();
        assert_eq!(config.num_receivers, 4);
    }

    #[test]
    fn test_sample_rates() {
        let mut config = RxConfig {
            sample_rate_code: 0,
            ..Default::default()
        };

        assert_eq!(config.get_sample_rate(), 48_000);
        assert_eq!(config.get_decimation(), 1280);

        config.sample_rate_code = 3;
        assert_eq!(config.get_sample_rate(), 384_000);
        assert_eq!(config.get_decimation(), 160);
    }
}
