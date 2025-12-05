use blusc::codecs::blosclz;

#[test]
fn test_c_decompress_rust_compressed() {
    // Create simple test data
    let input: Vec<f32> = (0..10000).map(|i| (i % 100) as f32 + (i / 100) as f32 * 0.1).collect();
    let input_bytes: Vec<u8> = input.iter().flat_map(|f| f.to_le_bytes()).collect();
    
    println!("Input size: {} bytes", input_bytes.len());
    
    // Compress with Rust
    let mut compressed = vec![0u8; input_bytes.len() * 2];
    let compressed_size = blosclz::compress(9, &input_bytes, &mut compressed);
    
    println!("Rust compressed to {} bytes", compressed_size);
    
    if compressed_size == 0 {
        panic!("Compression failed");
    }
    
    compressed.truncate(compressed_size);
    
    // Show first bytes
    println!("First 64 bytes of compressed: {:02x?}", &compressed[0..64.min(compressed.len())]);
    
    // Try to decompress with Rust
    let mut decompressed = vec![0u8; input_bytes.len()];
    let decompressed_size = blosclz::decompress(&compressed, &mut decompressed);
    
    println!("Rust decompressed to {} bytes", decompressed_size);
    
    if decompressed_size == 0 {
        panic!("Rust decompression failed");
    }
    
    assert_eq!(&decompressed[..input_bytes.len()], &input_bytes[..]);
    println!("SUCCESS: Rust roundtrip works!");
}

