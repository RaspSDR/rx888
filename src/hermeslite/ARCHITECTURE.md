# Hermes Lite Server Architecture

## Overview

This document describes the technical architecture of the Hermes Lite protocol server implementation for RX888 SDR.

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Client Software                          │
│              (SDR Console, PowerSDR, Quisk)                  │
└───────────────────┬─────────────────────────────────────────┘
                    │ UDP Port 1024
                    │ Hermes Lite Protocol
┌───────────────────▼─────────────────────────────────────────┐
│                  HermesLiteServer                            │
│  ┌────────────────────────────┐  ┌────────────────────────┐ │
│  │   Command Handler Thread   │  │  Data Handler Thread   │ │
│  │  - Discovery replies       │  │  - Sample buffering    │ │
│  │  - Start/Stop commands     │  │  - Packet building     │ │
│  │  - Control parsing         │  │  - UDP transmission    │ │
│  └────────────────────────────┘  └────────────────────────┘ │
│                                                               │
│  ┌───────────────────────────────────────────────────────┐  │
│  │          Channel Callbacks (8x)                       │  │
│  │  - Complex32 → 24-bit I/Q conversion                  │  │
│  │  - Per-channel sample buffering                       │  │
│  └───────────────────────────────────────────────────────┘  │
└───────────────────┬─────────────────────────────────────────┘
                    │ Callback Interface
┌───────────────────▼─────────────────────────────────────────┐
│                    VirtualRadio                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  FFT Processing (Main Thread)                          │ │
│  │  - Forward R2C FFT (8192 → 4096 complex)              │ │
│  │  - Broadcast spectrum to all channels                  │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                               │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐           │
│  │ Channel 0   │ │ Channel 1   │ │ Channel N   │  ...      │
│  │ - Freq shift│ │ - Freq shift│ │ - Freq shift│           │
│  │ - FIR filter│ │ - FIR filter│ │ - FIR filter│           │
│  │ - Decimation│ │ - Decimation│ │ - Decimation│           │
│  │ - Inverse   │ │ - Inverse   │ │ - Inverse   │           │
│  │   FFT       │ │   FFT       │ │   FFT       │           │
│  └─────────────┘ └─────────────┘ └─────────────┘           │
└───────────────────┬─────────────────────────────────────────┘
                    │ USB Interface (libusb/nusb)
┌───────────────────▼─────────────────────────────────────────┐
│                  RX888 Hardware                              │
│              ADC @ 64 MHz (or configured rate)               │
└─────────────────────────────────────────────────────────────┘
```

## Module Breakdown

### 1. Protocol Module (`protocol.rs`)

**Purpose**: Hermes Lite protocol implementation

**Key Components**:
- `DiscoveryReply`: 60-byte discovery response packet
- `ControlPacket`: Parser for 5-byte control frames
- `RxConfig`: Receiver configuration state
- `DataPacketBuilder`: Constructs 1032-byte UDP data packets
- `HEADERS`: Rotating 8-byte header patterns

**Protocol Details**:
- Magic numbers for packet identification
- 24-bit I/Q sample encoding (big-endian)
- Sequence number tracking
- Sample rate codes (0=48k, 1=96k, 2=192k, 3=384k)

### 2. Server Module (`server.rs`)

**Purpose**: Network server and data pipeline

**Key Components**:

#### HermesLiteServer
Main server structure managing:
- UDP socket (port 1024)
- Client connection state
- Multi-threaded packet handling
- Sample buffering per channel

#### ChannelState
Per-channel sample buffer:
- Stores (I, Q) 24-bit pairs
- Accumulates samples until packet-ready
- Thread-safe via Mutex

#### Thread Model

**Command Handler Thread**:
```rust
loop {
    recv_from(socket) // 100ms timeout
    match magic {
        DISCOVERY → send discovery_reply
        START     → activate_client
        STOP      → deactivate_client
        DATA_TX   → parse_control_frames
    }
}
```

**Data Handler Thread**:
```rust
loop {
    if client.active {
        if all_channels_have_126_samples() {
            samples = collect_from_all_channels()
            packet = build_hermes_packet(samples)
            send_to(client, packet)
        }
    }
    sleep(100us)
}
```

### 3. Main Binary (`main.rs`)

**Purpose**: CLI interface and system integration

**Initialization Flow**:
1. Parse command-line arguments (clap)
2. Open RX888 device
3. Create VirtualRadio with specified sample rate
4. Configure physical radio (freq, gain, direct sampling)
5. Create HermesLiteServer
6. Create virtual channels with callbacks
7. Start VirtualRadio streaming
8. Install Ctrl+C handler
9. Main loop (wait for shutdown)
10. Cleanup (stop radio, stop server)

**Callback Integration**:
```rust
let callback = server.create_channel_callback(channel_idx);

