//! Simple wall-clock benchmark for decompression throughput.
//!
//! Run with: `cargo run --release --example bench_decompress`

use blusc::*;
use std::time::Instant;

fn make_data(n_elems: usize, typesize: usize) -> Vec<u8> {
    // Smoothly varying values with a little pseudo-random noise, which is
    // representative of scientific array data (compressible after shuffle).
    let mut out = Vec::with_capacity(n_elems * typesize);
    let mut state: u64 = 0x9E3779B97F4A7C15;
    for i in 0..n_elems {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let noise = if state & 0xF == 0 { 1 } else { 0 };
        let v = (i as u64 / 32).wrapping_add(noise);
        out.extend_from_slice(&v.to_le_bytes()[..typesize]);
    }
    out
}

fn main() {
    let nbytes = 16 * 1024 * 1024;
    let iters = 20;
    let codecs = [
        ("blosclz", BLOSC_BLOSCLZ),
        ("lz4", BLOSC_LZ4),
        ("snappy", BLOSC_SNAPPY),
        ("zlib", BLOSC_ZLIB),
        ("zstd", BLOSC_ZSTD),
    ];
    let filters = [
        ("noshuffle", BLOSC_NOSHUFFLE),
        ("shuffle", BLOSC_SHUFFLE),
        ("bitshuffle", BLOSC_BITSHUFFLE),
    ];

    for typesize in [4usize, 8] {
        let src = make_data(nbytes / typesize, typesize);
        for (cname, compcode) in codecs {
            for (fname, filter) in filters {
                let mut cparams = BLOSC2_CPARAMS_DEFAULTS;
                cparams.compcode = compcode;
                cparams.typesize = typesize as i32;
                cparams.filters[5] = filter;
                let cctx = blosc2_create_cctx(cparams);
                let mut compressed = vec![0u8; src.len() + BLOSC2_MAX_OVERHEAD];
                let cbytes = blosc2_compress_ctx(&cctx, &src, &mut compressed);
                assert!(cbytes > 0);
                compressed.truncate(cbytes as usize);

                let mut dest = vec![0u8; src.len()];
                // Warm-up + correctness check.
                assert_eq!(blosc2_decompress(&compressed, &mut dest), src.len() as i32);
                assert!(dest == src);

                let t0 = Instant::now();
                for _ in 0..iters {
                    blosc2_decompress(&compressed, &mut dest);
                }
                let secs = t0.elapsed().as_secs_f64();
                let gbps = (src.len() * iters) as f64 / secs / 1e9;

                // getitem on a range spanning a few blocks in the middle.
                let start = (src.len() / typesize) / 3;
                let nitems = 100_000usize;
                let mut gdest = vec![0u8; nitems * typesize];
                let t1 = Instant::now();
                for _ in 0..iters {
                    blosc1_getitem(&compressed, start as i32, nitems as i32, &mut gdest);
                }
                let gsecs = t1.elapsed().as_secs_f64() / iters as f64;
                assert!(gdest[..] == src[start * typesize..(start + nitems) * typesize]);

                println!(
                    "ts={typesize} {cname:8} {fname:10} ratio={:6.2} decompress={gbps:6.2} GB/s getitem={:8.1} us",
                    src.len() as f64 / cbytes as f64,
                    gsecs * 1e6
                );
            }
        }
    }
}
