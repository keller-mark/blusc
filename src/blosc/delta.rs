// Corresponds to c-blosc2/blosc/delta.c (and .h)

/// Apply the delta filter to src.
///
/// This function applies delta coding to the source data:
/// - If offset == 0 (reference block): delta codes elements relative to previous elements
/// - If offset != 0: delta codes elements relative to the reference block
///
/// # Arguments
/// * `dref` - Reference data
/// * `offset` - Offset value (0 for reference block, non-zero otherwise)
/// * `nbytes` - Number of bytes to process
/// * `typesize` - Size of each element type in bytes
/// * `src` - Source data
/// * `dest` - Destination buffer for encoded data
pub fn delta_encoder(
    dref: &[u8],
    offset: i32,
    nbytes: i32,
    typesize: i32,
    src: &[u8],
    dest: &mut [u8],
) {
    let nbytes = nbytes as usize;
    let typesize = typesize as usize;

    if offset == 0 {
        // This is the reference block, use delta coding in elements
        match typesize {
            1 => {
                dest[0] = dref[0];
                for i in 1..nbytes {
                    dest[i] = src[i] ^ dref[i - 1];
                }
            }
            2 => {
                // Cast slices to u16
                let dref_u16 = unsafe {
                    std::slice::from_raw_parts(dref.as_ptr() as *const u16, nbytes / 2)
                };
                let src_u16 = unsafe {
                    std::slice::from_raw_parts(src.as_ptr() as *const u16, nbytes / 2)
                };
                let dest_u16 = unsafe {
                    std::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut u16, nbytes / 2)
                };
                dest_u16[0] = dref_u16[0];
                for i in 1..(nbytes / 2) {
                    dest_u16[i] = src_u16[i] ^ dref_u16[i - 1];
                }
            }
            4 => {
                // Cast slices to u32
                let dref_u32 = unsafe {
                    std::slice::from_raw_parts(dref.as_ptr() as *const u32, nbytes / 4)
                };
                let src_u32 = unsafe {
                    std::slice::from_raw_parts(src.as_ptr() as *const u32, nbytes / 4)
                };
                let dest_u32 = unsafe {
                    std::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut u32, nbytes / 4)
                };
                dest_u32[0] = dref_u32[0];
                for i in 1..(nbytes / 4) {
                    dest_u32[i] = src_u32[i] ^ dref_u32[i - 1];
                }
            }
            8 => {
                // Cast slices to u64
                let dref_u64 = unsafe {
                    std::slice::from_raw_parts(dref.as_ptr() as *const u64, nbytes / 8)
                };
                let src_u64 = unsafe {
                    std::slice::from_raw_parts(src.as_ptr() as *const u64, nbytes / 8)
                };
                let dest_u64 = unsafe {
                    std::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut u64, nbytes / 8)
                };
                dest_u64[0] = dref_u64[0];
                for i in 1..(nbytes / 8) {
                    dest_u64[i] = src_u64[i] ^ dref_u64[i - 1];
                }
            }
            _ => {
                // For other sizes, recurse with appropriate typesize
                if (typesize % 8) == 0 {
                    delta_encoder(dref, offset, nbytes as i32, 8, src, dest);
                } else {
                    delta_encoder(dref, offset, nbytes as i32, 1, src, dest);
                }
            }
        }
    } else {
        // Use delta coding wrt reference block
        match typesize {
            1 => {
                for i in 0..nbytes {
                    dest[i] = src[i] ^ dref[i];
                }
            }
            2 => {
                // Cast slices to u16
                let dref_u16 = unsafe {
                    std::slice::from_raw_parts(dref.as_ptr() as *const u16, nbytes / 2)
                };
                let src_u16 = unsafe {
                    std::slice::from_raw_parts(src.as_ptr() as *const u16, nbytes / 2)
                };
                let dest_u16 = unsafe {
                    std::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut u16, nbytes / 2)
                };
                for i in 0..(nbytes / 2) {
                    dest_u16[i] = src_u16[i] ^ dref_u16[i];
                }
            }
            4 => {
                // Cast slices to u32
                let dref_u32 = unsafe {
                    std::slice::from_raw_parts(dref.as_ptr() as *const u32, nbytes / 4)
                };
                let src_u32 = unsafe {
                    std::slice::from_raw_parts(src.as_ptr() as *const u32, nbytes / 4)
                };
                let dest_u32 = unsafe {
                    std::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut u32, nbytes / 4)
                };
                for i in 0..(nbytes / 4) {
                    dest_u32[i] = src_u32[i] ^ dref_u32[i];
                }
            }
            8 => {
                // Cast slices to u64
                let dref_u64 = unsafe {
                    std::slice::from_raw_parts(dref.as_ptr() as *const u64, nbytes / 8)
                };
                let src_u64 = unsafe {
                    std::slice::from_raw_parts(src.as_ptr() as *const u64, nbytes / 8)
                };
                let dest_u64 = unsafe {
                    std::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut u64, nbytes / 8)
                };
                for i in 0..(nbytes / 8) {
                    dest_u64[i] = src_u64[i] ^ dref_u64[i];
                }
            }
            _ => {
                // For other sizes, recurse with appropriate typesize
                if (typesize % 8) == 0 {
                    delta_encoder(dref, offset, nbytes as i32, 8, src, dest);
                } else {
                    delta_encoder(dref, offset, nbytes as i32, 1, src, dest);
                }
            }
        }
    }
}

