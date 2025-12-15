// Corresponds to c-blosc2/blosc/b2nd_utils.c

const B2ND_MAX_DIM: usize = 16;
const BLOSC2_ERROR_SUCCESS: i32 = 0;

fn blosc2_multidim_to_unidim(index: &[i64], ndim: usize, strides: &[i64]) -> i64 {
    let mut i = 0;
    for j in 0..ndim {
        i += index[j] * strides[j];
    }
    i
}

fn blosc2_unidim_to_multidim(ndim: usize, shape: &[i64], i: i64, index: &mut [i64]) {
    if ndim == 0 {
        return;
    }
    let mut strides = [0i64; B2ND_MAX_DIM];
    strides[ndim - 1] = 1;
    for j in (0..ndim - 1).rev() {
        strides[j] = shape[j + 1] * strides[j + 1];
    }

    index[0] = i / strides[0];
    for j in 1..ndim {
        index[j] = (i % strides[j - 1]) / strides[j];
    }
}

pub fn b2nd_copy_buffer2(
    ndim: i8,
    itemsize: i32,
    src: &[u8],
    src_pad_shape: &[i64],
    src_start: &[i64],
    src_stop: &[i64],
    dst: &mut [u8],
    dst_pad_shape: &[i64],
    dst_start: &[i64],
) -> i32 {
    let ndim = ndim as usize;
    let mut copy_shape = [0i64; B2ND_MAX_DIM];
    for i in 0..ndim {
        copy_shape[i] = src_stop[i] - src_start[i];
        if copy_shape[i] == 0 {
            return BLOSC2_ERROR_SUCCESS;
        }
    }

    // Compute the strides
    let mut src_strides = [0i64; B2ND_MAX_DIM];
    src_strides[ndim - 1] = 1;
    for i in (0..ndim - 1).rev() {
        src_strides[i] = src_strides[i + 1] * src_pad_shape[i + 1];
    }

    let mut dst_strides = [0i64; B2ND_MAX_DIM];
    dst_strides[ndim - 1] = 1;
    for i in (0..ndim - 1).rev() {
        dst_strides[i] = dst_strides[i + 1] * dst_pad_shape[i + 1];
    }

    // Align the buffers removing unnecessary data
    let src_start_n = blosc2_multidim_to_unidim(src_start, ndim, &src_strides);
    let src_offset = (src_start_n * itemsize as i64) as usize;
    let bsrc = &src[src_offset..];

    let dst_start_n = blosc2_multidim_to_unidim(dst_start, ndim, &dst_strides);
    let dst_offset = (dst_start_n * itemsize as i64) as usize;
    let bdst = &mut dst[dst_offset..];

    if ndim == 1 {
        let copy_nbytes = (copy_shape[0] * itemsize as i64) as usize;
        bdst[..copy_nbytes].copy_from_slice(&bsrc[..copy_nbytes]);
        return BLOSC2_ERROR_SUCCESS;
    }

    // Fallback logic for all dimensions > 1 (ignoring specialized loops for simplicity and single-threadedness)
    // Corresponds to copy_ndim_fallback in C
    let copy_nbytes = (copy_shape[ndim - 1] * itemsize as i64) as usize;
    let mut number_of_copies = 1;
    for i in 0..ndim - 1 {
        number_of_copies *= copy_shape[i];
    }

    for ncopy in 0..number_of_copies {
        // Compute the start of the copy
        let mut copy_start = [0i64; B2ND_MAX_DIM];
        blosc2_unidim_to_multidim(ndim - 1, &copy_shape, ncopy, &mut copy_start);

        // Translate this index to the src buffer
        let src_copy_start = blosc2_multidim_to_unidim(&copy_start, ndim - 1, &src_strides);

        // Translate this index to the dst buffer
        let dst_copy_start = blosc2_multidim_to_unidim(&copy_start, ndim - 1, &dst_strides);

        // Perform the copy
        let src_idx = (src_copy_start * itemsize as i64) as usize;
        let dst_idx = (dst_copy_start * itemsize as i64) as usize;

        bdst[dst_idx..dst_idx + copy_nbytes].copy_from_slice(&bsrc[src_idx..src_idx + copy_nbytes]);
    }

    BLOSC2_ERROR_SUCCESS
}

pub fn b2nd_copy_buffer(
    ndim: i8,
    itemsize: u8,
    src: &[u8],
    src_pad_shape: &[i64],
    src_start: &[i64],
    src_stop: &[i64],
    dst: &mut [u8],
    dst_pad_shape: &[i64],
    dst_start: &[i64],
) -> i32 {
    b2nd_copy_buffer2(
        ndim,
        itemsize as i32,
        src,
        src_pad_shape,
        src_start,
        src_stop,
        dst,
        dst_pad_shape,
        dst_start,
    )
}

