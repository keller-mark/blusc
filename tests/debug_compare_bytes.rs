use blusc::api::{BLOSC2_CPARAMS_DEFAULTS as RUST_CPARAMS, blosc2_create_cctx, blosc2_compress_ctx};
use blosc2_src::{BLOSC2_CPARAMS_DEFAULTS as C_CPARAMS, BLOSC2_DPARAMS_DEFAULTS, blosc2_create_cctx as c_create_cctx, blosc2_compress_ctx as c_compress_ctx, blosc2_create_dctx, blosc2_decompress_ctx};

#[test]
fn compare_rust_and_c_output() {
    // Small test data
    let src: Vec<f32> = (0..100).map(|i| i as f32).collect();
    let typesize = std::mem::size_of::<f32>();
    let src_size = src.len() * typesize;
    
    // Compress with Rust
    let rust_compressed = {
        let mut dest = vec![0u8; src_size + 16 + 4 + 100]; // Extra space
        let mut cparams = RUST_CPARAMS;
        cparams.clevel = 5;
        cparams.typesize = typesize as i32;
        cparams.compcode = 0; // BloscLZ
        let context = blosc2_create_cctx(cparams);
        
        let src_bytes = unsafe {
            std::slice::from_raw_parts(
                src.as_ptr() as *const u8,
                src_size
            )
        };

        let rsize = blosc2_compress_ctx(
            &context,
            src_bytes,
            &mut dest,
        );

        println!("Rust compressed to {} bytes", rsize);
        dest.into_iter().take(rsize as usize).collect::<Vec<u8>>()
    };
    
    // Compress with C
    let c_compressed = {
        let mut dest = vec![0u8; src_size + 16 + 4 + 100];
        let mut cparams = C_CPARAMS;
        cparams.clevel = 5;
        cparams.typesize = typesize as i32;
        cparams.compcode = 0; // BloscLZ
        let context = unsafe { c_create_cctx(cparams) };
        
        let rsize = unsafe {
            c_compress_ctx(
                context,
                src.as_ptr().cast(),
                src_size as i32,
                dest.as_mut_ptr().cast(),
                dest.len() as i32,
            )
        };

        println!("C compressed to {} bytes", rsize);
        dest.into_iter().take(rsize as usize).collect::<Vec<u8>>()
    };
    
    println!("\nRust header: {:02x?}", &rust_compressed[0..16]);
    println!("C    header: {:02x?}", &c_compressed[0..16]);
    
    println!("\nRust csize+data (32 bytes): {:02x?}", &rust_compressed[16..48.min(rust_compressed.len())]);
    println!("C    csize+data (32 bytes): {:02x?}", &c_compressed[16..48.min(c_compressed.len())]);
    
    // Compare first few tokens
    if rust_compressed.len() >= 24 && c_compressed.len() >= 24 {
        println!("\nRust csize: {:02x?}", &rust_compressed[16..20]);
        println!("C    csize: {:02x?}", &c_compressed[16..20]);
        
        println!("\nRust first 20 compressed bytes: {:02x?}", &rust_compressed[20..40.min(rust_compressed.len())]);
        println!("C    first 20 compressed bytes: {:02x?}", &c_compressed[20..40.min(c_compressed.len())]);
    }
    
    // Try decompressing Rust output with C
    println!("\n\nDecompressing Rust output with C...");
    let mut result = vec![0f32; 100];
    let error = unsafe {
        let dparams = BLOSC2_DPARAMS_DEFAULTS;
        let context = blosc2_create_dctx(dparams);
        
        blosc2_decompress_ctx(
            context,
            rust_compressed.as_ptr().cast(),
            rust_compressed.len() as i32,
            result.as_mut_ptr().cast(),
            result.len() as i32 * std::mem::size_of::<f32>() as i32,
        )
    };
    println!("C decompress returned: {}", error);
    println!("First 10 values: {:?}", &result[0..10]);
    println!("Expected: {:?}", &src[0..10]);
    
    // Verify match
    for (i, (&got, &expected)) in result.iter().zip(src.iter()).enumerate() {
        if got != expected {
            println!("Mismatch at index {}: got {}, expected {}", i, got, expected);
        }
    }
}
