pub fn shuffle(bytesoftype: usize, blocksize: usize, src: &[u8], dest: &mut [u8]) {
    let neblock_quot = blocksize / bytesoftype;
    let neblock_rem = blocksize % bytesoftype;

    for j in 0..bytesoftype {
        for i in 0..neblock_quot {
            dest[j * neblock_quot + i] = src[i * bytesoftype + j];
        }
    }

    if neblock_rem > 0 {
        let start = blocksize - neblock_rem;
        dest[start..blocksize].copy_from_slice(&src[start..blocksize]);
    }
}

pub fn unshuffle(bytesoftype: usize, blocksize: usize, src: &[u8], dest: &mut [u8]) {
    let neblock_quot = blocksize / bytesoftype;
    let neblock_rem = blocksize % bytesoftype;

    for i in 0..neblock_quot {
        for j in 0..bytesoftype {
            dest[i * bytesoftype + j] = src[j * neblock_quot + i];
        }
    }

    if neblock_rem > 0 {
        let start = blocksize - neblock_rem;
        dest[start..blocksize].copy_from_slice(&src[start..blocksize]);
    }
}

fn trans_bit_8x8(mut x: u64) -> u64 {
    let mut t;
    t = (x ^ (x >> 7)) & 0x00AA00AA00AA00AA;
    x = x ^ t ^ (t << 7);
    t = (x ^ (x >> 14)) & 0x0000CCCC0000CCCC;
    x = x ^ t ^ (t << 14);
    t = (x ^ (x >> 28)) & 0x00000000F0F0F0F0;
    x = x ^ t ^ (t << 28);
    x
}

fn bshuf_trans_byte_elem_scal(in_buf: &[u8], out_buf: &mut [u8], size: usize, elem_size: usize) {
    for ii in (0..size).step_by(8) {
        if ii + 7 < size {
            for jj in 0..elem_size {
                for kk in 0..8 {
                    out_buf[jj * size + ii + kk] = in_buf[(ii + kk) * elem_size + jj];
                }
            }
        } else {
             for k in ii..size {
                for jj in 0..elem_size {
                    out_buf[jj * size + k] = in_buf[k * elem_size + jj];
                }
             }
        }
    }
}

fn bshuf_trans_bit_byte_scal(in_buf: &[u8], out_buf: &mut [u8], size: usize, elem_size: usize) {
    let nbyte = elem_size * size;
    let nbyte_bitrow = nbyte / 8;
    
    // Assuming little endian
    let bit_row_skip = nbyte_bitrow;
    let bit_row_offset = 0;

    let in_u64 = unsafe { std::slice::from_raw_parts(in_buf.as_ptr() as *const u64, nbyte / 8) };
    
    for ii in 0..nbyte_bitrow {
        let mut x = in_u64[ii];
        x = trans_bit_8x8(x);
        for kk in 0..8 {
            out_buf[bit_row_offset + kk * bit_row_skip + ii] = x as u8;
            x >>= 8;
        }
    }
}

fn bshuf_trans_bitrow_eight(in_buf: &[u8], out_buf: &mut [u8], size: usize, elem_size: usize) {
    let nbyte_bitrow = size / 8;
    let lda = 8;
    let ldb = elem_size;
    let block_size = nbyte_bitrow;
    
    for ii in 0..lda {
        for jj in 0..ldb {
            let src_idx = (ii * ldb + jj) * block_size;
            let dst_idx = (jj * lda + ii) * block_size;
            out_buf[dst_idx..dst_idx+block_size].copy_from_slice(&in_buf[src_idx..src_idx+block_size]);
        }
    }
}

pub fn bitshuffle(bytesoftype: usize, blocksize: usize, src: &[u8], dest: &mut [u8]) -> Result<(), i32> {
    let size = blocksize / bytesoftype;
    if size % 8 != 0 { return Err(-80); }

    let mut tmp_buf = vec![0u8; blocksize];
    
    bshuf_trans_byte_elem_scal(src, dest, size, bytesoftype);
    bshuf_trans_bit_byte_scal(dest, &mut tmp_buf, size, bytesoftype);
    bshuf_trans_bitrow_eight(&tmp_buf, dest, size, bytesoftype);
    
    Ok(())
}

fn bshuf_untrans_bitrow_eight(in_buf: &[u8], out_buf: &mut [u8], size: usize, elem_size: usize) {
    let nbyte_bitrow = size / 8;
    let lda = elem_size;
    let ldb = 8;
    let block_size = nbyte_bitrow;
    
    for ii in 0..lda {
        for jj in 0..ldb {
            let src_idx = (ii * ldb + jj) * block_size;
            let dst_idx = (jj * lda + ii) * block_size;
            out_buf[dst_idx..dst_idx+block_size].copy_from_slice(&in_buf[src_idx..src_idx+block_size]);
        }
    }
}

fn bshuf_untrans_bit_byte_scal(in_buf: &[u8], out_buf: &mut [u8], size: usize, elem_size: usize) {
    let nbyte = elem_size * size;
    let nbyte_bitrow = nbyte / 8;
    let bit_row_skip = nbyte_bitrow;

    let out_u64 = unsafe { std::slice::from_raw_parts_mut(out_buf.as_mut_ptr() as *mut u64, nbyte / 8) };

    for ii in 0..nbyte_bitrow {
        let mut x: u64 = 0;
        for kk in 0..8 {
            let val = in_buf[kk * bit_row_skip + ii];
            x |= (val as u64) << (kk * 8);
        }
        x = trans_bit_8x8(x);
        out_u64[ii] = x;
    }
}

pub fn bitunshuffle(bytesoftype: usize, blocksize: usize, src: &[u8], dest: &mut [u8]) -> Result<(), i32> {
    let size = blocksize / bytesoftype;
    if size % 8 != 0 { return Err(-80); }

    let mut tmp_buf = vec![0u8; blocksize];
    let mut tmp_buf2 = vec![0u8; blocksize];

    // 1. Reverse Step 3: Untranspose bitrows
    bshuf_untrans_bitrow_eight(src, &mut tmp_buf, size, bytesoftype);

    // 2. Reverse Step 2: Untranspose bits
    bshuf_untrans_bit_byte_scal(&tmp_buf, &mut tmp_buf2, size, bytesoftype);

    // 3. Reverse Step 1: Untranspose bytes/elements
    // Note: we swap size and bytesoftype to reverse the transpose
    bshuf_trans_byte_elem_scal(&tmp_buf2, dest, bytesoftype, size);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitshuffle_roundtrip() {
        let size = 128;
        let typesize = 4;
        let blocksize = size * typesize;
        let mut src = vec![0u8; blocksize];
        for i in 0..blocksize {
            src[i] = (i % 255) as u8;
        }
        let mut dest = vec![0u8; blocksize];
        let mut recovered = vec![0u8; blocksize];

        bitshuffle(typesize, blocksize, &src, &mut dest).unwrap();
        bitunshuffle(typesize, blocksize, &dest, &mut recovered).unwrap();

        assert_eq!(src, recovered);
    }
}
