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
    let ldb = nbyte_bitrow;
    
    for ii in 0..lda {
        for jj in 0..ldb {
            let src_idx = (ii * ldb + jj) * elem_size;
            let dst_idx = (jj * lda + ii) * elem_size;
            out_buf[dst_idx..dst_idx+elem_size].copy_from_slice(&in_buf[src_idx..src_idx+elem_size]);
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

fn bshuf_trans_byte_bitrow_scal(in_buf: &[u8], out_buf: &mut [u8], size: usize, elem_size: usize) {
    let nbyte_row = size / 8;
    for jj in 0..elem_size {
        for ii in 0..nbyte_row {
            for kk in 0..8 {
                out_buf[ii * 8 * elem_size + jj * 8 + kk] = in_buf[(jj * 8 + kk) * nbyte_row + ii];
            }
        }
    }
}

fn bshuf_shuffle_bit_eightelem_scal(in_buf: &[u8], out_buf: &mut [u8], size: usize, elem_size: usize) {
    let nbyte = elem_size * size;
    let elem_skip = elem_size;
    let elem_offset = 0;

    // We need to iterate over bytes but read as u64.
    // The C code does: x = *((uint64_t*) &in_b[ii + jj]);
    // ii steps by 8*elem_size, jj steps by 8.
    
    // Let's rewrite loops to match C structure more closely but safely if possible.
    // Or just use unsafe pointer arithmetic if needed, but slices are better.
    
    for jj in (0..8*elem_size).step_by(8) {
        let mut ii = 0;
        while ii + 8 * elem_size - 1 < nbyte {
             let idx = ii + jj;
             // Read u64 from in_buf at idx
             let bytes = [in_buf[idx], in_buf[idx+1], in_buf[idx+2], in_buf[idx+3],
                          in_buf[idx+4], in_buf[idx+5], in_buf[idx+6], in_buf[idx+7]];
             let mut x = u64::from_le_bytes(bytes);
             
             x = trans_bit_8x8(x);
             
             for kk in 0..8 {
                 let out_index = ii + jj / 8 + elem_offset + kk * elem_skip;
                 out_buf[out_index] = x as u8;
                 x >>= 8;
             }
             
             ii += 8 * elem_size;
        }
    }
}

pub fn bitunshuffle(bytesoftype: usize, blocksize: usize, src: &[u8], dest: &mut [u8]) -> Result<(), i32> {
    let size = blocksize / bytesoftype;
    if size % 8 != 0 { return Err(-80); }

    let mut tmp_buf = vec![0u8; blocksize];

    bshuf_trans_byte_bitrow_scal(src, &mut tmp_buf, size, bytesoftype);
    bshuf_shuffle_bit_eightelem_scal(&tmp_buf, dest, size, bytesoftype);

    Ok(())
}
