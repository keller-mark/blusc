// Corresponds to c-blosc2/blosc/frame.c (and .h)

use crate::blosc::context::{Blosc2Schunk, Blosc2Context};
use crate::blosc::blosc_private::to_big;
use crate::blosc::blosc2::{blosc2_cbuffer_sizes, blosc2_decompress_ctx};
use crate::include::blosc2_include::{
    BLOSC2_VERSION_FRAME_FORMAT, BLOSC2_MAX_FILTERS, BLOSC_LAST_CODEC, 
    BLOSC_UDCODEC_FORMAT, BLOSC2_ERROR_PLUGIN_IO, BLOSC2_ERROR_FILE_OPEN,
    BLOSC2_ERROR_FILE_WRITE, BLOSC2_ERROR_DATA, BLOSC2_ERROR_READ_BUFFER,
    BLOSC2_ERROR_MEMORY_ALLOC, BLOSC2_ERROR_FILE_TRUNCATE,
    BLOSC_EXTENDED_HEADER_LENGTH, BLOSC2_ERROR_WRITE_BUFFER, BLOSC2_ERROR_FAILURE,
    BLOSC2_ERROR_INVALID_PARAM, BLOSC2_ERROR_INVALID_HEADER,
    BLOSC_MIN_HEADER_LENGTH, BLOSC2_CHUNK_BLOSC2_FLAGS, BLOSC2_CHUNK_FLAGS,
    BLOSC2_SPECIAL_MASK, BLOSC2_SPECIAL_VALUE, BLOSC_MEMCPYED
};
use crate::include::b2nd_include::BLOSC2_MAX_METALAYERS;

// Different types of frames
const FRAME_CONTIGUOUS_TYPE: u8 = 0;
const FRAME_DIRECTORY_TYPE: u8 = 1;

// Constants for metadata placement in header
const FRAME_HEADER_MAGIC: usize = 2;
const FRAME_HEADER_LEN: usize = FRAME_HEADER_MAGIC + 8 + 1; // 11
const FRAME_LEN: usize = FRAME_HEADER_LEN + 4 + 1; // 16
const FRAME_FLAGS: usize = FRAME_LEN + 8 + 1; // 25
const FRAME_TYPE: usize = FRAME_FLAGS + 1; // 26
const FRAME_CODECS: usize = FRAME_FLAGS + 2; // 27
const FRAME_OTHER_FLAGS: usize = FRAME_FLAGS + 3; // 28
const FRAME_NBYTES: usize = FRAME_FLAGS + 4 + 1; // 30
const FRAME_CBYTES: usize = FRAME_NBYTES + 8 + 1; // 39
const FRAME_TYPESIZE: usize = FRAME_CBYTES + 8 + 1; // 48
const FRAME_BLOCKSIZE: usize = FRAME_TYPESIZE + 4 + 1; // 53
const FRAME_CHUNKSIZE: usize = FRAME_BLOCKSIZE + 4 + 1; // 58
const FRAME_NTHREADS_C: usize = FRAME_CHUNKSIZE + 4 + 1; // 63
const FRAME_NTHREADS_D: usize = FRAME_NTHREADS_C + 2 + 1; // 66
const FRAME_HAS_VLMETALAYERS: usize = FRAME_NTHREADS_D + 2; // 68
const FRAME_FILTER_PIPELINE: usize = FRAME_HAS_VLMETALAYERS + 1 + 1; // 70
const FRAME_UDCODEC: usize = FRAME_FILTER_PIPELINE + 1 + 6; // 77
const FRAME_CODEC_META: usize = FRAME_FILTER_PIPELINE + 1 + 7; // 78
const FRAME_HEADER_MINLEN: usize = FRAME_FILTER_PIPELINE + 1 + 16; // 87 <- minimum length
const FRAME_METALAYERS: usize = FRAME_HEADER_MINLEN; // 87
const FRAME_IDX_SIZE: usize = FRAME_METALAYERS + 1 + 1; // 89

