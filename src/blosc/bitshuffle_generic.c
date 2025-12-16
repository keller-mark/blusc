
/*
Original C code for reference:

#include "blosc2/blosc2-common.h"

// Macros.
#define CHECK_MULT_EIGHT(n) if (n % 8) return -80;
#define MAX(X,Y) ((X) > (Y) ? (X) : (Y))

#define MIN(X,Y) ((X) < (Y) ? (X) : (Y))
#define CHECK_ERR(count)                  \
        do {                              \
          if ((count) < 0)                \
            return count;                 \
         } while (0)

#define CHECK_ERR_FREE(count, buf) if (count < 0) { free(buf); return count; }

/* ---- Worker code not requiring special instruction sets. ----
 *
 * The following code does not use any x86 specific vectorized instructions
 * and should compile on any machine
 *
 */

/* Transpose 8x8 bit array packed into a single quadword *x*.
 * *t* is workspace. */
#define TRANS_BIT_8X8(x, t) {                                               \
        t = (x ^ (x >> 7)) & 0x00AA00AA00AA00AALL;                          \
        x = x ^ t ^ (t << 7);                                               \
        t = (x ^ (x >> 14)) & 0x0000CCCC0000CCCCLL;                         \
        x = x ^ t ^ (t << 14);                                              \
        t = (x ^ (x >> 28)) & 0x00000000F0F0F0F0LL;                         \
        x = x ^ t ^ (t << 28);                                              \
    }

/* Transpose 8x8 bit array along the diagonal from upper right
   to lower left */
#define TRANS_BIT_8X8_BE(x, t) {                                            \
        t = (x ^ (x >> 9)) & 0x0055005500550055LL;                          \
        x = x ^ t ^ (t << 9);                                               \
        t = (x ^ (x >> 18)) & 0x0000333300003333LL;                         \
        x = x ^ t ^ (t << 18);                                              \
        t = (x ^ (x >> 36)) & 0x000000000F0F0F0FLL;                         \
        x = x ^ t ^ (t << 36);                                              \
    }

/* Transpose of an array of arbitrarily typed elements. */
#define TRANS_ELEM_TYPE(in, out, lda, ldb, type_t) {                        \
        size_t ii, jj, kk;                                                  \
        const type_t* in_type = (const type_t*) in;                                 \
        type_t* out_type = (type_t*) out;                                   \
        for(ii = 0; ii + 7 < lda; ii += 8) {                                \
            for(jj = 0; jj < ldb; jj++) {                                   \
                for(kk = 0; kk < 8; kk++) {                                 \
                    out_type[jj*lda + ii + kk] =                            \
                        in_type[ii*ldb + kk * ldb + jj];                    \
                }                                                           \
            }                                                               \
        }                                                                   \
        for(ii = lda - lda % 8; ii < lda; ii ++) {                          \
            for(jj = 0; jj < ldb; jj++) {                                   \
                out_type[jj*lda + ii] = in_type[ii*ldb + jj];                            \
            }                                                               \
        }                                                                   \
    }



/* Memory copy with bshuf call signature. For testing and profiling. */
BLOSC_NO_EXPORT int64_t
bshuf_copy(const void* in, void* out, const size_t size, const size_t elem_size);

/* Private functions */
BLOSC_NO_EXPORT int64_t
bshuf_trans_byte_elem_remainder(const void* in, void* out, const size_t size,
                                const size_t elem_size, const size_t start);

BLOSC_NO_EXPORT int64_t
bshuf_trans_byte_elem_scal(const void* in, void* out, const size_t size,
                           const size_t elem_size);

BLOSC_NO_EXPORT int64_t
bshuf_trans_bit_byte_remainder(const void* in, void* out, const size_t size,
                               const size_t elem_size, const size_t start_byte);

BLOSC_NO_EXPORT int64_t
bshuf_trans_elem(const void* in, void* out, const size_t lda,
                 const size_t ldb, const size_t elem_size);

BLOSC_NO_EXPORT int64_t
bshuf_trans_bitrow_eight(const void* in, void* out, const size_t size,
                         const size_t elem_size);

BLOSC_NO_EXPORT int64_t
bshuf_shuffle_bit_eightelem_scal(const void* in, void* out,
                                 const size_t size, const size_t elem_size);

BLOSC_NO_EXPORT int64_t
bshuf_trans_byte_bitrow_scal(const void* in, void* out, const size_t size,
                             const size_t elem_size);

