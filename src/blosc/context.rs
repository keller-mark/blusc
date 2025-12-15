// Corresponds to c-blosc2/blosc/context.h

/// Thread context for individual thread operations.
///
/// Contains temporary buffers and codec-specific contexts for a single thread.
#[repr(C)]
pub struct ThreadContext {
    pub parent_context: *mut Blosc2Context,
    pub tid: i32,
    pub tmp: *mut u8,
    pub tmp2: *mut u8,
    pub tmp3: *mut u8,
    pub tmp4: *mut u8,
    /// The blocksize for different temporaries
    pub tmp_blocksize: i32,
    /// Keep track of how big the temporary buffers are
    pub tmp_nbytes: usize,
    /// Cell starter index for ZFP fixed-rate mode
    pub zfp_cell_start: i32,
    /// Number of items to get for ZFP fixed-rate mode
    pub zfp_cell_nitems: i32,
    // Note: ZSTD contexts and IPP hash table omitted as per AGENTS.md
    // (no external dependencies, pure Rust)
}

/// Main compression/decompression context.
///
/// Contains all parameters and state needed for blosc2 compression/decompression operations.
#[repr(C)]
pub struct Blosc2Context {
    /// The source buffer
    pub src: *const u8,
    /// The destination buffer
    pub dest: *mut u8,
    /// Flags for header
    pub header_flags: u8,
    /// Flags specific for blosc2
    pub blosc2_flags: u8,
    /// Number of bytes in source buffer
    pub sourcesize: i32,
    /// The number of bytes in chunk header
    pub header_overhead: i32,
    /// Number of total blocks in buffer
    pub nblocks: i32,
    /// Extra bytes at end of buffer
    pub leftover: i32,
    /// Length of the block in bytes
    pub blocksize: i32,
    /// Whether the blocks should be split or not
    pub splitmode: i32,
    /// Counter for the number of input bytes
    pub output_bytes: i32,
    /// Counter for the number of output bytes
    pub srcsize: i32,
    /// Maximum size for destination buffer
    pub destsize: i32,
    /// Type size
    pub typesize: i32,
    /// Starts for every block inside the compressed buffer
    pub bstarts: *mut i32,
    /// Special type for chunk. 0 if not special.
    pub special_type: i32,
    /// Compressor code to use
    pub compcode: i32,
    /// The metainfo for the compressor code
    pub compcode_meta: u8,
    /// Compression level (1-9)
    pub clevel: i32,
    /// Whether to use dicts or not
    pub use_dict: i32,
    /// The buffer to keep the trained dictionary
    pub dict_buffer: *mut u8,
    /// The size of the trained dictionary
    pub dict_size: i32,
    /// The dictionary in digested form for compression
    pub dict_cdict: *mut u8,
    /// The dictionary in digested form for decompression
    pub dict_ddict: *mut u8,
    /// The filter flags in the filter pipeline
    pub filter_flags: u8,
    /// The (sequence of) filters
    pub filters: [u8; BLOSC2_MAX_FILTERS],
    /// The metainfo for filters
    pub filters_meta: [u8; BLOSC2_MAX_FILTERS],
    /// The user-defined filters
    pub urfilters: [Blosc2Filter; BLOSC2_MAX_UDFILTERS],
    /// Prefilter function
    pub prefilter: Blosc2PrefilterFn,
    /// Postfilter function
    pub postfilter: Blosc2PostfilterFn,
    /// Prefilter params
    pub preparams: *mut Blosc2PrefilterParams,
    /// Postfilter params
    pub postparams: *mut Blosc2PostfilterParams,
    /// The blocks that are not meant to be decompressed.
    /// If NULL (default), all blocks in a chunk should be read.
    pub block_maskout: *mut bool,
    /// The number of items in block_maskout array (must match the number of blocks in chunk)
    pub block_maskout_nitems: i32,
    /// Associated super-chunk (if available)
    pub schunk: *mut Blosc2Schunk,
    /// Cache for temporaries for serial operation
    pub serial_context: *mut ThreadContext,
    /// 1 if we are compressing, 0 if decompressing
    pub do_compress: i32,
    /// Entry point for tuner persistence between runs
    pub tuner_params: *mut u8,
    /// User-defined tuner id
    pub tuner_id: i32,
    /// User defined parameters for the codec
    pub codec_params: *mut u8,
    /// User defined parameters for the filters
    pub filter_params: [*mut u8; BLOSC2_MAX_FILTERS],
    // Note: Threading fields omitted as per AGENTS.md (single-threaded only)
    /// nthreads
    pub nthreads: i16,
    /// new_nthreads
    pub new_nthreads: i16,
    /// threads_started
    pub threads_started: i16,
    /// end_threads
    pub end_threads: i16,
    /// threads (unused in single-threaded context)
    pub threads: *mut u8,
    /// Thread contexts (only for user-managed threads)
    pub thread_contexts: *mut ThreadContext,
    // Note: Mutex and barrier fields omitted as per AGENTS.md (single-threaded)
    /// error code when give up
    pub thread_giveup_code: i32,
    /// block counter
    pub thread_nblock: i32,
    /// data ref in delta not initialized
    pub dref_not_init: i32,
}

