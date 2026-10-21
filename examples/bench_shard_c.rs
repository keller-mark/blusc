//! Compares blusc against C blosc2 (blosc2-src) on zarr shard inner chunks.
//! Usage: cargo run --release --example bench_shard_c -- <shard_path>
use std::time::Instant;

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let data = std::fs::read(path).unwrap();
    let idx = &data[data.len() - (3 * 16 + 4)..];
    let chunks: Vec<&[u8]> = (0..3)
        .map(|i| {
            let off = u64::from_le_bytes(idx[i * 16..i * 16 + 8].try_into().unwrap()) as usize;
            let len = u64::from_le_bytes(idx[i * 16 + 8..i * 16 + 16].try_into().unwrap()) as usize;
            &data[off..off + len]
        })
        .collect();
    unsafe { blosc2_src::blosc2_init() };
    let n = 2 * 1024 * 1024;
    let (mut a, mut b) = (vec![0u8; n], vec![0u8; n]);
    for c in &chunks {
        assert_eq!(blusc::blosc2_decompress(c, &mut a), n as i32);
        let r = unsafe { blosc2_src::blosc2_decompress(c.as_ptr().cast(), c.len() as i32, b.as_mut_ptr().cast(), n as i32) };
        assert_eq!(r, n as i32);
        assert!(a == b, "output mismatch vs C");
    }
    println!("outputs identical to C for {} chunks", chunks.len());
    let iters = 50;
    let t = Instant::now();
    for _ in 0..iters { for c in &chunks { blusc::blosc2_decompress(c, &mut a); } }
    let tb = t.elapsed().as_secs_f64();
    let t = Instant::now();
    for _ in 0..iters { for c in &chunks { unsafe { blosc2_src::blosc2_decompress(c.as_ptr().cast(), c.len() as i32, b.as_mut_ptr().cast(), n as i32) }; } }
    let tc = t.elapsed().as_secs_f64();
    let gb = (iters * chunks.len() * n) as f64 / 1e9;
    println!("blusc: {:.2} GB/s   C blosc2 (1 thread): {:.2} GB/s", gb / tb, gb / tc);
    unsafe { blosc2_src::blosc2_destroy() };
}
