use blosc2_src::{
    blosc2_init as bound_blosc2_init,
    blosc2_destroy as bound_blosc2_destroy,
    blosc2_create_cctx as bound_blosc2_create_cctx,
    blosc2_compress_ctx as bound_blosc2_compress_ctx,
    BLOSC2_MAX_OVERHEAD as BOUND_BLOSC2_MAX_OVERHEAD,
    BLOSC2_CPARAMS_DEFAULTS as BOUND_BLOSC2_CPARAMS_DEFAULTS,
};

#[test]
fn compare_c_vs_rust_compression() {
    unsafe {
        bound_blosc2_init();

        // Same test data as the failing test
        let src: Vec<f32> = (0..10000)
            .map(|num| ((num * 8923) % 100) as f32 / 2f32)
            .collect();

        let typesize = std::mem::size_of::<f32>();
        let src_size = src.len() * typesize;
        let src_bytes = std::slice::from_raw_parts(
            src.as_ptr() as *const u8,
            src_size
        );

        // Compress with C library
        let mut c_dest = vec![0u8; src_size + BOUND_BLOSC2_MAX_OVERHEAD as usize];
        let c_size = {
            let mut cparams = BOUND_BLOSC2_CPARAMS_DEFAULTS;
            cparams.clevel = 5;
            cparams.typesize = typesize as i32;
            let context = bound_blosc2_create_cctx(cparams);
            
            bound_blosc2_compress_ctx(
                context,
                src_bytes.as_ptr().cast(),
                src_size as i32,
                c_dest.as_mut_ptr().cast(),
                c_dest.len() as i32,
            )
        };

        println!("C compressed to {} bytes", c_size);
        println!("C Header: {:02x?}", &c_dest[0..16]);
        println!("C First 48 bytes of compressed data: {:02x?}", &c_dest[16..64.min(c_size as usize)]);

        // Try to decompress C output with Rust
        println!("\nTrying to decompress C output with Rust blosc2_decompress_ctx...");
        let mut rust_result = vec![0f32; src.len()];
        let rust_result_bytes = std::slice::from_raw_parts_mut(
            rust_result.as_mut_ptr() as *mut u8,
            src_size
        );
        
        let rust_decomp_size = blusc::api::blosc2_decompress_ctx(
            &blusc::api::blosc2_create_dctx(blusc::api::BLOSC2_DPARAMS_DEFAULTS),
            &c_dest[0..c_size as usize],
            rust_result_bytes,
        );
        
        println!("Rust decompressed C data: {} bytes (expected {})", rust_decomp_size, src_size);
        if rust_decomp_size as usize == src_size {
            println!("✓ Rust can decompress C's output!");
            assert_eq!(rust_result, src, "Decompressed data doesn't match original");
        } else {
            println!("✗ Rust failed to decompress C's output");
        }

        bound_blosc2_destroy();
    }
}
