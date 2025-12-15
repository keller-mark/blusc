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
fn blosc2_unidim_to_multidim(ndim: usize, shape: &[i64], i: i64, index: &mut [i64]) {
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

/*

/** @file b2nd.h
 * @brief Blosc2 NDim header file.
 *
//  * This file contains Blosc2 NDim public API and the structures needed to use it.
//  * @author Blosc Development Team <blosc@blosc.org>
//  */
// 
// 
// #include "blosc2.h"
// 
// 
// #if defined(_MSC_VER)
// #define B2ND_DEPRECATED(msg) __declspec(deprecated(msg))
// #elif defined(__GNUC__) || defined(__clang__)
// #define B2ND_DEPRECATED(msg) __attribute__((deprecated(msg)))
// #else
// #define B2ND_DEPRECATED(msg)
// #endif
// 
// /* The version for metalayer format; starts from 0 and it must not exceed 127 */
// #define B2ND_METALAYER_VERSION 0
// 
// /* The maximum number of dimensions for b2nd arrays */
// #define B2ND_MAX_DIM 16
// 
// /* The maximum number of metalayers for b2nd arrays */
// #define B2ND_MAX_METALAYERS (BLOSC2_MAX_METALAYERS - 1)
// 
// /* NumPy dtype format
//  * https://numpy.org/doc/stable/reference/arrays.dtypes.html#arrays-dtypes-constructing
//  */
// #define DTYPE_NUMPY_FORMAT 0
// 
// /* The default data type */
// #define B2ND_DEFAULT_DTYPE "|u1"
// /* The default data format */
// #define B2ND_DEFAULT_DTYPE_FORMAT DTYPE_NUMPY_FORMAT
// 
// /**
//  * @brief An *optional* cache for a single block.
//  *
//  * When a chunk is needed, it is copied into this cache. In this way, if the same chunk is needed
//  * again afterwards, it is not necessary to recover it because it is already in the cache.
//  */
// struct chunk_cache_s {
//   uint8_t *data;
//   //!< The chunk data.
//   int64_t nchunk;
//   //!< The chunk number in cache. If @p nchunk equals to -1, it means that the cache is empty.
// };
// 
// /**
//  * @brief General parameters needed for the creation of a b2nd array.
//  */
// typedef struct b2nd_context_s b2nd_context_t;   /* opaque type */
// 
// /**
//  * @brief A multidimensional array of data that can be compressed.
//  */
// typedef struct {
//   blosc2_schunk *sc;
//   //!< Pointer to a Blosc super-chunk
//   int64_t shape[B2ND_MAX_DIM];
//   //!< Shape of original data.
//   int32_t chunkshape[B2ND_MAX_DIM];
//   //!< Shape of each chunk.
//   int64_t extshape[B2ND_MAX_DIM];
//   //!< Shape of padded data.
//   int32_t blockshape[B2ND_MAX_DIM];
//   //!< Shape of each block.
//   int64_t extchunkshape[B2ND_MAX_DIM];
//   //!< Shape of padded chunk.
//   int64_t nitems;
//   //!< Number of items in original data.
//   int32_t chunknitems;
//   //!< Number of items in each chunk.
//   int64_t extnitems;
//   //!< Number of items in padded data.
//   int32_t blocknitems;
//   //!< Number of items in each block.
//   int64_t extchunknitems;
//   //!< Number of items in a padded chunk.
//   int8_t ndim;
//   //!< Data dimensions.
//   struct chunk_cache_s chunk_cache;
//   //!< A partition cache.
//   int64_t item_array_strides[B2ND_MAX_DIM];
//   //!< Item - shape strides.
//   int64_t item_chunk_strides[B2ND_MAX_DIM];
//   //!< Item - shape strides.
//   int64_t item_extchunk_strides[B2ND_MAX_DIM];
//   //!< Item - shape strides.
//   int64_t item_block_strides[B2ND_MAX_DIM];
//   //!< Item - shape strides.
//   int64_t block_chunk_strides[B2ND_MAX_DIM];
//   //!< Item - shape strides.
//   int64_t chunk_array_strides[B2ND_MAX_DIM];
//   //!< Item - shape strides.
//   char *dtype;
//   //!< Data type. Different formats can be supported (see dtype_format).
//   int8_t dtype_format;
//   //!< The format of the data type.  Default is DTYPE_NUMPY_FORMAT.
// } b2nd_array_t;
// 
// 
// /**
//  * @brief Create b2nd params.
//  *
//  * @param b2_storage The Blosc2 storage params.
//  * @param ndim The dimensions.
//  * @param shape The shape.
//  * @param chunkshape The chunk shape.
//  * @param blockshape The block shape.
//  * @param dtype The data type expressed as a string version.
//  * @param dtype_format The data type format; DTYPE_NUMPY_FORMAT should be chosen for NumPy compatibility.
//  * @param metalayers The memory pointer to the list of the metalayers desired.
//  * @param nmetalayers The number of metalayers.
//  *
//  * @return A pointer to the new b2nd params. NULL is returned if this fails.
//  *
//  * @note The pointer returned must be freed when not used anymore with #b2nd_free_ctx.
//  *
//  */
// BLOSC_EXPORT b2nd_context_t *
// b2nd_create_ctx(const blosc2_storage *b2_storage, int8_t ndim, const int64_t *shape, const int32_t *chunkshape,
//                 const int32_t *blockshape, const char *dtype, int8_t dtype_format, const blosc2_metalayer *metalayers,
//                 int32_t nmetalayers);
// 
// 
// /**
//  * @brief Free the resources associated with b2nd_context_t.
//  *
//  * @param ctx The b2nd context to free.
//  *
//  * @return An error code.
//  *
//  * @note This is safe in the sense that it will not free the schunk pointer in internal cparams.
//  *
//  */
// BLOSC_EXPORT int b2nd_free_ctx(b2nd_context_t *ctx);
// 
// 
// /**
//  * @brief Create an uninitialized array.
//  *
//  * @param ctx The b2nd context for the new array.
//  * @param array The memory pointer where the array will be created.
//  *
//  * @return An error code.
//  */
// BLOSC_EXPORT int b2nd_uninit(b2nd_context_t *ctx, b2nd_array_t **array);
// 
// 
// /**
//  * @brief Create an empty array.
//  *
//  * @param ctx The b2nd context for the new array.
//  * @param array The memory pointer where the array will be created.
//  *
//  * @return An error code.
//  */
// BLOSC_EXPORT int b2nd_empty(b2nd_context_t *ctx, b2nd_array_t **array);
// 
// 
// /**
//  * Create an array, with zero being used as the default value for
//  * uninitialized portions of the array.
//  *
//  * @param ctx The b2nd context for the new array.
//  * @param array The memory pointer where the array will be created.
//  *
//  * @return An error code.
//  */
// BLOSC_EXPORT int b2nd_zeros(b2nd_context_t *ctx, b2nd_array_t **array);
// 
// 
// /**
//  * Create an array, with NaN being used as the default value for
//  * uninitialized portions of the array. Should only be used with type sizes
//  * of either 4 or 8. Other sizes generate an error.
//  *
//  * @param ctx The b2nd context for the new array.
//  * @param array The memory pointer where the array will be created.
//  *
//  * @return An error code.
//  */
// BLOSC_EXPORT int b2nd_nans(b2nd_context_t *ctx, b2nd_array_t **array);
// 
// 
// /**
//  * Create an array, with @p fill_value being used as the default value for
//  * uninitialized portions of the array.
//  *
//  * @param ctx The b2nd context for the new array.
//  * @param array The memory pointer where the array will be created.
//  * @param fill_value Default value for uninitialized portions of the array.
//  *
//  * @return An error code.
//  */
// BLOSC_EXPORT int b2nd_full(b2nd_context_t *ctx, b2nd_array_t **array, const void *fill_value);
// 
// /**
//  * @brief Free an array.
//  *
//  * @param array The array.
//  *
//  * @return An error code.
//  */
// BLOSC_EXPORT int b2nd_free(b2nd_array_t *array);
// 
// /**
//  * @brief Create a b2nd array from a super-chunk. It can only be used if the array
//  * is backed by a blosc super-chunk.
//  *
//  * @param schunk The blosc super-chunk where the b2nd array is stored.
//  * @param array The memory pointer where the array will be created.
//  *
//  * @return An error code.
//  */
// BLOSC_EXPORT int b2nd_from_schunk(blosc2_schunk *schunk, b2nd_array_t **array);
// 
// /**
//  * Create a serialized super-chunk from a b2nd array.
//  *
//  * @param array The b2nd array to be serialized.
//  * @param cframe The pointer of the buffer where the in-memory array will be copied.
//  * @param cframe_len The length of the in-memory array buffer.
//  * @param needs_free Whether the buffer should be freed or not.
//  *
//  * @return An error code
//  */
// BLOSC_EXPORT int b2nd_to_cframe(const b2nd_array_t *array, uint8_t **cframe,
//                                 int64_t *cframe_len, bool *needs_free);
// 
// /**
//  * @brief Create a b2nd array from a serialized super-chunk.
//  *
//  * @param cframe The buffer of the in-memory array.
//  * @param cframe_len The size (in bytes) of the in-memory array.
//  * @param copy Whether b2nd should make a copy of the cframe data or not. The copy will be made to an internal sparse frame.
//  * @param array The memory pointer where the array will be created.
//  *
//  * @return An error code.
//  */
// BLOSC_EXPORT int b2nd_from_cframe(uint8_t *cframe, int64_t cframe_len, bool copy, b2nd_array_t **array);
// 
// /**
//  * @brief Open a b2nd array from a file.
//  *
//  * @param urlpath The path of the b2nd array on disk.
//  * @param array The memory pointer where the array info will be stored.
//  *
//  * @return An error code.
//  */
// BLOSC_EXPORT int b2nd_open(const char *urlpath, b2nd_array_t **array);
// 
// /**
//  * @brief Open a b2nd array from a file using an offset.
//  *
//  * @param urlpath The path of the b2nd array on disk.
//  * @param array The memory pointer where the array info will be stored.
//  * @param offset The offset in the file where the b2nd array frame starts.
//  *
//  * @return An error code.
//  */
// BLOSC_EXPORT int b2nd_open_offset(const char *urlpath, b2nd_array_t **array, int64_t offset);
// 
// /**
//  * @brief Save b2nd array into a specific urlpath.
//  *
//  * @param array The array to be saved.
//  * @param urlpath The urlpath where the array will be stored.
//  *
//  * @return An error code.
//  */
// BLOSC_EXPORT int b2nd_save(const b2nd_array_t *array, char *urlpath);
// 
// /**
//  * @brief Append a b2nd array into a file.
//  *
//  * @param array The array to write.
//  * @param urlpath The path for persistent storage.
//  *
//  * @return If successful, return the offset where @p array has been appended in @p urlpath.
//  * Else, a negative value.
//  */
// BLOSC_EXPORT int64_t b2nd_save_append(const b2nd_array_t *array, const char *urlpath);
// 
// /**
//  * @brief Create a b2nd array from a C buffer.
//  *
//  * @param ctx The b2nd context for the new array.
//  * @param array The memory pointer where the array will be created.
//  * @param buffer The buffer where source data is stored.
//  * @param buffersize The size (in bytes) of the buffer.
//  *
//  * @return An error code.
//  */
// BLOSC_EXPORT int b2nd_from_cbuffer(b2nd_context_t *ctx, b2nd_array_t **array, const void *buffer, int64_t buffersize);
// 
// /**
//  * @brief Extract the data from a b2nd array into a C buffer.
//  *
//  * @param array The b2nd array.
//  * @param buffer The buffer where the data will be stored.
//  * @param buffersize Size (in bytes) of the buffer.
//  *
//  * @return An error code.
//  */
// BLOSC_EXPORT int b2nd_to_cbuffer(const b2nd_array_t *array, void *buffer, int64_t buffersize);
// 
// /**
//  * @brief Get a slice from an array and store it into a new array.
//  *
//  * @param ctx The b2nd context for the new array.
//  * @param array The memory pointer where the array will be created.
//  * @param src The array from which the slice will be extracted
//  * @param start The coordinates where the slice will begin.
//  * @param stop The coordinates where the slice will end.
//  *
//  * @return An error code.
//  *
//  * @note The ndim and shape from ctx will be overwritten by the src and stop-start respectively.
//  *
//  */
// BLOSC_EXPORT int b2nd_get_slice(b2nd_context_t *ctx, b2nd_array_t **array, const b2nd_array_t *src,
//                                 const int64_t *start, const int64_t *stop);
// 
// /**
//  * @brief Squeeze a b2nd array
//  *
//  * This function remove selected single-dimensional entries from the shape of a
//  b2nd array.
//  *
//  * @param array The b2nd array.
//  * @param view The memory pointer where the new view will be created.
//  * @param index Indexes of the single-dimensional entries to remove.
//  *
//  * @return An error code
//  */
// BLOSC_EXPORT int b2nd_squeeze_index(b2nd_array_t *array, b2nd_array_t **view, const bool *index);
// 
// /**
//  * @brief Squeeze a b2nd array
//  *
//  * This function remove single-dimensional entries from the shape of a b2nd array.
//  *
//  * @param array The b2nd array.
//  *  @param view The memory pointer where the new view will be created.
//  *
//  * @return An error code
//  */
// BLOSC_EXPORT int b2nd_squeeze(b2nd_array_t *array, b2nd_array_t **view);
// 
// /**
//  * @brief Add a newaxis to a b2nd array at location @p axis.
//  *
//  * @param array The b2nd array to be expanded.
//  * @param axis The axes where the new dimensions will be added.
//  * @param view The memory pointer where the new view will be created.
//  * @param final_dims The final number of dimensions. Should be same as the number of elements in @p axis.
//  *
//  * @return An error code.
//  */
// BLOSC_EXPORT int b2nd_expand_dims(const b2nd_array_t *array, b2nd_array_t **view, const bool *axis,
//     const uint8_t final_dims);
// 
// /**
//  * @brief Get a slice from an array and store it into a C buffer.
//  *
//  * @param array The array from which the slice will be extracted.
//  * @param start The coordinates where the slice will begin.
//  * @param stop The coordinates where the slice will end.
//  * @param buffershape The shape of the buffer.
//  * @param buffer The buffer where the data will be stored.
//  * @param buffersize The size (in bytes) of the buffer.
//  *
//  * @return An error code.
//  */
// BLOSC_EXPORT int b2nd_get_slice_cbuffer(const b2nd_array_t *array, const int64_t *start, const int64_t *stop,
//                                         void *buffer, const int64_t *buffershape, int64_t buffersize);
// 
// /**
//  * @brief Set a slice in a b2nd array using a C buffer.
//  *
//  * @param buffer The buffer where the slice data is.
//  * @param buffershape The shape of the buffer.
//  * @param buffersize The size (in bytes) of the buffer.
//  * @param start The coordinates where the slice will begin.
//  * @param stop The coordinates where the slice will end.
//  * @param array The b2nd array where the slice will be set
//  *
//  * @return An error code.
//  */
// BLOSC_EXPORT int b2nd_set_slice_cbuffer(const void *buffer, const int64_t *buffershape, int64_t buffersize,
//                                         const int64_t *start, const int64_t *stop, b2nd_array_t *array);
// 
// /**
//  * @brief Make a copy of the array data. The copy is done into a new b2nd array.
//  *
//  * @param ctx The b2nd context for the new array.
//  * @param src The array from which data is copied.
//  * @param array The memory pointer where the array will be created.
//  *
//  * @return An error code
//  *
//  * @note The ndim and shape in ctx will be overwritten by the src ctx.
//  *
//  */
// BLOSC_EXPORT int b2nd_copy(b2nd_context_t *ctx, const b2nd_array_t *src, b2nd_array_t **array);
// 
// /**
//  * @brief Concatenate arrays. The result is stored in a new b2nd array, or an enlarged one.
//  *
//  * @param ctx The b2nd context for the new array.
//  * @param src1 The first array from which data is copied.
//  * @param src2 The second array from which data is copied.
//  * @param axis The axis along which the arrays will be concatenated.
//  * @param copy Whether the data should be copied or not. If false, the @p src1 array
//  *   will be expanded as needed to keep the result.
//  * @param array The memory pointer where the array will be created.  It will have the same
//  *   metalayers of @p src1, except for the b2nd metalayer, which will be updated with the
//  *   new shape.
//  *
//  * @ note The two arrays must have the same shape in all dimensions except the concatenation axis.
//  * Also, the typesize of the two arrays must be the same.
//  *
//  * @return An error code
//  *
//  * @note The ndim and shape in ctx will be overwritten by the src1 ctx.
//  *
//  */
// BLOSC_EXPORT int b2nd_concatenate(b2nd_context_t *ctx, const b2nd_array_t *src1, const b2nd_array_t *src2,
//                                   int8_t axis, bool copy, b2nd_array_t **array);
// 
// /**
//  * @brief Print metalayer parameters.
//  *
//  * @param array The array where the metalayer is stored.
//  *
//  * @return An error code
//  */
// BLOSC_EXPORT int b2nd_print_meta(const b2nd_array_t *array);
// 
// /**
//  * @brief Resize the shape of an array
//  *
//  * @param array The array to be resized.
//  * @param new_shape The new shape from the array.
//  * @param start The position in which the array will be extended or shrunk.
//  *
//  * @return An error code
//  */
// BLOSC_EXPORT int b2nd_resize(b2nd_array_t *array, const int64_t *new_shape, const int64_t *start);
// 
// 
// /**
//  * @brief Insert given buffer in an array extending the given axis.
//  *
//  * @param array The array to insert the data in.
//  * @param buffer The buffer data to be inserted.
//  * @param buffersize The size (in bytes) of the buffer.
//  * @param axis The axis that will be extended.
//  * @param insert_start The position inside the axis to start inserting the data.
//  *
//  * @return An error code.
//  */
// BLOSC_EXPORT int b2nd_insert(b2nd_array_t *array, const void *buffer, int64_t buffersize,
//                              int8_t axis, int64_t insert_start);
// 
// /**
//  * Append a buffer at the end of a b2nd array.
//  *
//  * @param array The array to append the data in.
//  * @param buffer The buffer data to be appended.
//  * @param buffersize Size (in bytes) of the buffer.
//  * @param axis The axis that will be extended to append the data.
//  *
//  * @return An error code.
//  */
// BLOSC_EXPORT int b2nd_append(b2nd_array_t *array, const void *buffer, int64_t buffersize,
//                              int8_t axis);
// 
// /**
//  * @brief Delete shrinking the given axis delete_len items.
//  *
//  * @param array The array to shrink.
//  * @param axis The axis to shrink.
//  * @param delete_start The start position from the axis to start deleting chunks.
//  * @param delete_len The number of items to delete to the array->shape[axis].
//  *   The newshape[axis] will be the old array->shape[axis] - delete_len
//  *
//  * @return An error code.
//  *
//  * @note See also b2nd_resize
//  */
// BLOSC_EXPORT int b2nd_delete(b2nd_array_t *array, int8_t axis,
//                              int64_t delete_start, int64_t delete_len);
// 
// 
// // Indexing section
// 
// /**
//  * @brief Get an element selection along each dimension of an array independently.
//  *
//  * @param array The array to get the data from.
//  * @param selection The elements along each dimension.
//  * @param selection_size The size of the selection along each dimension.
//  * @param buffer The buffer for getting the data.
//  * @param buffershape The shape of the buffer.
//  * @param buffersize The buffer size (in bytes).
//  *
//  * @return An error code.
//  *
//  * @note See also b2nd_set_orthogonal_selection.
//  */
// BLOSC_EXPORT int b2nd_get_orthogonal_selection(const b2nd_array_t *array, int64_t **selection,
//                                                int64_t *selection_size, void *buffer,
//                                                int64_t *buffershape, int64_t buffersize);
// 
// /**
//  * @brief Set an element selection along each dimension of an array independently.
//  *
//  * @param array The array to set the data to.
//  * @param selection The elements along each dimension.
//  * @param selection_size The size of the selection along each dimension.
//  * @param buffer The buffer with the data for setting.
//  * @param buffershape The shape of the buffer.
//  * @param buffersize The buffer size (in bytes).
//  *
//  * @return An error code.
//  *
//  * @note See also b2nd_get_orthogonal_selection.
//  */
// BLOSC_EXPORT int b2nd_set_orthogonal_selection(b2nd_array_t *array, int64_t **selection,
//                                                int64_t *selection_size, const void *buffer,
//                                                int64_t *buffershape, int64_t buffersize);
// 
// 
// /**
//  * @brief Create the metainfo for the b2nd metalayer.
//  *
//  * @param ndim The number of dimensions in the array.
//  * @param shape The shape of the array.
//  * @param chunkshape The shape of the chunks in the array.
//  * @param blockshape The shape of the blocks in the array.
//  * @param dtype A string representation of the data type of the array.
//  * @param dtype_format The format of the dtype representation. 0 means NumPy.
//  * @param smeta The msgpack buffer (output).
//  *
//  * @return An error code.
//  */
// BLOSC_EXPORT int b2nd_serialize_meta(int8_t ndim, const int64_t *shape, const int32_t *chunkshape,
//                                      const int32_t *blockshape, const char *dtype,
//                                      int8_t dtype_format, uint8_t **smeta);
// 
// /**
//  * @brief Read the metainfo in the b2nd metalayer.
//  *
//  * @param smeta The msgpack buffer (input).
//  * @param smeta_len The length of the smeta buffer (input).
//  * @param ndim The number of dimensions in the array (output).
//  * @param shape The shape of the array (output).
//  * @param chunkshape The shape of the chunks in the array (output).
//  * @param blockshape The shape of the blocks in the array (output).
//  * @param dtype A string representation of the data type of the array (output).
//  * @param dtype_format The format of the dtype representation (output). 0 means NumPy (the default).
//  *
//  * @note This function is inlined and available even when not linking with libblosc2.
//  *
//  * @return An error code.
//  */
// BLOSC_EXPORT int b2nd_deserialize_meta(const uint8_t *smeta, int32_t smeta_len, int8_t *ndim, int64_t *shape,
//                                        int32_t *chunkshape, int32_t *blockshape, char **dtype, int8_t *dtype_format);
// 
// // Utilities for C buffers representing multidimensional arrays
// 
// /**
//  * @brief Copy a slice of a source array into another array. The arrays have
//  * the same number of dimensions (though their shapes may differ), the same
//  * item size, and they are stored as C buffers with contiguous data (any
//  * padding is considered part of the array).
//  *
//  * @param ndim The number of dimensions in both arrays.
//  * @param itemsize The size of the individual data item in both arrays.
//  * @param src The buffer for getting the data from the source array.
//  * @param src_pad_shape The shape of the source array, including padding.
//  * @param src_start The source coordinates where the slice will begin.
//  * @param src_stop The source coordinates where the slice will end.
//  * @param dst The buffer for setting the data into the destination array.
//  * @param dst_pad_shape The shape of the destination array, including padding.
//  * @param dst_start The destination coordinates where the slice will be placed.
//  *
//  * @return An error code.
//  *
//  * @note This is kept for backward compatibility with existing code out there.  New code should use
//  * b2nd_copy_buffer2 instead.
//  *
//  * @note Please make sure that slice boundaries fit within the source and
//  * destination arrays before using this function, as it does not perform these
//  * checks itself.
//  */
// B2ND_DEPRECATED("Use b2nd_copy_buffer2 instead.")
// BLOSC_EXPORT int b2nd_copy_buffer(int8_t ndim,
//                                   uint8_t itemsize,
//                                   const void *src, const int64_t *src_pad_shape,
//                                   const int64_t *src_start, const int64_t *src_stop,
//                                   void *dst, const int64_t *dst_pad_shape,
//                                   const int64_t *dst_start);
// 
// /**
//  * @brief Copy a slice of a source array into another array. The arrays have
//  * the same number of dimensions (though their shapes may differ), the same
//  * item size, and they are stored as C buffers with contiguous data (any
//  * padding is considered part of the array).
//  *
//  * @param ndim The number of dimensions in both arrays.
//  * @param itemsize The size of the individual data item in both arrays.
//  * @param src The buffer for getting the data from the source array.
//  * @param src_pad_shape The shape of the source array, including padding.
//  * @param src_start The source coordinates where the slice will begin.
//  * @param src_stop The source coordinates where the slice will end.
//  * @param dst The buffer for setting the data into the destination array.
//  * @param dst_pad_shape The shape of the destination array, including padding.
//  * @param dst_start The destination coordinates where the slice will be placed.
//  *
//  * @return An error code.
//  *
//  * @note This is a version of (now deprecated) b2nd_copy_buffer() that uses
//  * signed 32-bit integers for copying data. This is useful when data is stored
//  * in a buffer that uses itemsizes that are larger than 255 bytes.
//  *
//  * @note Please make sure that slice boundaries fit within the source and
//  * destination arrays before using this function, as it does not perform these
//  * checks itself.
//  */
// BLOSC_EXPORT int b2nd_copy_buffer2(int8_t ndim,
//                                    int32_t itemsize,
//                                    const void *src, const int64_t *src_pad_shape,
//                                    const int64_t *src_start, const int64_t *src_stop,
//                                    void *dst, const int64_t *dst_pad_shape,
//                                    const int64_t *dst_start);
// 
// 
// 
// 
// 
// */