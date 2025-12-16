// Corresponds to c-blosc2/blosc/bitshuffle-generic.c (and .h)

// Macros translated as inline functions

/// Check if n is a multiple of 8, return error -80 if not
#[inline]
fn check_mult_eight(n: usize) -> Result<(), i64> {
    if n % 8 != 0 {
        Err(-80)
    } else {
        Ok(())
    }
}

/// Transpose 8x8 bit array packed into a single quadword
/// This modifies x in place and uses t as workspace
#[inline]
fn trans_bit_8x8(x: &mut u64) {
    let mut t: u64;
    t = (*x ^ (*x >> 7)) & 0x00AA00AA00AA00AA;
    *x = *x ^ t ^ (t << 7);
    t = (*x ^ (*x >> 14)) & 0x0000CCCC0000CCCC;
    *x = *x ^ t ^ (t << 14);
    t = (*x ^ (*x >> 28)) & 0x00000000F0F0F0F0;
    *x = *x ^ t ^ (t << 28);
}

/// Transpose 8x8 bit array along the diagonal from upper right to lower left
#[inline]
fn trans_bit_8x8_be(x: &mut u64) {
    let mut t: u64;
    t = (*x ^ (*x >> 9)) & 0x0055005500550055;
    *x = *x ^ t ^ (t << 9);
    t = (*x ^ (*x >> 18)) & 0x0000333300003333;
    *x = *x ^ t ^ (t << 18);
    t = (*x ^ (*x >> 36)) & 0x000000000F0F0F0F;
    *x = *x ^ t ^ (t << 36);
}

/// Memory copy with bshuf call signature. For testing and profiling.
pub fn bshuf_copy(input: &[u8], output: &mut [u8], size: usize, elem_size: usize) -> i64 {
    let total_bytes = size * elem_size;
    output[..total_bytes].copy_from_slice(&input[..total_bytes]);
    total_bytes as i64
}

/// Transpose bytes within elements, starting partway through input.
pub fn bshuf_trans_byte_elem_remainder(
    input: &[u8],
    output: &mut [u8],
    size: usize,
    elem_size: usize,
    start: usize,
) -> i64 {
    if let Err(e) = check_mult_eight(start) {
        return e;
    }

    if size > start {
        // ii loop separated into 2 loops so the compiler can unroll the inner one.
        let mut ii = start;
        while ii + 7 < size {
            for jj in 0..elem_size {
                for kk in 0..8 {
                    output[jj * size + ii + kk] = input[ii * elem_size + kk * elem_size + jj];
                }
            }
            ii += 8;
        }
        
        for ii in (size - size % 8)..size {
            for jj in 0..elem_size {
                output[jj * size + ii] = input[ii * elem_size + jj];
            }
        }
    }
    
    (size * elem_size) as i64
}

/// Transpose bytes within elements.
pub fn bshuf_trans_byte_elem_scal(
    input: &[u8],
    output: &mut [u8],
    size: usize,
    elem_size: usize,
) -> i64 {
    bshuf_trans_byte_elem_remainder(input, output, size, elem_size, 0)
}

/// Transpose bits within bytes.
pub fn bshuf_trans_bit_byte_remainder(
    input: &[u8],
    output: &mut [u8],
    size: usize,
    elem_size: usize,
    start_byte: usize,
) -> i64 {
    let nbyte = elem_size * size;
    let nbyte_bitrow = nbyte / 8;

    if let Err(e) = check_mult_eight(nbyte) {
        return e;
    }
    if let Err(e) = check_mult_eight(start_byte) {
        return e;
    }

    // Check endianness
    let e: u64 = 1;
    let little_endian = e.to_ne_bytes()[0] == 1;
    
    let bit_row_skip = if little_endian {
        nbyte_bitrow as isize
    } else {
        -(nbyte_bitrow as isize)
    };
    
    let bit_row_offset = if little_endian {
        0isize
    } else {
        (7 * nbyte_bitrow) as isize
    };

    for ii in (start_byte / 8)..nbyte_bitrow {
        // Read as u64 from input
        let mut x = u64::from_ne_bytes([
            input[ii * 8],
            input[ii * 8 + 1],
            input[ii * 8 + 2],
            input[ii * 8 + 3],
            input[ii * 8 + 4],
            input[ii * 8 + 5],
            input[ii * 8 + 6],
            input[ii * 8 + 7],
        ]);
        
        if little_endian {
            trans_bit_8x8(&mut x);
        } else {
            trans_bit_8x8_be(&mut x);
        }
        
        for kk in 0..8 {
            let idx = (bit_row_offset + kk * bit_row_skip + ii as isize) as usize;
            output[idx] = (x & 0xFF) as u8;
            x >>= 8;
        }
    }
    
    (size * elem_size) as i64
}

/// Transpose bits within bytes.
pub fn bshuf_trans_bit_byte_scal(
    input: &[u8],
    output: &mut [u8],
    size: usize,
    elem_size: usize,
) -> i64 {
    bshuf_trans_bit_byte_remainder(input, output, size, elem_size, 0)
}

