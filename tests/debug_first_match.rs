use blusc::codecs::blosclz;

#[test]
fn debug_compression_first_match() {
    // Test the exact data from the failing test
    let src: Vec<f32> = (0..10000)
        .map(|num| ((num * 8923) % 100) as f32 / 2f32)
        .collect();
    
    let src_bytes = unsafe {
        std::slice::from_raw_parts(
            src.as_ptr() as *const u8,
            src.len() * std::mem::size_of::<f32>()
        )
    };
    
    // Just compress with blosclz directly to see what happens
    let mut compressed = vec![0u8; src_bytes.len() * 2];
    let result = blosclz::compress(0, src_bytes, &mut compressed);
    
    println!("Compressed {} bytes to {} bytes", src_bytes.len(), result);
    
    // Check first few bytes
    println!("First 32 bytes of input: {:02x?}", &src_bytes[0..32]);
    println!("Bytes at position 7936: {:02x?}", &src_bytes[7936..7936+4]);
    
    // Check if they match
    let seq_0 = u32::from_le_bytes([src_bytes[0], src_bytes[1], src_bytes[2], src_bytes[3]]);
    let seq_7936 = u32::from_le_bytes([src_bytes[7936], src_bytes[7936+1], src_bytes[7936+2], src_bytes[7936+3]]);
    println!("Seq at 0: 0x{:08x}", seq_0);
    println!("Seq at 7936: 0x{:08x}", seq_7936);
    println!("Match: {}", seq_0 == seq_7936);
    
    if result > 0 {
        println!("\nFirst 64 bytes of compressed: {:02x?}", &compressed[0..64.min(result)]);
        
        // Parse first token (strip marker bit from first byte)
        let ctrl = compressed[0] & 31;  // Strip bit 5 (format marker)
        println!("\nFirst control byte: 0x{:02x} (after stripping marker bit)", ctrl);
        if ctrl >= 32 {
            let len = ((ctrl >> 5) - 1) as usize;
            let ofs_high = (ctrl & 31) as usize;
            let ofs_low = compressed[1] as usize;
            let distance = (ofs_high << 8) | ofs_low;
            println!("\nFirst token is a MATCH:");
            println!("  ctrl=0x{:02x}, len={}, distance_raw={}", ctrl, len, distance);
            println!("  Final len={}, distance={}", len + 3, distance + 1);
        } else {
            let lit_count = ctrl + 1;
            println!("\nFirst token is a LITERAL run of {} bytes", lit_count);
        }
    }
}
