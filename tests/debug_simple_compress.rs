use blusc::codecs::blosclz;

#[test]
fn test_simple_compress() {
    // Test 1: Repeated pattern that SHOULD compress
    println!("=== Test 1: Repeated floats ===");
    let mut input = Vec::new();
    for _i in 0..100 {
        input.extend_from_slice(&(42.0f32).to_le_bytes());
    }
    
    println!("Input length: {} bytes", input.len());
    println!("First 40 bytes: {:02x?}", &input[0..40]);
    
    // Compress
    let mut output = vec![0u8; input.len() * 2];
    let compressed_size = blosclz::compress(5, &input, &mut output);
    
    println!("Compressed size: {} bytes", compressed_size);
    println!("First 40 compressed bytes: {:02x?}", &output[0..40]);
    println!("Compression ratio: {:.2}x\n", input.len() as f64 / compressed_size as f64);
    
    // Test 2: Sequential pattern (won't compress well)
    println!("=== Test 2: Sequential floats ===");
    let mut input2 = Vec::new();
    for i in 0..100 {
        input2.extend_from_slice(&(i as f32).to_le_bytes());
    }
    
    println!("Input length: {} bytes", input2.len());
    let compressed_size2 = blosclz::compress(5, &input2, &mut output);
    println!("Compressed size: {} bytes", compressed_size2);
    println!("Compression ratio: {:.2}x\n", input2.len() as f64 / compressed_size2 as f64);
    
    // Check that repeated data compresses well
    assert!(compressed_size > 0, "Compression failed");
    assert!(compressed_size < input.len() / 2, "Repeated data should compress to <50%");
}
