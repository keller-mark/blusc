// Corresponds to c-blosc2/include/b2nd.h

use crate::include::blosc2_include::*;

// Constants
pub const B2ND_METALAYER_VERSION: u8 = 0;
pub const B2ND_MAX_DIM: usize = 16;
pub const BLOSC2_MAX_METALAYERS: usize = 16;  // From blosc2.h
pub const B2ND_MAX_METALAYERS: usize = BLOSC2_MAX_METALAYERS - 1;
pub const DTYPE_NUMPY_FORMAT: i8 = 0;
pub const B2ND_DEFAULT_DTYPE: &str = "|u1";
pub const B2ND_DEFAULT_DTYPE_FORMAT: i8 = DTYPE_NUMPY_FORMAT;

/// Helper function to swap bytes for endianness conversion
/// This is used for serializing/deserializing metadata in big-endian format
fn swap_store(dest: &mut [u8], src: &[u8]) {
    let size = src.len();
    
    // Check if we're on little endian (most common)
    // We'll always swap to big-endian for msgpack format
    let is_little_endian = cfg!(target_endian = "little");
    
    if is_little_endian {
        // Little endian: need to swap bytes to big endian
        match size {
            8 => {
                dest[0] = src[7];
                dest[1] = src[6];
                dest[2] = src[5];
                dest[3] = src[4];
                dest[4] = src[3];
                dest[5] = src[2];
                dest[6] = src[1];
                dest[7] = src[0];
            },
            4 => {
                dest[0] = src[3];
                dest[1] = src[2];
                dest[2] = src[1];
                dest[3] = src[0];
            },
            2 => {
                dest[0] = src[1];
                dest[1] = src[0];
            },
            1 => {
                dest[0] = src[0];
            },
            _ => {
                eprintln!("Unhandled size: {}", size);
            }
        }
    } else {
        // Big endian: just copy
        dest.copy_from_slice(src);
    }
}

/// Create the metainfo for the b2nd metalayer.
/// 
/// This serializes the array metadata into MessagePack format for storage in the blosc2 metalayer.
/// 
/// # Arguments
/// * `ndim` - The number of dimensions in the array
/// * `shape` - The shape of the array
/// * `chunkshape` - The shape of the chunks in the array
/// * `blockshape` - The shape of the blocks in the array
/// * `dtype` - A string representation of the data type of the array
/// * `dtype_format` - The format of the dtype representation (0 means NumPy)
/// 
/// # Returns
/// The serialized metadata as a vector of bytes, or an error code if negative
pub fn b2nd_serialize_meta(
    ndim: i8,
    shape: &[i64],
    chunkshape: &[i32],
    blockshape: &[i32],
    dtype: Option<&str>,
    dtype_format: i8,
) -> Result<Vec<u8>, i32> {
    let dtype = dtype.unwrap_or(B2ND_DEFAULT_DTYPE);
    
    // dtype checks
    if dtype_format < 0 {
        eprintln!("dtype_format cannot be negative");
        return Err(BLOSC2_ERROR_FAILURE);
    }
    
    let dtype_len = dtype.len();
    if dtype_len > i32::MAX as usize {
        eprintln!("dtype is too large (len > {})", i32::MAX);
        return Err(BLOSC2_ERROR_FAILURE);
    }
    let dtype_len = dtype_len as i32;
    
    // Allocate space for b2nd metalayer
    let max_smeta_len = 1 + 1 + 1 
        + (1 + ndim as usize * (1 + std::mem::size_of::<i64>()))
        + (1 + ndim as usize * (1 + std::mem::size_of::<i32>()))
        + (1 + ndim as usize * (1 + std::mem::size_of::<i32>()))
        + 1 + 1 + std::mem::size_of::<i32>() + dtype_len as usize;
    
    let mut smeta = Vec::with_capacity(max_smeta_len);
    
    // Build an array with 7 entries (version, ndim, shape, chunkshape, blockshape, dtype_format, dtype)
    smeta.push(0x90 + 7);
    
    // version entry
    smeta.push(B2ND_METALAYER_VERSION); // positive fixnum (7-bit positive integer)
    
    // ndim entry
    smeta.push(ndim as u8); // positive fixnum (7-bit positive integer)
    
    // shape entry
    smeta.push(0x90 + ndim as u8); // fix array with ndim elements
    for i in 0..ndim as usize {
        smeta.push(0xd3); // int64
        let mut buf = [0u8; 8];
        let bytes = shape[i].to_ne_bytes();
        swap_store(&mut buf, &bytes);
        smeta.extend_from_slice(&buf);
    }
    
    // chunkshape entry
    smeta.push(0x90 + ndim as u8); // fix array with ndim elements
    for i in 0..ndim as usize {
        smeta.push(0xd2); // int32
        let mut buf = [0u8; 4];
        let bytes = chunkshape[i].to_ne_bytes();
        swap_store(&mut buf, &bytes);
        smeta.extend_from_slice(&buf);
    }
    
    // blockshape entry
    smeta.push(0x90 + ndim as u8); // fix array with ndim elements
    for i in 0..ndim as usize {
        smeta.push(0xd2); // int32
        let mut buf = [0u8; 4];
        let bytes = blockshape[i].to_ne_bytes();
        swap_store(&mut buf, &bytes);
        smeta.extend_from_slice(&buf);
    }
    
    // dtype format entry
    smeta.push(dtype_format as u8); // positive fixint (7-bit positive integer)
    
    // dtype entry
    smeta.push(0xdb); // str with up to 2^31 elements
    let mut buf = [0u8; 4];
    let bytes = dtype_len.to_ne_bytes();
    swap_store(&mut buf, &bytes);
    smeta.extend_from_slice(&buf);
    smeta.extend_from_slice(dtype.as_bytes());
    
    let slen = smeta.len();
    if max_smeta_len != slen {
        eprintln!("meta length is inconsistent!");
        return Err(BLOSC2_ERROR_FAILURE);
    }
    
    Ok(smeta)
}

