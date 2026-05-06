use std::ptr;

use fun_kv::{constants::{ALIGNMENT, BLOCK_SIZE}, utils::allocator::AlignedBuffer};
use rand::rand_core::block::BlockRng;

#[test]
fn test_aligned_buffer_creation() {
    let buffer = AlignedBuffer::new(BLOCK_SIZE).unwrap();

    assert_eq!(buffer.len(), 0);
    assert_eq!(buffer.capacity(), BLOCK_SIZE);
}

#[test]
fn test_aligned_buffer_alignment() {
    let buffer = AlignedBuffer::new(BLOCK_SIZE).unwrap();

    let ptr = buffer.as_ptr() as usize;
    assert_eq!(ptr % ALIGNMENT, 0);
}

#[test]
fn test_aligned_buffer_write_read() {
    let mut buffer = AlignedBuffer::new(BLOCK_SIZE).unwrap();

    let data = vec![0x10u8; 512];
    unsafe {
        ptr::copy_nonoverlapping(data.as_ptr(), buffer.as_mut_ptr(), data.len());
    }
    buffer.set_len(data.len());

    assert_eq!(buffer.len(), data.len());
    assert_eq!(buffer.as_slice(), &data[..]);
}

#[test]
fn test_aligned_buffer_set_len() {
    let mut buffer = AlignedBuffer::new(BLOCK_SIZE).unwrap();

    assert_eq!(buffer.len(), 0);
    
    buffer.set_len(10);
    assert_eq!(buffer.len(), 10);

    buffer.set_len(3);
    assert_eq!(buffer.len(), 3);
}

#[test]
fn test_aligned_buffer_extend() {
    let mut buffer = AlignedBuffer::new(BLOCK_SIZE).unwrap();

    unsafe {
        ptr::copy_nonoverlapping([1u8, 2, 3].as_ptr(), buffer.as_mut_ptr(), 3);
    }
    buffer.set_len(3);
    assert_eq!(buffer.as_slice(), &[1, 2, 3]);

    unsafe {
        ptr::copy_nonoverlapping([4u8, 5, 6].as_ptr(), buffer.as_mut_ptr().add(3), 3);
    }
    buffer.set_len(6);
    assert_eq!(buffer.as_slice(), &[1, 2, 3, 4, 5, 6])
}

#[test]
fn test_aligned_buffer_clear() {
    let mut buffer = AlignedBuffer::new(BLOCK_SIZE).unwrap();

    unsafe {
        ptr::copy_nonoverlapping([1u8, 2, 3, 4, 5, 6].as_ptr(), buffer.as_mut_ptr(), 6);
    }
    buffer.set_len(6);
    assert_eq!(buffer.as_slice(), &[1, 2 , 3, 4, 5, 6]);

    buffer.clear();
    assert_eq!(buffer.as_slice(), &[]);
}

#[test]
fn test_aligned_buffer_mut_access() {
    let mut buffer = AlignedBuffer::new(BLOCK_SIZE).unwrap();

    unsafe {
        ptr::write_bytes(buffer.as_mut_ptr(), 0, 10);
    }
    buffer.set_len(10);

    let slice = buffer.as_mut_slice();
    slice[0] = 0xff;
    slice[9] = 0xee;

    assert_eq!(buffer.as_slice()[0], 0xff);
    assert_eq!(buffer.as_slice()[9], 0xee);
}

#[test]
fn test_multiple_sized_alignments() {
    let sizes = vec![512, 1024, 4096, 8192];

    for size in sizes {
        let buffer = AlignedBuffer::new(size).unwrap();
        let ptr = buffer.as_ptr() as usize;
        assert_eq!(ptr % ALIGNMENT, 0, "Failed for size {}", size);
    }
}

#[test]
fn test_aligned_buffer_capacity_exact() {
    let size = 16384 - 1; // Non-power-of-2 size
    let buffer = AlignedBuffer::new(size).unwrap();

    assert_eq!(buffer.capacity(), 16384);
}

#[test]
fn test_aligned_buffer_zero_size() {
    let buffer = AlignedBuffer::new(0).unwrap();
    assert_eq!(buffer.len(), 0);
    assert_eq!(buffer.capacity(), 0);
}

#[test]
fn test_aligned_buffer_large_size() {
    let size = 1024 * 1024; // 1MB
    let mut buffer = AlignedBuffer::new(size).unwrap();

    let pattern = vec![0xAB; size];
    unsafe {
        std::ptr::copy_nonoverlapping(pattern.as_ptr(), buffer.as_mut_ptr(), size);
    }
    buffer.set_len(size);

    assert_eq!(buffer.len(), size);
    assert_eq!(buffer.as_slice(), &pattern[..]);
}

#[test]
fn test_aligned_buffer_reuse() {
    let mut buffer = AlignedBuffer::new(1024).unwrap();

    for i in 0..10 {
        buffer.clear();
        let data = [i as u8; 100];
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), buffer.as_mut_ptr(), 100);
        }
        buffer.set_len(100);
        assert_eq!(buffer.as_slice(), &data[..]);
    }
}