// Constants from c-blosc2/include/blosc2.h

// Also see https://github.com/maiteko/blosc2-src-rs/blob/9aa1ba98a9fe7a59c112d691bfea0caed6f00382/src/bindings.rs#L3C1-L21C53

/* Version numbers */
pub const BLOSC2_VERSION_MAJOR: u8 = 2; /* for major interface/format changes  */
pub const BLOSC2_VERSION_MINOR: u8 = 22; /* for minor interface/format changes  */
pub const BLOSC2_VERSION_RELEASE: &str = "1.dev"; /* for tweaks, bug-fixes, or development */
pub const BLOSC2_VERSION_STRING: &str = "2.22.1.dev"; /* string version.  Sync with above! */
pub const BLOSC2_VERSION_DATE: &str = "$Date:: 2025-10-28 #$"; /* date version year-month-day */

/* The maximum number of dimensions for Blosc2 NDim arrays */
pub const BLOSC2_MAX_DIM: u8 = 8;

/* The VERSION_FORMAT symbols below should be just 1-byte long */

/* Blosc format version, starting at 1
    1 -> Blosc pre-1.0
    2 -> Blosc 1.x stable series
    3 -> Blosc 2-alpha.x series
    4 -> Blosc 2.x beta.1 series
    5 -> Blosc 2.x stable series
*/
pub const BLOSC1_VERSION_FORMAT_PRE1: u8 = 1;
pub const BLOSC1_VERSION_FORMAT: u8 = 2;
pub const BLOSC2_VERSION_FORMAT_ALPHA: u8 = 3;
pub const BLOSC2_VERSION_FORMAT_BETA1: u8 = 4;
pub const BLOSC2_VERSION_FORMAT_STABLE: u8 = 5;
pub const BLOSC2_VERSION_FORMAT: u8 = BLOSC2_VERSION_FORMAT_STABLE;


/* The FRAME_FORMAT_VERSION symbols below should be just 4-bit long */

/* Blosc format version
*  1 -> First version (introduced in beta.2)
*  2 -> Second version (introduced in rc.1)
*/
pub const BLOSC2_VERSION_FRAME_FORMAT_BETA2: u8 = 1;  // for 2.0.0-beta2 and after
pub const BLOSC2_VERSION_FRAME_FORMAT_RC1: u8 = 2;    // for 2.0.0-rc1 and after
pub const BLOSC2_VERSION_FRAME_FORMAT: u8 = BLOSC2_VERSION_FRAME_FORMAT_RC1;

// Minimum header length (Blosc1)
pub const BLOSC_MIN_HEADER_LENGTH: usize = 16;

// Extended header length (Blosc2, see README_HEADER)
pub const BLOSC_EXTENDED_HEADER_LENGTH: usize = 32;

// The maximum overhead during compression in bytes. This equals
// to @ref BLOSC_EXTENDED_HEADER_LENGTH now, but can be higher in future
// implementations.
pub const BLOSC2_MAX_OVERHEAD: usize = BLOSC_EXTENDED_HEADER_LENGTH;

pub const INT_MAX: usize = 2147483647;
pub const UINT8_MAX: usize = 255;

// Maximum source buffer size to be compressed
pub const BLOSC2_MAX_BUFFERSIZE: usize = INT_MAX - BLOSC2_MAX_OVERHEAD;

// Maximum typesize before considering source buffer as a stream of bytes.
// Cannot be larger than 255.
pub const BLOSC_MAX_TYPESIZE: usize = UINT8_MAX;

// Minimum buffer size to be compressed.
pub const BLOSC_MIN_BUFFERSIZE: usize = 32;

// L1 and L2 cache sizes (typical values)
pub const L1: usize = 32 * 1024;
pub const L2: usize = 256 * 1024;

// Maximum block size
pub const BLOSC_MAX_BLOCKSIZE: usize = BLOSC2_MAX_BUFFERSIZE;

// Blosc-defined tuners must be between 0 - 31.
pub const BLOSC2_DEFINED_TUNER_START: u8 = 0;
pub const BLOSC2_DEFINED_TUNER_STOP: u8 = 31;