/// Undo the delta filter in dest.
///
/// This function decodes delta-coded data:
/// - If offset == 0 (reference block): decodes elements relative to previous elements
/// - If offset != 0: decodes elements relative to the reference block
///
/// # Arguments
/// * `dref` - Reference data
/// * `offset` - Offset value (0 for reference block, non-zero otherwise)
/// * `nbytes` - Number of bytes to process
/// * `typesize` - Size of each element type in bytes
/// * `dest` - Buffer containing encoded data, will be decoded in-place
pub fn delta_decoder(dref: &[u8], offset: i32, nbytes: i32, typesize: i32, dest: &mut [u8]) {
    let nbytes = nbytes as usize;
    let typesize = typesize as usize;

    if offset == 0 {
        // Decode delta for the reference block
        match typesize {
            1 => {
                for i in 1..nbytes {
                    dest[i] ^= dref[i - 1];
                }
            }
            2 => {
                // Cast slices to u16
                let dref_u16 = unsafe {
                    std::slice::from_raw_parts(dref.as_ptr() as *const u16, nbytes / 2)
                };
                let dest_u16 = unsafe {
                    std::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut u16, nbytes / 2)
                };
                for i in 1..(nbytes / 2) {
                    dest_u16[i] ^= dref_u16[i - 1];
                }
            }
            4 => {
                // Cast slices to u32
                let dref_u32 = unsafe {
                    std::slice::from_raw_parts(dref.as_ptr() as *const u32, nbytes / 4)
                };
                let dest_u32 = unsafe {
                    std::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut u32, nbytes / 4)
                };
                for i in 1..(nbytes / 4) {
                    dest_u32[i] ^= dref_u32[i - 1];
                }
            }
            8 => {
                // Cast slices to u64
                let dref_u64 = unsafe {
                    std::slice::from_raw_parts(dref.as_ptr() as *const u64, nbytes / 8)
                };
                let dest_u64 = unsafe {
                    std::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut u64, nbytes / 8)
                };
                for i in 1..(nbytes / 8) {
                    dest_u64[i] ^= dref_u64[i - 1];
                }
            }
            _ => {
                // For other sizes, recurse with appropriate typesize
                if (typesize % 8) == 0 {
                    delta_decoder(dref, offset, nbytes as i32, 8, dest);
                } else {
                    delta_decoder(dref, offset, nbytes as i32, 1, dest);
                }
            }
        }
    } else {
        // Decode delta for the non-reference blocks
        match typesize {
            1 => {
                for i in 0..nbytes {
                    dest[i] ^= dref[i];
                }
            }
            2 => {
                // Cast slices to u16
                let dref_u16 = unsafe {
                    std::slice::from_raw_parts(dref.as_ptr() as *const u16, nbytes / 2)
                };
                let dest_u16 = unsafe {
                    std::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut u16, nbytes / 2)
                };
                for i in 0..(nbytes / 2) {
                    dest_u16[i] ^= dref_u16[i];
                }
            }
            4 => {
                // Cast slices to u32
                let dref_u32 = unsafe {
                    std::slice::from_raw_parts(dref.as_ptr() as *const u32, nbytes / 4)
                };
                let dest_u32 = unsafe {
                    std::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut u32, nbytes / 4)
                };
                for i in 0..(nbytes / 4) {
                    dest_u32[i] ^= dref_u32[i];
                }
            }
            8 => {
                // Cast slices to u64
                let dref_u64 = unsafe {
                    std::slice::from_raw_parts(dref.as_ptr() as *const u64, nbytes / 8)
                };
                let dest_u64 = unsafe {
                    std::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut u64, nbytes / 8)
                };
                for i in 0..(nbytes / 8) {
                    dest_u64[i] ^= dref_u64[i];
                }
            }
            _ => {
                // For other sizes, recurse with appropriate typesize
                if (typesize % 8) == 0 {
                    delta_decoder(dref, offset, nbytes as i32, 8, dest);
                } else {
                    delta_decoder(dref, offset, nbytes as i32, 1, dest);
                }
            }
        }
    }
}

