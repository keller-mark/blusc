//! Checks whether blusc zstd output is byte-identical to C blosc2 (blosc2-src).
//! Usage: cargo run --release --example zstd_vs_c
use blusc::*;

fn main() {
    unsafe { blosc2_src::blosc2_init() };
    let (mut same, mut total) = (0, 0);
    for typesize in [1usize, 2, 4, 8] {
        let src: Vec<u8> = (0..1_000_000usize).map(|i| ((i / 37) ^ (i % 251 / 50)) as u8).collect();
        for filter in [BLOSC_NOSHUFFLE, BLOSC_SHUFFLE, BLOSC_BITSHUFFLE] {
            for clevel in 1..=9u8 {
                let mut cp = BLOSC2_CPARAMS_DEFAULTS;
                cp.compcode = BLOSC_ZSTD;
                cp.typesize = typesize as i32;
                cp.filters[5] = filter;
                cp.clevel = clevel;
                let mut a = vec![0u8; src.len() + BLOSC2_MAX_OVERHEAD];
                let na = blosc2_compress_ctx(&blosc2_create_cctx(cp), &src, &mut a) as usize;

                let mut b = vec![0u8; src.len() + BLOSC2_MAX_OVERHEAD];
                let nb = unsafe {
                    let mut cpc = blosc2_src::BLOSC2_CPARAMS_DEFAULTS;
                    cpc.compcode = BLOSC_ZSTD;
                    cpc.typesize = typesize as i32;
                    cpc.filters[5] = filter;
                    cpc.clevel = clevel;
                    let ctx = blosc2_src::blosc2_create_cctx(cpc);
                    let n = blosc2_src::blosc2_compress_ctx(ctx, src.as_ptr().cast(), src.len() as i32, b.as_mut_ptr().cast(), b.len() as i32);
                    blosc2_src::blosc2_free_ctx(ctx);
                    n as usize
                };
                total += 1;
                if a[..na] == b[..nb] { same += 1; } else {
                    println!("differs: ts={typesize} filter={filter} clevel={clevel} blusc={na} C={nb}");
                }
            }
        }
    }
    println!("{same}/{total} configurations byte-identical to C");
    unsafe { blosc2_src::blosc2_destroy() };
}
