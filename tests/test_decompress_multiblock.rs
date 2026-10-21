/// Cross-validates blusc decompression and getitem on multi-block chunks compressed by
/// the C reference, for every codec and filter. Exercises the shared block decoder
/// (reused scratch buffers and codec contexts across blocks/streams), the leftover
/// block, whole-block getitem (decoded straight into dest), and partial-block getitem
/// (partial unshuffle / in-place bitunshuffle).
use blosc2_src::{
    blosc2_compress_ctx as bound_blosc2_compress_ctx,
    blosc2_create_cctx as bound_blosc2_create_cctx, blosc2_destroy as bound_blosc2_destroy,
    blosc2_free_ctx as bound_blosc2_free_ctx, blosc2_init as bound_blosc2_init,
    BLOSC2_CPARAMS_DEFAULTS as BOUND_BLOSC2_CPARAMS_DEFAULTS,
};
use blusc::api::{
    blosc1_getitem, blosc2_cbuffer_sizes, blosc2_compress_ctx, blosc2_create_cctx,
    blosc2_decompress, BLOSC2_CPARAMS_DEFAULTS,
};
use blusc::{
    BLOSC2_MAX_OVERHEAD, BLOSC_BITSHUFFLE, BLOSC_BLOSCLZ, BLOSC_LZ4, BLOSC_LZ4HC, BLOSC_NOSHUFFLE,
    BLOSC_SHUFFLE, BLOSC_SNAPPY, BLOSC_ZLIB, BLOSC_ZSTD,
};

use ctor::{ctor, dtor};

#[ctor]
fn blosc2_init() {
    unsafe {
        bound_blosc2_init();
    }
}

#[dtor]
fn blosc2_cleanup() {
    unsafe {
        bound_blosc2_destroy();
    }
}

/// Compressible data with some structure in every byte position.
fn make_data(nbytes: usize) -> Vec<u8> {
    let mut state: u32 = 12345;
    (0..nbytes)
        .map(|i| {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            let noise = if (state >> 16) & 0xF == 0 { 1 } else { 0 };
            ((i / 64) as u8).wrapping_add(noise)
        })
        .collect()
}

fn c_compress(src: &[u8], typesize: i32, compcode: u8, filter: u8, clevel: u8) -> Option<Vec<u8>> {
    let mut compressed = vec![0u8; src.len() + BLOSC2_MAX_OVERHEAD];
    let csize = unsafe {
        let mut cparams = BOUND_BLOSC2_CPARAMS_DEFAULTS;
        cparams.typesize = typesize;
        cparams.compcode = compcode;
        cparams.filters[5] = filter;
        cparams.clevel = clevel;
        let cctx = bound_blosc2_create_cctx(cparams);
        let csize = bound_blosc2_compress_ctx(
            cctx,
            src.as_ptr().cast(),
            src.len() as i32,
            compressed.as_mut_ptr().cast(),
            compressed.len() as i32,
        );
        bound_blosc2_free_ctx(cctx);
        csize
    };
    if csize <= 0 {
        return None;
    }
    compressed.truncate(csize as usize);
    Some(compressed)
}

fn blusc_compress(src: &[u8], typesize: i32, compcode: u8, filter: u8, clevel: u8) -> Vec<u8> {
    let mut cparams = BLOSC2_CPARAMS_DEFAULTS;
    cparams.typesize = typesize;
    cparams.compcode = compcode;
    cparams.filters[5] = filter;
    cparams.clevel = clevel;
    let cctx = blosc2_create_cctx(cparams);
    let mut compressed = vec![0u8; src.len() + BLOSC2_MAX_OVERHEAD];
    let csize = blosc2_compress_ctx(&cctx, src, &mut compressed);
    assert!(csize > 0, "blusc compression failed for compcode={compcode}");
    compressed.truncate(csize as usize);
    compressed
}

fn check(typesize: usize, nelems: usize, compcode: u8, filter: u8, clevel: u8) {
    let src = make_data(typesize * nelems);
    // blosc2-src is built without snappy/zlib; fall back to blusc's compressor for those.
    let compressed = c_compress(&src, typesize as i32, compcode, filter, clevel)
        .unwrap_or_else(|| blusc_compress(&src, typesize as i32, compcode, filter, clevel));
    let ctx = format!("ts={typesize} n={nelems} codec={compcode} filter={filter} clevel={clevel}");

    // Full decompression.
    let mut dest = vec![0u8; src.len()];
    assert_eq!(
        blosc2_decompress(&compressed, &mut dest),
        src.len() as i32,
        "decompress failed: {ctx}"
    );
    assert!(dest == src, "decompress mismatch: {ctx}");

    // getitem over ranges: whole chunk, block-aligned, block-straddling, tiny, tail.
    let (_, _, blocksize) = blosc2_cbuffer_sizes(&compressed);
    let block_elems = (blocksize / typesize).max(1);
    let mut ranges = vec![
        (0, nelems),
        (0, 1),
        (nelems - 1, 1),
        (block_elems.min(nelems - 1), (block_elems).min(nelems - block_elems.min(nelems - 1))),
        (block_elems / 2, (block_elems * 2).min(nelems - block_elems / 2)),
        (nelems / 3, (nelems / 5).max(1)),
        (nelems.saturating_sub(block_elems / 3 + 1), block_elems / 3 + 1),
    ];
    for k in 0..7 {
        let start = (k * 7919 * typesize + 13) % nelems;
        let n = ((k + 1) * 1237) % (nelems - start) + 1;
        ranges.push((start, n));
    }

    for (start, nitems) in ranges {
        if nitems == 0 || start + nitems > nelems {
            continue;
        }
        let mut out = vec![0u8; nitems * typesize];
        let n = blosc1_getitem(&compressed, start as i32, nitems as i32, &mut out);
        assert_eq!(n as usize, out.len(), "getitem size: {ctx} start={start} nitems={nitems}");
        assert!(
            out[..] == src[start * typesize..(start + nitems) * typesize],
            "getitem mismatch: {ctx} start={start} nitems={nitems}"
        );
    }
}

#[test]
fn multiblock_all_codecs_and_filters() {
    let codecs = [BLOSC_BLOSCLZ, BLOSC_LZ4, BLOSC_LZ4HC, BLOSC_SNAPPY, BLOSC_ZLIB, BLOSC_ZSTD];
    let filters = [BLOSC_NOSHUFFLE, BLOSC_SHUFFLE, BLOSC_BITSHUFFLE];
    for compcode in codecs {
        for filter in filters {
            for (typesize, nelems) in [(1, 300_001), (2, 200_003), (4, 250_007), (8, 140_001), (3, 100_003), (12, 50_021)] {
                for clevel in [1, 5, 9] {
                    check(typesize, nelems, compcode, filter, clevel);
                }
            }
        }
    }
}
