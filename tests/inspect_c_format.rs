use blosc2_src::{
    blosc2_init as bound_blosc2_init,
    blosc2_compress as bound_blosc2_compress,
    blosc2_destroy as bound_blosc2_destroy,
    BLOSC_NOSHUFFLE as BOUND_BLOSC_NOSHUFFLE,
};

#[test]
fn inspect_c_compressed_format() {
    unsafe {
        bound_blosc2_init();

        // Small test: just 10 floats
        let src: Vec<f32> = vec![42.0; 10];
        let bytes: Vec<u8> = src.iter()
            .flat_map(|&f| f.to_le_bytes())
            .collect();

        let mut compressed = vec![0u8; bytes.len() * 2 + 100];
        
        println!("Original data ({} bytes): {:?}", bytes.len(), bytes);
        
        let csize = bound_blosc2_compress(
            5,  // clevel
            BOUND_BLOSC_NOSHUFFLE as _,
            std::mem::size_of::<u8>() as i32,
            bytes.as_ptr().cast(),
            bytes.len() as i32,
            compressed.as_mut_ptr().cast(),
            compressed.len() as i32,
        );
        
        assert!(csize > 0);
        println!("C compressed to {} bytes", csize);
        println!("Header: {:02x?}", &compressed[0..16]);
        println!("All compressed data: {:02x?}", &compressed[16..csize as usize]);
        
        bound_blosc2_destroy();
    }
}