/// General transpose of an array, optimized for large element sizes.
pub fn bshuf_trans_elem(
    input: &[u8],
    output: &mut [u8],
    lda: usize,
    ldb: usize,
    elem_size: usize,
) -> i64 {
    for ii in 0..lda {
        for jj in 0..ldb {
            let out_start = (jj * lda + ii) * elem_size;
            let in_start = (ii * ldb + jj) * elem_size;
            output[out_start..out_start + elem_size]
                .copy_from_slice(&input[in_start..in_start + elem_size]);
        }
    }
    (lda * ldb * elem_size) as i64
}

/// Transpose rows of shuffled bits (size / 8 bytes) within groups of 8.
pub fn bshuf_trans_bitrow_eight(
    input: &[u8],
    output: &mut [u8],
    size: usize,
    elem_size: usize,
) -> i64 {
    if let Err(e) = check_mult_eight(size) {
        return e;
    }

    let nbyte_bitrow = size / 8;
    bshuf_trans_elem(input, output, 8, elem_size, nbyte_bitrow)
}

/// Transpose bits within elements.
pub fn bshuf_trans_bit_elem_scal(
    input: &[u8],
    output: &mut [u8],
    size: usize,
    elem_size: usize,
) -> i64 {
    if let Err(e) = check_mult_eight(size) {
        return e;
    }

    let total_bytes = size * elem_size;
    let mut tmp_buf = vec![0u8; total_bytes];

    let count = bshuf_trans_byte_elem_scal(input, output, size, elem_size);
    if count < 0 {
        return count;
    }
    
    let count = bshuf_trans_bit_byte_scal(output, &mut tmp_buf, size, elem_size);
    if count < 0 {
        return count;
    }
    
    let count = bshuf_trans_bitrow_eight(&tmp_buf, output, size, elem_size);
    count
}

/// For data organized into a row for each bit (8 * elem_size rows), transpose the bytes.
pub fn bshuf_trans_byte_bitrow_scal(
    input: &[u8],
    output: &mut [u8],
    size: usize,
    elem_size: usize,
) -> i64 {
    let nbyte_row = size / 8;

    if let Err(e) = check_mult_eight(size) {
        return e;
    }

    for jj in 0..elem_size {
        for ii in 0..nbyte_row {
            for kk in 0..8 {
                output[ii * 8 * elem_size + jj * 8 + kk] =
                    input[(jj * 8 + kk) * nbyte_row + ii];
            }
        }
    }
    
    (size * elem_size) as i64
}

/// Shuffle bits within the bytes of eight element blocks.
pub fn bshuf_shuffle_bit_eightelem_scal(
    input: &[u8],
    output: &mut [u8],
    size: usize,
    elem_size: usize,
) -> i64 {
    if let Err(e) = check_mult_eight(size) {
        return e;
    }

    let nbyte = elem_size * size;

    // Check endianness
    let e: u64 = 1;
    let little_endian = e.to_ne_bytes()[0] == 1;
    
    let elem_skip = if little_endian {
        elem_size as isize
    } else {
        -(elem_size as isize)
    };
    
    let elem_offset = if little_endian {
        0usize
    } else {
        7 * elem_size
    };

    let mut jj = 0;
    while jj < 8 * elem_size {
        let mut ii = 0;
        while ii + 8 * elem_size - 1 < nbyte {
            // Read u64 from input
            let mut x = u64::from_ne_bytes([
                input[ii + jj],
                input[ii + jj + 1],
                input[ii + jj + 2],
                input[ii + jj + 3],
                input[ii + jj + 4],
                input[ii + jj + 5],
                input[ii + jj + 6],
                input[ii + jj + 7],
            ]);
            
            if little_endian {
                trans_bit_8x8(&mut x);
            } else {
                trans_bit_8x8_be(&mut x);
            }
            
            for kk in 0..8 {
                let out_index = ((ii + jj / 8) as isize + elem_offset as isize + kk * elem_skip) as usize;
                output[out_index] = (x & 0xFF) as u8;
                x >>= 8;
            }
            
            ii += 8 * elem_size;
        }
        jj += 8;
    }
    
    (size * elem_size) as i64
}

/// Untranspose bits within elements.
pub fn bshuf_untrans_bit_elem_scal(
    input: &[u8],
    output: &mut [u8],
    size: usize,
    elem_size: usize,
) -> i64 {
    if let Err(e) = check_mult_eight(size) {
        return e;
    }

    let total_bytes = size * elem_size;
    let mut tmp_buf = vec![0u8; total_bytes];

    let count = bshuf_trans_byte_bitrow_scal(input, &mut tmp_buf, size, elem_size);
    if count < 0 {
        return count;
    }
    
    let count = bshuf_shuffle_bit_eightelem_scal(&tmp_buf, output, size, elem_size);
    count
}
