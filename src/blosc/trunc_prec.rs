// Corresponds to c-blosc2/blosc/trunc-prec.c (and .h)

// Constants for IEEE 754 floating point mantissa bits
const BITS_MANTISSA_FLOAT: i8 = 23;
const BITS_MANTISSA_DOUBLE: i8 = 52;

/// Truncate precision for 32-bit floats
/// 
/// prec_bits: Number of bits to keep in mantissa (positive) or reduce (negative)
/// nelems: Number of elements to process
/// src: Source array
/// dest: Destination array
/// 
/// Returns 0 on success, -1 on error
fn truncate_precision32(prec_bits: i8, nelems: i32, src: &[i32], dest: &mut [i32]) -> i32 {
    // Make sure that we don't remove all the bits in mantissa so that we
    // don't mess with NaNs or Infinite representation in IEEE 754:
    // https://en.wikipedia.org/wiki/NaN
    if prec_bits.abs() > BITS_MANTISSA_FLOAT {
        eprintln!(
            "The precision cannot be larger than {} bits for floats (asking for {} bits)",
            BITS_MANTISSA_FLOAT, prec_bits
        );
        return -1;
    }
    let zeroed_bits = if prec_bits >= 0 {
        BITS_MANTISSA_FLOAT - prec_bits
    } else {
        -prec_bits
    };
    if zeroed_bits >= BITS_MANTISSA_FLOAT {
        eprintln!(
            "The reduction in precision cannot be larger or equal than {} bits for floats (asking for {} bits)",
            BITS_MANTISSA_FLOAT, zeroed_bits
        );
        return -1;
    }
    let mask = !((1 << zeroed_bits) - 1);
    for i in 0..nelems as usize {
        dest[i] = src[i] & mask;
    }
    0
}

/// Truncate precision for 64-bit doubles
/// 
/// prec_bits: Number of bits to keep in mantissa (positive) or reduce (negative)
/// nelems: Number of elements to process
/// src: Source array
/// dest: Destination array
/// 
/// Returns 0 on success, -1 on error
fn truncate_precision64(prec_bits: i8, nelems: i32, src: &[i64], dest: &mut [i64]) -> i32 {
    // Make sure that we don't remove all the bits in mantissa so that we
    // don't mess with NaNs or Infinite representation in IEEE 754:
    // https://en.wikipedia.org/wiki/NaN
    if prec_bits.abs() > BITS_MANTISSA_DOUBLE {
        eprintln!(
            "The precision cannot be larger than {} bits for floats (asking for {} bits)",
            BITS_MANTISSA_DOUBLE, prec_bits
        );
        return -1;
    }
    let zeroed_bits = if prec_bits >= 0 {
        BITS_MANTISSA_DOUBLE - prec_bits
    } else {
        -prec_bits
    };
    if zeroed_bits >= BITS_MANTISSA_DOUBLE {
        eprintln!(
            "The reduction in precision cannot be larger or equal than {} bits for floats (asking for {} bits)",
            BITS_MANTISSA_DOUBLE, zeroed_bits
        );
        return -1;
    }
    let mask: u64 = !((1u64 << zeroed_bits) - 1u64);
    for i in 0..nelems as usize {
        dest[i] = (src[i] as u64 & mask) as i64;
    }
    0
}

/// Apply truncate precision to src. This can never fail.
/// 
/// prec_bits: Number of bits to keep in mantissa (positive) or reduce (negative)
/// typesize: Size of each element (4 for float, 8 for double)
/// nbytes: Total number of bytes in the array
/// src: Source byte array
/// dest: Destination byte array
/// 
/// Returns 0 on success, -1 on error
/// 
/// Positive values of prec_bits will set absolute precision bits, whereas negative
/// values will reduce the precision bits (similar to Python slicing convention).
pub fn truncate_precision(
    prec_bits: i8,
    typesize: i32,
    nbytes: i32,
    src: &[u8],
    dest: &mut [u8],
) -> i32 {
    match typesize {
        4 => {
            let nelems = nbytes / typesize;
            // Reinterpret byte slices as i32 slices
            let src_i32 = unsafe {
                std::slice::from_raw_parts(src.as_ptr() as *const i32, nelems as usize)
            };
            let dest_i32 = unsafe {
                std::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut i32, nelems as usize)
            };
            truncate_precision32(prec_bits, nelems, src_i32, dest_i32)
        }
        8 => {
            let nelems = nbytes / typesize;
            // Reinterpret byte slices as i64 slices
            let src_i64 = unsafe {
                std::slice::from_raw_parts(src.as_ptr() as *const i64, nelems as usize)
            };
            let dest_i64 = unsafe {
                std::slice::from_raw_parts_mut(dest.as_mut_ptr() as *mut i64, nelems as usize)
            };
            truncate_precision64(prec_bits, nelems, src_i64, dest_i64)
        }
        _ => {
            eprintln!(
                "Error in trunc-prec filter: Precision for typesize {} not handled",
                typesize
            );
            -1
        }
    }
}