// Blosc-registered tuners must be between 31 - 159.
pub const BLOSC2_GLOBAL_REGISTERED_TUNER_START: u8 = 32;
pub const BLOSC2_GLOBAL_REGISTERED_TUNER_STOP: u8 = 159;
    
// Number of Blosc-registered tuners at the moment.
pub const BLOSC2_GLOBAL_REGISTERED_TUNERS: u8 = 0;

// User-defined tuners must be between 160 - 255.
pub const BLOSC2_USER_REGISTERED_TUNER_START: u8 = 160;
pub const BLOSC2_USER_REGISTERED_TUNER_STOP: u8 = 255;


/**
 * @brief Codes for the different tuners shipped with Blosc
 */

// Determine the last tuner defined by Blosc.
pub const BLOSC_STUNE: u8 = 0;
pub const BLOSC_LAST_TUNER: u8 = 1;

pub const BLOSC_LAST_REGISTERED_TUNER: u8 = BLOSC2_GLOBAL_REGISTERED_TUNER_START + BLOSC2_GLOBAL_REGISTERED_TUNERS - 1;


// Blosc-defined filters must be between 0 - 31.
pub const BLOSC2_DEFINED_FILTERS_START: u8 = 0;
pub const BLOSC2_DEFINED_FILTERS_STOP: u8 = 31;

// Blosc-registered filters must be between 32 - 159.
pub const BLOSC2_GLOBAL_REGISTERED_FILTERS_START: u8 = 32;
pub const BLOSC2_GLOBAL_REGISTERED_FILTERS_STOP: u8 = 159;

// Number of Blosc-registered filters at the moment.
pub const BLOSC2_GLOBAL_REGISTERED_FILTERS: u8 = 5;
  
// User-defined filters must be between 128 - 255.
pub const BLOSC2_USER_REGISTERED_FILTERS_START: u8 = 160;
pub const BLOSC2_USER_REGISTERED_FILTERS_STOP: u8 = 255;

// Maximum number of filters in the filter pipeline.
pub const BLOSC2_MAX_FILTERS: u8 = 6;

// Maximum number of filters that a user can register.
pub const BLOSC2_MAX_UDFILTERS: u8 = 16;




/**
 * @brief Codes for filters.
 *
 * @sa #blosc1_compress
 */

// No shuffle (for compatibility with Blosc1).
pub const BLOSC_NOSHUFFLE: u8 = 0;

// No filter.
pub const BLOSC_NOFILTER: u8 = 0;

// Byte-wise shuffle. `filters_meta` does not have any effect here.
pub const BLOSC_SHUFFLE: u8 = 1;

// Bit-wise shuffle. `filters_meta` does not have any effect here.
pub const BLOSC_BITSHUFFLE: u8 = 2;

// Delta filter. `filters_meta` does not have any effect here.
pub const BLOSC_DELTA: u8 = 3;

// Truncate mantissa precision.
// Positive values in `filters_meta` will keep bits; negative values will zero bits.
pub const BLOSC_TRUNC_PREC: u8 = 4;

// sentinel
pub const BLOSC_LAST_FILTER: u8 = 5;

// Determine the last registered filter. It is used to check if a filter is registered or not.
pub const BLOSC_LAST_REGISTERED_FILTER: u8 = BLOSC2_GLOBAL_REGISTERED_FILTERS_START + BLOSC2_GLOBAL_REGISTERED_FILTERS - 1;


/**
 * @brief Codes for internal flags (see blosc1_cbuffer_metainfo)
 */
pub const BLOSC_DOSHUFFLE: u8 = 0x1;     // byte-wise shuffle
pub const BLOSC_MEMCPYED: u8 = 0x2;      // plain copy
pub const BLOSC_DOBITSHUFFLE: u8 = 0x4;  // bit-wise shuffle
pub const BLOSC_DODELTA: u8 = 0x8;       // delta coding


/**
 * @brief Codes for new internal flags in Blosc2
 */
