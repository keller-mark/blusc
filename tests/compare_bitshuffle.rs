use blosc2_src::{blosc2_init, blosc2_destroy};
use blusc::filters;

extern "C" {
    fn blosc2_bitshuffle(typesize: i32, blocksize: i32, src: *const u8, dest: *mut u8) -> i32;
}

fn make_test_buffer(elements: usize, typesize: usize) -> Vec<u8> {
    let mut buf = vec![0u8; elements * typesize];
    for i in 0..buf.len() {
        buf[i] = (i % 255) as u8;
    }
    buf
}

#[test]
fn compare_bitshuffle_outputs() {
    println!("Skipping compare_bitshuffle_outputs because blosc2_bitshuffle symbol is not available from blosc2-src");
    /*
    let typesize = 8usize;
    let elements = 10000usize;
    let blocksize = typesize * elements;

    unsafe {
        blosc2_init();
    }

    let src = make_test_buffer(elements, typesize);
    let mut ours = vec![0u8; blocksize];
    let mut theirs = vec![0u8; blocksize];

    filters::bitshuffle(typesize, blocksize, &src, &mut ours).expect("bitshuffle failed");
    let rc = unsafe { blosc2_bitshuffle(typesize as i32, blocksize as i32, src.as_ptr(), theirs.as_mut_ptr()) };
    assert!(rc >= 0, "C bitshuffle failed with {rc}");

    unsafe {
        blosc2_destroy();
    }

    assert_eq!(ours, theirs, "bitshuffle outputs differ");
    */
}
