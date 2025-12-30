# rx-888 SDR Driver (Rust)

A native Rust driver for the rx-888 family of software-defined radios (SDR).

Overview
- Implements a user-space driver that exposes an interface similar to librtlsdr, making it easy to reuse existing tooling and examples.
- Written in Rust and uses libusb for cross-platform USB access.

Key points
- Interface: Compatible with librtlsdr-style APIs for discovery, device open/close, and streaming.
- Language: Implemented in Rust for safety and performance.
- USB: Uses libusb (via Rust nusb) to communicate with devices.

Compatibility & Firmware notes
- This project targets the rx-888 family of devices, include RX-888 and RX-888 MK2.
- The original Cypress-based driver is deprecated — this repository provides the modern replacement.
- NOTE: PID/VID values changed in new firmware revisions. If you are using the firmware in this repo, update any udev rules or platform-specific device mappings accordingly.

Building
- Prerequisites: Rust toolchain (stable), libusb development headers/libraries installed on your platform.
- To build (release):

```
cargo build --release
```

Usage
- The library exposes an API similar to librtlsdr; consult the crate docs in `src/sddc` for examples and API details.
- On platforms that require device permissions, ensure your system rules allow access to the rx-888 device (update VID/PID if using newer firmware).

Deprecation
- The older Cypress driver is deprecated in favor of this Rust/libusb implementation. This repo should be used for new development and user deployments.

Contributing
- Contributions, bug reports, and firmware compatibility notes are welcome. Please open issues or pull requests.

License
- See Cargo.toml for license information.

