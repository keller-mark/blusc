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
#[derive(Clone, Copy)]
pub struct Blosc2Filter {
    pub _placeholder: u8,
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
    pub version: u8,
    pub compcode: u8,
    pub compcode_meta: u8,
    pub clevel: u8,
    pub splitmode: i32,
    pub typesize: i32,
    pub blocksize: i32,
    pub chunksize: i32,
    pub filters: [u8; BLOSC2_MAX_FILTERS],
    pub filters_meta: [u8; BLOSC2_MAX_FILTERS],
    pub nchunks: i64,
    pub nmetalayers: i16,
    pub metalayers: [*mut Blosc2Metalayer; B2ND_MAX_METALAYERS],
    pub nvlmetalayers: i16,
    pub vlmetalayers: [*mut Blosc2Metalayer; B2ND_MAX_METALAYERS],
    pub storage: *mut Blosc2Storage,
    pub cctx: *mut Blosc2Context,
    pub dctx: *mut Blosc2Context,
    pub frame: *mut std::ffi::c_void,
    pub tuner_params: *mut u8,
    pub tuner_id: i32,
    pub view: bool,
    pub current_nchunk: i64,
}

#[repr(C)]
pub struct Blosc2Storage {
    pub contiguous: bool,
    pub urlpath: *mut i8,
    pub cparams: *mut crate::blosc::blosc2::Blosc2Cparams,
    pub dparams: *mut crate::blosc::blosc2::Blosc2Dparams,
    pub io: *mut std::ffi::c_void,
}

#[repr(C)]
pub struct Blosc2Metalayer {
    pub name: *mut i8,
    pub content: *mut u8,
    pub content_len: i32,
}

// Constants from blosc2.h (will be properly defined in blosc2_include.rs)
pub const BLOSC2_MAX_FILTERS: usize = 6;
pub const BLOSC2_MAX_UDFILTERS: usize = 4;
pub const B2ND_MAX_DIM: usize = 8;
pub const B2ND_MAX_METALAYERS: usize = 16;
