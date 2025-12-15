// Corresponds to c-blosc2/include/blosc2/filters-registry.h

// Simple filter for grouping NDim cell data together.
// See https://github.com/Blosc/c-blosc2/blob/main/plugins/filters/ndcell/README.md
pub const BLOSC_FILTER_NDCELL: u32 = 32;

// Simple filter for replacing content of a NDim cell with its mean value.
// See https://github.com/Blosc/c-blosc2/blob/main/plugins/filters/ndmean/README.md
pub const BLOSC_FILTER_NDMEAN: u32 = 33;

// See #524
pub const BLOSC_FILTER_BYTEDELTA_BUGGY: u32 = 34;

// Byteshuffle + delta.  The typesize should be specified in the `filters_meta` slot.
//  Sometimes this can represent an advantage over
// @ref BLOSC_SHUFFLE or @ref BLOSC_BITSHUFFLE.
// See https://www.blosc.org/posts/bytedelta-enhance-compression-toolset/
pub const BLOSC_FILTER_BYTEDELTA: u32 = 35;

// Truncate int precision; positive values in `filters_meta` slot will keep bits;
// negative values will remove (set to zero) bits.
// This is similar to @ref BLOSC_TRUNC_PREC, but for integers instead of floating point data.
pub const BLOSC_FILTER_INT_TRUNC: u32 = 36;