pub const BLOSC2_USEDICT: u8 = 0x1;          // use dictionaries with codec
pub const BLOSC2_BIGENDIAN: u8 = 0x2;        // data is in big-endian ordering
pub const BLOSC2_INSTR_CODEC: u8 = 0x80;     // codec is instrumented (mainly for development)


/**
 * @brief Values for different Blosc2 capabilities
 */
pub const BLOSC2_MAXDICTSIZE: u32 = 128 * 1024; // maximum size for compression dicts
pub const BLOSC2_MAXBLOCKSIZE: u32 = 536866816; // maximum size for blocks
pub const BLOSC2_MAXTYPESIZE: u32 = BLOSC2_MAXBLOCKSIZE; // maximum size for types

// Blosc-defined codecs must be between 0 - 31.
pub const BLOSC2_DEFINED_CODECS_START: u8 = 0;
pub const BLOSC2_DEFINED_CODECS_STOP: u8 = 31;

// Blosc-registered codecs must be between 31 - 159.
pub const BLOSC2_GLOBAL_REGISTERED_CODECS_START: u8 = 32;
pub const BLOSC2_GLOBAL_REGISTERED_CODECS_STOP: u8 = 159;

// Number of Blosc-registered codecs at the moment.
pub const BLOSC2_GLOBAL_REGISTERED_CODECS: u8 = 5;

// User-defined codecs must be between 160 - 255.
pub const BLOSC2_USER_REGISTERED_CODECS_START: u8 = 160;
pub const BLOSC2_USER_REGISTERED_CODECS_STOP: u8 = 255;


/**
 * @brief Codes for the different compressors shipped with Blosc
 */
pub const BLOSC_BLOSCLZ: u8 = 0;
pub const BLOSC_LZ4: u8 = 1;
pub const BLOSC_LZ4HC: u8 = 2;
pub const BLOSC_SNAPPY: u8 = 3;
pub const BLOSC_ZLIB: u8 = 4;
pub const BLOSC_ZSTD: u8 = 5;

// Determine the last codec defined by Blosc.
pub const BLOSC_LAST_CODEC: u8 = 6;

// Determine the last registered codec. It is used to check if a codec is registered or not.
pub const BLOSC_LAST_REGISTERED_CODEC: u8 = BLOSC2_GLOBAL_REGISTERED_CODECS_START + BLOSC2_GLOBAL_REGISTERED_CODECS - 1;


// Names for the different compressors shipped with Blosc

pub const BLOSC_BLOSCLZ_COMPNAME: &str = "blosclz";
pub const BLOSC_LZ4_COMPNAME: &str = "lz4";
pub const BLOSC_LZ4HC_COMPNAME: &str = "lz4hc";
pub const BLOSC_SNAPPY_COMPNAME: &str = "snappy";
pub const BLOSC_ZLIB_COMPNAME: &str = "zlib";
pub const BLOSC_ZSTD_COMPNAME: &str = "zstd";


/**
 * @brief Codes for compression libraries shipped with Blosc (code must be < 8)
 */
pub const BLOSC_BLOSCLZ_LIB: u8 = 0;
pub const BLOSC_LZ4_LIB: u8 = 1;
pub const BLOSC_ZLIB_LIB: u8 = 3;
pub const BLOSC_ZSTD_LIB: u8 = 4;

pub const BLOSC_UDCODEC_LIB: u8 = 6;
pub const BLOSC_SCHUNK_LIB: u8 = 7;   // compressor library in super-chunk header


/**
 * @brief Names for the different compression libraries shipped with Blosc
 */
pub const BLOSC_BLOSCLZ_LIBNAME: &str = "BloscLZ";
pub const BLOSC_LZ4_LIBNAME: &str = "LZ4";
pub const BLOSC_ZLIB_LIBNAME: &str = "Zlib";
pub const BLOSC_ZSTD_LIBNAME: &str = "Zstd";

/**
 * @brief The codes for compressor formats shipped with Blosc
 */

