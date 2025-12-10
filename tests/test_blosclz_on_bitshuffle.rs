use blusc::codecs::blosclz;
use blusc::filters::bitshuffle;

#[test]
fn test_blosclz_on_bitshuffled_data() {
    let type_size = 8;
    let num_elements = 10000;
    let buffer_size = type_size * num_elements;
    let mut original = vec![0u8; buffer_size];
    
    // Fill with sequential data
    for j in 0..buffer_size {
        original[j] = (j % 255) as u8;
    }

    let mut bitshuffled = vec![0u8; buffer_size];
    let result = bitshuffle(type_size, buffer_size, &original, &mut bitshuffled);
    assert!(result.is_ok());
    
    println!("First 100 bytes of bitshuffled data:");
    for i in (0..100).step_by(10) {
        println!("{:3}: {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x}",
            i,
            bitshuffled[i], bitshuffled[i+1], bitshuffled[i+2], bitshuffled[i+3], bitshuffled[i+4],
            bitshuffled[i+5], bitshuffled[i+6], bitshuffled[i+7], bitshuffled[i+8], bitshuffled[i+9]);
    }
    
    // Now compress with blosclz
    let mut compressed = vec![0u8; buffer_size * 2];
    let csize = blosclz::compress(5, &bitshuffled, &mut compressed);
    
    println!("\nOriginal size: {}", buffer_size);
    println!("Compressed size: {}", csize);
    println!("Compression ratio: {:.2}", buffer_size as f64 / csize as f64);
    
    println!("\nFirst 100 bytes of compressed data:");
    for i in (0..(csize.min(100))).step_by(10) {
        if i + 9 < csize {
            println!("{:3}: {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x}",
                i,
                compressed[i], compressed[i+1], compressed[i+2], compressed[i+3], compressed[i+4],
                compressed[i+5], compressed[i+6], compressed[i+7], compressed[i+8], compressed[i+9]);
        }
    }
    
    // The C implementation gets 2188 bytes, which is about 36.6:1 ratio
    // We should get something similar
    let expected_ratio = 36.0;
    let actual_ratio = buffer_size as f64 / csize as f64;
    
    println!("Expected ratio: ~{:.1}:1", expected_ratio);
    println!("Actual ratio: {:.2}:1", actual_ratio);
    
    if actual_ratio < expected_ratio / 2.0 {
        panic!("Compression ratio is too low! Expected ~{:.1}:1, got {:.2}:1", 
               expected_ratio, actual_ratio);
    }
}
