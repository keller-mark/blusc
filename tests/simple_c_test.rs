use blosc2_src::{
    blosc2_compress as bound_blosc2_compress,
    blosc2_decompress as bound_blosc2_decompress,
    BLOSC_NOSHUFFLE as BOUND_BLOSC_NOSHUFFLE,
    BLOSC2_MAX_OVERHEAD as BOUND_BLOSC2_MAX_OVERHEAD,
};

#[test]
fn simple_repeating_pattern() {
    // Create a simple repeating pattern that C should definitely compress
    let mut bytes = vec![0u8; 1000];
    // Fill with pattern "abcd" repeated
    for i in 0..bytes.len() {
        bytes[i] = b"abcd"[i % 4];
    }
    
    let mut compressed = vec![0; bytes.len() * 2 + BOUND_BLOSC2_MAX_OVERHEAD as usize];
    
    let stat = unsafe {
        bound_blosc2_compress(
            0,  // compressor: blosclz
            BOUND_BLOSC_NOSHUFFLE as _,
            std::mem::size_of::<u8>() as i32,
            bytes.as_ptr().cast(),
            bytes.len() as i32,
            compressed.as_mut_ptr().cast(),
            compressed.len() as i32,
        )
    };
    
    println!("Compressed {} bytes to {} bytes", bytes.len(), stat);
    
    // Skip the blosc header (16 bytes)
    let blosclz_data = &compressed[16..stat as usize];
    println!("Blosclz payload: {} bytes", blosclz_data.len());
    println!("First 50 bytes: {:02x?}", &blosclz_data[0..50.min(blosclz_data.len())]);
    
    // Check if it's compressed or uncompressed (storeblock)
    let all_zeros = blosclz_data.iter().all(|&b| b == 0);
    if all_zeros {
        println!("WARNING: All zeros - this is likely a storeblock (uncompressed)");
        return;
    }
    
    // Parse first few elements
    let mut idx = 0;
    for i in 0..5 {
        if idx >= blosclz_data.len() {
            break;
        }
        
        let ctrl = blosclz_data[idx];
        idx += 1;
        
        println!("\n[Segment {}] Control byte 0x{:02x} at position {}", i, ctrl, idx - 1);
        
        if ctrl < 32 {
            let lit_count = (ctrl & 31) + 1;
            println!("  LITERAL: {} bytes", lit_count);
            if idx + lit_count as usize <= blosclz_data.len() {
                println!("  Data: {:02x?}", &blosclz_data[idx..idx + lit_count as usize]);
            }
            idx += lit_count as usize;
        } else {
            // Match
            let mut len_field = ((ctrl >> 5) - 1) as usize;
            let dist_high = (ctrl & 31) as usize;
            
            println!("  MATCH:");
            println!("    len_field (initial): {}", len_field);
            println!("    dist_high: {}", dist_high);
            
            // Read extension bytes if needed
            if len_field == 6 {
                while idx < blosclz_data.len() && blosclz_data[idx] == 255 {
                    len_field += 255;
                    idx += 1;
                }
                if idx < blosclz_data.len() {
                    len_field += blosclz_data[idx] as usize;
                    idx += 1;
                }
            }
            
            // Read distance
            if idx < blosclz_data.len() {
                let dist_low = blosclz_data[idx];
                idx += 1;
                
                let mut distance = (dist_high << 8) | dist_low as usize;
                
                if dist_low == 255 && dist_high == 31 {
                    if idx + 1 < blosclz_data.len() {
                        distance = ((blosclz_data[idx] as usize) << 8) | (blosclz_data[idx + 1] as usize);
                        idx += 2;
                        distance += 8191;
                    }
                }
                
                distance += 1; // Unbias
                let final_len = len_field + 3;
                
                println!("    Final: len={}, dist={}", final_len, distance);
            }
        }
    }
    
    // Verify decompression works
    let mut decompressed = vec![0u8; bytes.len()];
    let dstat = unsafe {
        bound_blosc2_decompress(
            compressed.as_ptr().cast(),
            stat,
            decompressed.as_mut_ptr().cast(),
            decompressed.len() as i32,
        )
    };
    assert!(dstat > 0);
    assert_eq!(&decompressed, &bytes);
}
