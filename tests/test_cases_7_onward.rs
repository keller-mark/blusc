use blusc::api::{
    blosc2_compress as blusc_blosc2_compress,
    blosc2_decompress as blusc_blosc2_decompress,
};
use blusc::BLOSC2_MAX_OVERHEAD;

use blosc2_src::{
    blosc2_init,
    blosc2_compress as bound_blosc2_compress,
    blosc2_decompress as bound_blosc2_decompress,
    blosc2_destroy,
};

use ctor::{ctor, dtor};

#[ctor]
fn blosc2_init_fn() {
    unsafe {
        blosc2_init();
    }
}

#[dtor]
fn blosc2_cleanup() {
    unsafe {
        blosc2_destroy();
    }
}

struct TestCase {
    type_size: usize,
    num_elements: usize,
    clevel: i32,
    doshuffle: i32,
}

#[test]
fn test_cases_7_onward() {
    let cases = vec![
        TestCase { type_size: 4, num_elements: 5000, clevel: 1, doshuffle: 1 },  // Case 7
        TestCase { type_size: 4, num_elements: 5000, clevel: 9, doshuffle: 1 },  // Case 8
        TestCase { type_size: 3, num_elements: 1000, clevel: 5, doshuffle: 1 },  // Case 9
        TestCase { type_size: 16, num_elements: 1000, clevel: 5, doshuffle: 1 }, // Case 10
        TestCase { type_size: 33, num_elements: 100, clevel: 5, doshuffle: 0 },  // Case 11
        TestCase { type_size: 1, num_elements: 702713, clevel: 5, doshuffle: 0 }, // Case 12
    ];

    for (i, case) in cases.iter().enumerate() {
        println!("Running case {}: type_size={}, num_elements={}, clevel={}, doshuffle={}", 
                 i + 7, case.type_size, case.num_elements, case.clevel, case.doshuffle);
        
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

        if csize as usize != bound_csize as usize {
            println!("  Compressed size mismatch: blusc={}, bound={}", csize, bound_csize);
            println!("  blusc header: {:?}", &intermediate[0..32.min(intermediate.len())]);
            println!("  bound header: {:?}", &bound_intermediate[0..32.min(bound_intermediate.len())]);
            panic!("Test case {} failed", i + 7);
        }

        let dsize = blusc_blosc2_decompress(
            &intermediate,
            &mut result,
        );

        assert_eq!(dsize, buffer_size as i32, "Decompression size mismatch");
        assert_eq!(original, result, "Data mismatch after roundtrip");
        
        println!("  PASSED");
    }
}