/// Read the metainfo in the b2nd metalayer.
/// 
/// This deserializes array metadata from MessagePack format stored in the blosc2 metalayer.
/// 
/// # Arguments
/// * `smeta` - The msgpack buffer (input)
/// * `ndim` - Output: The number of dimensions in the array
/// * `shape` - Output: The shape of the array (must be at least B2ND_MAX_DIM in size)
/// * `chunkshape` - Output: The shape of the chunks (must be at least B2ND_MAX_DIM in size)
/// * `blockshape` - Output: The shape of the blocks (must be at least B2ND_MAX_DIM in size)
/// * `dtype` - Output: The dtype string (if requested)
/// * `dtype_format` - Output: The format of the dtype representation (if requested)
/// 
/// # Returns
/// The number of bytes consumed from smeta, or an error code if negative
pub fn b2nd_deserialize_meta(
    smeta: &[u8],
    ndim: &mut i8,
    shape: &mut [i64],
    chunkshape: &mut [i32],
    blockshape: &mut [i32],
    dtype: &mut Option<String>,
    dtype_format: &mut Option<i8>,
) -> Result<i32, i32> {
    let mut pmeta = 0usize;
    
    // Check that we have an array with 7 entries (version, ndim, shape, chunkshape, blockshape, dtype_format, dtype)
    pmeta += 1;
    
    // version entry
    // let version = smeta[pmeta] as i8; // positive fixnum (7-bit positive integer) - unused to avoid warning
    pmeta += 1;
    
    // ndim entry
    *ndim = smeta[pmeta] as i8; // positive fixnum (7-bit positive integer)
    let ndim_aux = *ndim;
    pmeta += 1;
    
    // shape entry
    // Initialize to ones, as required by b2nd
    for i in 0..ndim_aux as usize {
        shape[i] = 1;
    }
    pmeta += 1;
    for _i in 0..ndim_aux as usize {
        pmeta += 1;
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&smeta[pmeta..pmeta + 8]);
        let mut result = [0u8; 8];
        swap_store(&mut result, &buf);
        shape[_i] = i64::from_ne_bytes(result);
        pmeta += 8;
    }
    
    // chunkshape entry
    // Initialize to ones, as required by b2nd
    for i in 0..ndim_aux as usize {
        chunkshape[i] = 1;
    }
    pmeta += 1;
    for _i in 0..ndim_aux as usize {
        pmeta += 1;
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&smeta[pmeta..pmeta + 4]);
        let mut result = [0u8; 4];
        swap_store(&mut result, &buf);
        chunkshape[_i] = i32::from_ne_bytes(result);
        pmeta += 4;
    }
    
    // blockshape entry
    // Initialize to ones, as required by b2nd
    for i in 0..ndim_aux as usize {
        blockshape[i] = 1;
    }
    pmeta += 1;
    for _i in 0..ndim_aux as usize {
        pmeta += 1;
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&smeta[pmeta..pmeta + 4]);
        let mut result = [0u8; 4];
        swap_store(&mut result, &buf);
        blockshape[_i] = i32::from_ne_bytes(result);
        pmeta += 4;
    }
    
    // dtype entry
    if dtype.is_none() && dtype_format.is_none() {
        return Ok(pmeta as i32);
    }
    
    if pmeta < smeta.len() {
        // dtype info is here
        if let Some(ref mut fmt) = dtype_format {
            *fmt = smeta[pmeta] as i8;
        }
        pmeta += 1;
        pmeta += 1; // skip str marker
        
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&smeta[pmeta..pmeta + 4]);
        let mut result = [0u8; 4];
        swap_store(&mut result, &buf);
        let dtype_len = i32::from_ne_bytes(result) as usize;
        pmeta += 4;
        
        if let Some(ref mut dt) = dtype {
            let dtype_bytes = &smeta[pmeta..pmeta + dtype_len];
            *dt = String::from_utf8_lossy(dtype_bytes).to_string();
        }
        pmeta += dtype_len;
    } else {
        // dtype is mandatory in b2nd metalayer, but this is mainly meant as
        // a fall-back for deprecated caterva headers
        *dtype = None;
        if let Some(ref mut fmt) = dtype_format {
            *fmt = 0;
        }
    }
    
    Ok(pmeta as i32)
}

