// Corresponds to c-blosc2/blosc/b2nd_utils.c

use crate::include::blosc2_include::{
    BLOSC2_ERROR_SUCCESS,
};
use crate::include::b2nd_include::{
    B2ND_MAX_DIM,
    blosc2_multidim_to_unidim,
    blosc2_unidim_to_multidim,
};

pub fn b2nd_copy_buffer2(
    ndim: i8,
    itemsize: i32,
    src: &[u8],
    src_pad_shape: &[i64],
    src_start: &[i64],
    src_stop: &[i64],
    dst: &mut [u8],
    dst_pad_shape: &[i64],
    dst_start: &[i64],
) -> i32 {
    let ndim = ndim as usize;
    let mut copy_shape = [0i64; B2ND_MAX_DIM];
    for i in 0..ndim {
        copy_shape[i] = src_stop[i] - src_start[i];
        if copy_shape[i] == 0 {
            return BLOSC2_ERROR_SUCCESS;
        }
    }

    // Compute the strides
    let mut src_strides = [0i64; B2ND_MAX_DIM];
    src_strides[ndim - 1] = 1;
    for i in (0..ndim - 1).rev() {
        src_strides[i] = src_strides[i + 1] * src_pad_shape[i + 1];
    }

    let mut dst_strides = [0i64; B2ND_MAX_DIM];
    dst_strides[ndim - 1] = 1;
    for i in (0..ndim - 1).rev() {
        dst_strides[i] = dst_strides[i + 1] * dst_pad_shape[i + 1];
    }

    // Align the buffers removing unnecessary data
    let src_start_n = blosc2_multidim_to_unidim(src_start, ndim as i8, &src_strides);
    let src_offset = (src_start_n * itemsize as i64) as usize;
    let bsrc = &src[src_offset..];

    let dst_start_n = blosc2_multidim_to_unidim(dst_start, ndim as i8, &dst_strides);
    let dst_offset = (dst_start_n * itemsize as i64) as usize;
    let bdst = &mut dst[dst_offset..];

    if ndim == 1 {
        let copy_nbytes = (copy_shape[0] * itemsize as i64) as usize;
        bdst[..copy_nbytes].copy_from_slice(&bsrc[..copy_nbytes]);
        return BLOSC2_ERROR_SUCCESS;
    }

    // Fallback logic for all dimensions > 1 (ignoring specialized loops for simplicity and single-threadedness)
    // Corresponds to copy_ndim_fallback in C
    let copy_nbytes = (copy_shape[ndim - 1] * itemsize as i64) as usize;
    let mut number_of_copies = 1;
    for i in 0..ndim - 1 {
        number_of_copies *= copy_shape[i];
    }

    for ncopy in 0..number_of_copies {
        // Compute the start of the copy
        let mut copy_start = [0i64; B2ND_MAX_DIM];
        blosc2_unidim_to_multidim(ndim - 1, &copy_shape, ncopy, &mut copy_start);

        // Translate this index to the src buffer
        let src_copy_start = blosc2_multidim_to_unidim(&copy_start, (ndim - 1) as i8, &src_strides);

        // Translate this index to the dst buffer
        let dst_copy_start = blosc2_multidim_to_unidim(&copy_start, (ndim - 1) as i8, &dst_strides);

        // Perform the copy
        let src_idx = (src_copy_start * itemsize as i64) as usize;
        let dst_idx = (dst_copy_start * itemsize as i64) as usize;

        bdst[dst_idx..dst_idx + copy_nbytes].copy_from_slice(&bsrc[src_idx..src_idx + copy_nbytes]);
    }

    BLOSC2_ERROR_SUCCESS
}

pub fn b2nd_copy_buffer(
    ndim: i8,
    itemsize: u8,
    src: &[u8],
    src_pad_shape: &[i64],
    src_start: &[i64],
    src_stop: &[i64],
    dst: &mut [u8],
    dst_pad_shape: &[i64],
    dst_start: &[i64],
) -> i32 {
    b2nd_copy_buffer2(
        ndim,
        itemsize as i32,
        src,
        src_pad_shape,
        src_start,
        src_stop,
        dst,
        dst_pad_shape,
        dst_start,
    )
}