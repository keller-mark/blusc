use blosc2_src::{
    blosc2_compress as bound_blosc2_compress,
    blosc2_decompress as bound_blosc2_decompress,
    blosc2_create_cctx as bound_blosc2_create_cctx,
    blosc2_compress_ctx as bound_blosc2_compress_ctx,
    BLOSC_NOSHUFFLE as BOUND_BLOSC_NOSHUFFLE,
    BLOSC2_MAX_OVERHEAD as BOUND_BLOSC2_MAX_OVERHEAD,
    BLOSC2_CPARAMS_DEFAULTS as BOUND_BLOSC2_CPARAMS_DEFAULTS,
};
use blusc::api::{
    blosc2_create_cctx as blusc_blosc2_create_cctx,
    blosc2_compress_ctx as blusc_blosc2_compress_ctx,
    BLOSC2_CPARAMS_DEFAULTS as BLUSC_BLOSC2_CPARAMS_DEFAULTS,
};

#[test]
fn compare_compression_output() {
    // Create some data with patterns that should compress (same as failing test)
    let src: Vec<f32> = (0..10000)
        .map(|num| ((num * 8923) % 100) as f32 / 2f32)
        .collect();
    
    let src_bytes = unsafe {
        std::slice::from_raw_parts(
            src.as_ptr() as *const u8,
            src.len() * std::mem::size_of::<f32>()
        )
    };
    
    // Compress with C
    let c_compressed = {
        let mut dest = vec![0; src_bytes.len() * 2 + BOUND_BLOSC2_MAX_OVERHEAD as usize];
        let rsize = unsafe {
            let mut cparams = BOUND_BLOSC2_CPARAMS_DEFAULTS;
            cparams.clevel = 0;
            cparams.typesize = std::mem::size_of::<f32>() as i32;
            let context = bound_blosc2_create_cctx(cparams);
            
            bound_blosc2_compress_ctx(
                context,
                src_bytes.as_ptr().cast(),
                src_bytes.len() as i32,
                dest.as_mut_ptr().cast(),
                dest.len() as i32,
            )
        };
        dest.truncate(rsize as usize);
        dest
    };
    
    // Compress with Rust
    let rust_compressed = {
        let mut dest = vec![0; src_bytes.len() * 2 + BOUND_BLOSC2_MAX_OVERHEAD as usize];
        let context = {
            let mut cparams = BLUSC_BLOSC2_CPARAMS_DEFAULTS;
            cparams.clevel = 0;
            cparams.typesize = std::mem::size_of::<f32>() as i32;
            blusc_blosc2_create_cctx(cparams)
        };
        
        let rsize = blusc_blosc2_compress_ctx(
            &context,
            src_bytes,
            &mut dest,
        );
        dest.truncate(rsize as usize);
        dest
    };
    
    println!("C compressed to {} bytes", c_compressed.len());
    println!("Rust compressed to {} bytes", rust_compressed.len());
    
    println!("\nC header: {:02x?}", &c_compressed[0..16]);
    println!("Rust header: {:02x?}", &rust_compressed[0..16]);
    
    println!("\nC payload (first 64 bytes): {:02x?}", &c_compressed[16..80.min(c_compressed.len())]);
    println!("Rust payload (first 64 bytes): {:02x?}", &rust_compressed[16..80.min(rust_compressed.len())]);
    
    // Parse the Rust payload manually to find the error
    println!("\nParsing Rust blosclz payload:");
    let payload = &rust_compressed[16..];
    let mut pos = 0;
    let mut segment_count = 0;
    while pos < payload.len() && segment_count < 10 {
        let ctrl = payload[pos];
        pos += 1;
        
        if ctrl < 32 {
            // Literal
            let count = (ctrl & 31) + 1;
            println!("  [{}] Literal: {} bytes", segment_count, count);
            pos += count as usize;
        } else {
            // Match
            let mut len = ((ctrl >> 5) - 1) as usize;
            let mut ofs = ((ctrl & 31) as usize) << 8;
            
            println!("  [{}] Match ctrl=0x{:02x}: len={}, ofs_high={}", segment_count, ctrl, len, ctrl & 31);
            
            // Check for length extensions
            if len == 7 - 1 {
                println!("      Reading length extensions...");
                loop {
                    if pos >= payload.len() {
                        println!("      ERROR: Ran out of bytes reading length extension!");
                        break;
                    }
                    let ext = payload[pos];
                    pos += 1;
                    len += ext as usize;
                    println!("        ext byte: 0x{:02x} (len now {})", ext, len);
                    if ext != 255 {
                        break;
                    }
                }
            }
            
            // Read distance low byte
            if pos >= payload.len() {
                println!("      ERROR: Ran out of bytes reading distance!");
                break;
            }
            let code = payload[pos];
            pos += 1;
            
            len += 3;
            
            // Check for far distance
            if code == 255 && ofs == (31 << 8) {
                println!("      Far distance encoding");
                if pos + 1 >= payload.len() {
                    println!("      ERROR: Ran out of bytes reading far distance!");
                    break;
                }
                ofs = (payload[pos] as usize) << 8;
                pos += 1;
                ofs |= payload[pos] as usize;
                pos += 1;
                ofs += 8191; // MAX_DISTANCE
            } else {
                ofs |= code as usize;
            }
            ofs += 1;
            
            println!("      Final: len={}, distance={}", len, ofs);
        }
        
        segment_count += 1;
    }
    
    // Try to decompress both with C
    let mut c_result = vec![0u8; src_bytes.len()];
    let c_dstat = unsafe {
        bound_blosc2_decompress(
            c_compressed.as_ptr().cast(),
            c_compressed.len() as i32,
            c_result.as_mut_ptr().cast(),
            c_result.len() as i32,
        )
    };
    println!("\nC decompress C-compressed: {}", c_dstat);
    
    let mut rust_result = vec![0u8; src_bytes.len()];
    let rust_dstat = unsafe {
        bound_blosc2_decompress(
            rust_compressed.as_ptr().cast(),
            rust_compressed.len() as i32,
            rust_result.as_mut_ptr().cast(),
            rust_result.len() as i32,
        )
    };
    println!("C decompress Rust-compressed: {}", rust_dstat);
    
    if rust_dstat > 0 {
        assert_eq!(rust_result, src_bytes);
        println!("✓ Rust compression is compatible with C decompression");
    } else {
        println!("✗ Rust compression is NOT compatible with C decompression");
        panic!("Incompatible compression");
    }
}