/* 

void delta_encoder(const uint8_t* dref, int32_t offset, int32_t nbytes,
                   int32_t typesize, const uint8_t* src, uint8_t* dest);

void delta_decoder(const uint8_t* dref, int32_t offset, int32_t nbytes,
                   int32_t typesize, uint8_t* dest);

/* Apply the delta filters to src.  This can never fail. */
void delta_encoder(const uint8_t* dref, int32_t offset, int32_t nbytes, int32_t typesize,
                   const uint8_t* src, uint8_t* dest) {
  int32_t i;
  if (offset == 0) {
    /* This is the reference block, use delta coding in elements */
    switch (typesize) {
      case 1:
        dest[0] = dref[0];
        for (i = 1; i < nbytes; i++) {
          dest[i] = src[i] ^ dref[i-1];
        }
        break;
      case 2:
        ((uint16_t *)dest)[0] = ((uint16_t *)dref)[0];
        for (i = 1; i < nbytes / 2; i++) {
          ((uint16_t *)dest)[i] =
                  ((uint16_t *)src)[i] ^ ((uint16_t *)dref)[i-1];
        }
        break;
      case 4:
        ((uint32_t *)dest)[0] = ((uint32_t *)dref)[0];
        for (i = 1; i < nbytes / 4; i++) {
          ((uint32_t *)dest)[i] =
                  ((uint32_t *)src)[i] ^ ((uint32_t *)dref)[i-1];
        }
        break;
      case 8:
        ((uint64_t *)dest)[0] = ((uint64_t *)dref)[0];
        for (i = 1; i < nbytes / 8; i++) {
          ((uint64_t *)dest)[i] =
                  ((uint64_t *)src)[i] ^ ((uint64_t *)dref)[i-1];
        }
        break;
      default:
        if ((typesize % 8) == 0) {
          delta_encoder(dref, offset, nbytes, 8, src, dest);
        } else {
          delta_encoder(dref, offset, nbytes, 1, src, dest);
        }
    }
  } else {
    /* Use delta coding wrt reference block */
    switch (typesize) {
      case 1:
        for (i = 0; i < nbytes; i++) {
          dest[i] = src[i] ^ dref[i];
        }
        break;
      case 2:
        for (i = 0; i < nbytes / 2; i++) {
          ((uint16_t *) dest)[i] =
                  ((uint16_t *) src)[i] ^ ((uint16_t *) dref)[i];
        }
        break;
      case 4:
        for (i = 0; i < nbytes / 4; i++) {
          ((uint32_t *) dest)[i] =
                  ((uint32_t *) src)[i] ^ ((uint32_t *) dref)[i];
        }
        break;
      case 8:
        for (i = 0; i < nbytes / 8; i++) {
          ((uint64_t *) dest)[i] =
                  ((uint64_t *) src)[i] ^ ((uint64_t *) dref)[i];
        }
        break;
      default:
        if ((typesize % 8) == 0) {
          delta_encoder(dref, offset, nbytes, 8, src, dest);
        } else {
          delta_encoder(dref, offset, nbytes, 1, src, dest);
        }
    }
  }
}


/* Undo the delta filter in dest.  This can never fail. */
void delta_decoder(const uint8_t* dref, int32_t offset, int32_t nbytes,
                   int32_t typesize, uint8_t* dest) {
  int32_t i;

  if (offset == 0) {
    /* Decode delta for the reference block */
    switch (typesize) {
      case 1:
        for (i = 1; i < nbytes; i++) {
          dest[i] ^= dref[i-1];
        }
        break;
      case 2:
        for (i = 1; i < nbytes / 2; i++) {
          ((uint16_t *)dest)[i] ^= ((uint16_t *)dref)[i-1];
        }
        break;
      case 4:
        for (i = 1; i < nbytes / 4; i++) {
          ((uint32_t *)dest)[i] ^= ((uint32_t *)dref)[i-1];
        }
        break;
      case 8:
        for (i = 1; i < nbytes / 8; i++) {
          ((uint64_t *)dest)[i] ^= ((uint64_t *)dref)[i-1];
        }
        break;
      default:
        if ((typesize % 8) == 0) {
          delta_decoder(dref, offset, nbytes, 8, dest);
        } else {
          delta_decoder(dref, offset, nbytes, 1, dest);
        }
    }
  } else {
    /* Decode delta for the non-reference blocks */
    switch (typesize) {
      case 1:
        for (i = 0; i < nbytes; i++) {
          dest[i] ^= dref[i];
        }
        break;
      case 2:
        for (i = 0; i < nbytes / 2; i++) {
          ((uint16_t *)dest)[i] ^= ((uint16_t *)dref)[i];
        }
        break;
      case 4:
        for (i = 0; i < nbytes / 4; i++) {
          ((uint32_t *)dest)[i] ^= ((uint32_t *)dref)[i];
        }
        break;
      case 8:
        for (i = 0; i < nbytes / 8; i++) {
          ((uint64_t *)dest)[i] ^= ((uint64_t *)dref)[i];
        }
        break;
      default:
        if ((typesize % 8) == 0) {
          delta_decoder(dref, offset, nbytes, 8, dest);
        } else {
          delta_decoder(dref, offset, nbytes, 1, dest);
        }
    }
  }
}

*/