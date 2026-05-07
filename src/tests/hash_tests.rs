use crate::utils::hash::murmur3_32;

#[test]
fn test_murmur3_consistency() {
    let test_cases = vec![
        (&b""[..], 0u32, 0u32),
        (&b"a"[..], 0, 0x3c2569b2),
        (&b"ab"[..], 0, 0x9bbfd75f),
        (&b"abc"[..], 0, 0xb3dd93fa),
        (&b"abcd"[..], 0, 0x43ed676a),
        (&b"hello"[..], 0, 0x248bfa47),
        (&b"hello world"[..], 0, 0x5e928f0f),
    ];

    for (input, seed, expected) in test_cases {
        let result = murmur3_32(input, seed);
        assert_eq!(result, expected, "Failed for input: {:?}", input);
    }
}

#[test]
fn test_murmur3_different_seeds() {
    let data = b"test data";

    let hash1 = murmur3_32(data, 0);
    let hash2 = murmur3_32(data, 1);
    let hash3 = murmur3_32(data, 12345);

    // Different seeds should produce different hashes
    assert_ne!(hash1, hash2);
    assert_ne!(hash1, hash3);
    assert_ne!(hash2, hash3);
}

#[test]
fn test_murmur3_various_lengths() {
    // Test with various input lengths to cover different code paths
    let test_cases = vec![
        vec![],              // Empty
        vec![1],             // 1 byte
        vec![1, 2, 3],       // 3 bytes (< 4)
        vec![1, 2, 3, 4],    // Exactly 4 bytes
        vec![1, 2, 3, 4, 5], // 5 bytes (> 4)
        vec![1; 100],        // Large input
    ];

    for data in test_cases {
        let hash = murmur3_32(&data, 0);
        // Just verify it doesn't panic and produces a value
        assert!(hash != 0 || data.is_empty());
    }
}

#[test]
fn test_murmur3_known_values() {
    // Test against known good values for murmur3
    assert_eq!(murmur3_32(b"", 0), 0);
    assert_eq!(murmur3_32(b"", 1), 0x514e28b7);
    assert_eq!(murmur3_32(b"", 0xffffffff), 0x81f16f39);

    assert_eq!(murmur3_32(b"\0\0\0\0", 0), 0x2362f9de);
    assert_eq!(murmur3_32(b"aaaa", 0x9747b28c), 0x5a97808a);
    assert_eq!(murmur3_32(b"Hello, world!", 0x9747b28c), 0x24884cba);
}