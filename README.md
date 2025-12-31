# rx-888 SDR Driver (Rust)

A native Rust driver for the rx-888 family of software-defined radios (SDR).

## Overview

- Implements a user-space driver that exposes an interface similar to librtlsdr, making it easy to reuse existing tooling and examples.
- Written in Rust and uses libusb for cross-platform USB access.
- Expose both Rust and C/C++ inteface for down stream application to consume.

## Key points
- Interface: Compatible with librtlsdr-style APIs for discovery, device open/close, and streaming.
- Language: Implemented in Rust for safety and performance.
- USB: Uses libusb (via Rust nusb) to communicate with devices.

## Compatibility & Firmware notes
- This project targets the rx-888 family of devices, include RX-888 and RX-888 MK2.
- The original Cypress-based driver is deprecated — this repository provides the modern replacement.
- NOTE: PID/VID values changed in new firmware revisions. If you are using the firmware in this repo, update any udev rules or platform-specific device mappings accordingly. The bootloader device is not changed (still 0x04B4/0x00f3) but after uploading firmware, the device PID/VID is 0x04B4/*0x3DDC*.
- The interface between driver and firmware is also changed. It is using a register based approach now. You can find the reference from src/interface.rs, which contains the detailed comments for the interface.

## Building
- Prerequisites: Rust toolchain (stable), libusb development headers/libraries installed on your platform.
- To build (release):

```
cargo build --release
```

- In order to build C dynamic lib, please enable cbinging feature

```
cargo build --release --features cbinding
```

## Unit Testing
- Prerequisites: Plug a rx-888 device on your dev box
- To Test:

```
cargo t --features cbinding
```

## Usage

### Rust Interface

The library exposes a Rust version of API. The following is the core logic of API. You can also rereference the code under src/bin, which demostrate a completely application.

```
    // Open device
    let mut radio = Radio::open(0)?;

    // Configure for typical HF operation
    const SAMPLE_RATE: u32 = 64_000_000; // 64 MHz
    radio.set_xtal_freq(SAMPLE_RATE)?;
    radio.set_direct_sampling(true)?;
    radio.set_center_freq(7_100_000)?; // 7.1 MHz (40m band)
    radio.set_if_gain(30.0)?;
    radio.set_rf_gain(10.0)?;

    radio.read_async(Box::new(move |data: &[i16]| {
        // Captured raw ADC input in data
    }));

    std::thread::sleep(1000);

    radio.read_cancel(); // stop reading

```

### C/C++ Interface

The library exposes an API similar to librtlsdr; read the include/libsddc.h for the details.

The following example is the core logic:

```C
    struct sddc_dev_t *device;
    sddc_open(&device, 0);
    sddc_set_xtal_freq(device, 64000000);
    sddc_set_direct_sampling(device, 1);
    sddc_read_async(device, async_callback); // Note, this API is not blocking!

    // sleep some time

    sddc_cancel_read(device);
    sddc_close(device);
```

The file cmake/sddcTargets.cmake shows how to integrate the logic to download the pre-build release binaries from Github Release page.

Contributing
- Contributions, bug reports, and firmware compatibility notes are welcome. Please open issues or pull requests.

License
- LGPL-2.1-or-later

