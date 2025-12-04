use blusc::{
    blosc1_cbuffer_metainfo,
    blosc1_cbuffer_validate,
    blosc1_cbuffer_sizes,
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
    unsafe {

        let text =
            "I am here writing some very cool and novel words which I will compress and decompress";

        let bytes = text.as_bytes();

        let mut compressed = vec![0; bytes.len() * 2];

        let stat = blosc2_compress(
            9,
            BLOSC_NOSHUFFLE as _,
            std::mem::size_of::<u8>(),
            bytes.as_ptr().cast(),
            bytes.len(),
            compressed.as_mut_ptr().cast(),
            compressed.len(),
        );
        assert!(stat > 0);

        let mut outtext = vec![0_u8; bytes.len()];
        let stat = blosc2_decompress(
            compressed.as_ptr().cast(),
            compressed.len(),
            outtext.as_mut_ptr().cast(),
            outtext.len(),
        );
        assert!(stat > 0);

        assert_eq!(text, std::str::from_utf8(&outtext).unwrap());
    }
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

        let rsize = unsafe {
            let mut cparams = BLOSC2_CPARAMS_DEFAULTS;
            cparams.typesize = typesize as i32;
            cparams.compcode = 0; // Use BloscLZ
            cparams.filters[5] = 2; // Enable BitShuffle
            let context = blosc2_create_cctx(cparams);

            blosc2_compress_ctx(
                context,
                src.as_ptr().cast(),
                src_size,
                dest.as_mut_ptr().cast(),
                dest_size,
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
        let mut nbytes: usize = 0;
        let mut _cbytes: usize = 0;
        let mut _blocksize: usize = 0;
        unsafe {
            blosc2_cbuffer_sizes(
                dest.as_ptr().cast(),
                &mut nbytes,
                &mut _cbytes,
                &mut _blocksize,
            )
        };
        assert!(nbytes != 0);
        let dest_size = nbytes / std::mem::size_of::<f32>();
        let mut result = vec![0f32; dest_size];
        let error = unsafe {
            let dparams = BLOSC2_DPARAMS_DEFAULTS;
            let context = blosc2_create_dctx(dparams);
            blosc2_decompress_ctx(
                context,
                dest.as_ptr().cast(),
                dest.len(),
                result.as_mut_ptr().cast(),
                nbytes,
            )
        };
        assert!(error >= 1);
        result
    };

    // check if the values in both arrays are equal
    assert_eq!(src, result);
}
