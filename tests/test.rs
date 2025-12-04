use blusc::{
    blosc1_getitem,
    blosc2_get_complib_info,
    blosc2_compress,
    blosc2_decompress,
    BLOSC_NOSHUFFLE,
    BLOSC2_MAX_OVERHEAD,
    BLOSC2_CPARAMS_DEFAULTS,
    BLOSC2_DPARAMS_DEFAULTS,
    blosc2_create_cctx,
    blosc2_compress_ctx,
    blosc2_cbuffer_sizes,
    blosc2_create_dctx,
    blosc2_decompress_ctx,
};

#[test]
fn roundtrip() {
    let text =
        "I am here writing some very cool and novel words which I will compress and decompress";

    let bytes = text.as_bytes();

    let mut compressed = vec![0; bytes.len() * 2];

    let stat = blosc2_compress(
        9,
        BLOSC_NOSHUFFLE,
        std::mem::size_of::<u8>(),
        bytes,
        &mut compressed,
    );
    assert!(stat > 0);

    let mut outtext = vec![0_u8; bytes.len()];
    let stat = blosc2_decompress(
        &compressed,
        &mut outtext,
    );
    assert!(stat > 0);

    assert_eq!(text, std::str::from_utf8(&outtext).unwrap());
}

#[test]
fn floats_roundtrip() {
    // generate numerical data
    let src: Vec<f32> = (0..10000)
        .map(|num| ((num * 8923) % 100) as f32 / 2f32) // multiply by big prime number
        .collect();

    // compress
    let dest: Vec<u8> = {
        let typesize = std::mem::size_of::<f32>();
        let src_size = src.len() * typesize;
        let dest_size = src_size + BLOSC2_MAX_OVERHEAD as usize;
        let mut dest = vec![0; dest_size];

        let rsize = {
            let mut cparams = BLOSC2_CPARAMS_DEFAULTS;
            cparams.typesize = typesize as i32;
            cparams.compcode = 0; // Use BloscLZ
            cparams.filters[5] = 2; // Enable BitShuffle
            let context = blosc2_create_cctx(cparams);

            // Convert f32 slice to u8 slice for compression
            let src_u8 = unsafe {
                std::slice::from_raw_parts(
                    src.as_ptr() as *const u8,
                    src_size
                )
            };

            blosc2_compress_ctx(
                &context,
                src_u8,
                &mut dest,
            )
        };

        assert!(rsize > 0);
        dest.drain(rsize as usize..);
        dest
    };

    // make sure it actually compresses
    assert!(src.len() * std::mem::size_of::<f32>() > dest.len());

    // decompress
    let result = {
        let (nbytes, _cbytes, _blocksize) = blosc2_cbuffer_sizes(&dest);
        assert!(nbytes != 0);
        let dest_size = nbytes / std::mem::size_of::<f32>();
        let mut result = vec![0f32; dest_size];
        
        let error = {
            let dparams = BLOSC2_DPARAMS_DEFAULTS;
            let context = blosc2_create_dctx(dparams);
            
            // Convert f32 slice to u8 slice for decompression
            let result_u8 = unsafe {
                std::slice::from_raw_parts_mut(
                    result.as_mut_ptr() as *mut u8,
                    nbytes
                )
            };

            blosc2_decompress_ctx(
                &context,
                &dest,
                result_u8,
            )
        };
        assert!(error >= 1);
        result
    };

    // check if the values in both arrays are equal
    assert_eq!(src, result);
}

#[test]
fn test_getitem() {
    let text = "This is a test string for getitem.";
    let bytes = text.as_bytes();
    let mut compressed = vec![0; bytes.len() * 2 + 32];
    
    let size = blosc2_compress(
        5,
        1, // Shuffle
        1, // typesize 1
        bytes,
        &mut compressed,
    );
    assert!(size > 0);
    
    let start = 10;
    let nitems = 4; // "test"
    let mut dest = vec![0u8; nitems];
    
    let ret = blosc1_getitem(
        &compressed,
        start as i32,
        nitems as i32,
        &mut dest,
    );
    
    assert_eq!(ret, nitems as i32);
    assert_eq!(&dest, b"test");
}

#[test]
fn test_complib_info() {
    let info = blosc2_get_complib_info("blosclz");
    assert!(info.is_some());
    
    let (lib, ver, code) = info.unwrap();
    assert_eq!(lib, "BloscLZ");
    assert!(!ver.is_empty());
    assert_eq!(code, 0);
}
