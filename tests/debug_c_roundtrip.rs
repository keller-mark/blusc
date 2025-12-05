use blosc2_src::{
    blosc2_init as bound_blosc2_init,
    blosc2_decompress_ctx as bound_blosc2_decompress_ctx,
    blosc2_create_dctx as bound_blosc2_create_dctx,
    blosc2_destroy as bound_blosc2_destroy,
    BLOSC2_DPARAMS_DEFAULTS as BOUND_BLOSC2_DPARAMS_DEFAULTS,
};
use blusc::api::{
    blosc2_compress as blusc_blosc2_compress,
};

#[test]
fn test_small_array() {
    unsafe {
        bound_blosc2_init();

        // Small test: just 10 floats
        let src: Vec<f32> = vec![42.0; 10];
        let bytes: Vec<u8> = src.iter()
            .flat_map(|&f| f.to_le_bytes())
            .collect();

        let mut compressed = vec![0u8; bytes.len() * 2 + 100];
        
        println!("Original data ({} bytes): {:?}", bytes.len(), bytes);
        
        let csize = blusc_blosc2_compress(
            5,
            0,  // No shuffle
            std::mem::size_of::<f32>(),
            &bytes,
            &mut compressed,
        );
        assert!(csize > 0, "Compression failed");
        let csize = csize as usize;
        
        println!("Compressed to {} bytes", csize);
        println!("Header: {:02x?}", &compressed[0..16]);
        println!("Compressed data: {:02x?}", &compressed[16..csize]);
        
        // Try to decompress with C
        let mut decompressed = vec![0f32; 10];
        let error = {
            let dparams = BOUND_BLOSC2_DPARAMS_DEFAULTS;
            let context = bound_blosc2_create_dctx(dparams);
            
            bound_blosc2_decompress_ctx(
                context,
                compressed.as_ptr().cast(),
                csize as i32,
                decompressed.as_mut_ptr().cast(),
                40,
            )
        };
        
        println!("C decompress returned: {}", error);
        if error > 0 {
            println!("Decompressed data: {:?}", decompressed);
            assert_eq!(src, decompressed);
        } else {
            println!("ERROR: C decompression failed with code {}", error);
        }
        
        bound_blosc2_destroy();
    }
}