/*

int truncate_precision(int8_t prec_bits, int32_t typesize, int32_t nbytes,
                       const uint8_t* src, uint8_t* dest);

#include "trunc-prec.h"
#include "blosc2.h"

#define BITS_MANTISSA_FLOAT 23
#define BITS_MANTISSA_DOUBLE 52


int truncate_precision32(int8_t prec_bits, int32_t nelems,
                         const int32_t* src, int32_t* dest) {
  // Make sure that we don't remove all the bits in mantissa so that we
  // don't mess with NaNs or Infinite representation in IEEE 754:
  // https://en.wikipedia.org/wiki/NaN
  if ((abs(prec_bits) > BITS_MANTISSA_FLOAT)) {
    BLOSC_TRACE_ERROR("The precision cannot be larger than %d bits for floats (asking for %d bits)",
                      BITS_MANTISSA_FLOAT, prec_bits);
    return -1;
  }
  int zeroed_bits = (prec_bits >= 0) ? BITS_MANTISSA_FLOAT - prec_bits : -prec_bits;
  if (zeroed_bits >= BITS_MANTISSA_FLOAT) {
    BLOSC_TRACE_ERROR("The reduction in precision cannot be larger or equal than %d bits for floats (asking for %d bits)",
                      BITS_MANTISSA_FLOAT, zeroed_bits);
    return -1;
  }
  int32_t mask = ~((1 << zeroed_bits) - 1);
  for (int i = 0; i < nelems; i++) {
    dest[i] = src[i] & mask;
  }
  return 0;
}

int truncate_precision64(int8_t prec_bits, int32_t nelems,
                          const int64_t* src, int64_t* dest) {
  // Make sure that we don't remove all the bits in mantissa so that we
  // don't mess with NaNs or Infinite representation in IEEE 754:
  // https://en.wikipedia.org/wiki/NaN
  if ((abs(prec_bits) > BITS_MANTISSA_DOUBLE)) {
    BLOSC_TRACE_ERROR("The precision cannot be larger than %d bits for floats (asking for %d bits)",
                      BITS_MANTISSA_DOUBLE, prec_bits);
    return -1;
  }
  int zeroed_bits = (prec_bits >= 0) ? BITS_MANTISSA_DOUBLE - prec_bits : -prec_bits;
  if (zeroed_bits >= BITS_MANTISSA_DOUBLE) {
    BLOSC_TRACE_ERROR("The reduction in precision cannot be larger or equal than %d bits for floats (asking for %d bits)",
                      BITS_MANTISSA_DOUBLE, zeroed_bits);
    return -1;
  }
  uint64_t mask = ~((1ULL << zeroed_bits) - 1ULL);
  for (int i = 0; i < nelems; i++) {
    dest[i] = (int64_t)(src[i] & mask);
  }
  return 0;
}

/* Apply the truncate precision to src.  This can never fail. */
int truncate_precision(int8_t prec_bits, int32_t typesize, int32_t nbytes,
                       const uint8_t* src, uint8_t* dest) {
  // Positive values of prec_bits will set absolute precision bits, whereas negative
  // values will reduce the precision bits (similar to Python slicing convention).
  switch (typesize) {
    case 4:
      return truncate_precision32(prec_bits, nbytes / typesize,
                              (int32_t *)src, (int32_t *)dest);
    case 8:
      return truncate_precision64(prec_bits, nbytes / typesize,
                              (int64_t *)src, (int64_t *)dest);
    default:
      BLOSC_TRACE_ERROR("Error in trunc-prec filter: Precision for typesize %d not handled",
                        (int)typesize);
      return -1;
  }
}

 */