/*

#include "b2nd.h"

// copyNdim where N = {2-8} - specializations of copy loops to be used by b2nd_copy_buffer
// since we don't have c++ templates, substitute manual specializations for up to known B2ND_MAX_DIM (8)
// it's not pretty, but it substantially reduces overhead vs. the generic method
void copy8dim(const int32_t itemsize,
              const int64_t *copy_shape,
              const uint8_t *bsrc, const int64_t *src_strides,
              uint8_t *bdst, const int64_t *dst_strides) {
  int64_t copy_nbytes = copy_shape[7] * itemsize;
  int64_t copy_start[7] = {0};
  do {
    do {
      do {
        do {
          do {
            do {
              do {
                int64_t src_copy_start = 0;
                int64_t dst_copy_start = 0;
                for (int j = 0; j < 7; ++j) {
                  src_copy_start += copy_start[j] * src_strides[j];
                  dst_copy_start += copy_start[j] * dst_strides[j];
                }
                memcpy(&bdst[dst_copy_start * itemsize], &bsrc[src_copy_start * itemsize], copy_nbytes);
                ++copy_start[6];
              } while (copy_start[6] < copy_shape[6]);
              ++copy_start[5];
              copy_start[6] = 0;
            } while (copy_start[5] < copy_shape[5]);
            ++copy_start[4];
            copy_start[5] = 0;
          } while (copy_start[4] < copy_shape[4]);
          ++copy_start[3];
          copy_start[4] = 0;
        } while (copy_start[3] < copy_shape[3]);
        ++copy_start[2];
        copy_start[3] = 0;
      } while (copy_start[2] < copy_shape[2]);
      ++copy_start[1];
      copy_start[2] = 0;
    } while (copy_start[1] < copy_shape[1]);
    ++copy_start[0];
    copy_start[1] = 0;
  } while (copy_start[0] < copy_shape[0]);
}

void copy7dim(const int32_t itemsize,
              const int64_t *copy_shape,
              const uint8_t *bsrc, const int64_t *src_strides,
              uint8_t *bdst, const int64_t *dst_strides) {
  int64_t copy_nbytes = copy_shape[6] * itemsize;
  int64_t copy_start[6] = {0};
  do {
    do {
      do {
        do {
          do {
            do {
              int64_t src_copy_start = 0;
              int64_t dst_copy_start = 0;
              for (int j = 0; j < 6; ++j) {
                src_copy_start += copy_start[j] * src_strides[j];
                dst_copy_start += copy_start[j] * dst_strides[j];
              }
              memcpy(&bdst[dst_copy_start * itemsize], &bsrc[src_copy_start * itemsize], copy_nbytes);
              ++copy_start[5];
            } while (copy_start[5] < copy_shape[5]);
            ++copy_start[4];
            copy_start[5] = 0;
          } while (copy_start[4] < copy_shape[4]);
          ++copy_start[3];
          copy_start[4] = 0;
        } while (copy_start[3] < copy_shape[3]);
        ++copy_start[2];
        copy_start[3] = 0;
      } while (copy_start[2] < copy_shape[2]);
      ++copy_start[1];
      copy_start[2] = 0;
    } while (copy_start[1] < copy_shape[1]);
    ++copy_start[0];
    copy_start[1] = 0;
  } while (copy_start[0] < copy_shape[0]);
}

void copy6dim(const int32_t itemsize,
              const int64_t *copy_shape,
              const uint8_t *bsrc, const int64_t *src_strides,
              uint8_t *bdst, const int64_t *dst_strides) {
  int64_t copy_nbytes = copy_shape[5] * itemsize;
  int64_t copy_start[5] = {0};
  do {
    do {
      do {
        do {
          do {
            int64_t src_copy_start = 0;
            int64_t dst_copy_start = 0;
            for (int j = 0; j < 5; ++j) {
              src_copy_start += copy_start[j] * src_strides[j];
              dst_copy_start += copy_start[j] * dst_strides[j];
            }
            memcpy(&bdst[dst_copy_start * itemsize], &bsrc[src_copy_start * itemsize], copy_nbytes);
            ++copy_start[4];
          } while (copy_start[4] < copy_shape[4]);
          ++copy_start[3];
          copy_start[4] = 0;
        } while (copy_start[3] < copy_shape[3]);
        ++copy_start[2];
        copy_start[3] = 0;
      } while (copy_start[2] < copy_shape[2]);
      ++copy_start[1];
      copy_start[2] = 0;
    } while (copy_start[1] < copy_shape[1]);
    ++copy_start[0];
    copy_start[1] = 0;
  } while (copy_start[0] < copy_shape[0]);
}

void copy5dim(const int32_t itemsize,
              const int64_t *copy_shape,
              const uint8_t *bsrc, const int64_t *src_strides,
              uint8_t *bdst, const int64_t *dst_strides) {
  int64_t copy_nbytes = copy_shape[4] * itemsize;
  int64_t copy_start[4] = {0};
  do {
    do {
      do {
        do {
          int64_t src_copy_start = 0;
          int64_t dst_copy_start = 0;
          for (int j = 0; j < 4; ++j) {
            src_copy_start += copy_start[j] * src_strides[j];
            dst_copy_start += copy_start[j] * dst_strides[j];
          }
          memcpy(&bdst[dst_copy_start * itemsize], &bsrc[src_copy_start * itemsize], copy_nbytes);
          ++copy_start[3];
        } while (copy_start[3] < copy_shape[3]);
        ++copy_start[2];
        copy_start[3] = 0;
      } while (copy_start[2] < copy_shape[2]);
      ++copy_start[1];
      copy_start[2] = 0;
    } while (copy_start[1] < copy_shape[1]);
    ++copy_start[0];
    copy_start[1] = 0;
  } while (copy_start[0] < copy_shape[0]);
}

void copy4dim(const int32_t itemsize,
              const int64_t *copy_shape,
              const uint8_t *bsrc, const int64_t *src_strides,
              uint8_t *bdst, const int64_t *dst_strides) {
  int64_t copy_nbytes = copy_shape[3] * itemsize;
  int64_t copy_start[3] = {0};
  do {
    do {
      do {
        int64_t src_copy_start = 0;
        int64_t dst_copy_start = 0;
        for (int j = 0; j < 3; ++j) {
          src_copy_start += copy_start[j] * src_strides[j];
          dst_copy_start += copy_start[j] * dst_strides[j];
        }
        memcpy(&bdst[dst_copy_start * itemsize], &bsrc[src_copy_start * itemsize], copy_nbytes);
        ++copy_start[2];
      } while (copy_start[2] < copy_shape[2]);
      ++copy_start[1];
      copy_start[2] = 0;
    } while (copy_start[1] < copy_shape[1]);
    ++copy_start[0];
    copy_start[1] = 0;
  } while (copy_start[0] < copy_shape[0]);
}

void copy3dim(const int32_t itemsize,
              const int64_t *copy_shape,
              const uint8_t *bsrc, const int64_t *src_strides,
              uint8_t *bdst, const int64_t *dst_strides) {
  int64_t copy_nbytes = copy_shape[2] * itemsize;
  int64_t copy_start[2] = {0};
  do {
    do {
      int64_t src_copy_start = 0;
      int64_t dst_copy_start = 0;
      for (int j = 0; j < 2; ++j) {
        src_copy_start += copy_start[j] * src_strides[j];
        dst_copy_start += copy_start[j] * dst_strides[j];
      }
      memcpy(&bdst[dst_copy_start * itemsize], &bsrc[src_copy_start * itemsize], copy_nbytes);
      ++copy_start[1];
    } while (copy_start[1] < copy_shape[1]);
    ++copy_start[0];
    copy_start[1] = 0;
  } while (copy_start[0] < copy_shape[0]);
}

void copy2dim(const int32_t itemsize,
              const int64_t *copy_shape,
              const uint8_t *bsrc, const int64_t *src_strides,
              uint8_t *bdst, const int64_t *dst_strides) {
  int64_t copy_nbytes = copy_shape[1] * itemsize;
  int64_t copy_start = 0;
  do {
    int64_t src_copy_start = copy_start * src_strides[0];
    int64_t dst_copy_start = copy_start * dst_strides[0];
    memcpy(&bdst[dst_copy_start * itemsize], &bsrc[src_copy_start * itemsize], copy_nbytes);
    ++copy_start;
  } while (copy_start < copy_shape[0]);
}


void copy_ndim_fallback(const int8_t ndim,
                        const int32_t itemsize,
                        int64_t *copy_shape,
                        const uint8_t *bsrc, int64_t *src_strides,
                        uint8_t *bdst, int64_t *dst_strides) {
  int64_t copy_nbytes = copy_shape[ndim - 1] * itemsize;
  int64_t number_of_copies = 1;
  for (int i = 0; i < ndim - 1; ++i) {
    number_of_copies *= copy_shape[i];
  }
  for (int ncopy = 0; ncopy < number_of_copies; ++ncopy) {
    // Compute the start of the copy
    int64_t copy_start[B2ND_MAX_DIM] = {0};
    blosc2_unidim_to_multidim((int8_t) (ndim - 1), copy_shape, ncopy, copy_start);

    // Translate this index to the src buffer
    int64_t src_copy_start;
    blosc2_multidim_to_unidim(copy_start, (int8_t) (ndim - 1), src_strides, &src_copy_start);

    // Translate this index to the dst buffer
    int64_t dst_copy_start;
    blosc2_multidim_to_unidim(copy_start, (int8_t) (ndim - 1), dst_strides, &dst_copy_start);

    // Perform the copy
    memcpy(&bdst[dst_copy_start * itemsize],
           &bsrc[src_copy_start * itemsize],
           copy_nbytes);
  }
}

int b2nd_copy_buffer2(int8_t ndim,
                      int32_t itemsize,
                      const void *src, const int64_t *src_pad_shape,
                      const int64_t *src_start, const int64_t *src_stop,
                      void *dst, const int64_t *dst_pad_shape,
                      const int64_t *dst_start) {
  // Compute the shape of the copy
  int64_t copy_shape[B2ND_MAX_DIM] = {0};
  for (int i = 0; i < ndim; ++i) {
    copy_shape[i] = src_stop[i] - src_start[i];
    if (copy_shape[i] == 0) {
      return BLOSC2_ERROR_SUCCESS;
    }
  }

  // Compute the strides
  int64_t src_strides[B2ND_MAX_DIM] = {0};
  src_strides[ndim - 1] = 1;
  for (int i = ndim - 2; i >= 0; --i) {
    src_strides[i] = src_strides[i + 1] * src_pad_shape[i + 1];
  }

  int64_t dst_strides[B2ND_MAX_DIM] = {0};
  dst_strides[ndim - 1] = 1;
  for (int i = ndim - 2; i >= 0; --i) {
    dst_strides[i] = dst_strides[i + 1] * dst_pad_shape[i + 1];
  }

  // Align the buffers removing unnecessary data
  int64_t src_start_n;
  blosc2_multidim_to_unidim(src_start, ndim, src_strides, &src_start_n);
  uint8_t *bsrc = (uint8_t *) src;
  bsrc = &bsrc[src_start_n * itemsize];

  int64_t dst_start_n;
  blosc2_multidim_to_unidim(dst_start, ndim, dst_strides, &dst_start_n);
  uint8_t *bdst = (uint8_t *) dst;
  bdst = &bdst[dst_start_n * itemsize];

  switch (ndim) {
    case 1:
      memcpy(&bdst[0], &bsrc[0], copy_shape[0] * itemsize);
      break;
    case 2:
      copy2dim(itemsize, copy_shape, bsrc, src_strides, bdst, dst_strides);
      break;
    case 3:
      copy3dim(itemsize, copy_shape, bsrc, src_strides, bdst, dst_strides);
      break;
    case 4:
      copy4dim(itemsize, copy_shape, bsrc, src_strides, bdst, dst_strides);
      break;
    case 5:
      copy5dim(itemsize, copy_shape, bsrc, src_strides, bdst, dst_strides);
      break;
    case 6:
      copy6dim(itemsize, copy_shape, bsrc, src_strides, bdst, dst_strides);
      break;
    case 7:
      copy7dim(itemsize, copy_shape, bsrc, src_strides, bdst, dst_strides);
      break;
    case 8:
      copy8dim(itemsize, copy_shape, bsrc, src_strides, bdst, dst_strides);
      break;
    default:
      // guard against potential future increase to B2ND_MAX_DIM
      copy_ndim_fallback(ndim, itemsize, copy_shape, bsrc, src_strides, bdst, dst_strides);
      break;
  }

  return BLOSC2_ERROR_SUCCESS;
}


// Keep the old signature for API compatibility
int b2nd_copy_buffer(int8_t ndim,
                     uint8_t itemsize,
                     const void *src, const int64_t *src_pad_shape,
                     const int64_t *src_start, const int64_t *src_stop,
                     void *dst, const int64_t *dst_pad_shape,
                     const int64_t *dst_start) {
  // Simply cast itemsize to int32_t and delegate
  return b2nd_copy_buffer2(ndim, (int32_t)itemsize, src, src_pad_shape,
                          src_start, src_stop, dst, dst_pad_shape, dst_start);
}

*/