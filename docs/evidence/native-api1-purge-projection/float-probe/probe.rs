fn main() {
    for value in [0.7_f32, 0.3_f32, 0.05_f32, 0.9_f32, 3.7145_f32] {
        let encoded = serde_json::to_value(value).unwrap();
        let text = encoded.to_string();
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        let typed: f32 = serde_json::from_value(parsed.clone()).unwrap();
        let reconstructed = serde_json::to_value(typed).unwrap();
        println!("bits={:08x} wire={} parsed={} reconstructed={} value_equal={} bits_equal={}", value.to_bits(), text, parsed, reconstructed, parsed == reconstructed, value.to_bits() == typed.to_bits());
        assert_eq!(value.to_bits(), typed.to_bits());
    }
}