/// B2ND context structure.
///
/// General parameters needed for the creation of a b2nd array.
#[repr(C)]
pub struct B2ndContextS {
    /// The array dimensions
    pub ndim: i8,
    /// The array shape
    pub shape: [i64; B2ND_MAX_DIM],
    /// The shape of each chunk of Blosc
    pub chunkshape: [i32; B2ND_MAX_DIM],
    /// The shape of each block of Blosc
    pub blockshape: [i32; B2ND_MAX_DIM],
    /// Data type. Different formats can be supported (see dtype_format).
    pub dtype: *mut i8,
    /// The format of the data type. Default is 0 (NumPy).
    pub dtype_format: i8,
    /// The Blosc storage properties
    pub b2_storage: *mut Blosc2Storage,
    /// List with the metalayers desired
    pub metalayers: [Blosc2Metalayer; B2ND_MAX_METALAYERS],
    /// The number of metalayers
    pub nmetalayers: i32,
}

// Placeholder types that will be defined in blosc2_include.rs
#[repr(C)]
pub struct Blosc2Filter {
    _placeholder: u8,
}

pub type Blosc2PrefilterFn = *mut u8;
pub type Blosc2PostfilterFn = *mut u8;

#[repr(C)]
pub struct Blosc2PrefilterParams {
    _placeholder: u8,
}

#[repr(C)]
pub struct Blosc2PostfilterParams {
    _placeholder: u8,
}

#[repr(C)]
pub struct Blosc2Schunk {
    _placeholder: u8,
}

#[repr(C)]
pub struct Blosc2Storage {
    _placeholder: u8,
}

#[repr(C)]
pub struct Blosc2Metalayer {
    _placeholder: u8,
}

// Constants from blosc2.h (will be properly defined in blosc2_include.rs)
const BLOSC2_MAX_FILTERS: usize = 6;
const BLOSC2_MAX_UDFILTERS: usize = 4;
const B2ND_MAX_DIM: usize = 8;
const B2ND_MAX_METALAYERS: usize = 16;

