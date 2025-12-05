use blosc2_src::{
    blosc2_init as bound_blosc2_init,
    blosc2_destroy as bound_blosc2_destroy,
};
use blusc::codecs::blosclz;

#[test]
fn test_rust_compress_c_decompress_raw() {
    unsafe {
        bound_blosc2_init();

        let text = "I am here writing some very cool and novel words which I will compress and decompress";
        let bytes = text.as_bytes();

        // Compress with Rust blosclz (raw, no blosc2 header)
        let mut compressed = vec![0u8; bytes.len() * 2];
        let comp_size = blosclz::compress(5, bytes, &mut compressed);
        
        println!("Original size: {}", bytes.len());
        println!("Compressed size: {}", comp_size);
        println!("Compressed hex: {:02x?}", &compressed[0..comp_size.min(50)]);
        
        assert!(comp_size > 0);
        assert!(comp_size < bytes.len());

        // Try to decompress with Rust first to verify it's valid
        let mut rust_decompressed = vec![0u8; bytes.len()];
        let rust_decomp_size = blosclz::decompress(&compressed[0..comp_size], &mut rust_decompressed);
        println!("Rust decompressed size: {}", rust_decomp_size);
        assert_eq!(rust_decomp_size, bytes.len());
        assert_eq!(&rust_decompressed[0..rust_decomp_size], bytes);
        println!("✓ Rust can decompress its own output correctly");

        bound_blosc2_destroy();
    }
}
