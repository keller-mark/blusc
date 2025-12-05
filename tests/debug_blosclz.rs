use blusc::codecs::blosclz;

#[test]
fn test_simple_compression() {
    // Simple test data with obvious repetition
    let input = b"aaaaaaaaaabbbbbbbbbb";
    let mut compressed = vec![0u8; 1000];
    
    let comp_size = blosclz::compress(5, input, &mut compressed);
    println!("Input: {:?}", input);
    println!("Input len: {}", input.len());
    println!("Compressed size: {}", comp_size);
    println!("Compressed data: {:?}", &compressed[0..comp_size]);
    
    assert!(comp_size > 0);
    assert!(comp_size < input.len());
    
    let mut decompressed = vec![0u8; input.len()];
    let decomp_size = blosclz::decompress(&compressed[0..comp_size], &mut decompressed);
    println!("Decompressed size: {}", decomp_size);
    println!("Decompressed: {:?}", &decompressed[0..decomp_size]);
    
    assert_eq!(decomp_size, input.len());
    assert_eq!(&decompressed[0..decomp_size], input);
}

#[test]
fn test_very_simple() {
    // Even simpler - just literal data
    let input = b"abcdefghijklmnop";
    let mut compressed = vec![0u8; 1000];
    
    let comp_size = blosclz::compress(5, input, &mut compressed);
    println!("\nInput: {:?}", std::str::from_utf8(input).unwrap());
    println!("Compressed size: {}", comp_size);
    println!("Compressed hex: {:02x?}", &compressed[0..comp_size]);
    
    assert!(comp_size > 0);
    
    let mut decompressed = vec![0u8; input.len()];
    let decomp_size = blosclz::decompress(&compressed[0..comp_size], &mut decompressed);
    
    println!("Decompressed size: {}", decomp_size);
    println!("Decompressed: {:?}", std::str::from_utf8(&decompressed[0..decomp_size]).unwrap());
    
    assert_eq!(decomp_size, input.len());
    assert_eq!(&decompressed[0..decomp_size], input);
}
