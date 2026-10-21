//! Benchmarks blusc decompression of the inner chunks of a zarr v3 sharded file.
//! Usage: cargo run --release --example bench_shard -- <shard_path> [iters]
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&args[1]).unwrap();
    let iters: usize = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(50);
    // Shard index at end: n inner chunks x (u64 offset, u64 nbytes) + 4-byte crc32c.
    let n = 3;
    let idx = &data[data.len() - (n * 16 + 4)..];
    let mut chunks = Vec::new();
    for i in 0..n {
        let off = u64::from_le_bytes(idx[i * 16..i * 16 + 8].try_into().unwrap()) as usize;
        let len = u64::from_le_bytes(idx[i * 16 + 8..i * 16 + 16].try_into().unwrap()) as usize;
        if off == u64::MAX as usize { continue; }
        let c = &data[off..off + len];
        let (nbytes, cbytes, blocksize) = blusc::blosc2_cbuffer_sizes(c);
        println!(
            "chunk {i}: version={} flags={:#010b} typesize={} nbytes={nbytes} cbytes={cbytes} blocksize={blocksize} ratio={:.3}",
            c[0], c[2], c[3], nbytes as f64 / cbytes as f64
        );
        chunks.push(c.to_vec());
    }
    let mut dest = vec![0u8; 1024 * 1024 * 2];
    let total: usize = chunks.len() * dest.len();
    for c in &chunks { assert_eq!(blusc::blosc2_decompress(c, &mut dest), dest.len() as i32); }
    let t = Instant::now();
    for _ in 0..iters {
        for c in &chunks { blusc::blosc2_decompress(c, &mut dest); }
    }
    let s = t.elapsed().as_secs_f64();
    println!("decompress: {:.2} GB/s ({:.2} ms per 2 MB chunk)", (total * iters) as f64 / s / 1e9, s * 1e3 / (iters * chunks.len()) as f64);
}