/* Bitshuffle the data.
 *
 * Transpose the bits within elements.
 *
 * Parameters
 * ----------
 *  in : input buffer, must be of size * elem_size bytes
 *  out : output buffer, must be of size * elem_size bytes
 *  size : number of elements in input
 *  elem_size : element size of typed data
 *  tmp_buffer : temporary buffer with the same `size` than `in` and `out`
 *
 * Returns
 * -------
 *  nothing -- this cannot fail
 *
 */

BLOSC_NO_EXPORT int64_t
bshuf_trans_bit_elem_scal(const void* in, void* out, const size_t size,
                          const size_t elem_size);

/* Unshuffle bitshuffled data.
 *
 * Untranspose the bits within elements.
 *
 * To properly unshuffle bitshuffled data, *size* and *elem_size* must
 * match the parameters used to shuffle the data.
 *
 * Parameters
 * ----------
 *  in : input buffer, must be of size * elem_size bytes
 *  out : output buffer, must be of size * elem_size bytes
 *  size : number of elements in input
 *  elem_size : element size of typed data
 *  tmp_buffer : temporary buffer with the same `size` than `in` and `out`
 *
 * Returns
 * -------
 *  nothing -- this cannot fail
 *
 */

BLOSC_NO_EXPORT int64_t
bshuf_untrans_bit_elem_scal(const void* in, void* out, const size_t size,
                            const size_t elem_size);






/* Memory copy with bshuf call signature. For testing and profiling. */
int64_t bshuf_copy(const void* in, void* out, const size_t size,
                   const size_t elem_size) {

  const char* in_b = (const char*) in;
  char* out_b = (char*) out;

  memcpy(out_b, in_b, size * elem_size);
  return size * elem_size;
}


/* Transpose bytes within elements, starting partway through input. */
int64_t bshuf_trans_byte_elem_remainder(const void* in, void* out, const size_t size,
                                        const size_t elem_size, const size_t start) {

  size_t ii, jj, kk;
  const char* in_b = (const char*) in;
  char* out_b = (char*) out;

  CHECK_MULT_EIGHT(start);

  if (size > start) {
    // ii loop separated into 2 loops so the compiler can unroll
    // the inner one.
    for (ii = start; ii + 7 < size; ii += 8) {
      for (jj = 0; jj < elem_size; jj++) {
        for (kk = 0; kk < 8; kk++) {
          out_b[jj * size + ii + kk]
              = in_b[ii * elem_size + kk * elem_size + jj];
        }
      }
    }
    for (ii = size - size % 8; ii < size; ii ++) {
      for (jj = 0; jj < elem_size; jj++) {
        out_b[jj * size + ii] = in_b[ii * elem_size + jj];
      }
    }
  }
  return size * elem_size;
}


/* Transpose bytes within elements. */
int64_t bshuf_trans_byte_elem_scal(const void* in, void* out, const size_t size,
                                   const size_t elem_size) {

  return bshuf_trans_byte_elem_remainder(in, out, size, elem_size, 0);
}


/* Transpose bits within bytes. */
int64_t bshuf_trans_bit_byte_remainder(const void* in, void* out, const size_t size,
                                       const size_t elem_size, const size_t start_byte) {

  const uint64_t* in_b = (const uint64_t*) in;
  uint8_t* out_b = (uint8_t*) out;

  uint64_t x, t;

  size_t ii, kk;
  size_t nbyte = elem_size * size;
  size_t nbyte_bitrow = nbyte / 8;

  uint64_t e=1;
  const int little_endian = *(uint8_t *) &e == 1;
  const size_t bit_row_skip = little_endian ? nbyte_bitrow : -nbyte_bitrow;
  const int64_t bit_row_offset = little_endian ? 0 : 7 * nbyte_bitrow;

  CHECK_MULT_EIGHT(nbyte);
  CHECK_MULT_EIGHT(start_byte);

  for (ii = start_byte / 8; ii < nbyte_bitrow; ii ++) {
    x = in_b[ii];
    if (little_endian) {
      TRANS_BIT_8X8(x, t);
    } else {
      TRANS_BIT_8X8_BE(x, t);
    }
    for (kk = 0; kk < 8; kk ++) {
      out_b[bit_row_offset + kk * bit_row_skip + ii] = x;
      x = x >> 8;
    }
  }
  return size * elem_size;
}


/* Transpose bits within bytes. */
int64_t bshuf_trans_bit_byte_scal(const void* in, void* out, const size_t size,
                                  const size_t elem_size) {

  return bshuf_trans_bit_byte_remainder(in, out, size, elem_size, 0);
}


