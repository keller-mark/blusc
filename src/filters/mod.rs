/// Byte-wise shuffle: rearranges bytes so that the most-significant bytes of all
/// elements are grouped together, then the next bytes, and so on.
///
/// This improves compression ratios because bytes at the same significance level
/// tend to be similar. `bytesoftype` is the element size (e.g. 4 for `f32`).
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

/// Inverse of [`shuffle`]: restores the original element-interleaved byte order
/// from a significance-grouped layout.
pub fn unshuffle(bytesoftype: usize, blocksize: usize, src: &[u8], dest: &mut [u8]) {
    // C: shuffle.c `unshuffle` -> `unshuffle_generic` (shuffle-generic.c), with
    // SIMD variants in shuffle-sse2.c / shuffle-avx2.c / shuffle-neon.c.
    // Here we rely on portable, bounds-check-free code that LLVM can auto-vectorize
    // (including for wasm32 with `simd128` enabled), instead of `std::arch` intrinsics.
    let neblock_quot = blocksize / bytesoftype;
    let neblock_rem = blocksize % bytesoftype;

    unshuffle_range(
        bytesoftype,
        neblock_quot,
        0,
        neblock_quot,
        src,
        &mut dest[..neblock_quot * bytesoftype],
    );

    if neblock_rem > 0 {
        let start = blocksize - neblock_rem;
        dest[start..blocksize].copy_from_slice(&src[start..blocksize]);
    }
}

/// Unshuffles the elements `[start, start + count)` of a byte-shuffled buffer.
///
/// `src` holds `bytesoftype` rows of `stride` bytes each (row `j` contains byte `j`
/// of every element). Writes `count * bytesoftype` bytes to `dest`:
/// `dest[k * bytesoftype + j] = src[j * stride + start + k]`.
///
/// A full [`unshuffle`] is `start = 0, count = stride`. `getitem` uses a sub-range
/// to avoid unshuffling a whole block when only a few elements are requested.
pub(crate) fn unshuffle_range(
    bytesoftype: usize,
    stride: usize,
    start: usize,
    count: usize,
    src: &[u8],
    dest: &mut [u8],
) {
    if count == 0 {
        return;
    }
    let dest = &mut dest[..count * bytesoftype];
    match bytesoftype {
        0 => {}
        1 => dest.copy_from_slice(&src[start..start + count]),
        2 => unshuffle_range_typed::<2>(stride, start, count, src, dest),
        4 => unshuffle_range_typed::<4>(stride, start, count, src, dest),
        8 => unshuffle_range_typed::<8>(stride, start, count, src, dest),
        16 => unshuffle_range_typed::<16>(stride, start, count, src, dest),
        _ => {
            // Generic typesize: scatter each row into its strided positions.
            // Iterators avoid per-byte bounds checks.
            for j in 0..bytesoftype {
                let row = &src[j * stride + start..j * stride + start + count];
                for (d, &s) in dest[j..].iter_mut().step_by(bytesoftype).zip(row) {
                    *d = s;
                }
            }
        }
    }
}

/// Number of elements processed per tile in [`unshuffle_range_typed`].
const UNSHUFFLE_TILE: usize = 16;

