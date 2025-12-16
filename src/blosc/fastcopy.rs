// Corresponds to c-blosc2/blosc/fastcopy.c (and .h)

/// Copy 1 byte from `from` to `out`. Returns the next position in `out`.
#[inline(always)]
fn copy_1_bytes(out: &mut [u8], from: &[u8]) -> usize {
    out[0] = from[0];
    1
}

/// Copy 2 bytes from `from` to `out`. Returns the next position in `out`.
#[inline(always)]
fn copy_2_bytes(out: &mut [u8], from: &[u8]) -> usize {
    out[0..2].copy_from_slice(&from[0..2]);
    2
}

/// Copy 3 bytes from `from` to `out`. Returns the next position in `out`.
#[inline(always)]
fn copy_3_bytes(out: &mut [u8], from: &[u8]) -> usize {
    let pos = copy_1_bytes(out, from);
    pos + copy_2_bytes(&mut out[pos..], &from[1..])
}

/// Copy 4 bytes from `from` to `out`. Returns the next position in `out`.
#[inline(always)]
fn copy_4_bytes(out: &mut [u8], from: &[u8]) -> usize {
    out[0..4].copy_from_slice(&from[0..4]);
    4
}

/// Copy 5 bytes from `from` to `out`. Returns the next position in `out`.
#[inline(always)]
fn copy_5_bytes(out: &mut [u8], from: &[u8]) -> usize {
    let pos = copy_1_bytes(out, from);
    pos + copy_4_bytes(&mut out[pos..], &from[1..])
}

/// Copy 6 bytes from `from` to `out`. Returns the next position in `out`.
#[inline(always)]
fn copy_6_bytes(out: &mut [u8], from: &[u8]) -> usize {
    let pos = copy_2_bytes(out, from);
    pos + copy_4_bytes(&mut out[pos..], &from[2..])
}

/// Copy 7 bytes from `from` to `out`. Returns the next position in `out`.
#[inline(always)]
fn copy_7_bytes(out: &mut [u8], from: &[u8]) -> usize {
    let pos = copy_3_bytes(out, from);
    pos + copy_4_bytes(&mut out[pos..], &from[3..])
}

/// Copy 8 bytes from `from` to `out`. Returns the next position in `out`.
#[inline(always)]
fn copy_8_bytes(out: &mut [u8], from: &[u8]) -> usize {
    out[0..8].copy_from_slice(&from[0..8]);
    8
}

/// Copy 16 bytes from `from` to `out`. Returns the next position in `out`.
#[inline(always)]
fn copy_16_bytes(out: &mut [u8], from: &[u8]) -> usize {
    // Pure Rust implementation without SIMD
    out[0..8].copy_from_slice(&from[0..8]);
    out[8..16].copy_from_slice(&from[8..16]);
    16
}

/// Copy 32 bytes from `from` to `out`. Returns the next position in `out`.
#[inline(always)]
fn copy_32_bytes(out: &mut [u8], from: &[u8]) -> usize {
    // Pure Rust implementation without SIMD
    out[0..8].copy_from_slice(&from[0..8]);
    out[8..16].copy_from_slice(&from[8..16]);
    out[16..24].copy_from_slice(&from[16..24]);
    out[24..32].copy_from_slice(&from[24..32]);
    32
}

/// Copy LEN bytes (7 or fewer) from FROM into OUT. Return the number of bytes copied.
#[inline(always)]
fn copy_bytes(out: &mut [u8], from: &[u8], len: usize) -> usize {
    debug_assert!(len < 8);
    
    match len {
        7 => copy_7_bytes(out, from),
        6 => copy_6_bytes(out, from),
        5 => copy_5_bytes(out, from),
        4 => copy_4_bytes(out, from),
        3 => copy_3_bytes(out, from),
        2 => copy_2_bytes(out, from),
        1 => copy_1_bytes(out, from),
        0 => 0,
        _ => panic!("len must be less than 8"),
    }
}

/// Byte by byte semantics: copy LEN bytes from FROM and write them to OUT.
/// Return the number of bytes copied.
#[inline]
fn chunk_memcpy(out: &mut [u8], from: &[u8], len: usize) -> usize {
    let sz = 8; // sizeof(uint64_t)
    let rem = len % sz;
    
    debug_assert!(len >= sz);
    
    // Copy a few bytes to make sure the loop below has a multiple of SZ bytes to be copied
    copy_8_bytes(out, from);
    
    let mut len_div = len / sz;
    let mut out_pos = rem;
    let mut from_pos = rem;
    
    let by8 = len_div % 8;
    len_div -= by8;
    
    // Handle the remainder with a switch statement (unrolled loop)
    for _ in 0..by8 {
        let copied = copy_8_bytes(&mut out[out_pos..], &from[from_pos..]);
        out_pos += copied;
        from_pos += sz;
    }
    
    // Main loop: copy 8 chunks of 8 bytes each iteration
    while len_div > 0 {
        for _ in 0..8 {
            let copied = copy_8_bytes(&mut out[out_pos..], &from[from_pos..]);
            out_pos += copied;
            from_pos += sz;
        }
        len_div -= 8;
    }
    
    out_pos
}

/// Byte by byte semantics: copy LEN bytes from FROM and write them to OUT.
/// Return the number of bytes copied.
/// This is the main public function with the same semantics as memcpy().
pub fn fastcopy(out: &mut [u8], from: &[u8], len: usize) -> usize {
    match len {
        32 => copy_32_bytes(out, from),
        16 => copy_16_bytes(out, from),
        8 => copy_8_bytes(out, from),
        _ => {
            if len < 8 {
                copy_bytes(out, from, len)
            } else {
                chunk_memcpy(out, from, len)
            }
        }
    }
}