pub const BLOSC_BLOSCLZ_FORMAT: u8 = BLOSC_BLOSCLZ_LIB;
pub const BLOSC_LZ4_FORMAT: u8 = BLOSC_LZ4_LIB;
// LZ4HC and LZ4 share the same format
pub const BLOSC_LZ4HC_FORMAT: u8 = BLOSC_LZ4_LIB;
pub const BLOSC_ZLIB_FORMAT: u8 = BLOSC_ZLIB_LIB;
pub const BLOSC_ZSTD_FORMAT: u8 = BLOSC_ZSTD_LIB;

pub const BLOSC_UDCODEC_FORMAT: u8 = BLOSC_UDCODEC_LIB;


/**
 * @brief The version formats for compressors shipped with Blosc.
 * All versions here starts at 1
 */
pub const BLOSC_BLOSCLZ_VERSION_FORMAT: u8 = 1;
pub const BLOSC_LZ4_VERSION_FORMAT: u8 = 1;
pub const BLOSC_LZ4HC_VERSION_FORMAT: u8 = 1;  /* LZ4HC and LZ4 share the same format */
pub const BLOSC_ZLIB_VERSION_FORMAT: u8 = 1;
pub const BLOSC_ZSTD_VERSION_FORMAT: u8 = 1;

pub const BLOSC_UDCODEC_VERSION_FORMAT: u8 = 1;


/**
 * @brief Split mode for blocks.
 * NEVER and ALWAYS are for experimenting with compression ratio.
 * AUTO for nearly optimal behaviour (based on heuristics).
 * FORWARD_COMPAT provides best forward compatibility (default).
 */
pub const BLOSC_ALWAYS_SPLIT: u8 = 1;
pub const BLOSC_NEVER_SPLIT: u8 = 2;
pub const BLOSC_AUTO_SPLIT: u8 = 3;
pub const BLOSC_FORWARD_COMPAT_SPLIT: u8 = 4;


/**
 * @brief Offsets for fields in Blosc2 chunk header.
 */

pub const BLOSC2_CHUNK_VERSION: u8 = 0x0;  // the version for the chunk format
pub const BLOSC2_CHUNK_VERSIONLZ: u8 = 0x1;     // the version for the format of internal codec
pub const BLOSC2_CHUNK_FLAGS: u8 = 0x2;        // flags and codec info
pub const BLOSC2_CHUNK_TYPESIZE: u8 = 0x3;    // (uint8) the number of bytes of the atomic type
pub const BLOSC2_CHUNK_NBYTES: u8 = 0x4;     // (int32) uncompressed size of the buffer (this header is not included)
pub const BLOSC2_CHUNK_BLOCKSIZE: u8 = 0x8;  // (int32) size of internal blocks
pub const BLOSC2_CHUNK_CBYTES: u8 = 0xc;        // (int32) compressed size of the buffer (including this header)
pub const BLOSC2_CHUNK_FILTER_CODES: u8 = 0x10; // the codecs for the filter pipeline (1 byte per code)
pub const BLOSC2_CHUNK_FILTER_META: u8 = 0x18;  // meta info for the filter pipeline (1 byte per code)
pub const BLOSC2_CHUNK_BLOSC2_FLAGS: u8 = 0x1F; // flags specific for Blosc2 functionality


/**
 * @brief Run lengths for special values for chunks/frames
 */

pub const BLOSC2_NO_SPECIAL: u8 = 0x0;      // no special value
pub const BLOSC2_SPECIAL_ZERO: u8 = 0x1;   // zero special value
pub const BLOSC2_SPECIAL_NAN: u8 = 0x2;  // NaN special value
pub const BLOSC2_SPECIAL_VALUE: u8 = 0x3; // repeated special value
pub const BLOSC2_SPECIAL_UNINIT: u8 = 0x4; // non initialized values
pub const BLOSC2_SPECIAL_LASTID: u8 = 0x4; // last valid ID for special value (update this adequately)
pub const BLOSC2_SPECIAL_MASK: u8 = 0x7;     // special value mask (prev IDs cannot be larger than this)


