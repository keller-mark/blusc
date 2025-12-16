// Corresponds to c-blosc2/blosc/stune.c (and .h)

use crate::blosc::context::Blosc2Context;
use crate::include::blosc2_include::{
    BLOSC2_ERROR_SUCCESS,
    BLOSC_ALWAYS_SPLIT,
    BLOSC_AUTO_SPLIT,
    BLOSC_BLOSCLZ,
    BLOSC_DOSHUFFLE,
    BLOSC_FORWARD_COMPAT_SPLIT,
    BLOSC_LZ4,
    BLOSC_LZ4HC,
    BLOSC_MIN_BUFFERSIZE,
    BLOSC_NEVER_SPLIT,
    BLOSC_ZLIB,
    BLOSC_ZSTD,
    L1,
    MAX_STREAMS,
};

/// Whether a codec is meant for High Compression Ratios
/// Includes LZ4 + BITSHUFFLE here, but not BloscLZ + BITSHUFFLE because,
/// for some reason, the latter does not work too well
fn is_hcr(context: &Blosc2Context) -> bool {
    match context.compcode {
        x if x == BLOSC_BLOSCLZ as i32 => false,
        x if x == BLOSC_LZ4 as i32 => {
            // return (context->filter_flags & BLOSC_DOBITSHUFFLE) ? true : false;
            // Do not treat LZ4 differently than BloscLZ here
            false
        }
        x if x == BLOSC_LZ4HC as i32 => true,
        x if x == BLOSC_ZLIB as i32 => true,
        x if x == BLOSC_ZSTD as i32 => true,
        _ => false,
    }
}

/// Initialize the stune tuner
pub fn blosc_stune_init(
    _config: *mut u8,
    _cctx: *mut Blosc2Context,
    _dctx: *mut Blosc2Context,
) -> i32 {
    // BLOSC_UNUSED_PARAM(config);
    // BLOSC_UNUSED_PARAM(cctx);
    // BLOSC_UNUSED_PARAM(dctx);
    
    BLOSC2_ERROR_SUCCESS
}

/// Set the automatic blocksize 0 to its real value
pub fn blosc_stune_next_blocksize(context: &mut Blosc2Context) -> i32 {
    let clevel = context.clevel;
    let typesize = context.typesize;
    let nbytes = context.sourcesize;
    let user_blocksize = context.blocksize;
    let mut blocksize = nbytes;

    // Protection against very small buffers
    if nbytes < typesize {
        context.blocksize = 1;
        return BLOSC2_ERROR_SUCCESS;
    }

    let splitmode = split_block(context, typesize, blocksize);
    if user_blocksize != 0 {
        blocksize = user_blocksize;
        // goto last;
    } else {
        if nbytes >= L1 as i32 {
            blocksize = L1 as i32;

            /* For HCR codecs, increase the block sizes by a factor of 2 because they
                are meant for compressing large blocks (i.e. they show a big overhead
                when compressing small ones). */
            if is_hcr(context) {
                blocksize *= 2;
            }

            // Choose a different blocksize depending on the compression level
            match clevel {
                0 => {
                    // Case of plain copy
                    blocksize /= 4;
                }
                1 => {
                    blocksize /= 2;
                }
                2 => {
                    blocksize *= 1;
                }
                3 => {
                    blocksize *= 2;
                }
                4 | 5 => {
                    blocksize *= 4;
                }
                6 | 7 | 8 => {
                    blocksize *= 8;
                }
                9 => {
                    // Do not exceed 256 KB for non HCR codecs
                    blocksize *= 8;
                    if is_hcr(context) {
                        blocksize *= 2;
                    }
                }
                _ => {}
            }
        }

        /* Now the blocksize for splittable codecs */
        if clevel > 0 && splitmode != 0 {
            // For performance reasons, do not exceed 256 KB (it must fit in L2 cache)
            blocksize = match clevel {
                1 | 2 | 3 => 32 * 1024,
                4 | 5 | 6 => 64 * 1024,
                7 => 128 * 1024,
                8 => 256 * 1024,
                9 | _ => 512 * 1024,
            };
            // Multiply by typesize to get proper split sizes
            blocksize *= typesize;
            // But do not exceed 4 MB per thread (having this capacity in L3 is normal in modern CPUs)
            if blocksize > 4 * 1024 * 1024 {
                blocksize = 4 * 1024 * 1024;
            }
            if blocksize < 32 * 1024 {
                /* Do not use a too small blocksize (< 32 KB) when typesize is small */
                blocksize = 32 * 1024;
            }
        }
    }

    // last:
    /* Check that blocksize is not too large */
    if blocksize > nbytes {
        blocksize = nbytes;
    }

    // blocksize *must absolutely* be a multiple of the typesize
    if blocksize > typesize {
        blocksize = blocksize / typesize * typesize;
    }

    context.blocksize = blocksize;
    // BLOSC_INFO("compcode: %d, clevel: %d, blocksize: %d, splitmode: %d, typesize: %d",
    //            context->compcode, context->clevel, blocksize, splitmode, typesize);

    BLOSC2_ERROR_SUCCESS
}

/// Get next compression parameters
pub fn blosc_stune_next_cparams(_context: &mut Blosc2Context) -> i32 {
    // BLOSC_UNUSED_PARAM(context);
    
    BLOSC2_ERROR_SUCCESS
}

/// Update tuner with compression time
pub fn blosc_stune_update(_context: &mut Blosc2Context, _ctime: f64) -> i32 {
    // BLOSC_UNUSED_PARAM(context);
    // BLOSC_UNUSED_PARAM(ctime);
    
    BLOSC2_ERROR_SUCCESS
}

/// Free tuner resources
pub fn blosc_stune_free(_context: &mut Blosc2Context) -> i32 {
    // BLOSC_UNUSED_PARAM(context);
    
    BLOSC2_ERROR_SUCCESS
}

/// Conditions for splitting a block before compressing with a codec.
pub fn split_block(context: &Blosc2Context, typesize: i32, blocksize: i32) -> i32 {
    match context.splitmode {
        x if x == BLOSC_ALWAYS_SPLIT as i32 => return 1,
        x if x == BLOSC_NEVER_SPLIT as i32 => return 0,
        x if x == BLOSC_FORWARD_COMPAT_SPLIT as i32 => {}
        x if x == BLOSC_AUTO_SPLIT as i32 => {}
        _ => {
            // BLOSC_TRACE_WARNING("Unrecognized split mode.  Default to BLOSC_FORWARD_COMPAT_SPLIT");
        }
    }

    let compcode = context.compcode;
    let result = 
        // Fast codecs like blosclz, lz4 seems to prefer to split
        ((compcode == BLOSC_BLOSCLZ as i32) || (compcode == BLOSC_LZ4 as i32)
            // and low levels of zstd too
            || ((compcode == BLOSC_ZSTD as i32) && (context.clevel <= 5))
        ) &&
        // ...but split seems to harm cratio too much when not using shuffle
        ((context.filter_flags & BLOSC_DOSHUFFLE) != 0) &&
        (typesize <= MAX_STREAMS as i32) &&
        ((blocksize / typesize) >= BLOSC_MIN_BUFFERSIZE as i32);
    
    if result { 1 } else { 0 }
}