const FRAME_FILTER_PIPELINE_MAX: usize = 8; // the maximum number of filters that can be stored in header

const FRAME_TRAILER_VERSION_BETA2: u32 = 0; // for beta.2 and former
const FRAME_TRAILER_VERSION: u32 = 1; // can be up to 127

const FRAME_TRAILER_MINLEN: usize = 25; // minimum length for the trailer (msgpack overhead)
const FRAME_TRAILER_LEN_OFFSET: usize = 22; // offset to trailer length (counting from the end)
const FRAME_TRAILER_VLMETALAYERS: usize = 2;

/// Frame structure for managing blosc2 frames
pub struct Blosc2Frame {
    /// The name of the file or directory if it's an sframe; if None, this is in-memory
    pub urlpath: Option<String>,
    /// The in-memory, contiguous frame buffer
    pub cframe: Option<Vec<u8>>,
    /// Whether the cframe can be freed (false) or not (true)
    pub avoid_cframe_free: bool,
    /// Pointers to the (compressed, on-disk) chunk offsets
    pub coffsets: Option<Vec<u8>>,
    /// Whether the coffsets memory need to be freed or not
    pub coffsets_needs_free: bool,
    /// The current length of the frame in (compressed) bytes
    pub len: i64,
    /// The maximum length of the frame; if 0, there is no maximum
    pub maxlen: i64,
    /// The current length of the trailer in (compressed) bytes
    pub trailer_len: u32,
    /// Whether the frame is sparse (true) or not
    pub sframe: bool,
    /// The schunk associated
    pub schunk: Option<Box<Blosc2Schunk>>,
    /// The offset where the frame starts inside the file
    pub file_offset: i64,
}

/// Create a new (empty) frame
///
/// # Arguments
/// * `urlpath` - The filename of the frame. If None, this is in-memory.
///
/// # Returns
/// The new frame
pub fn frame_new(urlpath: Option<&str>) -> Blosc2Frame {
    let urlpath = urlpath.map(|s| s.to_string());
    
    Blosc2Frame {
        urlpath,
        cframe: None,
        avoid_cframe_free: false,
        coffsets: None,
        coffsets_needs_free: false,
        len: 0,
        maxlen: 0,
        trailer_len: 0,
        sframe: false,
        schunk: None,
        file_offset: 0,
    }
}

/// Free all memory from a frame.
///
/// # Arguments
/// * `frame` - The frame to be freed.
///
/// # Returns
/// 0 if succeeds.
///
/// # Note
/// In Rust, memory is automatically freed when the frame goes out of scope.
/// This function exists for API compatibility but doesn't need to do anything.
pub fn frame_free(_frame: Blosc2Frame) -> i32 {
    // In Rust, Drop trait handles cleanup automatically
    // The frame will be dropped when it goes out of scope
    0
}

/// Set `avoid_cframe_free` from frame
///
/// # Arguments
/// * `frame` - The frame to set the property to
/// * `avoid_cframe_free` - The value to set in frame
///
/// # Warning
/// If you set it to `true` you will be responsible of freeing it.
pub fn frame_avoid_cframe_free(frame: &mut Blosc2Frame, avoid_cframe_free: bool) {
    frame.avoid_cframe_free = avoid_cframe_free;
}

/// Helper struct for sorting block offsets
#[derive(Debug, Clone, Copy)]
struct CsizeIdx {
    val: i32,
    idx: i32,
}

/// Helper function for sorting block offsets
fn sort_offset(a: &CsizeIdx, b: &CsizeIdx) -> std::cmp::Ordering {
    a.val.cmp(&b.val)
}

/// Get the trailer offset in a frame
///
/// # Arguments
/// * `frame` - The frame
/// * `header_len` - The header length
/// * `has_coffsets` - Whether the frame has chunk offsets
///
/// # Returns
/// The offset to the trailer
fn get_trailer_offset(frame: &Blosc2Frame, header_len: i32, has_coffsets: bool) -> i64 {
    if !has_coffsets {
        // No data chunks yet
        return header_len as i64;
    }
    frame.len - frame.trailer_len as i64
}

