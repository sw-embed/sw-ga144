use sw_ga144::{ENCODING, Fault, Ga144, Image, NodeId};

fn node(number: usize) -> NodeId {
    NodeId::from_number(number).unwrap()
}

fn listing(words: &[u32]) -> Image {
    words
        .iter()
        .enumerate()
        .map(|(address, word)| format!("{address} {word}\n"))
        .collect::<String>()
        .parse()
        .unwrap()
}

fn packed(opcodes: [u32; 4]) -> u32 {
    let [first, second, third, fourth] = opcodes;
    (first << 13 | second << 8 | third << 3 | fourth >> 2) ^ ENCODING
}

#[test]
fn literal_increment_preserves_mode_and_wraps_ram() {
    let mut chip = Ga144::new();
    let image = format!("63 {}\n0 17", packed([8, 0, 0, 0]))
        .parse()
        .unwrap();
    chip.load_ram(node(0), &image);
    chip.start(node(0), 0x27f).unwrap();
    chip.tick().unwrap();
    chip.tick().unwrap();
    let state = chip.inspect(node(0));
    assert_eq!((state.p, state.t), (0x201, 17));
}

#[test]
fn executed_port_word_does_not_advance_p() {
    let mut chip = Ga144::new();
    let writer = listing(&[
        packed([8, 30, 8, 28]),
        0x1d5,
        packed([17, 0, 0, 0]),
        packed([14, 0, 0, 0]),
    ]);
    chip.load_ram(node(0), &writer);
    chip.start(node(0), 0).unwrap();
    chip.start(node(1), 0x1d5).unwrap();
    assert_eq!(chip.tick().unwrap(), 1);
    assert_eq!(chip.inspect(node(1)).waiting_address, Some(0x1d5));
    for _ in 0..7 {
        chip.tick().unwrap();
    }
    let state = chip.inspect(node(1));
    assert_eq!(state.p, 0x1d5);
    assert_eq!(state.slot, 1);
}

#[test]
fn multiport_write_broadcasts_to_ready_readers() {
    let mut chip = Ga144::new();
    // At node 000, right and down lead to nodes 001 and 100.
    let writer = listing(&[packed([8, 30, 8, 28]), 0x195, 42, packed([14, 0, 0, 0])]);
    chip.load_ram(node(0), &writer);
    chip.start(node(0), 0).unwrap();
    for (id, port) in [(1, 0x1d5), (100, 0x115)] {
        chip.load_ram(node(id), &listing(&[packed([8, 30, 10, 0]), port]));
        chip.start(node(id), 0).unwrap();
    }
    for _ in 0..7 {
        chip.tick().unwrap();
    }
    assert_eq!(chip.inspect(node(1)).t, 42);
    assert_eq!(chip.inspect(node(100)).t, 42);
}

#[test]
fn disconnected_port_suspends_and_unsupported_bus_address_faults() {
    let mut chip = Ga144::new();
    chip.start(node(0), 0x175).unwrap();
    assert_eq!(chip.tick().unwrap(), 0);
    assert_eq!(chip.inspect(node(0)).waiting_address, Some(0x175));

    chip.start(node(0), 0x80).unwrap();
    assert_eq!(chip.tick().unwrap_err().fault, Fault::Unmapped(0x80));
}

#[test]
fn image_validation_checks_bounds_and_duplicate_addresses() {
    for (input, line) in [
        ("64 0", 1),
        ("0 0\n\n0 1", 3),
        ("0 262144", 1),
        ("0 1 2", 1),
    ] {
        assert_eq!(input.parse::<Image>().unwrap_err().line, line);
    }
}