/* General transpose of an array, optimized for large element sizes. */
int64_t bshuf_trans_elem(const void* in, void* out, const size_t lda,
                         const size_t ldb, const size_t elem_size) {

  size_t ii, jj;
  const char* in_b = (const char*) in;
  char* out_b = (char*) out;
  for(ii = 0; ii < lda; ii++) {
    for(jj = 0; jj < ldb; jj++) {
      memcpy(&out_b[(jj*lda + ii) * elem_size],
             &in_b[(ii*ldb + jj) * elem_size], elem_size);
    }
  }
  return lda * ldb * elem_size;
}


/* Transpose rows of shuffled bits (size / 8 bytes) within groups of 8. */
int64_t bshuf_trans_bitrow_eight(const void* in, void* out, const size_t size,
                                 const size_t elem_size) {

  size_t nbyte_bitrow = size / 8;

  CHECK_MULT_EIGHT(size);

  return bshuf_trans_elem(in, out, 8, elem_size, nbyte_bitrow);
}


/* Transpose bits within elements. */
int64_t bshuf_trans_bit_elem_scal(const void* in, void* out, const size_t size,
                                  const size_t elem_size) {

  int64_t count;
  void *tmp_buf;

  CHECK_MULT_EIGHT(size);

  tmp_buf = malloc(size * elem_size);
  if (tmp_buf == NULL) return -1;

  count = bshuf_trans_byte_elem_scal(in, out, size, elem_size);
  CHECK_ERR_FREE(count, tmp_buf);
  count = bshuf_trans_bit_byte_scal(out, tmp_buf, size, elem_size);
  CHECK_ERR_FREE(count, tmp_buf);
  count = bshuf_trans_bitrow_eight(tmp_buf, out, size, elem_size);

  free(tmp_buf);

  return count;
}


/* For data organized into a row for each bit (8 * elem_size rows), transpose
 * the bytes. */
int64_t bshuf_trans_byte_bitrow_scal(const void* in, void* out, const size_t size,
                                     const size_t elem_size) {
  size_t ii, jj, kk, nbyte_row;
  const char *in_b;
  char *out_b;


  in_b = (const char*) in;
  out_b = (char*) out;

  nbyte_row = size / 8;

  CHECK_MULT_EIGHT(size);

  for (jj = 0; jj < elem_size; jj++) {
    for (ii = 0; ii < nbyte_row; ii++) {
      for (kk = 0; kk < 8; kk++) {
        out_b[ii * 8 * elem_size + jj * 8 + kk] = \
                        in_b[(jj * 8 + kk) * nbyte_row + ii];
      }
    }
  }
  return size * elem_size;
}


/* Shuffle bits within the bytes of eight element blocks. */
int64_t bshuf_shuffle_bit_eightelem_scal(const void* in, void* out, \
        const size_t size, const size_t elem_size) {

  const char *in_b;
  char *out_b;
  uint64_t x, t;
  size_t ii, jj, kk;
  size_t nbyte, out_index;

  uint64_t e=1;
  const int little_endian = *(uint8_t *) &e == 1;
  const size_t elem_skip = little_endian ? elem_size : -elem_size;
  const uint64_t elem_offset = little_endian ? 0 : 7 * elem_size;

  CHECK_MULT_EIGHT(size);

  in_b = (const char*) in;
  out_b = (char*) out;

  nbyte = elem_size * size;

  for (jj = 0; jj < 8 * elem_size; jj += 8) {
    for (ii = 0; ii + 8 * elem_size - 1 < nbyte; ii += 8 * elem_size) {
      x = *((uint64_t*) &in_b[ii + jj]);
      if (little_endian) {
        TRANS_BIT_8X8(x, t);
      } else {
        TRANS_BIT_8X8_BE(x, t);
      }
      for (kk = 0; kk < 8; kk++) {
        out_index = ii + jj / 8 + elem_offset + kk * elem_skip;
        *((uint8_t*) &out_b[out_index]) = x;
        x = x >> 8;
      }
    }
  }
  return size * elem_size;
}


/* Untranspose bits within elements. */
int64_t bshuf_untrans_bit_elem_scal(const void* in, void* out, const size_t size,
                                    const size_t elem_size) {

  int64_t count;
  void *tmp_buf;

  CHECK_MULT_EIGHT(size);

  tmp_buf = malloc(size * elem_size);
  if (tmp_buf == NULL) return -1;

  count = bshuf_trans_byte_bitrow_scal(in, tmp_buf, size, elem_size);
  CHECK_ERR_FREE(count, tmp_buf);
  count =  bshuf_shuffle_bit_eightelem_scal(tmp_buf, out, size, elem_size);

  free(tmp_buf);

  return count;
}

*/