use blosc2_src::{
    blosc2_compress as bound_blosc2_compress,
    blosc2_decompress as bound_blosc2_decompress,
    BLOSC_NOSHUFFLE as BOUND_BLOSC_NOSHUFFLE,
    BLOSC2_MAX_OVERHEAD as BOUND_BLOSC2_MAX_OVERHEAD,
};
use blusc::api::{
    blosc2_compress as blusc_blosc2_compress,
    blosc2_decompress as blusc_blosc2_decompress,
};

#[test]
fn simple_compress_decompress_c() {
    // Test: Rust compress, C decompress
    let bytes = b"aaaabbbbccccddddeeee"; // 20 bytes with repeated patterns
    
    let mut compressed = vec![0; bytes.len() * 2 + BOUND_BLOSC2_MAX_OVERHEAD as usize];
    
    // Compress with Rust
    let stat = blusc_blosc2_compress(
        0,  // compressor: blosclz
        BOUND_BLOSC_NOSHUFFLE as _,
        std::mem::size_of::<u8>(),
        bytes,
        &mut compressed,
    );
    
    println!("Rust compressed {} bytes to {} bytes", bytes.len(), stat);
    println!("Compressed header: {:02x?}", &compressed[0..16]);
    println!("Compressed payload (first 32 bytes): {:02x?}", &compressed[16..48.min(stat as usize)]);
    
    // Try to decompress with C
    let mut decompressed = vec![0u8; bytes.len()];
    let dstat = unsafe {
        bound_blosc2_decompress(
            compressed.as_ptr().cast(),
            stat,
            decompressed.as_mut_ptr().cast(),
            decompressed.len() as i32,
        )
    };
    
    println!("C decompress returned: {}", dstat);
    
    if dstat > 0 {
        println!("Decompressed: {:?}", std::str::from_utf8(&decompressed).unwrap_or("(not utf8)"));
        assert_eq!(&decompressed, bytes);
        println!("✓ SUCCESS");
    } else {
        println!("✗ FAILED with error code: {}", dstat);
        panic!("Decompression failed");
    }
}
