// Corresponds to c-blosc2/blosc/shuffle.c (and .h)

use crate::include::blosc2_include::{BLOSC2_ERROR_INVALID_PARAM, BLOSC2_VERSION_FORMAT};
use super::shuffle_generic::{shuffle_generic, unshuffle_generic};
use super::bitshuffle_generic::{bshuf_trans_bit_elem_scal, bshuf_untrans_bit_elem_scal};
use std::sync::Once;

/* According to the instructions, we ignore multi-threading and architecture-specific optimizations.
   We do everything single-threaded and use only the generic implementations. */

// blosc_cpu_features enum from C code (simplified)
#[allow(dead_code)]
enum BloscCpuFeatures {
    Nothing = 0,
    // SSE2 = 1,
    // AVX2 = 2,
    // NEON = 4,
    // ALTIVEC = 8,
    // AVX512 = 16,
}

/* An implementation of shuffle/unshuffle routines. */
struct ShuffleImplementation {
    /* Name of this implementation. */
    #[allow(dead_code)]
    name: &'static str,
    /* Function pointer to the shuffle routine for this implementation. */
    shuffle: fn(i32, i32, &[u8], &mut [u8]),
    /* Function pointer to the unshuffle routine for this implementation. */
    unshuffle: fn(i32, i32, &[u8], &mut [u8]),
    /* Function pointer to the bitshuffle routine for this implementation. */
    bitshuffle: fn(&[u8], &mut [u8], usize, usize) -> i64,
    /* Function pointer to the bitunshuffle routine for this implementation. */
    bitunshuffle: fn(&[u8], &mut [u8], usize, usize) -> i64,
}

/* Detect hardware and set function pointers to the best shuffle/unshuffle
   implementations supported by the host processor. */
/* Since we ignore architecture-specific optimizations, always return NOTHING. */
fn blosc_get_cpu_features() -> BloscCpuFeatures {
    BloscCpuFeatures::Nothing
}

/* Get shuffle implementation. Since we ignore optimizations, always return generic. */
fn get_shuffle_implementation() -> ShuffleImplementation {
    /* Processor doesn't support any of the hardware-accelerated implementations,
       so use the generic implementation. */
    ShuffleImplementation {
        name: "generic",
        shuffle: shuffle_generic,
        unshuffle: unshuffle_generic,
        bitshuffle: bshuf_trans_bit_elem_scal,
        bitunshuffle: bshuf_untrans_bit_elem_scal,
    }
}

/* The dynamically-chosen shuffle/unshuffle implementation.
   This is only safe to use once initialized. */
static mut HOST_IMPLEMENTATION: Option<ShuffleImplementation> = None;
static INIT: Once = Once::new();

/* Initialize the shuffle implementation, if necessary. */
#[inline]
fn init_shuffle_implementation() {
    /* In Rust, we use std::sync::Once for thread-safe initialization.
       This is similar to the C implementation but uses Rust's safe primitives. */
    INIT.call_once(|| {
        /* Initialize the implementation. */
        let implementation = get_shuffle_implementation();
        
        unsafe {
            HOST_IMPLEMENTATION = Some(implementation);
        }
    });
}

/* Shuffle a block by dynamically dispatching to the appropriate
   hardware-accelerated routine at run-time. */
pub fn blosc2_shuffle(
    typesize: i32,
    blocksize: i32,
    src: &[u8],
    dest: &mut [u8],
) -> i32 {
    /* Initialize the shuffle implementation if necessary. */
    init_shuffle_implementation();

    if typesize < 1 || typesize > 256 || blocksize < 0 {
        return BLOSC2_ERROR_INVALID_PARAM;
    }

    /* The implementation is initialized.
       Dispatch to its shuffle routine. */
    unsafe {
        if let Some(ref implementation) = HOST_IMPLEMENTATION {
            (implementation.shuffle)(typesize, blocksize, src, dest);
        }
    }

    blocksize
}

/* Unshuffle a block by dynamically dispatching to the appropriate
   hardware-accelerated routine at run-time. */