/*
Original C code from c-blosc2/blosc/context.h:

#include "b2nd.h"
#include "blosc2.h"

struct blosc2_context_s {
  const uint8_t* src;  /* The source buffer */
  uint8_t* dest;  /* The destination buffer */
  uint8_t header_flags;  /* Flags for header */
  uint8_t blosc2_flags;  /* Flags specific for blosc2 */
  int32_t sourcesize;  /* Number of bytes in source buffer */
  int32_t header_overhead;  /* The number of bytes in chunk header */
  int32_t nblocks;  /* Number of total blocks in buffer */
  int32_t leftover;  /* Extra bytes at end of buffer */
  int32_t blocksize;  /* Length of the block in bytes */
  int32_t splitmode;  /* Whether the blocks should be split or not */
  int32_t output_bytes;  /* Counter for the number of input bytes */
  int32_t srcsize;  /* Counter for the number of output bytes */
  int32_t destsize;  /* Maximum size for destination buffer */
  int32_t typesize;  /* Type size */
  int32_t* bstarts;  /* Starts for every block inside the compressed buffer */
  int32_t special_type;  /* Special type for chunk.  0 if not special. */
  int compcode;  /* Compressor code to use */
  uint8_t compcode_meta;  /* The metainfo for the compressor code */
  int clevel;  /* Compression level (1-9) */
  int use_dict;  /* Whether to use dicts or not */
  void* dict_buffer;  /* The buffer to keep the trained dictionary */
  int32_t dict_size;  /* The size of the trained dictionary */
  void* dict_cdict;  /* The dictionary in digested form for compression */
  void* dict_ddict;  /* The dictionary in digested form for decompression */
  uint8_t filter_flags;  /* The filter flags in the filter pipeline */
  uint8_t filters[BLOSC2_MAX_FILTERS];  /* The (sequence of) filters */
  uint8_t filters_meta[BLOSC2_MAX_FILTERS];  /* The metainfo for filters */
  blosc2_filter urfilters[BLOSC2_MAX_UDFILTERS];  /* The user-defined filters */
  blosc2_prefilter_fn prefilter;  /* prefilter function */
  blosc2_postfilter_fn postfilter;  /* postfilter function */
  blosc2_prefilter_params *preparams;  /* prefilter params */
  blosc2_postfilter_params *postparams;  /* postfilter params */
  bool* block_maskout;  /* The blocks that are not meant to be decompressed.
                         * If NULL (default), all blocks in a chunk should be read. */
  int block_maskout_nitems;  /* The number of items in block_maskout array (must match
                              * the number of blocks in chunk) */
  blosc2_schunk* schunk;  /* Associated super-chunk (if available) */
  struct thread_context* serial_context;  /* Cache for temporaries for serial operation */
  int do_compress;  /* 1 if we are compressing, 0 if decompressing */
  void *tuner_params;  /* Entry point for tuner persistence between runs */
  int tuner_id;  /* User-defined tuner id */
  void *codec_params; /* User defined parameters for the codec */
  void *filter_params[BLOSC2_MAX_FILTERS]; /* User defined parameters for the filters */
  /* Threading */
  int16_t nthreads;
  int16_t new_nthreads;
  int16_t threads_started;
  int16_t end_threads;
  blosc2_pthread_t *threads;
  struct thread_context *thread_contexts;  /* Only for user-managed threads */
  blosc2_pthread_mutex_t count_mutex;
  blosc2_pthread_mutex_t nchunk_mutex;
#ifdef BLOSC_POSIX_BARRIERS
  pthread_barrier_t barr_init;
  pthread_barrier_t barr_finish;
#else
  int count_threads;
  blosc2_pthread_mutex_t count_threads_mutex;
  blosc2_pthread_cond_t count_threads_cv;
#endif
#if !defined(_WIN32)
  pthread_attr_t ct_attr;  /* creation time attrs for threads */
#endif
  int thread_giveup_code;  /* error code when give up */
  int thread_nblock;  /* block counter */
  int dref_not_init;  /* data ref in delta not initialized */
  blosc2_pthread_mutex_t delta_mutex;
  blosc2_pthread_cond_t delta_cv;
  // Add new fields here to avoid breaking the ABI.
};

struct b2nd_context_s {
  int8_t ndim;
  //!< The array dimensions.
  int64_t shape[B2ND_MAX_DIM];
  //!< The array shape.
  int32_t chunkshape[B2ND_MAX_DIM];
  //!< The shape of each chunk of Blosc.
  int32_t blockshape[B2ND_MAX_DIM];
  //!< The shape of each block of Blosc.
  char *dtype;
  //!< Data type. Different formats can be supported (see dtype_format).
  int8_t dtype_format;
  //!< The format of the data type.  Default is 0 (NumPy).
  blosc2_storage *b2_storage;
  //!< The Blosc storage properties
  blosc2_metalayer metalayers[B2ND_MAX_METALAYERS];
  //!< List with the metalayers desired.
  int32_t nmetalayers;
  //!< The number of metalayers.
};

struct thread_context {
  blosc2_context* parent_context;
  int tid;
  uint8_t* tmp;
  uint8_t* tmp2;
  uint8_t* tmp3;
  uint8_t* tmp4;
  int32_t tmp_blocksize;  /* the blocksize for different temporaries */
  size_t tmp_nbytes;   /* keep track of how big the temporary buffers are */
  int32_t zfp_cell_start;  /* cell starter index for ZFP fixed-rate mode */
  int32_t zfp_cell_nitems;  /* number of items to get for ZFP fixed-rate mode */
#if defined(HAVE_ZSTD)
  /* The contexts for ZSTD */
  ZSTD_CCtx* zstd_cctx;
  ZSTD_DCtx* zstd_dctx;
#endif /* HAVE_ZSTD */
#ifdef HAVE_IPP
  Ipp8u* lz4_hash_table;
#endif
};

*/