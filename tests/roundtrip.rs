use blusc::api::{
    blosc2_compress as blusc_blosc2_compress,
    blosc2_decompress as blusc_blosc2_decompress,
};
use blusc::BLOSC2_MAX_OVERHEAD;

use blosc2_src::{
    blosc2_init as bound_blosc2_init,
    blosc2_compress as bound_blosc2_compress,
    blosc2_decompress as bound_blosc2_decompress,
    blosc2_create_cctx as bound_blosc2_create_cctx,
    blosc2_compress_ctx as bound_blosc2_compress_ctx,
    blosc2_create_dctx as bound_blosc2_create_dctx,
    blosc2_decompress_ctx as bound_blosc2_decompress_ctx,
    blosc2_cbuffer_sizes as bound_blosc2_cbuffer_sizes,
    blosc2_destroy as bound_blosc2_destroy,
};

use ctor::{ctor, dtor};

#[ctor]
fn blosc2_init() {
    unsafe {
        bound_blosc2_init();
    }
}

#[dtor]
fn blosc2_cleanup() {
    unsafe {
        bound_blosc2_destroy();
    }
}




struct TestCase {
    type_size: usize,
    num_elements: usize,
    clevel: i32,
    doshuffle: i32,
}

#[test]
fn test_compress_roundtrip_cases() {
    let cases = vec![
        // Small buffers
        TestCase { type_size: 1, num_elements: 7, clevel: 5, doshuffle: 0 },
        TestCase { type_size: 2, num_elements: 7, clevel: 5, doshuffle: 0 },
        TestCase { type_size: 4, num_elements: 7, clevel: 5, doshuffle: 0 },
        TestCase { type_size: 8, num_elements: 7, clevel: 5, doshuffle: 0 },
        
        // Larger buffers
        TestCase { type_size: 1, num_elements: 10000, clevel: 5, doshuffle: 0 },
        TestCase { type_size: 4, num_elements: 10000, clevel: 5, doshuffle: 1 }, // Shuffle
        TestCase { type_size: 8, num_elements: 10000, clevel: 5, doshuffle: 2 }, // Bitshuffle
        
        // Different compression levels
        TestCase { type_size: 4, num_elements: 5000, clevel: 1, doshuffle: 1 },
        TestCase { type_size: 4, num_elements: 5000, clevel: 9, doshuffle: 1 },
        
        // Odd sizes
        TestCase { type_size: 3, num_elements: 1000, clevel: 5, doshuffle: 1 },
        TestCase { type_size: 16, num_elements: 1000, clevel: 5, doshuffle: 1 },
        TestCase { type_size: 33, num_elements: 100, clevel: 5, doshuffle: 0 },
        TestCase { type_size: 1, num_elements: 702713, clevel: 5, doshuffle: 0 }, // From CSV
    ];

    for (i, case) in cases.iter().enumerate() {
        println!("Running case {}: type_size={}, num_elements={}, clevel={}, doshuffle={}", 
                 i, case.type_size, case.num_elements, case.clevel, case.doshuffle);
        
        run_roundtrip(case);
    }
}

fn run_roundtrip(case: &TestCase) {
    let buffer_size = case.type_size * case.num_elements;
    let mut original = vec![0u8; buffer_size];
    
    // Fill with sequential data
    for j in 0..buffer_size {
        original[j] = (j % 255) as u8;
    }

    let dest_size = buffer_size + BLOSC2_MAX_OVERHEAD as usize;
    let mut intermediate = vec![0u8; dest_size];
    let mut result = vec![0u8; buffer_size];

    let csize = blusc_blosc2_compress(
        case.clevel,
        case.doshuffle,
        case.type_size,
        &original,
        &mut intermediate,
    );

    assert!(csize > 0, "Compression failed");
    intermediate.truncate(csize as usize);

    // Compare blusc intermediate to bound intermediate
    let mut bound_intermediate = vec![0; dest_size];
    let bound_csize = unsafe {
        bound_blosc2_compress(
            case.clevel,
            case.doshuffle,
            case.type_size as i32,
            original.as_ptr().cast(),
            original.len() as i32,
            bound_intermediate.as_mut_ptr().cast(),
            bound_intermediate.len() as i32,
        )
    };
    assert!(bound_csize > 0);
    bound_intermediate.truncate(bound_csize as usize);

    assert_eq!(csize as usize, bound_csize as usize, "Compressed size mismatch between blusc and bound");
    assert_eq!(intermediate, bound_intermediate, "Compressed data mismatch between blusc and bound");

    let dsize = blusc_blosc2_decompress(
        &intermediate,
        &mut result,
    );

    assert_eq!(dsize, buffer_size as i32, "Decompression size mismatch");

    if original != result {
        for (i, (a, b)) in original.iter().zip(result.iter()).enumerate() {
            if a != b {
                println!("Mismatch at index {}: original={}, result={}", i, a, b);
                // Print surrounding values
                let start = if i > 10 { i - 10 } else { 0 };
                let end = if i + 10 < original.len() { i + 10 } else { original.len() };
                println!("Original context: {:?}", &original[start..end]);
                println!("Result context:   {:?}", &result[start..end]);
                break;
            }
        }
    }

    assert_eq!(original, result, "Data mismatch after roundtrip");
}