/// Update the length in the header (in-memory only)
///
/// # Arguments
/// * `frame` - The frame to update
/// * `len` - The new length
///
/// # Returns
/// 1 on success, negative error code on failure
///
/// # Note
/// This implementation only handles in-memory frames (ignores file I/O for WebAssembly)
fn update_frame_len(frame: &mut Blosc2Frame, len: i64) -> i32 {
    // int update_frame_len(blosc2_frame_s* frame, int64_t len)
    if let Some(ref mut cframe) = frame.cframe {
        // if (frame->cframe != NULL) {
        //   to_big(frame->cframe + FRAME_LEN, &len, sizeof(int64_t));
        // }
        let len_bytes = len.to_be_bytes();
        cframe[FRAME_LEN..FRAME_LEN + 8].copy_from_slice(&len_bytes);
        1
    } else {
        // File-based frame - we're ignoring I/O for WebAssembly
        // In the original C code, this would write to file
        // For now, we just return success
        1
    }
    // return rc;
}

/// Remove a file:/// prefix from a URL path
///
/// This is a temporary workaround for allowing to use proper URLs for local files/dirs
///
/// # Arguments
/// * `urlpath` - The URL path to normalize
///
/// # Returns
/// The normalized path without the file:/// prefix
fn normalize_urlpath(urlpath: &str) -> &str {
    // static char* normalize_urlpath(const char* urlpath) {
    //   char* localpath = strstr(urlpath, "file:///");
    //   if (localpath == urlpath) {
    //     // There is a file:/// prefix.  Get rid of it.
    //     localpath += strlen("file:///");
    //   }
    //   else {
    //     localpath = (char*)urlpath;
    //   }
    //   return localpath;
    // }
    
    if urlpath.starts_with("file:///") {
        // There is a file:/// prefix. Get rid of it.
        &urlpath[8..] // strlen("file:///") = 8
    } else {
        urlpath
    }
}