vradio.create_channel(
    VirtualChannelConfig { 
        center_freq, 
        lsb, 
        decimation 
    },
    callback
)?;
```

## Data Flow

### Receive Path (ADC to Network)

```
1. RX888 ADC samples @ 64 MHz
   ↓ (USB streaming)
   
2. VirtualRadio receives buffer
   ↓ (overlap-save windowing)
   
3. Forward R2C FFT (8192 real → 4096 complex)
   ↓ (shared spectrum)
   
4. Per-channel processing:
   - Frequency shift (tunebin rotation)
   - FIR filtering (Kaiser window)
   - Inverse C2C FFT (decimation)
   ↓ (Complex32 samples)
   
5. Channel callback invoked
   - Convert float to 24-bit I/Q
   - Add to channel buffer
   ↓ (sample accumulation)
   
6. Data thread checks buffers
   - Collect 126 samples per channel
   - Interleave samples from all channels
   - Format Hermes Lite packet
   ↓ (1032-byte UDP packet)
   
7. Send to client
```

### Control Path (Network to Radio)

```
1. Client sends control packet
   ↓ (UDP port 1024)
   
2. Command handler receives
   ↓ (parse magic)
   
3. Extract control frames
   ↓ (C0 byte determines type)
   
4. Process command:
   - General: update RxConfig
   - Frequency: (future) update VirtualChannel
   - Attenuation: (future) update gains
   
5. Apply changes to radio/channels
```

## Threading Model

### Main Thread
- Runs CLI loop
- Handles Ctrl+C
- Manages VirtualRadio lifecycle

### Command Thread (server)
- Blocking recv_from with timeout
- Processes discovery, start, stop
- Parses embedded control packets
- Updates shared state (Mutex)

### Data Thread (server)
- Busy loop with sleep
- Checks channel buffers
- Builds and sends packets
- No blocking operations

### VirtualRadio Threads
- USB read thread (in Radio)
- FFT processing thread
- Per-channel worker threads
- Internal to VirtualRadio

## Synchronization

### Shared State Protection

```rust
// Server state
Arc<Mutex<RxConfig>>          // Read by data thread, written by command
Arc<Mutex<Option<ClientInfo>>> // Read/write by both threads
Arc<Mutex<Vec<ChannelState>>>  // Written by callbacks, read by data thread
Arc<AtomicBool>                // Running flag, lock-free

// Channel callbacks
move |idx, samples| {
    let mut states = channel_states.lock().unwrap();
    states[idx].add_sample(i, q);
}
```

### Lock Ordering (prevents deadlock)
1. Never hold multiple locks simultaneously
2. Acquire lock, do work, release immediately
3. Use atomic bool for simple flags

## Performance Characteristics

### CPU Usage (per receiver)
- FFT forward: ~5% (shared across all)
- FFT inverse: ~3% each
- Sample conversion: ~1% each
- Network I/O: <1%
- **Total**: ~5% + (4% × N receivers)

### Memory Usage (per receiver)
- FFT buffers: 256 KB
- Sample buffers: 100 KB
- Filter coefficients: 32 KB
- **Total**: ~400 KB per receiver

### Latency Budget
| Component | Typical | Max |
|-----------|---------|-----|
| USB transfer | 1ms | 5ms |
| FFT processing | 1ms | 2ms |
| Sample buffering | 1-3ms | 10ms |
| Network transmission | <1ms | 5ms |
| **Total** | **3-5ms** | **22ms** |

### Network Bandwidth
```
Packet size: 1032 bytes
Sample rate: 48 kHz
Samples per packet: 126
Packets per second: 48000 / 126 = 381

