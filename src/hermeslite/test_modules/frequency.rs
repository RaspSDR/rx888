use hermeslite::protocol::{MAGIC_DATA_TX, MAGIC_START};
use hermeslite::server::HermesLiteServer;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};

// Helper to build a MAGIC_DATA_TX packet with one control frame setting RX0 frequency
fn build_freq_packet(freq: u32) -> Vec<u8> {
    let mut packet = vec![0u8; 1032];
    packet[0..4].copy_from_slice(&MAGIC_DATA_TX.to_le_bytes());
    let control_offset = 11usize;
    packet[control_offset] = 0x04; // (0x04 >> 1) == 0x02 -> RxFreq(0)
    let be = freq.to_be_bytes();
    packet[control_offset + 1..control_offset + 5].copy_from_slice(&be);
    packet
}

#[test]
fn frequency_callback_invoked() {
    let port = 20042u16;
    let mut server = HermesLiteServer::new_with_port(None, port).expect("server create");

    let changes: Arc<Mutex<Vec<(usize, u64)>>> = Arc::new(Mutex::new(Vec::new()));
    let changes_clone = changes.clone();
    server.set_frequency_change_callback(move |idx, f| {
        changes_clone.lock().unwrap().push((idx, f));
    });

    server.start().expect("server start");

    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind client");
    sock.send_to(&MAGIC_START.to_le_bytes(), ("127.0.0.1", port))
        .expect("send start");

    let packet = build_freq_packet(7_074_000);
    sock.send_to(&packet, ("127.0.0.1", port))
        .expect("send freq packet");

    std::thread::sleep(std::time::Duration::from_millis(200));
    server.stop();

    let data = changes.lock().unwrap();
    assert!(!data.is_empty(), "No frequency changes captured");
    assert_eq!(data[0].0, 0);
    assert_eq!(data[0].1, 7_074_000u64);
}
