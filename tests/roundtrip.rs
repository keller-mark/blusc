use blusc::api::{blosc2_compress, blosc2_decompress, BLOSC2_MAX_OVERHEAD};
use std::ffi::c_void;

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

    unsafe {
        let csize = blosc2_compress(
            case.clevel,
            case.doshuffle,
            case.type_size,
            original.as_ptr() as *const c_void,
            buffer_size,
            intermediate.as_mut_ptr() as *mut c_void,
            dest_size,
        );

        assert!(csize > 0, "Compression failed");

        let dsize = blosc2_decompress(
            intermediate.as_ptr() as *const c_void,
            csize as usize,
            result.as_mut_ptr() as *mut c_void,
            buffer_size,
        );

        assert_eq!(dsize, buffer_size as i32, "Decompression size mismatch");
    }

    assert_eq!(original, result, "Data mismatch after roundtrip");
}