/// Copy a run. Same as fastcopy() but without overwriting origin or destination when they overlap.
pub fn copy_match(out: &mut [u8], from: &[u8], len: usize) -> usize {
    let sz = 8; // sizeof(uint64_t) - no SIMD
    
    // Calculate the overlap distance
    // In Rust, we handle this differently since we have slices
    // We need to check if the slices might overlap
    let out_ptr = out.as_ptr() as usize;
    let from_ptr = from.as_ptr() as usize;
    
    let overlap_dist = if out_ptr > from_ptr {
        out_ptr - from_ptr
    } else {
        // If from is ahead of out, use fastcopy (safe)
        return fastcopy(out, from, len);
    };
    
    // If out and from are away more than the size of the copy, then a fastcopy is safe
    if overlap_dist > sz {
        return fastcopy(out, from, len);
    }
    
    // Otherwise we need to be more careful so as not to overwrite destination
    let mut out_pos = 0;
    let mut from_pos = 0;
    let mut remaining = len;
    
    match overlap_dist {
        32 => {
            while remaining >= 32 {
                let copied = copy_32_bytes(&mut out[out_pos..], &from[from_pos..]);
                out_pos += copied;
                from_pos += 32;
                remaining -= 32;
            }
        }
        30 => {
            while remaining >= 30 {
                let mut pos = copy_16_bytes(&mut out[out_pos..], &from[from_pos..]);
                pos += copy_8_bytes(&mut out[out_pos + pos..], &from[from_pos + 16..]);
                pos += copy_4_bytes(&mut out[out_pos + pos..], &from[from_pos + 24..]);
                pos += copy_2_bytes(&mut out[out_pos + pos..], &from[from_pos + 28..]);
                out_pos += pos;
                from_pos += 30;
                remaining -= 30;
            }
        }
        28 => {
            while remaining >= 28 {
                let mut pos = copy_16_bytes(&mut out[out_pos..], &from[from_pos..]);
                pos += copy_8_bytes(&mut out[out_pos + pos..], &from[from_pos + 16..]);
                pos += copy_4_bytes(&mut out[out_pos + pos..], &from[from_pos + 24..]);
                out_pos += pos;
                from_pos += 28;
                remaining -= 28;
            }
        }
        26 => {
            while remaining >= 26 {
                let mut pos = copy_16_bytes(&mut out[out_pos..], &from[from_pos..]);
                pos += copy_8_bytes(&mut out[out_pos + pos..], &from[from_pos + 16..]);
                pos += copy_2_bytes(&mut out[out_pos + pos..], &from[from_pos + 24..]);
                out_pos += pos;
                from_pos += 26;
                remaining -= 26;
            }
        }
        24 => {
            while remaining >= 24 {
                let mut pos = copy_16_bytes(&mut out[out_pos..], &from[from_pos..]);
                pos += copy_8_bytes(&mut out[out_pos + pos..], &from[from_pos + 16..]);
                out_pos += pos;
                from_pos += 24;
                remaining -= 24;
            }
        }
        22 => {
            while remaining >= 22 {
                let mut pos = copy_16_bytes(&mut out[out_pos..], &from[from_pos..]);
                pos += copy_4_bytes(&mut out[out_pos + pos..], &from[from_pos + 16..]);
                pos += copy_2_bytes(&mut out[out_pos + pos..], &from[from_pos + 20..]);
                out_pos += pos;
                from_pos += 22;
                remaining -= 22;
            }
        }
        20 => {
            while remaining >= 20 {
                let mut pos = copy_16_bytes(&mut out[out_pos..], &from[from_pos..]);
                pos += copy_4_bytes(&mut out[out_pos + pos..], &from[from_pos + 16..]);
                out_pos += pos;
                from_pos += 20;
                remaining -= 20;
            }
        }
        18 => {
            while remaining >= 18 {
                let mut pos = copy_16_bytes(&mut out[out_pos..], &from[from_pos..]);
                pos += copy_2_bytes(&mut out[out_pos + pos..], &from[from_pos + 16..]);
                out_pos += pos;
                from_pos += 18;
                remaining -= 18;
            }
        }
        16 => {
            while remaining >= 16 {
                let copied = copy_16_bytes(&mut out[out_pos..], &from[from_pos..]);
                out_pos += copied;
                from_pos += 16;
                remaining -= 16;
            }
        }
        8 => {
            while remaining >= 8 {
                let copied = copy_8_bytes(&mut out[out_pos..], &from[from_pos..]);
                out_pos += copied;
                from_pos += 8;
                remaining -= 8;
            }
        }
        4 => {
            while remaining >= 4 {
                let copied = copy_4_bytes(&mut out[out_pos..], &from[from_pos..]);
                out_pos += copied;
                from_pos += 4;
                remaining -= 4;
            }
        }
        2 => {
            while remaining >= 2 {
                let copied = copy_2_bytes(&mut out[out_pos..], &from[from_pos..]);
                out_pos += copied;
                from_pos += 2;
                remaining -= 2;
            }
        }
        _ => {
            while remaining > 0 {
                out[out_pos] = from[from_pos];
                out_pos += 1;
                from_pos += 1;
                remaining -= 1;
            }
        }
    }
    
    // Copy the leftovers
    while remaining > 0 {
        out[out_pos] = from[from_pos];
        out_pos += 1;
        from_pos += 1;
        remaining -= 1;
    }
    
    out_pos
}