/// Unshuffle specialized for a compile-time typesize `T`.
///
/// Works on tiles of `UNSHUFFLE_TILE` elements: loads `T` rows of 16 bytes into a
/// fixed-size array and writes them back transposed. All indices inside the tile are
/// compile-time bounded, so LLVM removes bounds checks and emits byte shuffles.
#[inline(always)]
fn unshuffle_range_typed<const T: usize>(
    stride: usize,
    start: usize,
    count: usize,
    src: &[u8],
    dest: &mut [u8],
) {
    let rows: [&[u8]; T] =
        std::array::from_fn(|j| &src[j * stride + start..j * stride + start + count]);

    let ntiles = count / UNSHUFFLE_TILE;
    let (dest_tiles, dest_tail) = dest.split_at_mut(ntiles * UNSHUFFLE_TILE * T);

    for (t, out) in dest_tiles
        .chunks_exact_mut(UNSHUFFLE_TILE * T)
        .enumerate()
    {
        let base = t * UNSHUFFLE_TILE;
        let mut tile = [[0u8; UNSHUFFLE_TILE]; T];
        for j in 0..T {
            tile[j].copy_from_slice(&rows[j][base..base + UNSHUFFLE_TILE]);
        }
        for (i, elem) in out.chunks_exact_mut(T).enumerate() {
            for j in 0..T {
                elem[j] = tile[j][i];
            }
        }
    }

    // Remaining elements that do not fill a tile.
    let base = ntiles * UNSHUFFLE_TILE;
    for (i, elem) in dest_tail.chunks_exact_mut(T).enumerate() {
        for j in 0..T {
            elem[j] = rows[j][base + i];
        }
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

    for ii in 0..nbyte_bitrow {
        let offset = ii * 8;
        let bytes = &in_buf[offset..offset + 8];
        let mut x = u64::from_ne_bytes(bytes.try_into().unwrap());

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
            out_buf[dst_idx..dst_idx + block_size]
                .copy_from_slice(&in_buf[src_idx..src_idx + block_size]);
        }
    }
}

/// Bit-wise shuffle: transposes individual bits across elements, grouping
/// corresponding bits of all elements together.
///
/// More aggressive than [`shuffle`] — works at the bit level rather than
/// byte level, which can yield better compression for data with low bit-entropy.
/// The element count is rounded down to a multiple of 8; leftover bytes are copied as-is.
pub fn bitshuffle(
    bytesoftype: usize,
    blocksize: usize,
    src: &[u8],
    dest: &mut [u8],
) -> Result<(), i32> {
    let size = blocksize / bytesoftype;
    // Round down to multiple of 8 (bitshuffle only supports multiples of 8)
    let aligned_size = size - (size % 8);

    if aligned_size > 0 {
        let aligned_bytes = aligned_size * bytesoftype;
        let mut tmp_buf = vec![0u8; aligned_bytes];

        bshuf_trans_byte_elem_scal(&src[..aligned_bytes], dest, aligned_size, bytesoftype);
        bshuf_trans_bit_byte_scal(
            &dest[..aligned_bytes],
            &mut tmp_buf,
            aligned_size,
            bytesoftype,
        );
        bshuf_trans_bitrow_eight(&tmp_buf, dest, aligned_size, bytesoftype);
    }

    // Copy leftover bytes that can't be bitshuffled
    let offset = aligned_size * bytesoftype;
    if offset < blocksize {
        dest[offset..blocksize].copy_from_slice(&src[offset..blocksize]);
    }

    Ok(())
}

/// Reverses bitshuffle steps 3 and 2 in a single pass (bitrow untranspose followed by
/// the 8x8 bit untranspose).
///
/// The forward `bshuf_trans_bitrow_eight` moves `nbyte_bitrow`-sized blocks from index
/// `ii * elem_size + jj` to `jj * 8 + ii`. Rather than materializing that permutation in
/// a temporary buffer, we read directly from the permuted location: for element byte `e`,
/// bit row `kk` and offset `r`, the input byte lives at `e * 8 * B + kk * B + r`
/// (with `B = size / 8`). The 8 bytes for each `r` are gathered into a `u64`, bit-transposed
/// and written as 8 contiguous output bytes, matching the former
/// `bshuf_untrans_bitrow_eight` + `bshuf_untrans_bit_byte_scal` pair.
fn bshuf_untrans_bit_byte_bitrow(
    in_buf: &[u8],
    out_buf: &mut [u8],
    size: usize,
    elem_size: usize,
) {
    let nbyte = elem_size * size;
    let nbyte_bitrow = size / 8;
    let in_buf = &in_buf[..nbyte];
    let out_buf = &mut out_buf[..nbyte];

    for (in_chunk, out_chunk) in in_buf
        .chunks_exact(8 * nbyte_bitrow)
        .zip(out_buf.chunks_exact_mut(8 * nbyte_bitrow))
    {
        let rows: [&[u8]; 8] =
            std::array::from_fn(|kk| &in_chunk[kk * nbyte_bitrow..(kk + 1) * nbyte_bitrow]);

        // Fast path: 8 offsets `r` at a time. Load 8 contiguous bytes from each of the
        // 8 rows (an 8x8 byte tile), so the per-`r` gathers become fixed-size array
        // accesses that LLVM can keep in registers.
        let nfast = nbyte_bitrow / 8 * 8;
        let (out_fast, out_tail) = out_chunk.split_at_mut(nfast * 8);
        for (g, out64) in out_fast.chunks_exact_mut(64).enumerate() {
            let r0 = g * 8;
            let tile: [[u8; 8]; 8] =
                std::array::from_fn(|kk| rows[kk][r0..r0 + 8].try_into().unwrap());
            for (i, out8) in out64.chunks_exact_mut(8).enumerate() {
                let x = u64::from_le_bytes(std::array::from_fn(|kk| tile[kk][i]));
                out8.copy_from_slice(&trans_bit_8x8(x).to_ne_bytes());
            }
        }
        for (i, out8) in out_tail.chunks_exact_mut(8).enumerate() {
            let r = nfast + i;
            let x = u64::from_le_bytes(std::array::from_fn(|kk| rows[kk][r]));
            out8.copy_from_slice(&trans_bit_8x8(x).to_ne_bytes());
        }
    }
}