/// Decompress and return a chunk that is part of a frame
///
/// # Arguments
/// * `dctx` - The decompression context
/// * `frame` - The frame containing the chunk
/// * `nchunk` - The chunk number
/// * `dest` - Destination buffer
/// * `nbytes` - Size of destination buffer
///
/// # Returns
/// The number of decompressed bytes, or a negative error code
///
/// # Note
/// This function assumes frame_get_lazychunk, blosc2_cbuffer_sizes, and
/// blosc2_decompress_ctx are available (will be ported later)
pub fn frame_decompress_chunk(
    dctx: &mut Blosc2Context,
    frame: &Blosc2Frame,
    nchunk: i64,
    dest: &mut [u8],
    nbytes: i32,
) -> i32 {
    // int frame_decompress_chunk(blosc2_context *dctx, blosc2_frame_s* frame, int64_t nchunk, void *dest, int32_t nbytes)
    // uint8_t* src;
    // bool needs_free;
    // int32_t chunk_nbytes;
    // int32_t chunk_cbytes;
    // int rc;

    // // Use a lazychunk here in order to do a potential parallel read.
    // rc = frame_get_lazychunk(frame, nchunk, &src, &needs_free);
    // if (rc < 0) {
    //     BLOSC_TRACE_ERROR("Cannot get the chunk in position %" PRId64 ".", nchunk);
    //     goto end;
    // }
    // chunk_cbytes = rc;
    
    let (src, _needs_free) = match frame_get_lazychunk(frame, nchunk) {
        Ok((data, needs_free)) => (data, needs_free),
        Err(rc) => {
            // BLOSC_TRACE_ERROR("Cannot get the chunk in position %" PRId64 ".", nchunk);
            return rc;
        }
    };
    
    let chunk_cbytes = src.len() as i32;
    
    // if (chunk_cbytes < (signed)sizeof(int32_t)) {
    //     /* Not enough input to read `nbytes` */
    //     rc = BLOSC2_ERROR_READ_BUFFER;
    // }
    if chunk_cbytes < std::mem::size_of::<i32>() as i32 {
        // Not enough input to read nbytes
        return BLOSC2_ERROR_READ_BUFFER;
    }

    // rc = blosc2_cbuffer_sizes(src, &chunk_nbytes, &chunk_cbytes, NULL);
    // if (rc < 0) {
    //     goto end;
    // }
    let (chunk_nbytes, chunk_cbytes) = match blosc2_cbuffer_sizes(&src) {
        Ok((nbytes, cbytes, _)) => (nbytes, cbytes),
        Err(rc) => return rc,
    };

    // /* Create a buffer for destination */
    // if (chunk_nbytes > nbytes) {
    //     BLOSC_TRACE_ERROR("Not enough space for decompressing in dest.");
    //     rc = BLOSC2_ERROR_WRITE_BUFFER;
    //     goto end;
    // }
    if chunk_nbytes > nbytes {
        // BLOSC_TRACE_ERROR("Not enough space for decompressing in dest.");
        return BLOSC2_ERROR_WRITE_BUFFER;
    }

    // /* And decompress it */
    // dctx->header_overhead = BLOSC_EXTENDED_HEADER_LENGTH;
    // int chunksize = rc = blosc2_decompress_ctx(dctx, src, chunk_cbytes, dest, nbytes);
    // if (chunksize < 0 || chunksize != chunk_nbytes) {
    //     BLOSC_TRACE_ERROR("Error in decompressing chunk.");
    //     if (chunksize >= 0)
    //         rc = BLOSC2_ERROR_FAILURE;
    // }
    dctx.header_overhead = BLOSC_EXTENDED_HEADER_LENGTH as i32;
    let chunksize = blosc2_decompress_ctx(dctx, &src, chunk_cbytes, dest, nbytes);
    
    let rc = if chunksize < 0 || chunksize != chunk_nbytes {
        // BLOSC_TRACE_ERROR("Error in decompressing chunk.");
        if chunksize >= 0 {
            BLOSC2_ERROR_FAILURE
        } else {
            chunksize
        }
    } else {
        chunksize
    };

    // end:
    // if (needs_free) {
    //     free(src);
    // }
    // return rc;
    
    // In Rust, src will be automatically dropped if needs_free is true
    // (assuming frame_get_lazychunk returns owned data when needs_free is true)
    
    rc
}