Bandwidth per receiver: 1032 × 381 = 393 KB/s = 3.1 Mbps
For 4 receivers: ~12.5 Mbps
For 8 receivers: ~25 Mbps
```

## Protocol Compliance

### Implemented Features
✓ Discovery broadcast response  
✓ Start/Stop commands  
✓ Multi-receiver support (1-8)  
✓ Sample rate codes (48/96/192/384 kHz)  
✓ 24-bit I/Q data format  
✓ Sequence numbering  
✓ Header rotation  
✓ Control frame parsing  

### Planned Features
○ Dynamic frequency tuning  
○ Attenuation control  
○ LSB/USB mode switching  
○ Transmit support (RX-only currently)  

### Differences from Reference
- Uses VirtualRadio instead of FPGA for channelization
- Software-defined sample rate (not hardware decimation)
- No direct hardware register access (uses Radio API)

## Error Handling

### Graceful Degradation
- Socket timeout: continue (not fatal)
- Client disconnect: deactivate, continue serving
- Buffer underflow: send zeros, continue
- USB error: bubble up to main (restart needed)

### Recovery Strategies
```rust
// Example: command socket timeout
Err(e) if e.kind() == WouldBlock => {
    // Not an error, just no data
    continue;
}

// Example: send failure
if let Err(e) = socket.send_to(&packet, addr) {
    eprintln!("Send failed: {}", e);
    // Continue anyway, client may recover
}
```

## Future Enhancements

### 1. Dynamic Tuning
Allow client to retune receivers without restart:
- Parse RX frequency commands from control packets
- Call `vradio.set_channel_center_freq(idx, freq)`
- No streaming interruption

### 2. Direct Sampling Auto-Switch
Automatically switch modes based on frequency:
```rust
if new_freq < 30_000_000 && !direct_sampling {
    vradio.stop();
    vradio.set_direct_sampling(true);
    vradio.start();
}
```

### 3. TX Support
Add transmit path:
- Parse TX control packets
- Modulate IQ data
- Send to Radio TX API (when available)

### 4. Web Interface
Add HTTP server for:
- Status monitoring
- Configuration changes
- Spectrum display
- Log viewing

## Testing Strategy

### Unit Tests
- Protocol packet encoding/decoding
- RxConfig parsing
- Sample conversion accuracy
- Buffer management

### Integration Tests
- Server start/stop
- Client discovery
- Data streaming
- Control command handling

### Hardware Tests
- USB communication
- Sample rate accuracy
- Multi-channel operation
- Long-term stability

## Build and Deployment

### Release Build
```powershell
cargo build --release --bin hermeslite
```

Optimizations applied:
- LTO (Link Time Optimization)
- opt-level = 3
- Single codegen unit
- Debug symbols stripped

### Binary Size
- Debug: ~15 MB
- Release: ~3 MB (after strip)

### Dependencies
- Runtime: None (static linking)
- Hardware: RX888 SDR with firmware 1.8+
- OS: Windows (nusb WinUSB), Linux (libusb)

## References

- [Hermes Lite Protocol](https://github.com/softerhardware/Hermes-Lite2/wiki/Protocol)
- [Reference Implementation](../hermeslite/reference.cpp)
- [VirtualRadio Documentation](../../VIRTUAL_SDR.md)
- [RX888 Firmware](../../src/firmware/)
