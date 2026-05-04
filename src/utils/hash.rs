const C1: u32 = 0xcc9e2d51;
const C2: u32 = 0x1b873593;
const R1: u32 = 15;
const R2: u32 = 13;
const M: u32 = 5;
const N: u32 = 0xe6546b64;

#[inline(always)]
pub fn murmur3_32(key: &[u8], seed: u32) -> u32 {
    let mut h = seed;
    let mut chunks = key.chunks_exact(4);

    for chunk in chunks.by_ref() {
        let mut k = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        k = k.wrapping_mul(C1);
        k = k.rotate_left(R1);
        k = k.wrapping_mul(C2);

        h ^= k;
        h = h.rotate_left(R2);
        h = h.wrapping_mul(M).wrapping_add(N);
    }

    let remainder = chunks.remainder();
    if !remainder.is_empty() {
        let mut k = 0u32;
        for (i, &byte) in remainder.iter().enumerate() {
            k |= (byte as u32) << (i * 8);
        }

        k = k.wrapping_mul(C1);
        k = k.rotate_left(R1);
        k = k.wrapping_mul(C2);
        h ^= k;
    }

    h ^= key.len() as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85ebca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2ae35);
    h ^= h >> 16;

    h
}