/// Get a (lazy) chunk from a frame
///
/// This is a simplified in-memory-only version that ignores filesystem operations
/// as per AGENTS.md instructions.
///
/// # Arguments
/// * `frame` - The frame
/// * `nchunk` - The chunk number
///
/// # Returns
/// `Ok((chunk_data, needs_free))` on success, where:
/// - `chunk_data` is the lazy chunk data
/// - `needs_free` indicates if the data was allocated and needs to be freed
/// `Err(error_code)` on failure
///
/// # Note
/// This implementation is incomplete and only handles the basic in-memory case.
/// It requires helper functions (get_header_info, get_coffset, frame_special_chunk)
/// that have not yet been ported from C.
///
/// # C implementation reference
/// Corresponds to frame_get_lazychunk in c-blosc2/blosc/frame.c lines 2253-2496
fn frame_get_lazychunk(_frame: &Blosc2Frame, _nchunk: i64) -> Result<(Vec<u8>, bool), i32> {
    // From C implementation (c-blosc2/blosc/frame.c):
    //
    // int frame_get_lazychunk(blosc2_frame_s *frame, int64_t nchunk, uint8_t **chunk, bool *needs_free) {
    //   int32_t header_len;
    //   int64_t frame_len;
    //   int64_t nbytes;
    //   int64_t cbytes;
    //   int32_t blocksize;
    //   int32_t chunksize;
    //   int64_t nchunks;
    //   int32_t typesize;
    //   int32_t lazychunk_cbytes;
    //   int64_t offset;
    //   void* fp = NULL;
    //
    //   *chunk = NULL;
    //   *needs_free = false;
    //   int rc = get_header_info(frame, &header_len, &frame_len, &nbytes, &cbytes,
    //                            &blocksize, &chunksize, &nchunks,
    //                            &typesize, NULL, NULL, NULL, NULL, NULL, NULL,
    //                            frame->schunk->storage->io);
    //   if (rc < 0) {
    //     BLOSC_TRACE_ERROR("Unable to get meta info from frame.");
    //     return rc;
    //   }
    //
    //   if (nchunk >= nchunks) {
    //     BLOSC_TRACE_ERROR("nchunk ('%" PRId64 "') exceeds the number of chunks "
    //                       "('%" PRId64 "') in frame.", nchunk, nchunks);
    //     return BLOSC2_ERROR_INVALID_PARAM;
    //   }
    //
    //   // Get the offset to nchunk
    //   rc = get_coffset(frame, header_len, cbytes, nchunk, nchunks, &offset);
    //   if (rc < 0) {
    //     BLOSC_TRACE_ERROR("Unable to get offset to chunk %" PRId64 ".", nchunk);
    //     return rc;
    //   }
    //
    //   if (offset < 0) {
    //     // Special value
    //     lazychunk_cbytes = BLOSC_EXTENDED_HEADER_LENGTH;
    //     int32_t chunksize_ = chunksize;
    //     if ((nchunk == nchunks - 1) && (nbytes % chunksize)) {
    //       // Last chunk is incomplete.  Compute its actual size.
    //       chunksize_ = (int32_t) (nbytes % chunksize);
    //     }
    //     rc = frame_special_chunk(offset, chunksize_, typesize, blocksize, chunk,
    //                              (int32_t)lazychunk_cbytes, needs_free);
    //     goto end;
    //   }
    //
    //   blosc2_io_cb *io_cb = blosc2_get_io_cb(frame->schunk->storage->io->id);
    //   if (io_cb == NULL) {
    //     BLOSC_TRACE_ERROR("Error getting the input/output API");
    //     rc = BLOSC2_ERROR_PLUGIN_IO;
    //     goto end;
    //   }
    //
    //   if (frame->cframe == NULL) {
    //     // File I/O path - IGNORED per AGENTS.md
    //     // [lines 2303-2464 omitted - filesystem operations]
    //   } else {
    //     // The chunk is in memory and just one pointer away
    //     int64_t chunk_header_offset = header_len + offset;
    //     int64_t chunk_cbytes_offset = chunk_header_offset + BLOSC_MIN_HEADER_LENGTH;
    //
    //     *chunk = frame->cframe + chunk_header_offset;
    //
    //     if (chunk_cbytes_offset > frame->len) {
    //       BLOSC_TRACE_ERROR("Cannot read the header for chunk in the (contiguous) frame.");
    //       rc = BLOSC2_ERROR_READ_BUFFER;
    //     } else {
    //       rc = blosc2_cbuffer_sizes(*chunk, NULL, &lazychunk_cbytes, NULL);
    //       if (rc && chunk_cbytes_offset + lazychunk_cbytes > frame_len) {
    //         BLOSC_TRACE_ERROR("Compressed bytes exceed beyond frame length.");
    //         rc = BLOSC2_ERROR_READ_BUFFER;
    //       }
    //     }
    //   }
    //
    //   end:
    //   if (fp != NULL) {
    //     io_cb->close(fp);
    //   }
    //   if (rc < 0) {
    //     if (*needs_free) {
    //       free(*chunk);
    //       *chunk = NULL;
    //       *needs_free = false;
    //     }
    //     return rc;
    //   }
    //
    //   return (int)lazychunk_cbytes;
    // }
    
    // TODO: Implement full logic once helper functions are ported:
    // - get_header_info: extracts frame metadata from header
    // - get_coffset: gets offset to chunk within frame
    // - frame_special_chunk: handles special value chunks
    // - blosc2_cbuffer_sizes: gets chunk sizes (already ported in api.rs)
    
    // For now, return error since implementation is incomplete
    Err(BLOSC2_ERROR_FAILURE)
}