/// Inverse of [`bitshuffle`]: restores the original byte layout from a
/// bit-transposed representation.
pub fn bitunshuffle(
    bytesoftype: usize,
    blocksize: usize,
    src: &[u8],
    dest: &mut [u8],
) -> Result<(), i32> {
    if bytesoftype == 0 {
        return Err(-1);
    }
    let size = blocksize / bytesoftype;
    // Round down to multiple of 8 (bitunshuffle only supports multiples of 8)
    let aligned_size = size - (size % 8);

    if aligned_size > 0 {
        let aligned_bytes = aligned_size * bytesoftype;
        let mut tmp_buf = vec![0u8; aligned_bytes];

        // 1+2. Reverse Steps 3 and 2: untranspose bitrows and bits (fused).
        bshuf_untrans_bit_byte_bitrow(src, &mut tmp_buf, aligned_size, bytesoftype);

        // 3. Reverse Step 1: untranspose bytes/elements. This is exactly a byte
        // unshuffle of `aligned_size` elements of `bytesoftype` bytes.
        unshuffle(bytesoftype, aligned_bytes, &tmp_buf, dest);
    }

    // Copy leftover bytes
    let offset = aligned_size * bytesoftype;
    if offset < blocksize {
        dest[offset..blocksize].copy_from_slice(&src[offset..blocksize]);
    }

    Ok(())
}

/// In-place variant of [`bitunshuffle`]: `buf` holds `blocksize` bitshuffled bytes on
/// entry and the restored bytes on return. `tmp_buf` is caller-provided scratch of at
/// least `blocksize` bytes, so the caller can reuse it across blocks.
///
/// The leftover bytes (past the last multiple of 8 elements) are stored unmodified by
/// bitshuffle, so they are already in place. This lets `blosc_d` decode a block
/// directly into its final destination and needs only one scratch buffer, like C
/// blosc2's `pipeline_backward` which passes a preallocated `tmp` to `bitunshuffle`.
pub(crate) fn bitunshuffle_inplace(
    bytesoftype: usize,
    blocksize: usize,
    buf: &mut [u8],
    tmp_buf: &mut [u8],
) -> Result<(), i32> {
    if bytesoftype == 0 {
        return Err(-1);
    }
    let size = blocksize / bytesoftype;
    let aligned_size = size - (size % 8);

    if aligned_size > 0 {
        let aligned_bytes = aligned_size * bytesoftype;
        bshuf_untrans_bit_byte_bitrow(buf, tmp_buf, aligned_size, bytesoftype);
        unshuffle(
            bytesoftype,
            aligned_bytes,
            &tmp_buf[..aligned_bytes],
            &mut buf[..aligned_bytes],
        );
    }

    Ok(())
}