/// Utility function to convert multidimensional index to unidimensional index.
/// 
/// This is a helper function that converts a multidimensional array index into
/// a single linear index based on the strides of the array.
/// 
/// # Arguments
/// * `index` - The multidimensional index
/// * `ndim` - The number of dimensions
/// * `strides` - The stride for each dimension
/// 
/// # Returns
/// The linear (unidimensional) index
pub fn blosc2_multidim_to_unidim(index: &[i64], ndim: i8, strides: &[i64]) -> i64 {
    let mut i = 0;
    for j in 0..ndim as usize {
        i += index[j] * strides[j];
    }
    i
}

/// Copy a slice of a source array into another array. 
/// 
/// The arrays have the same number of dimensions (though their shapes may differ), 
/// the same item size, and they are stored as C buffers with contiguous data (any
/// padding is considered part of the array).
/// 
/// # Arguments
/// * `ndim` - The number of dimensions in both arrays
/// * `itemsize` - The size of the individual data item in both arrays
/// * `src` - The buffer for getting the data from the source array
/// * `src_pad_shape` - The shape of the source array, including padding
/// * `src_start` - The source coordinates where the slice will begin
/// * `src_stop` - The source coordinates where the slice will end
/// * `dst` - The buffer for setting the data into the destination array
/// * `dst_pad_shape` - The shape of the destination array, including padding
/// * `dst_start` - The destination coordinates where the slice will be placed
/// 
/// # Returns
/// An error code (BLOSC2_ERROR_SUCCESS on success)
/// 
/// # Note
/// This is a version that uses signed 32-bit integers for copying data. This is useful 
/// when data is stored in a buffer that uses itemsizes that are larger than 255 bytes.
/// 
/// Please make sure that slice boundaries fit within the source and destination arrays 
/// before using this function, as it does not perform these checks itself.
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
    let ndim_usize = ndim as usize;
    let mut copy_shape = [0i64; B2ND_MAX_DIM];
    
    // Compute the shape of the copy
    for i in 0..ndim_usize {
        copy_shape[i] = src_stop[i] - src_start[i];
        if copy_shape[i] == 0 {
            return BLOSC2_ERROR_SUCCESS;
        }
    }

    // Compute the strides
    let mut src_strides = [0i64; B2ND_MAX_DIM];
    src_strides[ndim_usize - 1] = 1;
    for i in (0..ndim_usize - 1).rev() {
        src_strides[i] = src_strides[i + 1] * src_pad_shape[i + 1];
    }

    let mut dst_strides = [0i64; B2ND_MAX_DIM];
    dst_strides[ndim_usize - 1] = 1;
    for i in (0..ndim_usize - 1).rev() {
        dst_strides[i] = dst_strides[i + 1] * dst_pad_shape[i + 1];
    }

    // Align the buffers removing unnecessary data
    let src_start_n = blosc2_multidim_to_unidim(src_start, ndim, &src_strides);
    let src_offset = (src_start_n * itemsize as i64) as usize;
    let bsrc = &src[src_offset..];

    let dst_start_n = blosc2_multidim_to_unidim(dst_start, ndim, &dst_strides);
    let dst_offset = (dst_start_n * itemsize as i64) as usize;
    let bdst = &mut dst[dst_offset..];

    // For 1D case, simple memcpy
    if ndim == 1 {
        let copy_nbytes = (copy_shape[0] * itemsize as i64) as usize;
        bdst[..copy_nbytes].copy_from_slice(&bsrc[..copy_nbytes]);
        return BLOSC2_ERROR_SUCCESS;
    }

    // For multi-dimensional arrays, use fallback implementation
    // (specialized loops for 2-8 dimensions are omitted for simplicity in single-threaded context)
    let copy_nbytes = (copy_shape[ndim_usize - 1] * itemsize as i64) as usize;
    let mut number_of_copies = 1i64;
    for i in 0..ndim_usize - 1 {
        number_of_copies *= copy_shape[i];
    }

    for ncopy in 0..number_of_copies {
        // Compute the start of the copy
        let mut copy_start = [0i64; B2ND_MAX_DIM];
        blosc2_unidim_to_multidim(ndim_usize - 1, &copy_shape, ncopy, &mut copy_start);

        // Translate this index to the src buffer
        let src_copy_start = blosc2_multidim_to_unidim(&copy_start, ndim - 1, &src_strides);

        // Translate this index to the dst buffer
        let dst_copy_start = blosc2_multidim_to_unidim(&copy_start, ndim - 1, &dst_strides);

        // Perform the copy
        let src_idx = (src_copy_start * itemsize as i64) as usize;
        let dst_idx = (dst_copy_start * itemsize as i64) as usize;

        bdst[dst_idx..dst_idx + copy_nbytes].copy_from_slice(&bsrc[src_idx..src_idx + copy_nbytes]);
    }

    BLOSC2_ERROR_SUCCESS
}

/// Helper function to convert unidimensional index to multidimensional index.
pub fn blosc2_unidim_to_multidim(ndim: usize, shape: &[i64], i: i64, index: &mut [i64]) {
    if ndim == 0 {
        return;
    }
    let mut strides = [0i64; B2ND_MAX_DIM];
    strides[ndim - 1] = 1;
    for j in (0..ndim - 1).rev() {
        strides[j] = shape[j + 1] * strides[j + 1];
    }

    index[0] = i / strides[0];
    for j in 1..ndim {
        index[j] = (i % strides[j - 1]) / strides[j];
    }
}

/// Copy a slice of a source array into another array (deprecated version).
/// 
/// # Note
/// This is kept for backward compatibility with existing code. New code should use
/// b2nd_copy_buffer2 instead.
/// 
/// # Deprecated
/// Use b2nd_copy_buffer2 instead, which supports itemsizes larger than 255 bytes.
#[deprecated(note = "Use b2nd_copy_buffer2 instead")]
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
    // Simply cast itemsize to i32 and delegate
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