/**
 * @brief Error codes
 * Each time an error code is added here, its corresponding message error should be added in
 * print_error()
 */

pub const BLOSC2_ERROR_SUCCESS: i32 = 0;           // Success
pub const BLOSC2_ERROR_FAILURE: i32 = -1;          // Generic failure
pub const BLOSC2_ERROR_STREAM: i32 = -2;          // Bad stream
pub const BLOSC2_ERROR_DATA: i32 = -3;             // Invalid data
pub const BLOSC2_ERROR_MEMORY_ALLOC: i32 = -4;    // Memory alloc/realloc failure
pub const BLOSC2_ERROR_READ_BUFFER: i32 = -5;    // Not enough space to read
pub const BLOSC2_ERROR_WRITE_BUFFER: i32 = -6;   // Not enough space to write
pub const BLOSC2_ERROR_CODEC_SUPPORT: i32 = -7;    // Codec not supported
pub const BLOSC2_ERROR_CODEC_PARAM: i32 = -8;      // Invalid parameter supplied to codec
pub const BLOSC2_ERROR_CODEC_DICT: i32 = -9;       // Codec dictionary error
pub const BLOSC2_ERROR_VERSION_SUPPORT: i32 = -10; // Version not supported
pub const BLOSC2_ERROR_INVALID_HEADER: i32 = -11;  // Invalid value in header
pub const BLOSC2_ERROR_INVALID_PARAM: i32 = -12;   // Invalid parameter supplied to function
pub const BLOSC2_ERROR_FILE_READ: i32 = -13;       // File read failure
pub const BLOSC2_ERROR_FILE_WRITE: i32 = -14;      // File write failure
pub const BLOSC2_ERROR_FILE_OPEN: i32 = -15;       // File open failure
pub const BLOSC2_ERROR_NOT_FOUND: i32 = -16;       // Not found
pub const BLOSC2_ERROR_RUN_LENGTH: i32 = -17;      // Bad run length encoding
pub const BLOSC2_ERROR_FILTER_PIPELINE: i32 = -18; // Filter pipeline error
pub const BLOSC2_ERROR_CHUNK_INSERT: i32 = -19;    // Chunk insert failure
pub const BLOSC2_ERROR_CHUNK_APPEND: i32 = -20;    // Chunk append failure
pub const BLOSC2_ERROR_CHUNK_UPDATE: i32 = -21;    // Chunk update failure
pub const BLOSC2_ERROR_2GB_LIMIT: i32 = -22;       // Sizes larger than 2gb not supported
pub const BLOSC2_ERROR_SCHUNK_COPY: i32 = -23;     // Super-chunk copy failure
pub const BLOSC2_ERROR_FRAME_TYPE: i32 = -24;      // Wrong type for frame
pub const BLOSC2_ERROR_FILE_TRUNCATE: i32 = -25;   // File truncate failure
pub const BLOSC2_ERROR_THREAD_CREATE: i32 = -26;   // Thread or thread context creation failure
pub const BLOSC2_ERROR_POSTFILTER: i32 = -27;      // Postfilter failure
pub const BLOSC2_ERROR_FRAME_SPECIAL: i32 = -28;   // Special frame failure
pub const BLOSC2_ERROR_SCHUNK_SPECIAL: i32 = -29;  // Special super-chunk failure
pub const BLOSC2_ERROR_PLUGIN_IO: i32 = -30;      // IO plugin error
pub const BLOSC2_ERROR_FILE_REMOVE: i32 = -31;     // Remove file failure
pub const BLOSC2_ERROR_NULL_POINTER: i32 = -32;    // Pointer is null
pub const BLOSC2_ERROR_INVALID_INDEX: i32 = -33;   // Invalid index
pub const BLOSC2_ERROR_METALAYER_NOT_FOUND: i32 = -34;   // Metalayer has not been found
pub const BLOSC2_ERROR_MAX_BUFSIZE_EXCEEDED: i32 = -35;  // Max buffer size exceeded
pub const BLOSC2_ERROR_TUNER: i32 = -36;           // Tuner failure