/// Unshuffles only bytes `[local_start, local_end)` of a byte-shuffled block of
/// `blocksize` bytes, writing `local_end - local_start` bytes into `dest`.
///
/// Used by `getitem` so that a partially requested block is not fully unshuffled.
pub(crate) fn unshuffle_partial(
    bytesoftype: usize,
    blocksize: usize,
    src: &[u8],
    local_start: usize,
    local_end: usize,
    dest: &mut [u8],
) {
    let neblock_quot = blocksize / bytesoftype;
    let shuffled_end = neblock_quot * bytesoftype;

    // Part of the request inside the shuffled region.
    let mid = local_end.min(shuffled_end);
    if local_start < mid {
        let out = &mut dest[..mid - local_start];
        if local_start % bytesoftype == 0 && mid % bytesoftype == 0 {
            unshuffle_range(
                bytesoftype,
                neblock_quot,
                local_start / bytesoftype,
                (mid - local_start) / bytesoftype,
                src,
                out,
            );
        } else {
            // Range not aligned to elements: compute each byte's shuffled position.
            for (k, d) in out.iter_mut().enumerate() {
                let p = local_start + k;
                *d = src[(p % bytesoftype) * neblock_quot + p / bytesoftype];
            }
        }
    }

    // The trailing `blocksize % bytesoftype` bytes are stored unshuffled.
    if local_end > shuffled_end {
        let from = local_start.max(shuffled_end);
        dest[from - local_start..].copy_from_slice(&src[from..local_end]);
    }
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

    fn test_data(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i.wrapping_mul(31) ^ (i >> 3)) as u8).collect()
    }

    /// Straightforward reference implementation of unshuffle.
    fn unshuffle_naive(ts: usize, blocksize: usize, src: &[u8]) -> Vec<u8> {
        let quot = blocksize / ts;
        let mut out = src[..blocksize].to_vec();
        for i in 0..quot {
            for j in 0..ts {
                out[i * ts + j] = src[j * quot + i];
            }
        }
        out
    }

    #[test]
    fn test_unshuffle_matches_naive() {
        for ts in 1..=18 {
            for blocksize in [0, 1, 7, 15, 16, 17, 63, 64, 100, 255, 256, 1000, 4099] {
                let src = test_data(blocksize);
                let mut dest = vec![0u8; blocksize];
                unshuffle(ts, blocksize, &src, &mut dest);
                assert_eq!(dest, unshuffle_naive(ts, blocksize, &src), "ts={ts} bs={blocksize}");

                // Round trip with shuffle.
                let mut shuffled = vec![0u8; blocksize];
                shuffle(ts, blocksize, &dest, &mut shuffled);
                assert_eq!(shuffled, src, "ts={ts} bs={blocksize}");
            }
        }
    }

    #[test]
    fn test_unshuffle_partial_matches_full() {
        for ts in 1..=17 {
            for blocksize in [1, 9, 40, 129, 515] {
                let src = test_data(blocksize);
                let full = unshuffle_naive(ts, blocksize, &src);
                for ls in 0..blocksize {
                    for le in [ls + 1, (ls + ts).min(blocksize), blocksize] {
                        if le <= ls || le > blocksize {
                            continue;
                        }
                        let mut dest = vec![0u8; le - ls];
                        unshuffle_partial(ts, blocksize, &src, ls, le, &mut dest);
                        assert_eq!(dest, full[ls..le], "ts={ts} bs={blocksize} {ls}..{le}");
                    }
                }
            }
        }
    }

    #[test]
    fn test_bitunshuffle_roundtrip_many_sizes() {
        for ts in 1..=17 {
            for blocksize in [1, 8, 63, 64, 65, 200, 1024, 1031, 4096 + 13] {
                let src = test_data(blocksize);
                let mut shuffled = vec![0u8; blocksize];
                bitshuffle(ts, blocksize, &src, &mut shuffled).unwrap();

                let mut recovered = vec![0u8; blocksize];
                bitunshuffle(ts, blocksize, &shuffled, &mut recovered).unwrap();
                assert_eq!(recovered, src, "ts={ts} bs={blocksize}");

                let mut inplace = shuffled.clone();
                let mut tmp = vec![0u8; blocksize];
                bitunshuffle_inplace(ts, blocksize, &mut inplace, &mut tmp).unwrap();
                assert_eq!(inplace, src, "inplace ts={ts} bs={blocksize}");
            }
        }
    }
}
