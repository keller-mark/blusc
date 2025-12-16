// Corresponds to c-blosc2/blosc/shuffle-generic.c (and .h)

/**
  Generic (non-hardware-accelerated) shuffle routine.
  This is the pure element-copying nested loop. It is used by the
  generic shuffle implementation and also by the vectorized shuffle
  implementations to process any remaining elements in a block which
  is not a multiple of (type_size * vector_size).
*/
#[inline]
fn shuffle_generic_inline(
    type_size: i32,
    vectorizable_blocksize: i32,
    blocksize: i32,
    src: &[u8],
    dest: &mut [u8],
) {
    /* Calculate the number of elements in the block. */
    let neblock_quot = blocksize / type_size;
    let neblock_rem = blocksize % type_size;
    let vectorizable_elements = vectorizable_blocksize / type_size;

    /* Non-optimized shuffle */
    for j in 0..type_size {
        for i in vectorizable_elements..neblock_quot {
            dest[(j * neblock_quot + i) as usize] = src[(i * type_size + j) as usize];
        }
    }

    /* Copy any leftover bytes in the block without shuffling them. */
    let start = (blocksize - neblock_rem) as usize;
    dest[start..start + neblock_rem as usize]
        .copy_from_slice(&src[start..start + neblock_rem as usize]);
}

/**
  Generic (non-hardware-accelerated) unshuffle routine.
  This is the pure element-copying nested loop. It is used by the
  generic unshuffle implementation and also by the vectorized unshuffle
  implementations to process any remaining elements in a block which
  is not a multiple of (type_size * vector_size).
*/
#[inline]
fn unshuffle_generic_inline(
    type_size: i32,
    vectorizable_blocksize: i32,
    blocksize: i32,
    src: &[u8],
    dest: &mut [u8],
) {
    /* Calculate the number of elements in the block. */
    let neblock_quot = blocksize / type_size;
    let neblock_rem = blocksize % type_size;
    let vectorizable_elements = vectorizable_blocksize / type_size;

    /* Non-optimized unshuffle */
    for i in vectorizable_elements..neblock_quot {
        for j in 0..type_size {
            dest[(i * type_size + j) as usize] = src[(j * neblock_quot + i) as usize];
        }
    }

    /* Copy any leftover bytes in the block without unshuffling them. */
    let start = (blocksize - neblock_rem) as usize;
    dest[start..start + neblock_rem as usize]
        .copy_from_slice(&src[start..start + neblock_rem as usize]);
}

/**
  Generic (non-hardware-accelerated) shuffle routine.
*/
/* Shuffle a block.  This can never fail. */
pub fn shuffle_generic(bytesoftype: i32, blocksize: i32, src: &[u8], dest: &mut [u8]) {
    /* Non-optimized shuffle */
    shuffle_generic_inline(bytesoftype, 0, blocksize, src, dest);
}

/**
  Generic (non-hardware-accelerated) unshuffle routine.
*/
/* Unshuffle a block.  This can never fail. */
pub fn unshuffle_generic(bytesoftype: i32, blocksize: i32, src: &[u8], dest: &mut [u8]) {
    /* Non-optimized unshuffle */
    unshuffle_generic_inline(bytesoftype, 0, blocksize, src, dest);
}