pub fn blosc2_unshuffle(
    typesize: i32,
    blocksize: i32,
    src: &[u8],
    dest: &mut [u8],
) -> i32 {
    /* Initialize the shuffle implementation if necessary. */
    init_shuffle_implementation();

    if typesize < 1 || typesize > 256 || blocksize < 0 {
        return BLOSC2_ERROR_INVALID_PARAM;
    }

    /* The implementation is initialized.
       Dispatch to its unshuffle routine. */
    unsafe {
        if let Some(ref implementation) = HOST_IMPLEMENTATION {
            (implementation.unshuffle)(typesize, blocksize, src, dest);
        }
    }

    blocksize
}

/*  Bit-shuffle a block by dynamically dispatching to the appropriate
    hardware-accelerated routine at run-time. */
pub fn blosc2_bitshuffle(
    typesize: i32,
    blocksize: i32,
    src: &[u8],
    dest: &mut [u8],
) -> i32 {
    /* Initialize the shuffle implementation if necessary. */
    init_shuffle_implementation();
    let mut size = (blocksize / typesize) as usize;

    if typesize < 1 || typesize > 256 || blocksize < 0 {
        return BLOSC2_ERROR_INVALID_PARAM;
    }

    /* bitshuffle only supports a number of elements that is a multiple of 8. */
    size -= size % 8;
    
    let ret = unsafe {
        if let Some(ref implementation) = HOST_IMPLEMENTATION {
            (implementation.bitshuffle)(src, dest, size, typesize as usize)
        } else {
            0
        }
    };
    
    if ret < 0 {
        // Some error in bitshuffle (should not happen)
        // BLOSC_TRACE_ERROR("the impossible happened: the bitshuffle filter failed!");
        return ret as i32;
    }

    // Copy the leftovers
    let offset = size * typesize as usize;
    let remaining = blocksize as usize - offset;
    dest[offset..offset + remaining].copy_from_slice(&src[offset..offset + remaining]);

    blocksize
}

/*  Bit-unshuffle a block by dynamically dispatching to the appropriate
    hardware-accelerated routine at run-time. */
fn bitunshuffle(
    typesize: i32,
    blocksize: i32,
    src: &[u8],
    dest: &mut [u8],
    format_version: u8,
) -> i32 {
    /* Initialize the shuffle implementation if necessary. */
    init_shuffle_implementation();
    let mut size = (blocksize / typesize) as usize;

    if format_version == 2 {
        /* Starting from version 3, bitshuffle() works differently */
        if (size % 8) == 0 {
            /* The number of elems is a multiple of 8 which is supported by
               bitshuffle. */
            let ret = unsafe {
                if let Some(ref implementation) = HOST_IMPLEMENTATION {
                    (implementation.bitunshuffle)(src, dest, blocksize as usize / typesize as usize, typesize as usize)
                } else {
                    0
                }
            };
            
            if ret < 0 {
                // Some error in bitshuffle (should not happen)
                // BLOSC_TRACE_ERROR("the impossible happened: the bitunshuffle filter failed!");
                return ret as i32;
            }
        } else {
            dest[..blocksize as usize].copy_from_slice(&src[..blocksize as usize]);
        }
    } else {
        /* bitshuffle only supports a number of bytes that is a multiple of 8. */
        size -= size % 8;
        
        let ret = unsafe {
            if let Some(ref implementation) = HOST_IMPLEMENTATION {
                (implementation.bitunshuffle)(src, dest, size, typesize as usize)
            } else {
                0
            }
        };
        
        if ret < 0 {
            // BLOSC_TRACE_ERROR("the impossible happened: the bitunshuffle filter failed!");
            return ret as i32;
        }

        /* Copy the leftovers */
        let offset = size * typesize as usize;
        let remaining = blocksize as usize - offset;
        dest[offset..offset + remaining].copy_from_slice(&src[offset..offset + remaining]);
    }

    blocksize
}

/* Stub public API that redirects to internal implementation. */
pub fn blosc2_bitunshuffle(
    typesize: i32,
    blocksize: i32,
    src: &[u8],
    dest: &mut [u8],
) -> i32 {
    bitunshuffle(typesize, blocksize, src, dest, BLOSC2_VERSION_FORMAT)
}

