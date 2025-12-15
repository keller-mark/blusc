// Corresponds to c-blosc2/blosc/frame.c (and .h)

use crate::blosc::context::Blosc2Schunk;
use crate::blosc::blosc_private::to_big;
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

/*

#include "blosc2.h"

// Different types of frames
#define FRAME_CONTIGUOUS_TYPE 0
#define FRAME_DIRECTORY_TYPE 1


// Constants for metadata placement in header
#define FRAME_HEADER_MAGIC 2
#define FRAME_HEADER_LEN (FRAME_HEADER_MAGIC + 8 + 1)  // 11
#define FRAME_LEN (FRAME_HEADER_LEN + 4 + 1)  // 16
#define FRAME_FLAGS (FRAME_LEN + 8 + 1)  // 25
#define FRAME_TYPE (FRAME_FLAGS + 1)  // 26
#define FRAME_CODECS (FRAME_FLAGS + 2)  // 27
#define FRAME_OTHER_FLAGS (FRAME_FLAGS + 3)  // 28
#define FRAME_NBYTES (FRAME_FLAGS + 4 + 1)  // 30
#define FRAME_CBYTES (FRAME_NBYTES + 8 + 1)  // 39
#define FRAME_TYPESIZE (FRAME_CBYTES + 8 + 1) // 48
#define FRAME_BLOCKSIZE (FRAME_TYPESIZE + 4 + 1)  // 53
#define FRAME_CHUNKSIZE (FRAME_BLOCKSIZE + 4 + 1)  // 58
#define FRAME_NTHREADS_C (FRAME_CHUNKSIZE + 4 + 1)  // 63
#define FRAME_NTHREADS_D (FRAME_NTHREADS_C + 2 + 1)  // 66
#define FRAME_HAS_VLMETALAYERS (FRAME_NTHREADS_D + 2)  // 68
#define FRAME_FILTER_PIPELINE (FRAME_HAS_VLMETALAYERS + 1 + 1) // 70
#define FRAME_UDCODEC (FRAME_FILTER_PIPELINE + 1 + 6) // 77
#define FRAME_CODEC_META (FRAME_FILTER_PIPELINE + 1 + 7) // 78
#define FRAME_HEADER_MINLEN (FRAME_FILTER_PIPELINE + 1 + 16)  // 87 <- minimum length
#define FRAME_METALAYERS (FRAME_HEADER_MINLEN)  // 87
#define FRAME_IDX_SIZE (FRAME_METALAYERS + 1 + 1)  // 89

#define FRAME_FILTER_PIPELINE_MAX (8)  // the maximum number of filters that can be stored in header

#define FRAME_TRAILER_VERSION_BETA2 (0U)  // for beta.2 and former
#define FRAME_TRAILER_VERSION (1U)        // can be up to 127

#define FRAME_TRAILER_MINLEN (25)  // minimum length for the trailer (msgpack overhead)
#define FRAME_TRAILER_LEN_OFFSET (22)  // offset to trailer length (counting from the end)
#define FRAME_TRAILER_VLMETALAYERS (2)


typedef struct {
  char* urlpath;            //!< The name of the file or directory if it's an sframe; if NULL, this is in-memory
  uint8_t* cframe;          //!< The in-memory, contiguous frame buffer
  bool avoid_cframe_free;   //!< Whether the cframe can be freed (false) or not (true).
  uint8_t* coffsets;        //!< Pointers to the (compressed, on-disk) chunk offsets
  bool coffsets_needs_free; //!< Whether the coffsets memory need to be freed or not.
  int64_t len;              //!< The current length of the frame in (compressed) bytes
  int64_t maxlen;           //!< The maximum length of the frame; if 0, there is no maximum
  uint32_t trailer_len;     //!< The current length of the trailer in (compressed) bytes
  bool sframe;              //!< Whether the frame is sparse (true) or not
  blosc2_schunk *schunk;    //!< The schunk associated
  int64_t file_offset;      //!< The offset where the frame starts inside the file
} blosc2_frame_s;


/*********************************************************************
  Frame struct related functions.
  These are rather low-level and the blosc2_schunk interface is
  recommended instead.
*********************************************************************/

/**
 * @brief Create a new frame.
 *
 * @param urlpath The filename of the frame.  If not persistent, pass NULL.
 *
 * @return The new frame.
 */
blosc2_frame_s* frame_new(const char* urlpath);

/**
 * @brief Create a frame from a super-chunk.
 *
 * @param schunk The super-chunk from where the frame will be created.
 * @param frame The pointer for the frame that will be populated.
 *
 * @note If frame->urlpath is NULL, a frame is created in-memory; else it is created
 * on-disk.
 *
 * @return The size in bytes of the frame. If an error occurs it returns a negative value.
 */
int64_t frame_from_schunk(blosc2_schunk* schunk, blosc2_frame_s* frame);

/**
 * @brief Set `avoid_cframe_free` from @param frame to @param avoid_cframe_free.
 *
 * @param frame The frame to set the property to.
 * @param avoid_cframe_free The value to set in @param frame.
 * @warning If you set it to `true` you will be responsible of freeing it.
 */
void frame_avoid_cframe_free(blosc2_frame_s* frame, bool avoid_cframe_free);

/**
 * @brief Free all memory from a frame.
 *
 * @param frame The frame to be freed.
 *
 * @return 0 if succeeds.
 */
int frame_free(blosc2_frame_s *frame);

/**
 * @brief Initialize a frame out of a file.
 *
 * @param urlpath The file name.
 *
 * @return The frame created from the file.
 */
blosc2_frame_s* frame_from_file_offset(const char *urlpath, const blosc2_io *io_cb, int64_t offset);

/**
 * @brief Initialize a frame out of a contiguous frame buffer.
 *
 * @param cframe The buffer for the frame.
 * @param len The length of buffer for the frame.
 * @param copy Whether the frame buffer should be copied internally or not.
 *
 * @return The frame created from the contiguous frame buffer.
 *
 * @note The user is responsible to `free` the returned frame.
 */
blosc2_frame_s* frame_from_cframe(uint8_t *cframe, int64_t len, bool copy);

/**
 * @brief Create a super-chunk from a frame.
 *
 * @param frame The frame from which the super-chunk will be created.
 * @param copy If true, a new frame buffer is created
 * internally to serve as storage for the super-chunk. Else, the
 * super-chunk will be backed by @p frame (i.e. no copies are made).
 *
 * @return The super-chunk corresponding to the frame.
 */
blosc2_schunk* frame_to_schunk(blosc2_frame_s* frame, bool copy, const blosc2_io *udio);

blosc2_storage *
get_new_storage(const blosc2_storage *storage, const blosc2_cparams *cdefaults, const blosc2_dparams *ddefaults,
                const blosc2_io *iodefaults);

void* frame_append_chunk(blosc2_frame_s* frame, void* chunk, blosc2_schunk* schunk);
void* frame_insert_chunk(blosc2_frame_s* frame, int64_t nchunk, void* chunk, blosc2_schunk* schunk);
void* frame_update_chunk(blosc2_frame_s* frame, int64_t nchunk, void* chunk, blosc2_schunk* schunk);
void* frame_delete_chunk(blosc2_frame_s* frame, int64_t nchunk, blosc2_schunk* schunk);
int frame_reorder_offsets(blosc2_frame_s *frame, const int64_t *offsets_order, blosc2_schunk* schunk);

int frame_get_chunk(blosc2_frame_s* frame, int64_t nchunk, uint8_t **chunk, bool *needs_free);
int frame_get_lazychunk(blosc2_frame_s* frame, int64_t nchunk, uint8_t **chunk, bool *needs_free);
int frame_decompress_chunk(blosc2_context* dctx, blosc2_frame_s* frame, int64_t nchunk,
                           void *dest, int32_t nbytes);

int frame_update_header(blosc2_frame_s* frame, blosc2_schunk* schunk, bool new);
int frame_update_trailer(blosc2_frame_s* frame, blosc2_schunk* schunk);

int64_t frame_fill_special(blosc2_frame_s* frame, int64_t nitems, int special_value,
                       int32_t chunksize, blosc2_schunk* schunk);


#include "sframe.h"
#include "context.h"
#include "blosc-private.h"
#include "blosc2.h"

/* Create a new (empty) frame */
blosc2_frame_s* frame_new(const char* urlpath) {
  blosc2_frame_s* new_frame = calloc(1, sizeof(blosc2_frame_s));
  if (urlpath != NULL) {
    char* new_urlpath = malloc(strlen(urlpath) + 1);  // + 1 for the trailing NULL
    new_frame->urlpath = strcpy(new_urlpath, urlpath);
    new_frame->file_offset = 0;
  }
  return new_frame;
}


/* Free memory from a frame. */
int frame_free(blosc2_frame_s* frame) {

  if (frame->cframe != NULL && !frame->avoid_cframe_free) {
    free(frame->cframe);
  }

  if (frame->coffsets != NULL && frame->coffsets_needs_free) {
    free(frame->coffsets);
  }

  if (frame->urlpath != NULL) {
    free(frame->urlpath);
  }

  free(frame);

  return 0;
}


void *new_header_frame(blosc2_schunk *schunk, blosc2_frame_s *frame) {
  if (frame == NULL) {
    return NULL;
  }
  uint8_t* h2 = calloc(FRAME_HEADER_MINLEN, 1);
  uint8_t* h2p = h2;

  // The msgpack header starts here
  *h2p = 0x90;  // fixarray...
  *h2p += 14;   // ...with 13 elements
  h2p += 1;

  // Magic number
  *h2p = 0xa0 + 8;  // str with 8 elements
  h2p += 1;
  if (h2p - h2 >= FRAME_HEADER_MINLEN) {
    return NULL;
  }
  strcpy((char*)h2p, "b2frame");
  h2p += 8;

  // Header size
  *h2p = 0xd2;  // int32
  h2p += 1 + 4;
  if (h2p - h2 >= FRAME_HEADER_MINLEN) {
    return NULL;
  }

  // Total frame size
  *h2p = 0xcf;  // uint64
  // Fill it with frame->len which is known *after* the creation of the frame (e.g. when updating the header)
  int64_t flen = frame->len;
  to_big(h2 + FRAME_LEN, &flen, sizeof(flen));
  h2p += 1 + 8;
  if (h2p - h2 >= FRAME_HEADER_MINLEN) {
    return NULL;
  }

  // Flags
  *h2p = 0xa0 + 4;  // str with 4 elements
  h2p += 1;
  if (h2p - h2 >= FRAME_HEADER_MINLEN) {
    return NULL;
  }
  // General flags
  *h2p = BLOSC2_VERSION_FRAME_FORMAT;  // version
  *h2p += 0x10;  // 64-bit offsets.  We only support this for now.
  h2p += 1;
  if (h2p - h2 >= FRAME_HEADER_MINLEN) {
    return NULL;
  }

  // Frame type
  // We only support contiguous and sparse directories frames currently
  *h2p = frame->sframe ? 1 : 0;
  h2p += 1;
  if (h2p - h2 >= FRAME_HEADER_MINLEN) {
    return NULL;
  }

  // Codec flags
  *h2p = schunk->compcode;
  if (schunk->compcode >= BLOSC_LAST_CODEC) {
    *h2p = BLOSC_UDCODEC_FORMAT;
  }
  *h2p += (schunk->clevel) << 4u;  // clevel
  h2p += 1;
  if (h2p - h2 >= FRAME_HEADER_MINLEN) {
    return NULL;
  }

  // Other flags
  *h2p = schunk->splitmode - 1;
  h2p += 1;
  if (h2p - h2 >= FRAME_HEADER_MINLEN) {
    return NULL;
  }

  // Uncompressed size
  *h2p = 0xd3;  // int64
  h2p += 1;
  int64_t nbytes = schunk->nbytes;
  to_big(h2p, &nbytes, sizeof(nbytes));
  h2p += 8;
  if (h2p - h2 >= FRAME_HEADER_MINLEN) {
    return NULL;
  }

  // Compressed size
  *h2p = 0xd3;  // int64
  h2p += 1;
  int64_t cbytes = schunk->cbytes;
  to_big(h2p, &cbytes, sizeof(cbytes));
  h2p += 8;
  if (h2p - h2 >= FRAME_HEADER_MINLEN) {
    return NULL;
  }

  // Type size
  *h2p = 0xd2;  // int32
  h2p += 1;
  int32_t typesize = schunk->typesize;
  to_big(h2p, &typesize, sizeof(typesize));
  h2p += 4;
  if (h2p - h2 >= FRAME_HEADER_MINLEN) {
    return NULL;
  }

  // Block size
  *h2p = 0xd2;  // int32
  h2p += 1;
  int32_t blocksize = schunk->blocksize;
  to_big(h2p, &blocksize, sizeof(blocksize));
  h2p += 4;
  if (h2p - h2 >= FRAME_HEADER_MINLEN) {
    return NULL;
  }

  // Chunk size
  *h2p = 0xd2;  // int32
  h2p += 1;
  int32_t chunksize = schunk->chunksize;
  to_big(h2p, &chunksize, sizeof(chunksize));
  h2p += 4;
  if (h2p - h2 >= FRAME_HEADER_MINLEN) {
    return NULL;
  }

  // Number of threads for compression
  *h2p = 0xd1;  // int16
  h2p += 1;
  int16_t nthreads = (int16_t)schunk->cctx->nthreads;
  to_big(h2p, &nthreads, sizeof(nthreads));
  h2p += 2;
  if (h2p - h2 >= FRAME_HEADER_MINLEN) {
    return NULL;
  }

  // Number of threads for decompression
  *h2p = 0xd1;  // int16
  h2p += 1;
  nthreads = (int16_t)schunk->dctx->nthreads;
  to_big(h2p, &nthreads, sizeof(nthreads));
  h2p += 2;
  if (h2p - h2 >= FRAME_HEADER_MINLEN) {
    return NULL;
  }

  // The boolean for variable-length metalayers
  *h2p = (schunk->nvlmetalayers > 0) ? (uint8_t)0xc3 : (uint8_t)0xc2;
  h2p += 1;
  if (h2p - h2 >= FRAME_HEADER_MINLEN) {
    return NULL;
  }

  // The space for FRAME_FILTER_PIPELINE
  *h2p = 0xd8;  //  fixext 16
  h2p += 1;
  if (BLOSC2_MAX_FILTERS > FRAME_FILTER_PIPELINE_MAX) {
    return NULL;
  }
  // Store the filter pipeline in header
  uint8_t* mp_filters = h2 + FRAME_FILTER_PIPELINE + 1;
  uint8_t* mp_meta = h2 + FRAME_FILTER_PIPELINE + 1 + FRAME_FILTER_PIPELINE_MAX;
  for (int i = 0; i < BLOSC2_MAX_FILTERS; i++) {
      mp_filters[i] = schunk->filters[i];
      mp_meta[i] = schunk->filters_meta[i];
  }
  *h2p = (uint8_t) BLOSC2_MAX_FILTERS;
  h2p += 1;
  h2p += 16;

  // User-defined codec and codec metadata
  uint8_t* udcodec = h2 + FRAME_UDCODEC;
  *udcodec = schunk->compcode;
  uint8_t* codec_meta = h2 + FRAME_CODEC_META;
  *codec_meta = schunk->compcode_meta;

  if (h2p - h2 != FRAME_HEADER_MINLEN) {
    return NULL;
  }

  int32_t hsize = FRAME_HEADER_MINLEN;

  // Now, deal with metalayers
  uint16_t nmetalayers = schunk->nmetalayers;
  if (nmetalayers > BLOSC2_MAX_METALAYERS) {
    return NULL;
  }

  // Make space for the header of metalayers (array marker, size, map of offsets)
  h2 = realloc(h2, (size_t)hsize + 1 + 1 + 2 + 1 + 2);
  h2p = h2 + hsize;

  // The msgpack header for the metalayers (array_marker, size, map of offsets, list of metalayers)
  *h2p = 0x90 + 3;  // array with 3 elements
  h2p += 1;

  // Size for the map (index) of offsets, including this uint16 size (to be filled out later on)
  *h2p = 0xcd;  // uint16
  h2p += 1 + 2;

  // Map (index) of offsets for optional metalayers
  *h2p = 0xde;  // map 16 with N keys
  h2p += 1;
  to_big(h2p, &nmetalayers, sizeof(nmetalayers));
  h2p += sizeof(nmetalayers);
  int32_t current_header_len = (int32_t)(h2p - h2);
  int32_t *offtooff = malloc(nmetalayers * sizeof(int32_t));
  for (int nmetalayer = 0; nmetalayer < nmetalayers; nmetalayer++) {
    if (frame == NULL) {
      return NULL;
    }
    blosc2_metalayer *metalayer = schunk->metalayers[nmetalayer];
    uint8_t namelen = (uint8_t) strlen(metalayer->name);
    h2 = realloc(h2, (size_t)current_header_len + 1 + namelen + 1 + 4);
    h2p = h2 + current_header_len;
    // Store the metalayer
    if (namelen >= (1U << 5U)) {  // metalayer strings cannot be longer than 32 bytes
      free(offtooff);
      return NULL;
    }
    *h2p = (uint8_t)0xa0 + namelen;  // str
    h2p += 1;
    memcpy(h2p, metalayer->name, namelen);
    h2p += namelen;
    // Space for storing the offset for the value of this metalayer
    *h2p = 0xd2;  // int32
    h2p += 1;
    offtooff[nmetalayer] = (int32_t)(h2p - h2);
    h2p += 4;
    current_header_len += 1 + namelen + 1 + 4;
  }
  int32_t hsize2 = (int32_t)(h2p - h2);
  if (hsize2 != current_header_len) {  // sanity check
    return NULL;
  }

  // Map size + int16 size
  if ((uint32_t) (hsize2 - hsize) >= (1U << 16U)) {
    return NULL;
  }
  uint16_t map_size = (uint16_t) (hsize2 - hsize);
  to_big(h2 + FRAME_IDX_SIZE, &map_size, sizeof(map_size));

  // Make space for an (empty) array
  hsize = (int32_t)(h2p - h2);
  h2 = realloc(h2, (size_t)hsize + 2 + 1 + 2);
  h2p = h2 + hsize;

  // Now, store the values in an array
  *h2p = 0xdc;  // array 16 with N elements
  h2p += 1;
  to_big(h2p, &nmetalayers, sizeof(nmetalayers));
  h2p += sizeof(nmetalayers);
  current_header_len = (int32_t)(h2p - h2);
  for (int nmetalayer = 0; nmetalayer < nmetalayers; nmetalayer++) {
    if (frame == NULL) {
      return NULL;
    }
    blosc2_metalayer *metalayer = schunk->metalayers[nmetalayer];
    h2 = realloc(h2, (size_t)current_header_len + 1 + 4 + metalayer->content_len);
    h2p = h2 + current_header_len;
    // Store the serialized contents for this metalayer
    *h2p = 0xc6;  // bin32
    h2p += 1;
    to_big(h2p, &(metalayer->content_len), sizeof(metalayer->content_len));
    h2p += 4;
    memcpy(h2p, metalayer->content, metalayer->content_len);  // buffer, no need to swap
    h2p += metalayer->content_len;
    // Update the offset now that we know it
    to_big(h2 + offtooff[nmetalayer], &current_header_len, sizeof(current_header_len));
    current_header_len += 1 + 4 + metalayer->content_len;
  }
  free(offtooff);
  hsize = (int32_t)(h2p - h2);
  if (hsize != current_header_len) {  // sanity check
    return NULL;
  }

  // Set the length of the whole header now that we know it
  to_big(h2 + FRAME_HEADER_LEN, &hsize, sizeof(hsize));

  return h2;
}


int get_header_info(blosc2_frame_s *frame, int32_t *header_len, int64_t *frame_len, int64_t *nbytes, int64_t *cbytes,
                    int32_t *blocksize, int32_t *chunksize, int64_t *nchunks, int32_t *typesize, uint8_t *compcode,
                    uint8_t *compcode_meta, uint8_t *clevel, uint8_t *filters, uint8_t *filters_meta,
                    uint8_t *splitmode, const blosc2_io *io) {
  uint8_t* framep = frame->cframe;
  uint8_t* header_ptr;
  uint8_t header[FRAME_HEADER_MINLEN];

  blosc2_io_cb *io_cb = blosc2_get_io_cb(io->id);
  if (io_cb == NULL) {
    BLOSC_TRACE_ERROR("Error getting the input/output API");
    return BLOSC2_ERROR_PLUGIN_IO;
  }

  if (frame->len <= 0) {
    return BLOSC2_ERROR_READ_BUFFER;
  }

  if (frame->cframe == NULL) {
    int64_t rbytes = 0;
    void* fp = NULL;
    int64_t io_pos = 0;
    if (frame->sframe) {
      fp = sframe_open_index(frame->urlpath, "rb", io);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return BLOSC2_ERROR_FILE_OPEN;
      }
    }
    else {
      fp = io_cb->open(frame->urlpath, "rb", io->params);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return BLOSC2_ERROR_FILE_OPEN;
      }
      io_pos = frame->file_offset;
    }
    if (io_cb->is_allocation_necessary)
      header_ptr = header;
    rbytes = io_cb->read((void**)&header_ptr, 1, FRAME_HEADER_MINLEN, io_pos, fp);
    io_cb->close(fp);
    if (rbytes != FRAME_HEADER_MINLEN) {
      return BLOSC2_ERROR_FILE_READ;
    }
    framep = header_ptr;
  }

  // Consistency check for frame type
  uint8_t frame_type = framep[FRAME_TYPE];
  if (frame->sframe) {
    if (frame_type != FRAME_DIRECTORY_TYPE) {
      return BLOSC2_ERROR_FRAME_TYPE;
    }
  } else {
    if (frame_type != FRAME_CONTIGUOUS_TYPE) {
      return BLOSC2_ERROR_FRAME_TYPE;
    }
  }

  // Fetch some internal lengths
  from_big(header_len, framep + FRAME_HEADER_LEN, sizeof(*header_len));
  if (*header_len < FRAME_HEADER_MINLEN) {
    BLOSC_TRACE_ERROR("Header length is zero or smaller than min allowed.");
    return BLOSC2_ERROR_INVALID_HEADER;
  }
  from_big(frame_len, framep + FRAME_LEN, sizeof(*frame_len));
  if (*header_len > *frame_len) {
    BLOSC_TRACE_ERROR("Header length exceeds length of the frame.");
    return BLOSC2_ERROR_INVALID_HEADER;
  }
  from_big(nbytes, framep + FRAME_NBYTES, sizeof(*nbytes));
  from_big(cbytes, framep + FRAME_CBYTES, sizeof(*cbytes));
  from_big(blocksize, framep + FRAME_BLOCKSIZE, sizeof(*blocksize));
  if (chunksize != NULL) {
    from_big(chunksize, framep + FRAME_CHUNKSIZE, sizeof(*chunksize));
  }
  if (typesize != NULL) {
    from_big(typesize, framep + FRAME_TYPESIZE, sizeof(*typesize));
    if (*typesize <= 0) {
      BLOSC_TRACE_ERROR("`typesize` cannot be zero or negative.");
      return BLOSC2_ERROR_INVALID_HEADER;
    }
  }

  // Codecs
  uint8_t frame_codecs = framep[FRAME_CODECS];
  if (clevel != NULL) {
    *clevel = frame_codecs >> 4u;
  }
  if (compcode != NULL) {
    *compcode = frame_codecs & 0xFu;
    if (*compcode == BLOSC_UDCODEC_FORMAT) {
      from_big(compcode, framep + FRAME_UDCODEC, sizeof(*compcode));
    }
  }

  // Other flags
  uint8_t other_flags = framep[FRAME_OTHER_FLAGS];
  if (splitmode != NULL) {
    *splitmode = other_flags & 0x4u;
    from_big(splitmode, framep + FRAME_OTHER_FLAGS, sizeof(*splitmode));
    *splitmode += 1;
  }

  if (compcode_meta != NULL) {
    from_big(compcode_meta, framep + FRAME_CODEC_META, sizeof(*compcode_meta));
  }

  // Filters
  if (filters != NULL && filters_meta != NULL) {
    uint8_t nfilters = framep[FRAME_FILTER_PIPELINE];
    if (nfilters > BLOSC2_MAX_FILTERS) {
      BLOSC_TRACE_ERROR("The number of filters in frame header are too large for Blosc2.");
      return BLOSC2_ERROR_INVALID_HEADER;
    }
    uint8_t *filters_ = framep + FRAME_FILTER_PIPELINE + 1;
    uint8_t *filters_meta_ = framep + FRAME_FILTER_PIPELINE + 1 + FRAME_FILTER_PIPELINE_MAX;
    for (int i = 0; i < nfilters; i++) {
      filters[i] = filters_[i];
      filters_meta[i] = filters_meta_[i];
    }
  }

  if (*nbytes > 0 && *chunksize > 0) {
    // We can compute the number of chunks only when the frame has actual data
    *nchunks = *nbytes / *chunksize;
    if (*nbytes % *chunksize > 0) {
      if (*nchunks == INT32_MAX) {
        BLOSC_TRACE_ERROR("Number of chunks exceeds maximum allowed.");
        return BLOSC2_ERROR_INVALID_HEADER;
      }
      *nchunks += 1;
    }

    // Sanity check for compressed sizes
    if ((*cbytes < 0) || ((int64_t)*nchunks * *chunksize < *nbytes)) {
      BLOSC_TRACE_ERROR("Invalid compressed size in frame header.");
      return BLOSC2_ERROR_INVALID_HEADER;
    }
  } else {
    *nchunks = 0;
  }

  return 0;
}


int64_t get_trailer_offset(blosc2_frame_s *frame, int32_t header_len, bool has_coffsets) {
  if (!has_coffsets) {
    // No data chunks yet
    return header_len;
  }
  return frame->len - frame->trailer_len;
}


// Update the length in the header
int update_frame_len(blosc2_frame_s* frame, int64_t len) {
  int rc = 1;
  blosc2_io_cb *io_cb = blosc2_get_io_cb(frame->schunk->storage->io->id);
  if (io_cb == NULL) {
    BLOSC_TRACE_ERROR("Error getting the input/output API");
    return BLOSC2_ERROR_PLUGIN_IO;
  }

  if (frame->cframe != NULL) {
    to_big(frame->cframe + FRAME_LEN, &len, sizeof(int64_t));
  }
  else {
    void* fp = NULL;
    if (frame->sframe) {
      fp = sframe_open_index(frame->urlpath, "rb+",
                             frame->schunk->storage->io);
    }
    else {
      fp = io_cb->open(frame->urlpath, "rb+", frame->schunk->storage->io->params);
    }
    if (fp == NULL) {
      BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
      return BLOSC2_ERROR_FILE_OPEN;
    }
    int64_t io_pos = frame->file_offset + FRAME_LEN;
    int64_t swap_len;
    to_big(&swap_len, &len, sizeof(int64_t));
    int64_t wbytes = io_cb->write(&swap_len, 1, sizeof(int64_t), io_pos, fp);
    io_cb->close(fp);
    if (wbytes != sizeof(int64_t)) {
      BLOSC_TRACE_ERROR("Cannot write the frame length in header.");
      return BLOSC2_ERROR_FILE_WRITE;
    }
  }
  return rc;
}


int frame_update_trailer(blosc2_frame_s* frame, blosc2_schunk* schunk) {
  if (frame != NULL && frame->len == 0) {
    BLOSC_TRACE_ERROR("The trailer cannot be updated on empty frames.");
  }

  // Create the trailer in msgpack (see the frame format document)
  uint32_t trailer_len = FRAME_TRAILER_MINLEN;
  uint8_t* trailer = (uint8_t*)calloc((size_t)trailer_len, 1);
  uint8_t* ptrailer = trailer;
  *ptrailer = 0x90 + 4;  // fixarray with 4 elements
  ptrailer += 1;
  // Trailer format version
  *ptrailer = FRAME_TRAILER_VERSION;
  ptrailer += 1;

  int32_t current_trailer_len = (int32_t)(ptrailer - trailer);

  // Now, deal with variable-length metalayers
  int16_t nvlmetalayers = schunk->nvlmetalayers;
  if (nvlmetalayers < 0 || nvlmetalayers > BLOSC2_MAX_METALAYERS) {
    return -1;
  }

  // Make space for the header of metalayers (array marker, size, map of offsets)
  trailer = realloc(trailer, (size_t) current_trailer_len + 1 + 1 + 2 + 1 + 2);
  ptrailer = trailer + current_trailer_len;

  // The msgpack header for the metalayers (array_marker, size, map of offsets, list of metalayers)
  *ptrailer = 0x90 + 3;  // array with 3 elements
  ptrailer += 1;

  int32_t tsize = (int32_t)(ptrailer - trailer);

  // Size for the map (index) of metalayer offsets, including this uint16 size (to be filled out later on)
  *ptrailer = 0xcd;  // uint16
  ptrailer += 1 + 2;

  // Map (index) of offsets for optional metalayers
  *ptrailer = 0xde;  // map 16 with N keys
  ptrailer += 1;
  to_big(ptrailer, &nvlmetalayers, sizeof(nvlmetalayers));
  ptrailer += sizeof(nvlmetalayers);
  current_trailer_len = (int32_t)(ptrailer - trailer);
  int32_t *offtodata = malloc(nvlmetalayers * sizeof(int32_t));
  for (int nvlmetalayer = 0; nvlmetalayer < nvlmetalayers; nvlmetalayer++) {
    if (frame == NULL) {
      return -1;
    }
    blosc2_metalayer *vlmetalayer = schunk->vlmetalayers[nvlmetalayer];
    uint8_t name_len = (uint8_t) strlen(vlmetalayer->name);
    trailer = realloc(trailer, (size_t)current_trailer_len + 1 + name_len + 1 + 4);
    ptrailer = trailer + current_trailer_len;
    // Store the vlmetalayer
    if (name_len >= (1U << 5U)) {  // metalayer strings cannot be longer than 32 bytes
      free(offtodata);
      return -1;
    }
    *ptrailer = (uint8_t)0xa0 + name_len;  // str
    ptrailer += 1;
    memcpy(ptrailer, vlmetalayer->name, name_len);
    ptrailer += name_len;
    // Space for storing the offset for the value of this vlmetalayer
    *ptrailer = 0xd2;  // int32
    ptrailer += 1;
    offtodata[nvlmetalayer] = (int32_t)(ptrailer - trailer);
    ptrailer += 4;
    current_trailer_len += 1 + name_len + 1 + 4;
  }
  int32_t tsize2 = (int32_t)(ptrailer - trailer);
  if (tsize2 != current_trailer_len) {  // sanity check
    return -1;
  }

  // Map size + int16 size
  if ((uint32_t) (tsize2 - tsize) >= (1U << 16U)) {
    return -1;
  }
  uint16_t map_size = (uint16_t) (tsize2 - tsize);
  to_big(trailer + 4, &map_size, sizeof(map_size));

  // Make space for an (empty) array
  tsize = (int32_t)(ptrailer - trailer);
  trailer = realloc(trailer, (size_t) tsize + 2 + 1 + 2);
  ptrailer = trailer + tsize;

  // Now, store the values in an array
  *ptrailer = 0xdc;  // array 16 with N elements
  ptrailer += 1;
  to_big(ptrailer, &nvlmetalayers, sizeof(nvlmetalayers));
  ptrailer += sizeof(nvlmetalayers);
  current_trailer_len = (int32_t)(ptrailer - trailer);
  for (int nvlmetalayer = 0; nvlmetalayer < nvlmetalayers; nvlmetalayer++) {
    if (frame == NULL) {
      return -1;
    }
    blosc2_metalayer *vlmetalayer = schunk->vlmetalayers[nvlmetalayer];
    trailer = realloc(trailer, (size_t)current_trailer_len + 1 + 4 + vlmetalayer->content_len);
    ptrailer = trailer + current_trailer_len;
    // Store the serialized contents for this vlmetalayer
    *ptrailer = 0xc6;  // bin32
    ptrailer += 1;
    to_big(ptrailer, &(vlmetalayer->content_len), sizeof(vlmetalayer->content_len));
    ptrailer += 4;
    memcpy(ptrailer, vlmetalayer->content, vlmetalayer->content_len);  // buffer, no need to swap
    ptrailer += vlmetalayer->content_len;
    // Update the offset now that we know it
    to_big(trailer + offtodata[nvlmetalayer], &current_trailer_len, sizeof(current_trailer_len));
    current_trailer_len += 1 + 4 + vlmetalayer->content_len;
  }
  free(offtodata);
  tsize = (int32_t)(ptrailer - trailer);
  if (tsize != current_trailer_len) {  // sanity check
    return -1;
  }

  trailer = realloc(trailer, (size_t)current_trailer_len + 23);
  ptrailer = trailer + current_trailer_len;
  trailer_len = (ptrailer - trailer) + 23;

  // Trailer length
  *ptrailer = 0xce;  // uint32
  ptrailer += 1;
  to_big(ptrailer, &trailer_len, sizeof(uint32_t));
  ptrailer += sizeof(uint32_t);
  // Up to 16 bytes for frame fingerprint (using XXH3 included in https://github.com/Cyan4973/xxHash)
  // Maybe someone would need 256-bit in the future, but for the time being 128-bit seems like a good tradeoff
  *ptrailer = 0xd8;  // fixext 16
  ptrailer += 1;
  *ptrailer = 0;  // fingerprint type: 0 -> no fp; 1 -> 32-bit; 2 -> 64-bit; 3 -> 128-bit
  ptrailer += 1;

  // Remove call to memset when we compute an actual fingerprint
  memset(ptrailer, 0, 16);
  // Uncomment call to memcpy when we compute an actual fingerprint
  // memcpy(ptrailer, xxh3_fingerprint, sizeof(xxh3_fingerprint));
  ptrailer += 16;

  // Sanity check
  if (ptrailer - trailer != trailer_len) {
    return BLOSC2_ERROR_DATA;
  }

  int32_t header_len;
  int64_t frame_len;
  int64_t nbytes;
  int64_t cbytes;
  int32_t blocksize;
  int32_t chunksize;
  int64_t nchunks;
  int ret = get_header_info(frame, &header_len, &frame_len, &nbytes, &cbytes,
                            &blocksize, &chunksize, &nchunks,
                            NULL, NULL, NULL, NULL, NULL, NULL, NULL,
                            frame->schunk->storage->io);
  if (ret < 0) {
    BLOSC_TRACE_ERROR("Unable to get meta info from frame.");
    return ret;
  }

  int64_t trailer_offset = get_trailer_offset(frame, header_len, nbytes > 0);

  if (trailer_offset < BLOSC_EXTENDED_HEADER_LENGTH) {
    BLOSC_TRACE_ERROR("Unable to get trailer offset in frame.");
    return BLOSC2_ERROR_READ_BUFFER;
  }

  blosc2_io_cb *io_cb = blosc2_get_io_cb(frame->schunk->storage->io->id);
  if (io_cb == NULL) {
    BLOSC_TRACE_ERROR("Error getting the input/output API");
    return BLOSC2_ERROR_PLUGIN_IO;
  }
  // Update the trailer.  As there are no internal offsets to the trailer section,
  // and it is always at the end of the frame, we can just write (or overwrite) it
  // at the end of the frame.
  if (frame->cframe != NULL) {
    frame->cframe = realloc(frame->cframe, (size_t)(trailer_offset + trailer_len));
    if (frame->cframe == NULL) {
      BLOSC_TRACE_ERROR("Cannot realloc space for the frame.");
      return BLOSC2_ERROR_MEMORY_ALLOC;
    }
    memcpy(frame->cframe + trailer_offset, trailer, trailer_len);
  }
  else {
    void* fp = NULL;
    if (frame->sframe) {
      fp = sframe_open_index(frame->urlpath, "rb+",
                             frame->schunk->storage->io);
    }
    else {
      fp = io_cb->open(frame->urlpath, "rb+", frame->schunk->storage->io->params);
    }
    if (fp == NULL) {
      BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
      return BLOSC2_ERROR_FILE_OPEN;
    }
    int64_t io_pos = frame->file_offset + trailer_offset;
    int64_t wbytes = io_cb->write(trailer, 1, trailer_len, io_pos, fp);
    if (wbytes != trailer_len) {
      BLOSC_TRACE_ERROR("Cannot write the trailer length in trailer.");
      return BLOSC2_ERROR_FILE_WRITE;
    }
    if (io_cb->truncate(fp, trailer_offset + trailer_len) != 0) {
      BLOSC_TRACE_ERROR("Cannot truncate the frame.");
      return BLOSC2_ERROR_FILE_TRUNCATE;
    }
    io_cb->close(fp);

  }
  free(trailer);

  int rc = update_frame_len(frame, trailer_offset + trailer_len);
  if (rc < 0) {
    return rc;
  }
  frame->len = trailer_offset + trailer_len;
  frame->trailer_len = trailer_len;

  return 1;
}


// Remove a file:/// prefix
// This is a temporary workaround for allowing to use proper URLs for local files/dirs
static char* normalize_urlpath(const char* urlpath) {
  char* localpath = strstr(urlpath, "file:///");
  if (localpath == urlpath) {
    // There is a file:/// prefix.  Get rid of it.
    localpath += strlen("file:///");
  }
  else {
    localpath = (char*)urlpath;
  }
  return localpath;
}


/* Initialize a frame out of a file */
blosc2_frame_s* frame_from_file_offset(const char* urlpath, const blosc2_io *io, int64_t offset) {
    // Get the length of the frame
    uint8_t* header_ptr;
    uint8_t header[FRAME_HEADER_MINLEN];
    uint8_t* trailer_ptr;
    uint8_t trailer[FRAME_TRAILER_MINLEN];

    void* fp = NULL;
    bool sframe = false;
    struct stat path_stat;

    urlpath = normalize_urlpath(urlpath);

    if(stat(urlpath, &path_stat) < 0) {
        BLOSC_TRACE_ERROR("Cannot get information about the path %s.", urlpath);
        return NULL;
    }

    blosc2_io_cb *io_cb = blosc2_get_io_cb(io->id);
    if (io_cb == NULL) {
        BLOSC_TRACE_ERROR("Error getting the input/output API");
        return NULL;
    }

    char* urlpath_cpy;
    if (path_stat.st_mode & S_IFDIR) {
        urlpath_cpy = malloc(strlen(urlpath) + 1);
        strcpy(urlpath_cpy, urlpath);
        char last_char = urlpath[strlen(urlpath) - 1];
        if (last_char == '\\' || last_char == '/') {
            urlpath_cpy[strlen(urlpath) - 1] = '\0';
        }
        else {
        }
        fp = sframe_open_index(urlpath_cpy, "rb", io);
        sframe = true;
    }
    else {
        urlpath_cpy = malloc(strlen(urlpath) + 1);
        strcpy(urlpath_cpy, urlpath);
        fp = io_cb->open(urlpath, "rb", io->params);
    }
    if (fp == NULL) {
      BLOSC_TRACE_ERROR("Error opening file in: %s", urlpath);
      return NULL;
    }

    if (io_cb->is_allocation_necessary)
      header_ptr = header;
    int64_t io_pos = offset;
    int64_t rbytes = io_cb->read((void**)&header_ptr, 1, FRAME_HEADER_MINLEN, io_pos, fp);
    if (rbytes != FRAME_HEADER_MINLEN) {
        BLOSC_TRACE_ERROR("Cannot read from file '%s'.", urlpath);
        io_cb->close(fp);
        free(urlpath_cpy);
        return NULL;
    }
    int64_t frame_len;
    to_big(&frame_len, header_ptr + FRAME_LEN, sizeof(frame_len));

    blosc2_frame_s* frame = calloc(1, sizeof(blosc2_frame_s));
    frame->urlpath = urlpath_cpy;
    frame->len = frame_len;
    frame->sframe = sframe;
    frame->file_offset = offset;

    // Now, the trailer length
    if (io_cb->is_allocation_necessary)
      trailer_ptr = trailer;
    io_pos = offset + frame_len - FRAME_TRAILER_MINLEN;
    rbytes = io_cb->read((void**)&trailer_ptr, 1, FRAME_TRAILER_MINLEN, io_pos, fp);
    io_cb->close(fp);
    if (rbytes != FRAME_TRAILER_MINLEN) {
        BLOSC_TRACE_ERROR("Cannot read from file '%s'.", urlpath);
        free(urlpath_cpy);
        free(frame);
        return NULL;
    }
    int trailer_offset = FRAME_TRAILER_MINLEN - FRAME_TRAILER_LEN_OFFSET;
    if (trailer_ptr[trailer_offset - 1] != 0xce) {
        BLOSC_TRACE_ERROR("Invalid trailer in file '%s'.", urlpath);
        free(urlpath_cpy);
        free(frame);
        return NULL;
    }
    uint32_t trailer_len;
    to_big(&trailer_len, trailer_ptr + trailer_offset, sizeof(trailer_len));
    frame->trailer_len = trailer_len;

    return frame;
}


/* Initialize a frame out of a contiguous frame buffer */
blosc2_frame_s* frame_from_cframe(uint8_t *cframe, int64_t len, bool copy) {
  // Get the length of the frame
  const uint8_t* header = cframe;
  int64_t frame_len;
  if (len < FRAME_HEADER_MINLEN) {
    return NULL;
  }

  from_big(&frame_len, header + FRAME_LEN, sizeof(frame_len));
  if (frame_len != len) {   // sanity check
    return NULL;
  }

  blosc2_frame_s* frame = calloc(1, sizeof(blosc2_frame_s));
  frame->len = frame_len;
  frame->file_offset = 0;

  // Now, the trailer length
  const uint8_t* trailer = cframe + frame_len - FRAME_TRAILER_MINLEN;
  int trailer_offset = FRAME_TRAILER_MINLEN - FRAME_TRAILER_LEN_OFFSET;
  if (trailer[trailer_offset - 1] != 0xce) {
    free(frame);
    return NULL;
  }
  uint32_t trailer_len;
  from_big(&trailer_len, trailer + trailer_offset, sizeof(trailer_len));
  frame->trailer_len = trailer_len;

  if (copy) {
    frame->cframe = malloc((size_t)len);
    memcpy(frame->cframe, cframe, (size_t)len);
  }
  else {
    frame->cframe = cframe;
    frame->avoid_cframe_free = true;
  }

  return frame;
}


/* Create a frame out of a super-chunk. */
int64_t frame_from_schunk(blosc2_schunk *schunk, blosc2_frame_s *frame) {
  frame->file_offset = 0;
  int64_t nchunks = schunk->nchunks;
  int64_t cbytes = schunk->cbytes;
  int32_t chunk_cbytes;
  int32_t chunk_nbytes;
  void* fp = NULL;
  int rc;

  uint8_t* h2 = new_header_frame(schunk, frame);
  if (h2 == NULL) {
    return BLOSC2_ERROR_DATA;
  }
  uint32_t h2len;
  from_big(&h2len, h2 + FRAME_HEADER_LEN, sizeof(h2len));
  // Build the offsets chunk
  int32_t chunksize = -1;
  int32_t off_cbytes = 0;
  uint64_t coffset = 0;
  int32_t off_nbytes = (int32_t) (nchunks * sizeof(int64_t));
  uint64_t* data_tmp = malloc(off_nbytes);
  bool needs_free = false;
  for (int i = 0; i < nchunks; i++) {
    uint8_t* data_chunk;
    data_chunk = schunk->data[i];
    rc = blosc2_cbuffer_sizes(data_chunk, &chunk_nbytes, &chunk_cbytes, NULL);
    if (rc < 0) {
      return rc;
    }
    data_tmp[i] = coffset;
    coffset += chunk_cbytes;
    int32_t chunksize_ = chunk_nbytes;
    if (i == 0) {
      chunksize = chunksize_;
    }
    else if (chunksize != chunksize_) {
      // Variable size  // TODO: update flags for this (or do not use them at all)
      chunksize = 0;
    }
    if (needs_free) {
      free(data_chunk);
    }
  }
  if ((int64_t)coffset != cbytes) {
    free(data_tmp);
    return BLOSC2_ERROR_DATA;
  }
  uint8_t *off_chunk = NULL;
  if (nchunks > 0) {
    // Compress the chunk of offsets
    off_chunk = malloc(off_nbytes + BLOSC2_MAX_OVERHEAD);
    blosc2_context *cctx = blosc2_create_cctx(BLOSC2_CPARAMS_DEFAULTS);
    if (cctx == NULL) {
      BLOSC_TRACE_ERROR("Error while creating the compression context");
      return BLOSC2_ERROR_NULL_POINTER;
    }
    cctx->typesize = sizeof(int64_t);
    off_cbytes = blosc2_compress_ctx(cctx, data_tmp, off_nbytes, off_chunk,
                                     off_nbytes + BLOSC2_MAX_OVERHEAD);
    blosc2_free_ctx(cctx);
    if (off_cbytes < 0) {
      free(off_chunk);
      free(h2);
      return off_cbytes;
    }
  }
  else {
    off_cbytes = 0;
  }
  free(data_tmp);

  // Now that we know them, fill the chunksize and frame length in header
  to_big(h2 + FRAME_CHUNKSIZE, &chunksize, sizeof(chunksize));
  frame->len = h2len + cbytes + off_cbytes + FRAME_TRAILER_MINLEN;
  if (frame->sframe) {
    frame->len = h2len + off_cbytes + FRAME_TRAILER_MINLEN;
  }
  int64_t tbytes = frame->len;
  to_big(h2 + FRAME_LEN, &tbytes, sizeof(tbytes));

  blosc2_io_cb *io_cb = blosc2_get_io_cb(frame->schunk->storage->io->id);
  if (io_cb == NULL) {
    BLOSC_TRACE_ERROR("Error getting the input/output API");
    return BLOSC2_ERROR_PLUGIN_IO;
  }

  // Create the frame and put the header at the beginning
  int64_t io_pos = 0;
  if (frame->urlpath == NULL) {
    frame->cframe = malloc((size_t)frame->len);
    memcpy(frame->cframe, h2, h2len);
  }
  else {
    if (frame->sframe) {
      fp = sframe_open_index(frame->urlpath, "wb",
                             frame->schunk->storage->io);
    }
    else {
      fp = io_cb->open(frame->urlpath, "wb", frame->schunk->storage->io->params);
    }
    if (fp == NULL) {
      BLOSC_TRACE_ERROR("Error creating file in: %s", frame->urlpath);
      return BLOSC2_ERROR_FILE_OPEN;
    }
    io_cb->write(h2, h2len, 1, io_pos, fp);
    io_pos += h2len;
  }
  free(h2);

  // Fill the frame with the actual data chunks
  if (!frame->sframe) {
    coffset = 0;
    for (int i = 0; i < nchunks; i++) {
      uint8_t* data_chunk = schunk->data[i];
      rc = blosc2_cbuffer_sizes(data_chunk, NULL, &chunk_cbytes, NULL);
      if (rc < 0) {
        return rc;
      }
      if (frame->urlpath == NULL) {
        memcpy(frame->cframe + h2len + coffset, data_chunk, (size_t)chunk_cbytes);
      } else {
        io_cb->write(data_chunk, chunk_cbytes, 1, io_pos, fp);
        io_pos += chunk_cbytes;
      }
      coffset += chunk_cbytes;
    }
    if ((int64_t)coffset != cbytes) {
      return BLOSC2_ERROR_FAILURE;
    }
  }

  // Copy the offsets chunk at the end of the frame
  if (frame->urlpath == NULL) {
    memcpy(frame->cframe + h2len + cbytes, off_chunk, off_cbytes);
  }
  else {
    io_cb->write(off_chunk, off_cbytes, 1, io_pos, fp);
    io_cb->close(fp);
  }
  free(off_chunk);
  rc = frame_update_trailer(frame, schunk);
  if (rc < 0) {
    return rc;
  }

  return frame->len;
}


// Get the compressed data offsets
uint8_t* get_coffsets(blosc2_frame_s *frame, int32_t header_len, int64_t cbytes,
                      int64_t nchunks, int32_t *off_cbytes) {
  int32_t chunk_cbytes;
  int rc;

  if (frame->coffsets != NULL) {
    if (off_cbytes != NULL) {
      rc = blosc2_cbuffer_sizes(frame->coffsets, NULL, &chunk_cbytes, NULL);
      if (rc < 0) {
        return NULL;
      }
      *off_cbytes = (int32_t)chunk_cbytes;
    }
    return frame->coffsets;
  }
  if (frame->cframe != NULL) {
    int64_t off_pos = header_len;
    if (cbytes < INT64_MAX - header_len) {
      off_pos += cbytes;
    }
    // Check that there is enough room to read Blosc header
    if (off_pos < 0 || off_pos > INT64_MAX - BLOSC_EXTENDED_HEADER_LENGTH ||
        off_pos + BLOSC_EXTENDED_HEADER_LENGTH > frame->len) {
      BLOSC_TRACE_ERROR("Cannot read the offsets outside of frame boundary.");
      return NULL;
    }
    // For in-memory frames, the coffset is just one pointer away
    uint8_t* off_start = frame->cframe + off_pos;
    if (off_cbytes != NULL) {
      int32_t chunk_nbytes;
      int32_t chunk_blocksize;
      rc = blosc2_cbuffer_sizes(off_start, &chunk_nbytes, &chunk_cbytes, &chunk_blocksize);
      if (rc < 0) {
        return NULL;
      }
      *off_cbytes = (int32_t)chunk_cbytes;
      if (*off_cbytes < 0 || off_pos + *off_cbytes > frame->len) {
        BLOSC_TRACE_ERROR("Cannot read the cbytes outside of frame boundary.");
        return NULL;
      }
      if ((uint64_t)chunk_nbytes != nchunks * sizeof(int64_t)) {
        BLOSC_TRACE_ERROR("The number of chunks in offset idx "
                          "does not match the ones in the header frame.");
        return NULL;
      }

    }
    return off_start;
  }

  int64_t trailer_offset = get_trailer_offset(frame, header_len, true);

  if (trailer_offset < BLOSC_EXTENDED_HEADER_LENGTH || trailer_offset + FRAME_TRAILER_MINLEN > frame->len) {
    BLOSC_TRACE_ERROR("Cannot read the trailer out of the frame.");
    return NULL;
  }

  int32_t coffsets_cbytes;
  if (frame->sframe) {
    coffsets_cbytes = (int32_t)(trailer_offset - (header_len + 0));
  }
  else {
    coffsets_cbytes = (int32_t)(trailer_offset - (header_len + cbytes));
  }

  if (off_cbytes != NULL) {
    *off_cbytes = coffsets_cbytes;
  }

  blosc2_io_cb *io_cb = blosc2_get_io_cb(frame->schunk->storage->io->id);
  if (io_cb == NULL) {
    BLOSC_TRACE_ERROR("Error getting the input/output API");
    return NULL;
  }

  void* fp = NULL;
  uint8_t* coffsets;
  if (io_cb->is_allocation_necessary) {
    coffsets = malloc((size_t)coffsets_cbytes);
    frame->coffsets_needs_free = true;
  }
  else {
    frame->coffsets_needs_free = false;
  }
  
  int64_t io_pos = 0;
  if (frame->sframe) {
    fp = sframe_open_index(frame->urlpath, "rb",
                           frame->schunk->storage->io);
    if (fp == NULL) {
      BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
      return NULL;
    }
    io_pos = header_len + 0;
  }
  else {
    fp = io_cb->open(frame->urlpath, "rb", frame->schunk->storage->io->params);
    if (fp == NULL) {
      BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
      return NULL;
    }
    io_pos = frame->file_offset + header_len + cbytes;
  }
  int64_t rbytes = io_cb->read((void**)&coffsets, 1, coffsets_cbytes, io_pos, fp);
  io_cb->close(fp);
  if (rbytes != coffsets_cbytes) {
    BLOSC_TRACE_ERROR("Cannot read the offsets out of the frame.");
    if (frame->coffsets_needs_free)
      free(coffsets);
    return NULL;
  }
  frame->coffsets = coffsets;
  return coffsets;
}


// Get the data offsets from a frame
int64_t* blosc2_frame_get_offsets(blosc2_schunk *schunk) {
  if (schunk->frame == NULL) {
    BLOSC_TRACE_ERROR("This function needs a frame.");
    return NULL;
  }
  blosc2_frame_s* frame = (blosc2_frame_s*)schunk->frame;

  // Get header info
  int32_t header_len;
  int64_t frame_len;
  int64_t nbytes;
  int64_t cbytes;
  int32_t blocksize;
  int32_t chunksize;
  int64_t nchunks;
  int ret = get_header_info(frame, &header_len, &frame_len, &nbytes, &cbytes,
                            &blocksize, &chunksize, &nchunks,
                            NULL, NULL, NULL, NULL, NULL, NULL, NULL,
                            frame->schunk->storage->io);
  if (ret < 0) {
    BLOSC_TRACE_ERROR("Cannot get the header info for the frame.");
    return NULL;
  }

  int32_t off_nbytes = (int32_t) (nchunks * sizeof(int64_t));
  int64_t* offsets = (int64_t *) malloc((size_t)off_nbytes);

  int32_t coffsets_cbytes = 0;
  uint8_t *coffsets = get_coffsets(frame, header_len, cbytes, nchunks, &coffsets_cbytes);
  // Decompress offsets
  blosc2_dparams off_dparams = BLOSC2_DPARAMS_DEFAULTS;
  blosc2_context *dctx = blosc2_create_dctx(off_dparams);
  if (dctx == NULL) {
    BLOSC_TRACE_ERROR("Error while creating the decompression context");
    return NULL;
  }
  int32_t prev_nbytes = blosc2_decompress_ctx(dctx, coffsets, coffsets_cbytes,
                                              offsets, off_nbytes);
  blosc2_free_ctx(dctx);
  if (prev_nbytes < 0) {
    free(offsets);
    BLOSC_TRACE_ERROR("Cannot decompress the offsets chunk.");
    return NULL;
  }
  return offsets;
}


int frame_update_header(blosc2_frame_s* frame, blosc2_schunk* schunk, bool new) {
  uint8_t* framep = frame->cframe;
  uint8_t* header_ptr;
  uint8_t header[FRAME_HEADER_MINLEN];

  if (frame->len <= 0) {
    return BLOSC2_ERROR_INVALID_PARAM;
  }

  if (new && schunk->cbytes > 0) {
    BLOSC_TRACE_ERROR("New metalayers cannot be added after actual data "
                      "has been appended.");
    return BLOSC2_ERROR_INVALID_PARAM;
  }

  blosc2_io_cb *io_cb = blosc2_get_io_cb(frame->schunk->storage->io->id);
  if (io_cb == NULL) {
    BLOSC_TRACE_ERROR("Error getting the input/output API");
    return BLOSC2_ERROR_PLUGIN_IO;
  }

  if (frame->cframe == NULL) {
    int64_t rbytes = 0;
    void* fp = NULL;
    int64_t io_pos = 0;
    if (frame->sframe) {
      fp = sframe_open_index(frame->urlpath, "rb+",
                             frame->schunk->storage->io);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return BLOSC2_ERROR_FILE_OPEN;
      }
    }
    else {
      fp = io_cb->open(frame->urlpath, "rb", frame->schunk->storage->io->params);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return BLOSC2_ERROR_FILE_OPEN;
      }
      io_pos = frame->file_offset;
    }
    if (fp != NULL) {
      if (io_cb->is_allocation_necessary)
        header_ptr = header;
      rbytes = io_cb->read((void**)&header_ptr, 1, FRAME_HEADER_MINLEN, io_pos, fp);
      io_cb->close(fp);
    }
    (void) rbytes;
    if (rbytes != FRAME_HEADER_MINLEN) {
      return BLOSC2_ERROR_FILE_WRITE;
    }
    framep = header_ptr;
  }
  uint32_t prev_h2len;
  from_big(&prev_h2len, framep + FRAME_HEADER_LEN, sizeof(prev_h2len));

  // Build a new header
  uint8_t* h2 = new_header_frame(schunk, frame);
  uint32_t h2len;
  from_big(&h2len, h2 + FRAME_HEADER_LEN, sizeof(h2len));

  // The frame length is outdated when adding a new metalayer, so update it
  if (new) {
    int64_t frame_len = h2len;  // at adding time, we only have to worry of the header for now
    to_big(h2 + FRAME_LEN, &frame_len, sizeof(frame_len));
    frame->len = frame_len;
  }

  if (!new && prev_h2len != h2len) {
    BLOSC_TRACE_ERROR("The new metalayer sizes should be equal the existing ones.");
    return BLOSC2_ERROR_DATA;
  }

  void* fp = NULL;
  if (frame->cframe == NULL) {
    // Write updated header down to file
    if (frame->sframe) {
      fp = sframe_open_index(frame->urlpath, "rb+",
                             frame->schunk->storage->io);
    }
    else {
      fp = io_cb->open(frame->urlpath, "rb+", frame->schunk->storage->io->params);
    }
    if (fp == NULL) {
      BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
      return BLOSC2_ERROR_FILE_OPEN;
    }
    int64_t io_pos = frame->file_offset;
    io_cb->write(h2, h2len, 1, io_pos, fp);
    io_cb->close(fp);
  }
  else {
    if (new) {
      frame->cframe = realloc(frame->cframe, h2len);
    }
    memcpy(frame->cframe, h2, h2len);
  }
  free(h2);

  return 1;
}


static int get_meta_from_header(blosc2_frame_s* frame, blosc2_schunk* schunk, uint8_t* header,
                                int32_t header_len) {
  BLOSC_UNUSED_PARAM(frame);
  int64_t header_pos = FRAME_IDX_SIZE;

  // Get the size for the index of metalayers
  uint16_t idx_size;
  header_pos += sizeof(idx_size);
  if (header_len < header_pos) {
    return BLOSC2_ERROR_READ_BUFFER;
  }
  from_big(&idx_size, header + FRAME_IDX_SIZE, sizeof(idx_size));

  // Get the actual index of metalayers
  uint8_t* metalayers_idx = header + FRAME_IDX_SIZE + 2;
  header_pos += 1;
  if (header_len < header_pos) {
    return BLOSC2_ERROR_READ_BUFFER;
  }
  if (metalayers_idx[0] != 0xde) {   // sanity check
    return BLOSC2_ERROR_DATA;
  }
  uint8_t* idxp = metalayers_idx + 1;
  uint16_t nmetalayers;
  header_pos += sizeof(nmetalayers);
  if (header_len < header_pos) {
    return BLOSC2_ERROR_READ_BUFFER;
  }
  from_big(&nmetalayers, idxp, sizeof(uint16_t));
  idxp += 2;
  if (nmetalayers > BLOSC2_MAX_METALAYERS) {
    return BLOSC2_ERROR_DATA;
  }
  schunk->nmetalayers = nmetalayers;

  // Populate the metalayers and its serialized values
  for (int nmetalayer = 0; nmetalayer < nmetalayers; nmetalayer++) {
    header_pos += 1;
    if (header_len < header_pos) {
      return BLOSC2_ERROR_READ_BUFFER;
    }
    if ((*idxp & 0xe0u) != 0xa0u) {   // sanity check
      return BLOSC2_ERROR_DATA;
    }
    blosc2_metalayer* metalayer = calloc(1, sizeof(blosc2_metalayer));
    schunk->metalayers[nmetalayer] = metalayer;

    // Populate the metalayer string
    uint8_t nslen = *idxp & (uint8_t)0x1F;
    idxp += 1;
    header_pos += nslen;
    if (header_len < header_pos) {
      return BLOSC2_ERROR_READ_BUFFER;
    }
    char* ns = malloc((size_t)nslen + 1);
    memcpy(ns, idxp, nslen);
    ns[nslen] = '\0';
    idxp += nslen;
    metalayer->name = ns;

    // Populate the serialized value for this metalayer
    // Get the offset
    header_pos += 1;
    if (header_len < header_pos) {
      return BLOSC2_ERROR_READ_BUFFER;
    }
    if ((*idxp & 0xffu) != 0xd2u) {   // sanity check
      return BLOSC2_ERROR_DATA;
    }
    idxp += 1;
    int32_t offset;
    header_pos += sizeof(offset);
    if (header_len < header_pos) {
      return BLOSC2_ERROR_READ_BUFFER;
    }
    from_big(&offset, idxp, sizeof(offset));
    idxp += 4;
    if (offset < 0 || offset >= header_len) {
      // Offset is less than zero or exceeds header length
      return BLOSC2_ERROR_DATA;
    }
    // Go to offset and see if we have the correct marker
    uint8_t* content_marker = header + offset;
    if (header_len < offset + 1 + 4) {
      return BLOSC2_ERROR_READ_BUFFER;
    }
    if (*content_marker != 0xc6) {
      return BLOSC2_ERROR_DATA;
    }

    // Read the size of the content
    int32_t content_len;
    from_big(&content_len, content_marker + 1, sizeof(content_len));
    if (content_len < 0) {
      return BLOSC2_ERROR_DATA;
    }
    metalayer->content_len = content_len;

    // Finally, read the content
    if (header_len < offset + 1 + 4 + content_len) {
      return BLOSC2_ERROR_READ_BUFFER;
    }
    char* content = malloc((size_t)content_len);
    memcpy(content, content_marker + 1 + 4, (size_t)content_len);
    metalayer->content = (uint8_t*)content;
  }

  return 1;
}

int frame_get_metalayers(blosc2_frame_s* frame, blosc2_schunk* schunk) {
  int32_t header_len;
  int64_t frame_len;
  int64_t nbytes;
  int64_t cbytes;
  int32_t blocksize;
  int32_t chunksize;
  int64_t nchunks;
  int ret = get_header_info(frame, &header_len, &frame_len, &nbytes, &cbytes,
                            &blocksize, &chunksize, &nchunks,
                            NULL, NULL, NULL, NULL, NULL, NULL, NULL,
                            schunk->storage->io);
  if (ret < 0) {
    BLOSC_TRACE_ERROR("Unable to get the header info from frame.");
    return ret;
  }

  // Get the header
  uint8_t* header = NULL;
  bool needs_free = false;
  if (frame->cframe != NULL) {
    header = frame->cframe;
  } else {
    int64_t rbytes = 0;
    blosc2_io_cb *io_cb = blosc2_get_io_cb(frame->schunk->storage->io->id);
    if (io_cb == NULL) {
      BLOSC_TRACE_ERROR("Error getting the input/output API");
      return BLOSC2_ERROR_PLUGIN_IO;
    }

    if (io_cb->is_allocation_necessary) {
      header = malloc(header_len);
      needs_free = true;
    }
    else {
      needs_free = false;
    }

    void* fp = NULL;
    int64_t io_pos = 0;
    if (frame->sframe) {
      fp = sframe_open_index(frame->urlpath, "rb",
                             frame->schunk->storage->io);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return BLOSC2_ERROR_FILE_OPEN;
      }
    }
    else {
      fp = io_cb->open(frame->urlpath, "rb", frame->schunk->storage->io->params);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return BLOSC2_ERROR_FILE_OPEN;
      }
      io_pos = frame->file_offset;
    }
    if (fp != NULL) {
      rbytes = io_cb->read((void**)&header, 1, header_len, io_pos, fp);
      io_cb->close(fp);
    }
    if (rbytes != header_len) {
      BLOSC_TRACE_ERROR("Cannot access the header out of the frame.");
      if (needs_free)
        free(header);
      return BLOSC2_ERROR_FILE_READ;
    }
  }

  ret = get_meta_from_header(frame, schunk, header, header_len);

  if (frame->cframe == NULL && needs_free) {
    free(header);
  }

  return ret;
}

static int get_vlmeta_from_trailer(blosc2_frame_s* frame, blosc2_schunk* schunk, uint8_t* trailer,
                                   int32_t trailer_len) {

  BLOSC_UNUSED_PARAM(frame);
  int64_t trailer_pos = FRAME_TRAILER_VLMETALAYERS + 2;
  uint8_t* idxp = trailer + trailer_pos;

  // Get the size for the index of metalayers
  trailer_pos += 2;
  if (trailer_len < trailer_pos) {
    return BLOSC2_ERROR_READ_BUFFER;
  }
  uint16_t idx_size;
  from_big(&idx_size, idxp, sizeof(idx_size));
  idxp += 2;

  trailer_pos += 1;
  // Get the actual index of metalayers
  if (trailer_len < trailer_pos) {
    return BLOSC2_ERROR_READ_BUFFER;
  }
  if (idxp[0] != 0xde) {   // sanity check
    return BLOSC2_ERROR_DATA;
  }
  idxp += 1;

  int16_t nmetalayers;
  trailer_pos += sizeof(nmetalayers);
  if (trailer_len < trailer_pos) {
    return BLOSC2_ERROR_READ_BUFFER;
  }
  from_big(&nmetalayers, idxp, sizeof(uint16_t));
  idxp += 2;
  if (nmetalayers > BLOSC2_MAX_VLMETALAYERS) {
    return BLOSC2_ERROR_DATA;
  }
  schunk->nvlmetalayers = nmetalayers;

  // Populate the metalayers and its serialized values
  for (int nmetalayer = 0; nmetalayer < nmetalayers; nmetalayer++) {
    trailer_pos += 1;
    if (trailer_len < trailer_pos) {
      return BLOSC2_ERROR_READ_BUFFER;
    }
    if ((*idxp & 0xe0u) != 0xa0u) {   // sanity check
      return BLOSC2_ERROR_DATA;
    }
    blosc2_metalayer* metalayer = calloc(1, sizeof(blosc2_metalayer));
    schunk->vlmetalayers[nmetalayer] = metalayer;

    // Populate the metalayer string
    uint8_t nslen = *idxp & (uint8_t)0x1F;
    idxp += 1;
    trailer_pos += nslen;
    if (trailer_len < trailer_pos) {
      return BLOSC2_ERROR_READ_BUFFER;
    }
    char* ns = malloc((size_t)nslen + 1);
    memcpy(ns, idxp, nslen);
    ns[nslen] = '\0';
    idxp += nslen;
    metalayer->name = ns;

    // Populate the serialized value for this metalayer
    // Get the offset
    trailer_pos += 1;
    if (trailer_len < trailer_pos) {
      return BLOSC2_ERROR_READ_BUFFER;
    }
    if ((*idxp & 0xffu) != 0xd2u) {   // sanity check
      return BLOSC2_ERROR_DATA;
    }
    idxp += 1;
    int32_t offset;
    trailer_pos += sizeof(offset);
    if (trailer_len < trailer_pos) {
      return BLOSC2_ERROR_READ_BUFFER;
    }
    from_big(&offset, idxp, sizeof(offset));
    idxp += 4;
    if (offset < 0 || offset >= trailer_len) {
      // Offset is less than zero or exceeds trailer length
      return BLOSC2_ERROR_DATA;
    }
    // Go to offset and see if we have the correct marker
    uint8_t* content_marker = trailer + offset;
    if (trailer_len < offset + 1 + 4) {
      return BLOSC2_ERROR_READ_BUFFER;
    }
    if (*content_marker != 0xc6) {
      return BLOSC2_ERROR_DATA;
    }

    // Read the size of the content
    int32_t content_len;
    from_big(&content_len, content_marker + 1, sizeof(content_len));
    if (content_len < 0) {
      return BLOSC2_ERROR_DATA;
    }
    metalayer->content_len = content_len;

    // Finally, read the content
    if (trailer_len < offset + 1 + 4 + content_len) {
      return BLOSC2_ERROR_READ_BUFFER;
    }
    char* content = malloc((size_t)content_len);
    memcpy(content, content_marker + 1 + 4, (size_t)content_len);
    metalayer->content = (uint8_t*)content;
  }
  return 1;
}

int frame_get_vlmetalayers(blosc2_frame_s* frame, blosc2_schunk* schunk) {
  int32_t header_len;
  int64_t frame_len;
  int64_t nbytes;
  int64_t cbytes;
  int32_t blocksize;
  int32_t chunksize;
  int64_t nchunks;
  int ret = get_header_info(frame, &header_len, &frame_len, &nbytes, &cbytes,
                            &blocksize, &chunksize, &nchunks,
                            NULL, NULL, NULL, NULL, NULL, NULL, NULL,
                            schunk->storage->io);
  if (ret < 0) {
    BLOSC_TRACE_ERROR("Unable to get the trailer info from frame.");
    return ret;
  }

  int64_t trailer_offset = get_trailer_offset(frame, header_len, nbytes > 0);
  int32_t trailer_len = (int32_t) frame->trailer_len;

  if (trailer_offset < BLOSC_EXTENDED_HEADER_LENGTH || trailer_offset + trailer_len > frame->len) {
    BLOSC_TRACE_ERROR("Cannot access the trailer out of the frame.");
    return BLOSC2_ERROR_READ_BUFFER;
  }

  // Get the trailer
  uint8_t* trailer = NULL;
  bool needs_free = false;
  if (frame->cframe != NULL) {
    trailer = frame->cframe + trailer_offset;
  } else {
    int64_t rbytes = 0;

    blosc2_io_cb *io_cb = blosc2_get_io_cb(frame->schunk->storage->io->id);
    if (io_cb == NULL) {
      BLOSC_TRACE_ERROR("Error getting the input/output API");
      return BLOSC2_ERROR_PLUGIN_IO;
    }

    if (io_cb->is_allocation_necessary) {
      trailer = malloc(trailer_len);
      needs_free = true;
    }
    else {
      needs_free = false;
    }

    void* fp = NULL;
    int64_t io_pos = 0;
    if (frame->sframe) {
      char* eframe_name = malloc(strlen(frame->urlpath) + strlen("/chunks.b2frame") + 1);
      sprintf(eframe_name, "%s/chunks.b2frame", frame->urlpath);
      fp = io_cb->open(eframe_name, "rb", frame->schunk->storage->io->params);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", eframe_name);
        free(eframe_name);
        return BLOSC2_ERROR_FILE_OPEN;
      }
      free(eframe_name);
      io_pos = trailer_offset;
    }
    else {
      fp = io_cb->open(frame->urlpath, "rb", frame->schunk->storage->io->params);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return BLOSC2_ERROR_FILE_OPEN;
      }
      io_pos = frame->file_offset + trailer_offset;
    }
    if (fp != NULL) {
      rbytes = io_cb->read((void**)&trailer, 1, trailer_len, io_pos, fp);
      io_cb->close(fp);
    }
    if (rbytes != trailer_len) {
      BLOSC_TRACE_ERROR("Cannot access the trailer out of the fileframe.");
      if (needs_free)
        free(trailer);
      return BLOSC2_ERROR_FILE_READ;
    }
  }

  ret = get_vlmeta_from_trailer(frame, schunk, trailer, trailer_len);

  if (frame->cframe == NULL && needs_free) {
    free(trailer);
  }

  return ret;
}


blosc2_storage* get_new_storage(const blosc2_storage* storage,
                                const blosc2_cparams* cdefaults,
                                const blosc2_dparams* ddefaults,
                                const blosc2_io* iodefaults) {

  blosc2_storage* new_storage = (blosc2_storage*)calloc(1, sizeof(blosc2_storage));
  memcpy(new_storage, storage, sizeof(blosc2_storage));
  if (storage->urlpath != NULL) {
    char* urlpath = normalize_urlpath(storage->urlpath);
    new_storage->urlpath = malloc(strlen(urlpath) + 1);
    strcpy(new_storage->urlpath, urlpath);
  }

  // cparams
  blosc2_cparams* cparams = malloc(sizeof(blosc2_cparams));
  if (storage->cparams != NULL) {
    memcpy(cparams, storage->cparams, sizeof(blosc2_cparams));
  } else {
    memcpy(cparams, cdefaults, sizeof(blosc2_cparams));
  }
  new_storage->cparams = cparams;

  // dparams
  blosc2_dparams* dparams = malloc(sizeof(blosc2_dparams));
  if (storage->dparams != NULL) {
    memcpy(dparams, storage->dparams, sizeof(blosc2_dparams));
  }
  else {
    memcpy(dparams, ddefaults, sizeof(blosc2_dparams));
  }
  new_storage->dparams = dparams;

  // iodefaults
  blosc2_io* udio = malloc(sizeof(blosc2_io));
  if (storage->io != NULL) {
    memcpy(udio, storage->io, sizeof(blosc2_io));
  }
  else {
    memcpy(udio, iodefaults, sizeof(blosc2_io));
  }
  new_storage->io = udio;

  return new_storage;
}


/* Get a super-chunk out of a frame */
blosc2_schunk* frame_to_schunk(blosc2_frame_s* frame, bool copy, const blosc2_io *udio) {
  int32_t header_len;
  int64_t frame_len;
  int rc;
  blosc2_schunk* schunk = calloc(1, sizeof(blosc2_schunk));
  schunk->frame = (blosc2_frame*)frame;
  frame->schunk = schunk;

  rc = get_header_info(frame, &header_len, &frame_len, &schunk->nbytes,
                       &schunk->cbytes, &schunk->blocksize,
                       &schunk->chunksize, &schunk->nchunks, &schunk->typesize,
                       &schunk->compcode, &schunk->compcode_meta, &schunk->clevel, schunk->filters,
                       schunk->filters_meta, &schunk->splitmode, udio);
  if (rc < 0) {
    BLOSC_TRACE_ERROR("Unable to get meta info from frame.");
    blosc2_schunk_free(schunk);
    return NULL;
  }
  int64_t nchunks = schunk->nchunks;
  int64_t nbytes = schunk->nbytes;
  (void) nbytes;
  int64_t cbytes = schunk->cbytes;

  // Compression and decompression contexts
  blosc2_cparams *cparams;
  blosc2_schunk_get_cparams(schunk, &cparams);
  schunk->cctx = blosc2_create_cctx(*cparams);
  if (schunk->cctx == NULL) {
    BLOSC_TRACE_ERROR("Error while creating the compression context");
    return NULL;
  }
  blosc2_dparams *dparams;
  blosc2_schunk_get_dparams(schunk, &dparams);
  schunk->dctx = blosc2_create_dctx(*dparams);
  if (schunk->dctx == NULL) {
    BLOSC_TRACE_ERROR("Error while creating the decompression context");
    return NULL;
  }
  blosc2_storage storage = {.contiguous = copy ? false : true};
  schunk->storage = get_new_storage(&storage, cparams, dparams, udio);
  free(cparams);
  free(dparams);
  if (!copy) {
    goto out;
  }

  // We are not attached to a frame anymore
  schunk->frame = NULL;

  if (nchunks == 0) {
    frame->schunk = NULL;
    goto out;
  }

  // Get the compressed offsets
  int32_t coffsets_cbytes = 0;
  uint8_t* coffsets = get_coffsets(frame, header_len, cbytes, nchunks, &coffsets_cbytes);
  if (coffsets == NULL) {
    blosc2_schunk_free(schunk);
    BLOSC_TRACE_ERROR("Cannot get the offsets for the frame.");
    return NULL;
  }

  // Decompress offsets
  blosc2_dparams off_dparams = BLOSC2_DPARAMS_DEFAULTS;
  blosc2_context *dctx = blosc2_create_dctx(off_dparams);
  if (dctx == NULL) {
    BLOSC_TRACE_ERROR("Error while creating the decompression context");
    return NULL;
  }
  int64_t* offsets = (int64_t *) malloc((size_t)nchunks * sizeof(int64_t));
  int32_t off_nbytes = blosc2_decompress_ctx(dctx, coffsets, coffsets_cbytes,
                                             offsets, (int32_t)(nchunks * sizeof(int64_t)));
  blosc2_free_ctx(dctx);
  if (off_nbytes < 0) {
    free(offsets);
    blosc2_schunk_free(schunk);
    BLOSC_TRACE_ERROR("Cannot decompress the offsets chunk.");
    return NULL;
  }

  // We want the contiguous schunk, so create the actual data chunks (and, while doing this,
  // get a guess at the blocksize used in this frame)
  int64_t acc_nbytes = 0;
  int64_t acc_cbytes = 0;
  int32_t blocksize = 0;
  int32_t chunk_nbytes;
  int32_t chunk_cbytes;
  int32_t chunk_blocksize;
  size_t prev_alloc = BLOSC_EXTENDED_HEADER_LENGTH;
  uint8_t* data_chunk = NULL;
  bool needs_free = false;
  const blosc2_io_cb *io_cb = blosc2_get_io_cb(udio->id);
  if (io_cb == NULL) {
    blosc2_schunk_free(schunk);
    BLOSC_TRACE_ERROR("Error getting the input/output API");
    return NULL;
  }

  void* fp = NULL;
  if (frame->cframe == NULL) {
    if (io_cb->is_allocation_necessary) {
      data_chunk = malloc((size_t)prev_alloc);
      needs_free = true;
    }
    else {
      needs_free = false;
    }
    
    if (!frame->sframe) {
      // If not the chunks won't be in the frame
      fp = io_cb->open(frame->urlpath, "rb", udio->params);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        rc = BLOSC2_ERROR_FILE_OPEN;
        goto end;
      }
    }
  }
  schunk->data = malloc(nchunks * sizeof(void*));
  for (int i = 0; i < nchunks; i++) {
    if (frame->cframe != NULL) {
      if (needs_free) {
        free(data_chunk);
      }
      if (offsets[i] < 0) {
        int64_t rbytes = frame_get_chunk(frame, i, &data_chunk, &needs_free);
        if (rbytes < 0) {
          break;
        }
      }
      else {
       data_chunk = frame->cframe + header_len + offsets[i];
      }
      rc = blosc2_cbuffer_sizes(data_chunk, NULL, &chunk_cbytes, NULL);
      if (rc < 0) {
        break;
      }
    }
    else {
      int64_t rbytes;
      if (frame->sframe) {
        if (needs_free) {
          free(data_chunk);
        }
        rbytes = frame_get_chunk(frame, i, &data_chunk, &needs_free);
        if (rbytes < 0) {
          break;
        }
      }
      else {
        int64_t io_pos = frame->file_offset + header_len + offsets[i];
        rbytes = io_cb->read((void**)&data_chunk, 1, BLOSC_EXTENDED_HEADER_LENGTH, io_pos, fp);
      }
      if (rbytes != BLOSC_EXTENDED_HEADER_LENGTH) {
        rc = BLOSC2_ERROR_READ_BUFFER;
        break;
      }
      rc = blosc2_cbuffer_sizes(data_chunk, NULL, &chunk_cbytes, NULL);
      if (rc < 0) {
        break;
      }
      if (chunk_cbytes > (int32_t)prev_alloc) {
        if (io_cb->is_allocation_necessary)
          data_chunk = realloc(data_chunk, chunk_cbytes);
        if (data_chunk == NULL) {
          BLOSC_TRACE_ERROR("Cannot realloc space for the data_chunk.");
          rc = BLOSC2_ERROR_MEMORY_ALLOC;
          break;
        }
        prev_alloc = chunk_cbytes;
      }
      if (!frame->sframe) {
        int64_t io_pos = frame->file_offset + header_len + offsets[i];
        rbytes = io_cb->read((void**)&data_chunk, 1, chunk_cbytes, io_pos, fp);
        if (rbytes != chunk_cbytes) {
          rc = BLOSC2_ERROR_READ_BUFFER;
          break;
        }
      }
    }
    uint8_t* new_chunk = malloc(chunk_cbytes);
    memcpy(new_chunk, data_chunk, chunk_cbytes);
    schunk->data[i] = new_chunk;
    rc = blosc2_cbuffer_sizes(data_chunk, &chunk_nbytes, NULL, &chunk_blocksize);
    if (rc < 0) {
      break;
    }
    acc_nbytes += chunk_nbytes;
    acc_cbytes += chunk_cbytes;
    if (i == 0) {
      blocksize = chunk_blocksize;
    }
    else if (blocksize != chunk_blocksize) {
      // Blocksize varies
      blocksize = 0;
    }
  }

  // We are not attached to a schunk anymore
  frame->schunk = NULL;

  end:
  if (needs_free) {
    free(data_chunk);
  }
  if (frame->cframe == NULL) {
    if (!frame->sframe) {
      io_cb->close(fp);
    }
  }
  free(offsets);

  // cframes and sframes have different ways to store chunks with special values:
  // 1) cframes represent special chunks as negative offsets
  // 2) sframes does not have the concept of offsets, but rather of data pointers (.data)
  //    so they always have a pointer to a special chunk
  // This is why cframes and sframes have different cbytes and hence, we cannot enforce acc_bytes == schunk->cbytes
  // In the future, maybe we could provide special meanings for .data[i] > 0x7FFFFFFF, but not there yet
  // if (rc < 0 || acc_nbytes != nbytes || acc_cbytes != cbytes) {
  if (rc < 0 || acc_nbytes != nbytes) {
    blosc2_schunk_free(schunk);
    return NULL;
  }
  // Update counters
  schunk->cbytes = acc_cbytes;
  schunk->blocksize = blocksize;

  out:
  rc = frame_get_metalayers(frame, schunk);
  if (rc < 0) {
    blosc2_schunk_free(schunk);
    BLOSC_TRACE_ERROR("Cannot access the metalayers.");
    return NULL;
  }

  rc = frame_get_vlmetalayers(frame, schunk);
  if (rc < 0) {
    blosc2_schunk_free(schunk);
    BLOSC_TRACE_ERROR("Cannot access the vlmetalayers.");
    return NULL;
  }

  return schunk;
}


void frame_avoid_cframe_free(blosc2_frame_s* frame, bool avoid_cframe_free) {
  frame->avoid_cframe_free = avoid_cframe_free;
}


struct csize_idx {
    int32_t val;
    int32_t idx;
};

// Helper function for qsorting block offsets
int sort_offset(const void* a, const void* b) {
  int32_t a_ = ((struct csize_idx*)a)->val;
  int32_t b_ = ((struct csize_idx*)b)->val;
  return a_ - b_;
}


int get_coffset(blosc2_frame_s* frame, int32_t header_len, int64_t cbytes,
                int64_t nchunk, int64_t nchunks, int64_t *offset) {
  int32_t off_cbytes;
  // Get the offset to nchunk
  uint8_t *coffsets = get_coffsets(frame, header_len, cbytes, nchunks, &off_cbytes);
  if (coffsets == NULL) {
    BLOSC_TRACE_ERROR("Cannot get the offset for chunk %" PRId64 " for the frame.", nchunk);
    return BLOSC2_ERROR_DATA;
  }

  // Get the 64-bit offset
  int rc = blosc2_getitem(coffsets, off_cbytes, (int32_t)nchunk, 1, offset, (int32_t)sizeof(int64_t));
  if (rc < 0) {
    BLOSC_TRACE_ERROR("Problems retrieving a chunk offset.");
  } else if (!frame->sframe && *offset > frame->len) {
    BLOSC_TRACE_ERROR("Cannot read chunk %" PRId64 " outside of frame boundary.", nchunk);
    rc = BLOSC2_ERROR_READ_BUFFER;
  }

  return rc;
}


// Detect and return a chunk with special values in offsets (only zeros, NaNs and non initialized)
int frame_special_chunk(int64_t special_value, int32_t nbytes, int32_t typesize, int32_t blocksize,
                        uint8_t** chunk, int32_t cbytes, bool *needs_free) {
  int rc = 0;
  *chunk = malloc(cbytes);
  *needs_free = true;

  // Detect the kind of special value
  uint64_t zeros_mask = (uint64_t) BLOSC2_SPECIAL_ZERO << (8 * 7);  // chunk of zeros
  uint64_t nans_mask = (uint64_t) BLOSC2_SPECIAL_NAN << (8 * 7);  // chunk of NaNs
  uint64_t uninit_mask = (uint64_t) BLOSC2_SPECIAL_UNINIT << (8 * 7);  // chunk of uninit values

  blosc2_cparams cparams = BLOSC2_CPARAMS_DEFAULTS;
  cparams.typesize = typesize;
  cparams.blocksize = blocksize;
  if (special_value & zeros_mask) {
    rc = blosc2_chunk_zeros(cparams, nbytes, *chunk, cbytes);
    if (rc < 0) {
      BLOSC_TRACE_ERROR("Error creating a zero chunk");
    }
  }
  else if (special_value & uninit_mask) {
    rc = blosc2_chunk_uninit(cparams, nbytes, *chunk, cbytes);
    if (rc < 0) {
      BLOSC_TRACE_ERROR("Error creating a non initialized chunk");
    }
  }
  else if (special_value & nans_mask) {
    rc = blosc2_chunk_nans(cparams, nbytes, *chunk, cbytes);
    if (rc < 0) {
      BLOSC_TRACE_ERROR("Error creating a nan chunk");
    }
  }
  else {
    BLOSC_TRACE_ERROR("Special value not recognized: %" PRId64 "", special_value);
    rc = BLOSC2_ERROR_DATA;
  }

  if (rc < 0) {
    free(*chunk);
    *needs_free = false;
    *chunk = NULL;
  }

  return rc;
}


/* Return a compressed chunk that is part of a frame in the `chunk` parameter.
 * If the frame is disk-based, a buffer is allocated for the (compressed) chunk,
 * and hence a free is needed.  You can check if the chunk requires a free with the `needs_free`
 * parameter.
 * If the chunk does not need a free, it means that a pointer to the location in frame is returned
 * in the `chunk` parameter.
 *
 * The size of the (compressed) chunk is returned.  If some problem is detected, a negative code
 * is returned instead.
*/
int frame_get_chunk(blosc2_frame_s *frame, int64_t nchunk, uint8_t **chunk, bool *needs_free) {
  int32_t header_len;
  int64_t frame_len;
  int64_t nbytes;
  int64_t cbytes;
  int32_t blocksize;
  int32_t chunksize;
  int64_t nchunks;
  int32_t typesize;
  int64_t offset;
  int32_t chunk_cbytes;
  int rc;

  *chunk = NULL;
  *needs_free = false;
  rc = get_header_info(frame, &header_len, &frame_len, &nbytes, &cbytes,
                       &blocksize, &chunksize, &nchunks,
                       &typesize, NULL, NULL, NULL, NULL, NULL, NULL,
                       frame->schunk->storage->io);
  if (rc < 0) {
    BLOSC_TRACE_ERROR("Unable to get meta info from frame.");
    return rc;
  }

  if ((nchunks > 0) && (nchunk >= nchunks)) {
    BLOSC_TRACE_ERROR("nchunk ('%" PRId64 "') exceeds the number of chunks "
                    "('%" PRId64 "') in frame.", nchunk, nchunks);
    return BLOSC2_ERROR_INVALID_PARAM;
  }

  // Get the offset to nchunk
  rc = get_coffset(frame, header_len, cbytes, nchunk, nchunks, &offset);
  if (rc < 0) {
    BLOSC_TRACE_ERROR("Unable to get offset to chunk %" PRId64 ".", nchunk);
    return rc;
  }

  if (offset < 0) {
    // Special value
    chunk_cbytes = BLOSC_EXTENDED_HEADER_LENGTH;
    int32_t chunksize_ = chunksize;
    if ((nchunk == nchunks - 1) && (nbytes % chunksize)) {
      // Last chunk is incomplete.  Compute its actual size.
      chunksize_ = (int32_t) (nbytes % chunksize);
    }
    rc = frame_special_chunk(offset, chunksize_, typesize, blocksize, chunk, chunk_cbytes, needs_free);
    if (rc < 0) {
      return rc;
    }
    goto end;
  }

  if (frame->sframe) {
    // Sparse on-disk
    nchunk = offset;
    return sframe_get_chunk(frame, nchunk, chunk, needs_free);
  }

  blosc2_io_cb *io_cb = blosc2_get_io_cb(frame->schunk->storage->io->id);
  if (io_cb == NULL) {
    BLOSC_TRACE_ERROR("Error getting the input/output API");
    return BLOSC2_ERROR_PLUGIN_IO;
  }

  if (frame->cframe == NULL) {
    uint8_t* header_ptr;
    uint8_t header[BLOSC_EXTENDED_HEADER_LENGTH];
    void* fp = io_cb->open(frame->urlpath, "rb", frame->schunk->storage->io->params);
    if (fp == NULL) {
      BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
      return BLOSC2_ERROR_FILE_OPEN;
    }
    if (io_cb->is_allocation_necessary)
      header_ptr = header;
    int64_t io_pos = frame->file_offset + header_len + offset;
    int64_t rbytes = io_cb->read((void**)&header_ptr, 1, sizeof(header), io_pos, fp);
    if (rbytes != BLOSC_EXTENDED_HEADER_LENGTH) {
      BLOSC_TRACE_ERROR("Cannot read the cbytes for chunk in the frame.");
      io_cb->close(fp);
      return BLOSC2_ERROR_FILE_READ;
    }
    rc = blosc2_cbuffer_sizes(header_ptr, NULL, &chunk_cbytes, NULL);
    if (rc < 0) {
      BLOSC_TRACE_ERROR("Cannot read the cbytes for chunk in the frame.");
      io_cb->close(fp);
      return rc;
    }
    if (io_cb->is_allocation_necessary) {
      *chunk = malloc(chunk_cbytes);
      *needs_free = true;
    }
    else {
      *needs_free = false;
    }
    
    io_pos = frame->file_offset + header_len + offset;
    rbytes = io_cb->read((void**)chunk, 1, chunk_cbytes, io_pos, fp);
    io_cb->close(fp);
    if (rbytes != chunk_cbytes) {
      BLOSC_TRACE_ERROR("Cannot read the chunk out of the frame.");
      return BLOSC2_ERROR_FILE_READ;
    }

  } else {
    // The chunk is in memory and just one pointer away
    *chunk = frame->cframe + header_len + offset;
    rc = blosc2_cbuffer_sizes(*chunk, NULL, &chunk_cbytes, NULL);
    if (rc < 0) {
      return rc;
    }
  }

  end:
  return (int32_t)chunk_cbytes;
}


/* Return a compressed chunk that is part of a frame in the `chunk` parameter.
 * If the frame is disk-based, a buffer is allocated for the (lazy) chunk,
 * and hence a free is needed.  You can check if the chunk requires a free with the `needs_free`
 * parameter.
 * If the chunk does not need a free, it means that the frame is in memory and that just a
 * pointer to the location of the chunk in memory is returned.
 *
 * The size of the (compressed, potentially lazy) chunk is returned.  If some problem is detected,
 * a negative code is returned instead.
*/
int frame_get_lazychunk(blosc2_frame_s *frame, int64_t nchunk, uint8_t **chunk, bool *needs_free) {
  int32_t header_len;
  int64_t frame_len;
  int64_t nbytes;
  int64_t cbytes;
  int32_t blocksize;
  int32_t chunksize;
  int64_t nchunks;
  int32_t typesize;
  int32_t lazychunk_cbytes;
  int64_t offset;
  void* fp = NULL;

  *chunk = NULL;
  *needs_free = false;
  int rc = get_header_info(frame, &header_len, &frame_len, &nbytes, &cbytes,
                           &blocksize, &chunksize, &nchunks,
                           &typesize, NULL, NULL, NULL, NULL, NULL, NULL,
                           frame->schunk->storage->io);
  if (rc < 0) {
    BLOSC_TRACE_ERROR("Unable to get meta info from frame.");
    return rc;
  }

  if (nchunk >= nchunks) {
    BLOSC_TRACE_ERROR("nchunk ('%" PRId64 "') exceeds the number of chunks "
                      "('%" PRId64 "') in frame.", nchunk, nchunks);
    return BLOSC2_ERROR_INVALID_PARAM;
  }

  // Get the offset to nchunk
  rc = get_coffset(frame, header_len, cbytes, nchunk, nchunks, &offset);
  if (rc < 0) {
    BLOSC_TRACE_ERROR("Unable to get offset to chunk %" PRId64 ".", nchunk);
    return rc;
  }

  if (offset < 0) {
    // Special value
    lazychunk_cbytes = BLOSC_EXTENDED_HEADER_LENGTH;
    int32_t chunksize_ = chunksize;
    if ((nchunk == nchunks - 1) && (nbytes % chunksize)) {
      // Last chunk is incomplete.  Compute its actual size.
      chunksize_ = (int32_t) (nbytes % chunksize);
    }
    rc = frame_special_chunk(offset, chunksize_, typesize, blocksize, chunk,
                             (int32_t)lazychunk_cbytes, needs_free);
    goto end;
  }

  blosc2_io_cb *io_cb = blosc2_get_io_cb(frame->schunk->storage->io->id);
  if (io_cb == NULL) {
    BLOSC_TRACE_ERROR("Error getting the input/output API");
    rc = BLOSC2_ERROR_PLUGIN_IO;
    goto end;
  }

  if (frame->cframe == NULL) {
    // TODO: make this portable across different endianness
    // Get info for building a lazy chunk
    int32_t chunk_nbytes;
    int32_t chunk_cbytes;
    int32_t chunk_blocksize;
    uint8_t* header_ptr;
    uint8_t header[BLOSC_EXTENDED_HEADER_LENGTH];
    int64_t io_pos = 0;
    if (frame->sframe) {
      // The chunk is not in the frame
      fp = sframe_open_chunk(frame->urlpath, offset, "rb",
                             frame->schunk->storage->io);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return BLOSC2_ERROR_FILE_OPEN;
      }
    }
    else {
      fp = io_cb->open(frame->urlpath, "rb", frame->schunk->storage->io->params);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return BLOSC2_ERROR_FILE_OPEN;
      }
      io_pos = frame->file_offset + header_len + offset;
    }
    if (io_cb->is_allocation_necessary)
      header_ptr = header;
    int64_t rbytes = io_cb->read((void**)&header_ptr, 1, BLOSC_EXTENDED_HEADER_LENGTH, io_pos, fp);
    if (rbytes != BLOSC_EXTENDED_HEADER_LENGTH) {
      BLOSC_TRACE_ERROR("Cannot read the header for chunk in the frame.");
      rc = BLOSC2_ERROR_FILE_READ;
      goto end;
    }
    rc = blosc2_cbuffer_sizes(header_ptr, &chunk_nbytes, &chunk_cbytes, &chunk_blocksize);
    if (rc < 0) {
      goto end;
    }
    size_t nblocks = chunk_nbytes / chunk_blocksize;
    size_t leftover_block = chunk_nbytes % chunk_blocksize;
    nblocks = leftover_block ? nblocks + 1 : nblocks;
    // Allocate space for the lazy chunk
    int32_t trailer_len;
    int32_t special_type = (header_ptr[BLOSC2_CHUNK_BLOSC2_FLAGS] >> 4) & BLOSC2_SPECIAL_MASK;
    int memcpyed = header_ptr[BLOSC2_CHUNK_FLAGS] & (uint8_t) BLOSC_MEMCPYED;

    int32_t trailer_offset = BLOSC_EXTENDED_HEADER_LENGTH;
    size_t streams_offset = BLOSC_EXTENDED_HEADER_LENGTH;
    if (special_type == 0) {
      // Regular values have offsets for blocks
      trailer_offset += (int32_t) (nblocks * sizeof(int32_t));
      if (memcpyed) {
        streams_offset += 0;
      } else {
        streams_offset += nblocks * sizeof(int32_t);
      }
      trailer_len = (int32_t) (sizeof(int32_t) + sizeof(int64_t) + nblocks * sizeof(int32_t));
      lazychunk_cbytes = trailer_offset + trailer_len;
    }
    else if (special_type == BLOSC2_SPECIAL_VALUE) {
      trailer_offset += typesize;
      streams_offset += typesize;
      trailer_len = 0;
      lazychunk_cbytes = trailer_offset + trailer_len;
    }
    else {
      rc = BLOSC2_ERROR_INVALID_HEADER;
      goto end;
    }

    // Read just the full header and bstarts section too (lazy partial length)
    if (frame->sframe) {
      io_pos = 0;
    }
    else {
      io_pos = frame->file_offset + header_len + offset;
    }

    // The case here is a bit special because more memory is allocated than read from the file
    // and the chunk is modified after reading. Due to the modification, we cannot directly use
    // the memory provided by the io
    *chunk = malloc(lazychunk_cbytes);
    *needs_free = true;

    if (io_cb->is_allocation_necessary) {
      rbytes = io_cb->read((void**)chunk, 1, (int64_t)streams_offset, io_pos, fp);
    }
    else {
      uint8_t* chunk_ptr;
      rbytes = io_cb->read((void**)&chunk_ptr, 1, (int64_t)streams_offset, io_pos, fp);
      memcpy(*chunk, chunk_ptr, streams_offset);
    }
    
    if (rbytes != (int64_t)streams_offset) {
      BLOSC_TRACE_ERROR("Cannot read the (lazy) chunk out of the frame.");
      rc = BLOSC2_ERROR_FILE_READ;
      goto end;
    }
    if (special_type == BLOSC2_SPECIAL_VALUE) {
      // Value runlen is not returning a lazy chunk.  We are done.
      goto end;
    }

    // Mark chunk as lazy
    uint8_t* blosc2_flags = *chunk + BLOSC2_CHUNK_BLOSC2_FLAGS;
    *blosc2_flags |= 0x08U;

    // Add the trailer (currently, nchunk + offset + block_csizes)
    if (frame->sframe) {
      *(int32_t*)(*chunk + trailer_offset) = (int32_t)offset;   // offset is nchunk for sframes
      *(int64_t*)(*chunk + trailer_offset + sizeof(int32_t)) = offset;
    }
    else {
      *(int32_t*)(*chunk + trailer_offset) = (int32_t)nchunk;
      *(int64_t*)(*chunk + trailer_offset + sizeof(int32_t)) = header_len + offset;
    }

    int32_t* block_csizes = malloc(nblocks * sizeof(int32_t));

    if (memcpyed) {
      // When memcpyed the blocksizes are trivial to compute
      for (int i = 0; i < (int)nblocks - 1; i++) {
        block_csizes[i] = (int)chunk_blocksize;
      }
      // The last block could be incomplete, mainly due to the fact that the block size is not divisible
      // by the typesize
      block_csizes[nblocks - 1] = (int32_t)leftover_block ? (int32_t)leftover_block : chunk_blocksize;
    }
    else {
      // In regular, compressed chunks, we need to sort the bstarts (they can be out
      // of order because of multi-threading), and get a reverse index too.
      memcpy(block_csizes, *chunk + BLOSC_EXTENDED_HEADER_LENGTH, nblocks * sizeof(int32_t));
      // Helper structure to keep track of original indexes
      struct csize_idx *csize_idx = malloc(nblocks * sizeof(struct csize_idx));
      for (int n = 0; n < (int)nblocks; n++) {
        csize_idx[n].val = block_csizes[n];
        csize_idx[n].idx = n;
      }
      qsort(csize_idx, nblocks, sizeof(struct csize_idx), &sort_offset);
      // Compute the actual csizes
      int idx;
      for (int n = 0; n < (int)nblocks - 1; n++) {
        idx = csize_idx[n].idx;
        block_csizes[idx] = csize_idx[n + 1].val - csize_idx[n].val;
      }
      idx = csize_idx[nblocks - 1].idx;
      block_csizes[idx] = (int)chunk_cbytes - csize_idx[nblocks - 1].val;
      free(csize_idx);
    }
    // Copy the csizes at the end of the trailer
    void *trailer_csizes = *chunk + lazychunk_cbytes - nblocks * sizeof(int32_t);
    memcpy(trailer_csizes, block_csizes, nblocks * sizeof(int32_t));
    free(block_csizes);
  } else {
    // The chunk is in memory and just one pointer away
    int64_t chunk_header_offset = header_len + offset;
    int64_t chunk_cbytes_offset = chunk_header_offset + BLOSC_MIN_HEADER_LENGTH;

    *chunk = frame->cframe + chunk_header_offset;

    if (chunk_cbytes_offset > frame->len) {
      BLOSC_TRACE_ERROR("Cannot read the header for chunk in the (contiguous) frame.");
      rc = BLOSC2_ERROR_READ_BUFFER;
    } else {
      rc = blosc2_cbuffer_sizes(*chunk, NULL, &lazychunk_cbytes, NULL);
      if (rc && chunk_cbytes_offset + lazychunk_cbytes > frame_len) {
        BLOSC_TRACE_ERROR("Compressed bytes exceed beyond frame length.");
        rc = BLOSC2_ERROR_READ_BUFFER;
      }
    }
  }

  end:
  if (fp != NULL) {
    io_cb->close(fp);
  }
  if (rc < 0) {
    if (*needs_free) {
      free(*chunk);
      *chunk = NULL;
      *needs_free = false;
    }
    return rc;
  }

  return (int)lazychunk_cbytes;
}


/* Fill an empty frame with special values (fast path). */
int64_t frame_fill_special(blosc2_frame_s* frame, int64_t nitems, int special_value,
                       int32_t chunksize, blosc2_schunk* schunk) {
  int32_t header_len;
  int64_t frame_len;
  int64_t nbytes;
  int64_t cbytes;
  int32_t blocksize;
  int32_t typesize;
  int64_t nchunks;

  int rc = get_header_info(frame, &header_len, &frame_len, &nbytes, &cbytes, &blocksize, NULL,
                           &nchunks, &typesize, NULL, NULL, NULL, NULL, NULL, NULL,
                           schunk->storage->io);
  if (rc < 0) {
    BLOSC_TRACE_ERROR("Unable to get meta info from frame.");
    return BLOSC2_ERROR_DATA;
  }

  if (nitems == 0) {
    return frame_len;
  }

  if ((nitems / chunksize) > INT_MAX) {
    BLOSC_TRACE_ERROR("nitems is too large.  Try increasing the chunksize.");
    return BLOSC2_ERROR_FRAME_SPECIAL;
  }

  if ((nbytes > 0) || (cbytes > 0)) {
    BLOSC_TRACE_ERROR("Filling with special values only works on empty frames");
    return BLOSC2_ERROR_FRAME_SPECIAL;
  }

  // Compute the number of chunks and the length of the offsets chunk
  int32_t chunkitems = chunksize / typesize;
  nchunks = nitems / chunkitems;
  int32_t leftover_items = (int32_t)(nitems % chunkitems);
  if (leftover_items) {
    nchunks += 1;
  }

  blosc2_cparams* cparams;
  blosc2_schunk_get_cparams(schunk, &cparams);

  // Build the offsets with a special chunk
  int new_off_cbytes = BLOSC_EXTENDED_HEADER_LENGTH + sizeof(int64_t);
  uint8_t* off_chunk = malloc(new_off_cbytes);
  uint64_t offset_value = ((uint64_t)1 << 63);
  uint8_t* sample_chunk = malloc(BLOSC_EXTENDED_HEADER_LENGTH);
  int csize;
  switch (special_value) {
    case BLOSC2_SPECIAL_ZERO:
      offset_value += (uint64_t) BLOSC2_SPECIAL_ZERO << (8 * 7);
      csize = blosc2_chunk_zeros(*cparams, chunksize, sample_chunk, BLOSC_EXTENDED_HEADER_LENGTH);
      break;
    case BLOSC2_SPECIAL_UNINIT:
      offset_value += (uint64_t) BLOSC2_SPECIAL_UNINIT << (8 * 7);
      csize = blosc2_chunk_uninit(*cparams, chunksize, sample_chunk, BLOSC_EXTENDED_HEADER_LENGTH);
      break;
    case BLOSC2_SPECIAL_NAN:
      offset_value += (uint64_t)BLOSC2_SPECIAL_NAN << (8 * 7);
      csize = blosc2_chunk_nans(*cparams, chunksize, sample_chunk, BLOSC_EXTENDED_HEADER_LENGTH);
      break;
    default:
      BLOSC_TRACE_ERROR("Only zeros, NaNs or non-initialized values are supported.");
      return BLOSC2_ERROR_FRAME_SPECIAL;
  }
  if (csize < 0) {
    BLOSC_TRACE_ERROR("Error creating sample chunk");
    return BLOSC2_ERROR_FRAME_SPECIAL;
  }
  cparams->typesize = sizeof(int64_t);  // change it to offsets typesize
  // cparams->blocksize = 0;   // automatic blocksize
  cparams->blocksize = 8 * 2 * 1024;  // based on experiments with create_frame.c bench
  cparams->clevel = 5;
  cparams->compcode = BLOSC_BLOSCLZ;
  int32_t special_nbytes = (int32_t) (nchunks * sizeof(int64_t));
  rc = blosc2_chunk_repeatval(*cparams, special_nbytes, off_chunk, new_off_cbytes, &offset_value);
  free(cparams);
  if (rc < 0) {
    BLOSC_TRACE_ERROR("Error creating a special offsets chunk");
    return BLOSC2_ERROR_DATA;
  }

  // Get the blocksize associated to the sample chunk
  blosc2_cbuffer_sizes(sample_chunk, NULL, NULL, &blocksize);
  free(sample_chunk);
  // and use it for the super-chunk
  schunk->blocksize = blocksize;
  // schunk->blocksize = 0;  // for experimenting with automatic blocksize

  // We have the new offsets; update the frame.
  blosc2_io_cb *io_cb = blosc2_get_io_cb(frame->schunk->storage->io->id);
  if (io_cb == NULL) {
    BLOSC_TRACE_ERROR("Error getting the input/output API");
    return BLOSC2_ERROR_PLUGIN_IO;
  }

  int64_t new_frame_len = header_len + new_off_cbytes + frame->trailer_len;
  void* fp = NULL;
  if (frame->cframe != NULL) {
    uint8_t* framep = frame->cframe;
    /* Make space for the new chunk and copy it */
    frame->cframe = framep = realloc(framep, (size_t)new_frame_len);
    if (framep == NULL) {
      BLOSC_TRACE_ERROR("Cannot realloc space for the frame.");
      return BLOSC2_ERROR_FRAME_SPECIAL;
    }
    /* Copy the offsets */
    memcpy(framep + header_len, off_chunk, (size_t)new_off_cbytes);
  }
  else {
    size_t wbytes;
    int64_t io_pos = 0;
    if (frame->sframe) {
      // Update the offsets chunk in the chunks frame
      fp = sframe_open_index(frame->urlpath, "rb+", frame->schunk->storage->io);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return BLOSC2_ERROR_FILE_OPEN;
      }
      io_pos = frame->file_offset + header_len;
    }
    else {
      // Regular frame
      fp = io_cb->open(frame->urlpath, "rb+", schunk->storage->io->params);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return BLOSC2_ERROR_FILE_OPEN;
      }
      io_pos = frame->file_offset + header_len + cbytes;
    }
    wbytes = io_cb->write(off_chunk, 1, new_off_cbytes, io_pos, fp);  // the new offsets
    io_cb->close(fp);
    if (wbytes != (size_t)new_off_cbytes) {
      BLOSC_TRACE_ERROR("Cannot write the offsets to frame.");
      return BLOSC2_ERROR_FRAME_SPECIAL;
    }
  }

  // Invalidate the cache for chunk offsets
  if (frame->coffsets != NULL) {
    if (frame->coffsets_needs_free)
      free(frame->coffsets);
    frame->coffsets = NULL;
  }
  free(off_chunk);

  frame->len = new_frame_len;
  rc = frame_update_header(frame, schunk, false);
  if (rc < 0) {
    return BLOSC2_ERROR_FRAME_SPECIAL;
  }

  rc = frame_update_trailer(frame, schunk);
  if (rc < 0) {
    return BLOSC2_ERROR_FRAME_SPECIAL;
  }

  return frame->len;
}


/* Append an existing chunk into a frame. */
void* frame_append_chunk(blosc2_frame_s* frame, void* chunk, blosc2_schunk* schunk) {
  int8_t* chunk_ = chunk;
  int32_t header_len;
  int64_t frame_len;
  int64_t nbytes;
  int64_t cbytes;
  int32_t blocksize;
  int32_t chunksize;
  int64_t nchunks;
  int rc = get_header_info(frame, &header_len, &frame_len, &nbytes, &cbytes, &blocksize, &chunksize,
                           &nchunks, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
                           frame->schunk->storage->io);
  if (rc < 0) {
    BLOSC_TRACE_ERROR("Unable to get meta info from frame.");
    return NULL;
  }

  /* The uncompressed and compressed sizes start at byte 4 and 12 */
  int32_t chunk_nbytes;
  int32_t chunk_cbytes;
  rc = blosc2_cbuffer_sizes(chunk, &chunk_nbytes, &chunk_cbytes, NULL);
  if (rc < 0) {
    return NULL;
  }

  if ((nchunks > 0) && (chunk_nbytes > chunksize)) {
    BLOSC_TRACE_ERROR("Appending chunks with a larger chunksize than frame is "
                      "not allowed yet %d != %d.", chunk_nbytes, chunksize);
    return NULL;
  }

  // Check that we are not appending a small chunk after another small chunk
  int32_t chunk_nbytes_last;
  if (chunksize == 0 && (nchunks > 0) && (chunk_nbytes < chunksize)) {
    uint8_t* last_chunk;
    bool needs_free;
    rc = frame_get_lazychunk(frame, nchunks - 1, &last_chunk, &needs_free);
    if (rc < 0) {
      BLOSC_TRACE_ERROR("Cannot get the last chunk (in position %" PRId64 ").", nchunks - 1);
    } else {
      rc = blosc2_cbuffer_sizes(last_chunk, &chunk_nbytes_last, NULL, NULL);
    }
    if (needs_free) {
      free(last_chunk);
    }
    if (rc < 0) {
      return NULL;
    }
    if ((chunk_nbytes_last < chunksize) && (nbytes < chunksize)) {
      BLOSC_TRACE_ERROR("Appending two consecutive chunks with a chunksize smaller "
                        "than the frame chunksize is not allowed yet: %d != %d.",
                        chunk_nbytes, chunksize);
      return NULL;
    }
  }

  // Get the current offsets and add one more
  int32_t off_nbytes = (int32_t) ((nchunks + 1) * sizeof(int64_t));
  int64_t* offsets = (int64_t *) malloc((size_t)off_nbytes);
  if (nchunks > 0) {
    int32_t coffsets_cbytes;
    uint8_t *coffsets = get_coffsets(frame, header_len, cbytes, nchunks, &coffsets_cbytes);
    if (coffsets == NULL) {
      BLOSC_TRACE_ERROR("Cannot get the offsets for the frame.");
      free(offsets);
      return NULL;
    }
    if (coffsets_cbytes == 0) {
      coffsets_cbytes = (int32_t)cbytes;
    }

    // Decompress offsets
    blosc2_dparams off_dparams = BLOSC2_DPARAMS_DEFAULTS;
    blosc2_context *dctx = blosc2_create_dctx(off_dparams);
    if (dctx == NULL) {
      BLOSC_TRACE_ERROR("Error while creating the decompression context");
      return NULL;
    }
    int32_t prev_nbytes = blosc2_decompress_ctx(dctx, coffsets, coffsets_cbytes, offsets,
                                                off_nbytes);
    blosc2_free_ctx(dctx);
    if (prev_nbytes < 0) {
      free(offsets);
      BLOSC_TRACE_ERROR("Cannot decompress the offsets chunk.");
      return NULL;
    }
  }

  // Add the new offset
  int64_t sframe_chunk_id = -1;
  int special_value = (chunk_[BLOSC2_CHUNK_BLOSC2_FLAGS] >> 4) & BLOSC2_SPECIAL_MASK;
  uint64_t offset_value = ((uint64_t)1 << 63);
  switch (special_value) {
    case BLOSC2_SPECIAL_ZERO:
      // Zero chunk.  Code it in a special way.
      offset_value += (uint64_t) BLOSC2_SPECIAL_ZERO << (8 * 7);  // chunk of zeros
      to_little(offsets + nchunks, &offset_value, sizeof(uint64_t));
      chunk_cbytes = 0;   // we don't need to store the chunk
      break;
    case BLOSC2_SPECIAL_UNINIT:
      // Non initizalized values chunk.  Code it in a special way.
      offset_value += (uint64_t) BLOSC2_SPECIAL_UNINIT << (8 * 7);  // chunk of uninit values
      to_little(offsets + nchunks, &offset_value, sizeof(uint64_t));
      chunk_cbytes = 0;   // we don't need to store the chunk
      break;
    case BLOSC2_SPECIAL_NAN:
      // NaN chunk.  Code it in a special way.
      offset_value += (uint64_t)BLOSC2_SPECIAL_NAN << (8 * 7);  // chunk of NANs
      to_little(offsets + nchunks, &offset_value, sizeof(uint64_t));
      chunk_cbytes = 0;   // we don't need to store the chunk
      break;
    default:
      if (frame->sframe) {
        // Compute the sframe_chunk_id value
        for (int i = 0; i < nchunks; ++i) {
          if (offsets[i] > sframe_chunk_id) {
            sframe_chunk_id = offsets[i];
          }
        }
        offsets[nchunks] = ++sframe_chunk_id;
      }
      else {
        offsets[nchunks] = cbytes;
      }
  }

  // Re-compress the offsets again
  blosc2_cparams cparams = BLOSC2_CPARAMS_DEFAULTS;
  cparams.splitmode = BLOSC_NEVER_SPLIT;
  cparams.typesize = sizeof(int64_t);
  cparams.blocksize = 16 * 1024;  // based on experiments with create_frame.c bench
  cparams.nthreads = 4;  // 4 threads seems a decent default for nowadays CPUs
  cparams.compcode = BLOSC_BLOSCLZ;
  blosc2_context* cctx = blosc2_create_cctx(cparams);
  if (cctx == NULL) {
    BLOSC_TRACE_ERROR("Error while creating the compression context");
    return NULL;
  }
  cctx->typesize = sizeof(int64_t);  // override a possible BLOSC_TYPESIZE env variable (or chaos may appear)
  void* off_chunk = malloc((size_t)off_nbytes + BLOSC2_MAX_OVERHEAD);
  int32_t new_off_cbytes = blosc2_compress_ctx(cctx, offsets, off_nbytes,
                                               off_chunk, off_nbytes + BLOSC2_MAX_OVERHEAD);
  blosc2_free_ctx(cctx);
  free(offsets);
  if (new_off_cbytes < 0) {
    free(off_chunk);
    return NULL;
  }
  // printf("%f\n", (double) off_nbytes / new_off_cbytes);

  int64_t new_cbytes = cbytes + chunk_cbytes;
  int64_t new_frame_len;
  if (frame->sframe) {
    new_frame_len = header_len + 0 + new_off_cbytes + frame->trailer_len;
  }
  else {
    new_frame_len = header_len + new_cbytes + new_off_cbytes + frame->trailer_len;
  }

  void* fp = NULL;
  if (frame->cframe != NULL) {
    uint8_t* framep = frame->cframe;
    /* Make space for the new chunk and copy it */
    frame->cframe = framep = realloc(framep, (size_t)new_frame_len);
    if (framep == NULL) {
      BLOSC_TRACE_ERROR("Cannot realloc space for the frame.");
      return NULL;
    }
    /* Copy the chunk */
    memcpy(framep + header_len + cbytes, chunk, (size_t)chunk_cbytes);
    /* Copy the offsets */
    memcpy(framep + header_len + new_cbytes, off_chunk, (size_t)new_off_cbytes);
  }
  else {
    int64_t wbytes;
    int64_t io_pos = 0;
    blosc2_io_cb *io_cb = blosc2_get_io_cb(frame->schunk->storage->io->id);
    if (io_cb == NULL) {
      BLOSC_TRACE_ERROR("Error getting the input/output API");
      return NULL;
    }

    if (frame->sframe) {
      // Update the offsets chunk in the chunks frame
      if (chunk_cbytes != 0) {
        if (sframe_chunk_id < 0) {
          BLOSC_TRACE_ERROR("The chunk id (%" PRId64 ") is not correct", sframe_chunk_id);
          return NULL;
        }
        if (sframe_create_chunk(frame, chunk, sframe_chunk_id, chunk_cbytes) == NULL) {
          BLOSC_TRACE_ERROR("Cannot write the full chunk.");
          return NULL;
        }
      }
      fp = sframe_open_index(frame->urlpath, "rb+",
                             frame->schunk->storage->io);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return NULL;
      }
      io_pos = frame->file_offset + header_len;
    }
    else {
      // Regular frame
      fp = io_cb->open(frame->urlpath, "rb+", frame->schunk->storage->io->params);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return NULL;
      }
      io_pos = frame->file_offset + header_len + cbytes;
      wbytes = io_cb->write(chunk, 1, chunk_cbytes, io_pos, fp);  // the new chunk
      io_pos += chunk_cbytes;
      if (wbytes != chunk_cbytes) {
        BLOSC_TRACE_ERROR("Cannot write the full chunk to frame.");
        io_cb->close(fp);
        return NULL;
      }
    }
    wbytes = io_cb->write(off_chunk, 1, new_off_cbytes, io_pos, fp);  // the new offsets
    io_cb->close(fp);
    if (wbytes != new_off_cbytes) {
      BLOSC_TRACE_ERROR("Cannot write the offsets to frame.");
      return NULL;
    }
  }
  // Invalidate the cache for chunk offsets
  if (frame->coffsets != NULL) {
    if (frame->coffsets_needs_free)
      free(frame->coffsets);
    frame->coffsets = NULL;
  }
  free(chunk);  // chunk has always to be a copy when reaching here...
  free(off_chunk);

  frame->len = new_frame_len;
  rc = frame_update_header(frame, schunk, false);
  if (rc < 0) {
    return NULL;
  }

  rc = frame_update_trailer(frame, schunk);
  if (rc < 0) {
    return NULL;
  }

  return frame;
}


void* frame_insert_chunk(blosc2_frame_s* frame, int64_t nchunk, void* chunk, blosc2_schunk* schunk) {
  uint8_t* chunk_ = chunk;
  int32_t header_len;
  int64_t frame_len;
  int64_t nbytes;
  int64_t cbytes;
  int32_t blocksize;
  int32_t chunksize;
  int64_t nchunks;
  int rc = get_header_info(frame, &header_len, &frame_len, &nbytes, &cbytes,
                           &blocksize, &chunksize, &nchunks,
                           NULL, NULL, NULL, NULL, NULL, NULL, NULL,
                           frame->schunk->storage->io);
  if (rc < 0) {
    BLOSC_TRACE_ERROR("Unable to get meta info from frame.");
    return NULL;
  }
  int32_t chunk_cbytes;
  rc = blosc2_cbuffer_sizes(chunk_, NULL, &chunk_cbytes, NULL);
  if (rc < 0) {
    return NULL;
  }

  // Get the current offsets
  int32_t off_nbytes = (int32_t) ((nchunks + 1) * sizeof(int64_t));
  int64_t* offsets = (int64_t *) malloc((size_t)off_nbytes);
  if (nchunks > 0) {
    int32_t coffsets_cbytes = 0;
    uint8_t *coffsets = get_coffsets(frame, header_len, cbytes, nchunks, &coffsets_cbytes);
    if (coffsets == NULL) {
      BLOSC_TRACE_ERROR("Cannot get the offsets for the frame.");
      return NULL;
    }
    if (coffsets_cbytes == 0) {
      coffsets_cbytes = (int32_t)cbytes;
    }

    // Decompress offsets
    blosc2_dparams off_dparams = BLOSC2_DPARAMS_DEFAULTS;
    blosc2_context *dctx = blosc2_create_dctx(off_dparams);
    if (dctx == NULL) {
      BLOSC_TRACE_ERROR("Error while creating the decompression context");
      return NULL;
    }
    int32_t prev_nbytes = blosc2_decompress_ctx(dctx, coffsets, coffsets_cbytes, offsets, off_nbytes);
    blosc2_free_ctx(dctx);
    if (prev_nbytes < 0) {
      free(offsets);
      BLOSC_TRACE_ERROR("Cannot decompress the offsets chunk.");
      return NULL;
    }
  }

  // TODO: Improvement: Check if new chunk is smaller than previous one

  // Move offsets
  for (int64_t i = nchunks; i > nchunk; i--) {
    offsets[i] = offsets[i - 1];
  }
  // Add the new offset
  int64_t sframe_chunk_id = -1;
  int special_value = (chunk_[BLOSC2_CHUNK_BLOSC2_FLAGS] >> 4) & BLOSC2_SPECIAL_MASK;
  uint64_t offset_value = ((uint64_t)1 << 63);
  switch (special_value) {
    case BLOSC2_SPECIAL_ZERO:
      // Zero chunk.  Code it in a special way.
      offset_value += (uint64_t)BLOSC2_SPECIAL_ZERO << (8 * 7);  // indicate a chunk of zeros
      to_little(offsets + nchunk, &offset_value, sizeof(uint64_t));
      chunk_cbytes = 0;   // we don't need to store the chunk
      break;
    case BLOSC2_SPECIAL_UNINIT:
      // Non initizalized values chunk.  Code it in a special way.
      offset_value += (uint64_t) BLOSC2_SPECIAL_UNINIT << (8 * 7);  // chunk of uninit values
      to_little(offsets + nchunk, &offset_value, sizeof(uint64_t));
      chunk_cbytes = 0;   // we don't need to store the chunk
      break;
    case BLOSC2_SPECIAL_NAN:
      // NaN chunk.  Code it in a special way.
      offset_value += (uint64_t)BLOSC2_SPECIAL_NAN << (8 * 7);  // indicate a chunk of NANs
      to_little(offsets + nchunk, &offset_value, sizeof(uint64_t));
      chunk_cbytes = 0;   // we don't need to store the chunk
      break;
    default:
      if (frame->sframe) {
        for (int i = 0; i <= nchunks; ++i) {
          // offsets[nchunk] is still uninitialized here
          if (i != nchunk && offsets[i] > sframe_chunk_id) {
            sframe_chunk_id = offsets[i];
          }
        }
        offsets[nchunk] = ++sframe_chunk_id;
      }
      else {
        offsets[nchunk] = cbytes;
      }
  }

  // Re-compress the offsets again
  blosc2_cparams cparams = BLOSC2_CPARAMS_DEFAULTS;
  cparams.splitmode = BLOSC_NEVER_SPLIT;
  cparams.typesize = sizeof(int64_t);
  cparams.blocksize = 16 * 1024;  // based on experiments with create_frame.c bench
  cparams.nthreads = 4;  // 4 threads seems a decent default for nowadays CPUs
  cparams.compcode = BLOSC_BLOSCLZ;
  blosc2_context* cctx = blosc2_create_cctx(cparams);
  if (cctx == NULL) {
    BLOSC_TRACE_ERROR("Error while creating the compression context");
    return NULL;
  }
  void* off_chunk = malloc((size_t)off_nbytes + BLOSC2_MAX_OVERHEAD);
  int32_t new_off_cbytes = blosc2_compress_ctx(cctx, offsets, off_nbytes,
                                               off_chunk, off_nbytes + BLOSC2_MAX_OVERHEAD);
  blosc2_free_ctx(cctx);

  free(offsets);
  if (new_off_cbytes < 0) {
    free(off_chunk);
    return NULL;
  }

  int64_t new_cbytes = cbytes + chunk_cbytes;

  int64_t new_frame_len;
  if (frame->sframe) {
    new_frame_len = header_len + 0 + new_off_cbytes + frame->trailer_len;
  }
  else {
    new_frame_len = header_len + new_cbytes + new_off_cbytes + frame->trailer_len;
  }

  // Add the chunk and update meta
  void* fp = NULL;
  if (frame->cframe != NULL) {
    uint8_t* framep = frame->cframe;
    /* Make space for the new chunk and copy it */
    frame->cframe = framep = realloc(framep, (size_t)new_frame_len);
    if (framep == NULL) {
      BLOSC_TRACE_ERROR("Cannot realloc space for the frame.");
      return NULL;
    }
    /* Copy the chunk */
    memcpy(framep + header_len + cbytes, chunk, (size_t)chunk_cbytes);
    /* Copy the offsets */
    memcpy(framep + header_len + new_cbytes, off_chunk, (size_t)new_off_cbytes);
  } else {
    int64_t wbytes;

    int64_t io_pos = 0;
    blosc2_io_cb *io_cb = blosc2_get_io_cb(frame->schunk->storage->io->id);
    if (io_cb == NULL) {
      BLOSC_TRACE_ERROR("Error getting the input/output API");
      return NULL;
    }

    if (frame->sframe) {
      if (chunk_cbytes != 0) {
        if (sframe_chunk_id < 0) {
          BLOSC_TRACE_ERROR("The chunk id (%" PRId64 ") is not correct", sframe_chunk_id);
          return NULL;
        }
        if (sframe_create_chunk(frame, chunk, sframe_chunk_id, chunk_cbytes) == NULL) {
          BLOSC_TRACE_ERROR("Cannot write the full chunk.");
          return NULL;
        }
      }
      // Update the offsets chunk in the chunks frame
      fp = sframe_open_index(frame->urlpath, "rb+",
                             frame->schunk->storage->io);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return NULL;
      }
      io_pos = frame->file_offset + header_len + 0;
    }
    else {
      // Regular frame
      fp = io_cb->open(frame->urlpath, "rb+", frame->schunk->storage->io->params);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return NULL;
      }
      io_pos = frame->file_offset + header_len + cbytes;
      wbytes = io_cb->write(chunk, 1, chunk_cbytes, io_pos, fp);  // the new chunk
      io_pos += chunk_cbytes;
      if (wbytes != chunk_cbytes) {
        BLOSC_TRACE_ERROR("Cannot write the full chunk to frame.");
        io_cb->close(fp);
        return NULL;
      }
    }
    wbytes = io_cb->write(off_chunk, 1, new_off_cbytes, io_pos, fp);  // the new offsets
    io_cb->close(fp);
    if (wbytes != new_off_cbytes) {
      BLOSC_TRACE_ERROR("Cannot write the offsets to frame.");
      return NULL;
    }
    // Invalidate the cache for chunk offsets
    if (frame->coffsets != NULL) {
      if (frame->coffsets_needs_free)
        free(frame->coffsets);
      frame->coffsets = NULL;
    }
  }
  free(chunk);  // chunk has always to be a copy when reaching here...
  free(off_chunk);

  frame->len = new_frame_len;
  rc = frame_update_header(frame, schunk, false);
  if (rc < 0) {
    return NULL;
  }

  rc = frame_update_trailer(frame, schunk);
  if (rc < 0) {
    return NULL;
  }

  return frame;
}


void* frame_update_chunk(blosc2_frame_s* frame, int64_t nchunk, void* chunk, blosc2_schunk* schunk) {
  uint8_t *chunk_ = (uint8_t *) chunk;
  int32_t header_len;
  int64_t frame_len;
  int64_t nbytes;
  int64_t cbytes;
  int32_t blocksize;
  int32_t chunksize;
  int64_t nchunks;
  int rc = get_header_info(frame, &header_len, &frame_len, &nbytes, &cbytes,
                           &blocksize, &chunksize, &nchunks,
                           NULL, NULL, NULL, NULL, NULL, NULL, NULL,
                           frame->schunk->storage->io);
  if (rc < 0) {
    BLOSC_TRACE_ERROR("Unable to get meta info from frame.");
    return NULL;
  }
  if (nchunk >= nchunks) {
    BLOSC_TRACE_ERROR("The chunk must already exist.");
    return NULL;
  }

  int32_t chunk_cbytes;
  rc = blosc2_cbuffer_sizes(chunk, NULL, &chunk_cbytes, NULL);
  if (rc < 0) {
    return NULL;
  }

  // Get the current offsets
  int32_t off_nbytes = (int32_t) (nchunks * sizeof(int64_t));
  int64_t* offsets = (int64_t *) malloc((size_t)off_nbytes);
  if (nchunks > 0) {
    int32_t coffsets_cbytes = 0;
    uint8_t *coffsets = get_coffsets(frame, header_len, cbytes, nchunks, &coffsets_cbytes);
    if (coffsets == NULL) {
      BLOSC_TRACE_ERROR("Cannot get the offsets for the frame.");
      return NULL;
    }
    if (coffsets_cbytes == 0) {
      coffsets_cbytes = (int32_t)cbytes;
    }

    // Decompress offsets
    blosc2_dparams off_dparams = BLOSC2_DPARAMS_DEFAULTS;
    blosc2_context *dctx = blosc2_create_dctx(off_dparams);
    if (dctx == NULL) {
      BLOSC_TRACE_ERROR("Error while creating the decompression context");
      return NULL;
    }
    int32_t prev_nbytes = blosc2_decompress_ctx(dctx, coffsets, coffsets_cbytes, offsets, off_nbytes);
    blosc2_free_ctx(dctx);
    if (prev_nbytes < 0) {
      free(offsets);
      BLOSC_TRACE_ERROR("Cannot decompress the offsets chunk.");
      return NULL;
    }
  }
  int32_t cbytes_old = 0;
  int64_t old_offset = 0;
  if (!frame->sframe) {
    // See how big would be the space
    old_offset = offsets[nchunk];
    bool needs_free;
    uint8_t *chunk_old;
    int err = blosc2_schunk_get_chunk(schunk, nchunk, &chunk_old, &needs_free);
    if (err < 0) {
      BLOSC_TRACE_ERROR("%" PRId64 " chunk can not be obtained from schunk.", nchunk);
      return NULL;
    }

    if (chunk_old == NULL) {
      cbytes_old = 0;
    }
    else {
      cbytes_old = sw32_(chunk_old + BLOSC2_CHUNK_CBYTES);
      if (cbytes_old == BLOSC2_MAX_OVERHEAD) {
        cbytes_old = 0;
      }
    }
    if (needs_free) {
      free(chunk_old);
    }
  }

  // Add the new offset
  int64_t sframe_chunk_id = -1;
  if (frame->sframe) {
    if (offsets[nchunk] < 0) {
      sframe_chunk_id = -1;
    }
    else {
      // In case there was a reorder in a sframe
      sframe_chunk_id = offsets[nchunk];
    }
  }
  int special_value = (chunk_[BLOSC2_CHUNK_BLOSC2_FLAGS] >> 4) & BLOSC2_SPECIAL_MASK;
  uint64_t offset_value = ((uint64_t)1 << 63);
  switch (special_value) {
    case BLOSC2_SPECIAL_ZERO:
      // Zero chunk.  Code it in a special way.
      offset_value += (uint64_t)BLOSC2_SPECIAL_ZERO << (8 * 7);  // indicate a chunk of zeros
      to_little(offsets + nchunk, &offset_value, sizeof(uint64_t));
      chunk_cbytes = 0;   // we don't need to store the chunk
      break;
    case BLOSC2_SPECIAL_UNINIT:
      // Non initizalized values chunk.  Code it in a special way.
      offset_value += (uint64_t)BLOSC2_SPECIAL_UNINIT << (8 * 7);  // indicate a chunk of uninit values
      to_little(offsets + nchunk, &offset_value, sizeof(uint64_t));
      chunk_cbytes = 0;   // we don't need to store the chunk
      break;
    case BLOSC2_SPECIAL_NAN:
      // NaN chunk.  Code it in a special way.
      offset_value += (uint64_t)BLOSC2_SPECIAL_NAN << (8 * 7);  // indicate a chunk of NANs
      to_little(offsets + nchunk, &offset_value, sizeof(uint64_t));
      chunk_cbytes = 0;   // we don't need to store the chunk
      break;
    default:
      if (frame->sframe) {
        if (sframe_chunk_id < 0) {
          for (int i = 0; i < nchunks; ++i) {
            if (offsets[i] > sframe_chunk_id) {
              sframe_chunk_id = offsets[i];
            }
          }
          offsets[nchunk] = ++sframe_chunk_id;
        }
      }
      else {
        // Add the new offset
        offsets[nchunk] = cbytes;
      }
  }

  if (!frame->sframe && chunk_cbytes != 0 && cbytes_old >= chunk_cbytes) {
    offsets[nchunk] = old_offset;
    cbytes = old_offset;
  }
  // Re-compress the offsets again
  blosc2_cparams cparams = BLOSC2_CPARAMS_DEFAULTS;
  cparams.splitmode = BLOSC_NEVER_SPLIT;
  cparams.typesize = sizeof(int64_t);
  cparams.blocksize = 16 * 1024;  // based on experiments with create_frame.c bench
  cparams.nthreads = 4;  // 4 threads seems a decent default for nowadays CPUs
  cparams.compcode = BLOSC_BLOSCLZ;
  blosc2_context* cctx = blosc2_create_cctx(cparams);
  if (cctx == NULL) {
    BLOSC_TRACE_ERROR("Error while creating the compression context");
    return NULL;
  }
  void* off_chunk = malloc((size_t)off_nbytes + BLOSC2_MAX_OVERHEAD);
  int32_t new_off_cbytes = blosc2_compress_ctx(cctx, offsets, off_nbytes,
                                               off_chunk, off_nbytes + BLOSC2_MAX_OVERHEAD);
  blosc2_free_ctx(cctx);

  free(offsets);
  if (new_off_cbytes < 0) {
    free(off_chunk);
    return NULL;
  }

  int64_t new_cbytes = schunk->cbytes;
  int64_t new_frame_len;
  if (frame->sframe) {
    // The chunk is not stored in the frame
    new_frame_len = header_len + 0 + new_off_cbytes + frame->trailer_len;
  }
  else {
    new_frame_len = header_len + new_cbytes + new_off_cbytes + frame->trailer_len;
  }

  void* fp = NULL;
  if (frame->cframe != NULL) {
    uint8_t* framep = frame->cframe;
    /* Make space for the new chunk and copy it */
    frame->cframe = framep = realloc(framep, (size_t)new_frame_len);
    if (framep == NULL) {
      BLOSC_TRACE_ERROR("Cannot realloc space for the frame.");
      return NULL;
    }
    /* Copy the chunk */
    memcpy(framep + header_len + cbytes, chunk, (size_t)chunk_cbytes);
    /* Copy the offsets */
    memcpy(framep + header_len + new_cbytes, off_chunk, (size_t)new_off_cbytes);
  } else {
    int64_t wbytes;

    int64_t io_pos = 0;
    blosc2_io_cb *io_cb = blosc2_get_io_cb(frame->schunk->storage->io->id);
    if (io_cb == NULL) {
      BLOSC_TRACE_ERROR("Error getting the input/output API");
      return NULL;
    }

    if (frame->sframe) {
      // Create the chunks file, if it's a special value this will delete its old content
      if (sframe_chunk_id >= 0) {
        if (sframe_create_chunk(frame, chunk, sframe_chunk_id, chunk_cbytes) == NULL) {
          BLOSC_TRACE_ERROR("Cannot write the full chunk.");
          return NULL;
        }
      }
      // Update the offsets chunk in the chunks frame
      fp = sframe_open_index(frame->urlpath, "rb+",
                             frame->schunk->storage->io);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return NULL;
      }
      io_pos = frame->file_offset + header_len + 0;
    }
    else {
      // Regular frame
      fp = io_cb->open(frame->urlpath, "rb+", frame->schunk->storage->io->params);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return NULL;
      }
      io_pos = frame->file_offset + header_len + cbytes;
      wbytes = io_cb->write(chunk, 1, chunk_cbytes, io_pos, fp);  // the new chunk
      io_pos += chunk_cbytes;
      if (wbytes != chunk_cbytes) {
        BLOSC_TRACE_ERROR("Cannot write the full chunk to frame.");
        io_cb->close(fp);
        return NULL;
      }
      io_pos = frame->file_offset + header_len + new_cbytes;
    }
    wbytes = io_cb->write(off_chunk, 1, new_off_cbytes, io_pos, fp);  // the new offsets
    io_cb->close(fp);
    if (wbytes != new_off_cbytes) {
      BLOSC_TRACE_ERROR("Cannot write the offsets to frame.");
      return NULL;
    }
    // Invalidate the cache for chunk offsets
    if (frame->coffsets != NULL) {
      if (frame->coffsets_needs_free)
        free(frame->coffsets);
      frame->coffsets = NULL;
    }
  }
  free(chunk);  // chunk has always to be a copy when reaching here...
  free(off_chunk);

  frame->len = new_frame_len;
  rc = frame_update_header(frame, schunk, false);
  if (rc < 0) {
    return NULL;
  }

  rc = frame_update_trailer(frame, schunk);
  if (rc < 0) {
    return NULL;
  }

  return frame;
}


void* frame_delete_chunk(blosc2_frame_s* frame, int64_t nchunk, blosc2_schunk* schunk) {
  int32_t header_len;
  int64_t frame_len;
  int64_t nbytes;
  int64_t cbytes;
  int32_t blocksize;
  int32_t chunksize;
  int64_t nchunks;
  int rc = get_header_info(frame, &header_len, &frame_len, &nbytes, &cbytes,
                           &blocksize, &chunksize,  &nchunks,
                           NULL, NULL, NULL, NULL, NULL, NULL, NULL, frame->schunk->storage->io);
  if (rc < 0) {
    BLOSC_TRACE_ERROR("Unable to get meta info from frame.");
    return NULL;
  }

  // Get the current offsets
  int32_t off_nbytes = (int32_t) (nchunks * sizeof(int64_t));
  int64_t* offsets = (int64_t *) malloc((size_t)off_nbytes);
  if (nchunks > 0) {
    int32_t coffsets_cbytes = 0;
    uint8_t *coffsets = get_coffsets(frame, header_len, cbytes, nchunks, &coffsets_cbytes);
    if (coffsets == NULL) {
      BLOSC_TRACE_ERROR("Cannot get the offsets for the frame.");
      return NULL;
    }
    if (coffsets_cbytes == 0) {
      coffsets_cbytes = (int32_t)cbytes;
    }

    // Decompress offsets
    blosc2_dparams off_dparams = BLOSC2_DPARAMS_DEFAULTS;
    blosc2_context *dctx = blosc2_create_dctx(off_dparams);
    if (dctx == NULL) {
      BLOSC_TRACE_ERROR("Error while creating the decompression context");
      return NULL;
    }
    int32_t prev_nbytes = blosc2_decompress_ctx(dctx, coffsets, coffsets_cbytes, offsets, off_nbytes);
    blosc2_free_ctx(dctx);
    if (prev_nbytes < 0) {
      free(offsets);
      BLOSC_TRACE_ERROR("Cannot decompress the offsets chunk.");
      return NULL;
    }
  }

  // Delete the new offset
  for (int64_t i = nchunk; i < nchunks - 1; i++) {
    offsets[i] = offsets[i + 1];
  }
  offsets[nchunks - 1] = 0;

  // Re-compress the offsets again
  blosc2_cparams cparams = BLOSC2_CPARAMS_DEFAULTS;
  cparams.splitmode = BLOSC_NEVER_SPLIT;
  cparams.typesize = sizeof(int64_t);
  cparams.blocksize = 16 * 1024;  // based on experiments with create_frame.c bench
  cparams.nthreads = 4;  // 4 threads seems a decent default for nowadays CPUs
  cparams.compcode = BLOSC_BLOSCLZ;
  blosc2_context* cctx = blosc2_create_cctx(cparams);
  if (cctx == NULL) {
    BLOSC_TRACE_ERROR("Error while creating the compression context");
    return NULL;
  }
  void* off_chunk = malloc((size_t)off_nbytes + BLOSC2_MAX_OVERHEAD);
  int32_t new_off_cbytes = blosc2_compress_ctx(cctx, offsets, off_nbytes - (int32_t)sizeof(int64_t),
                                               off_chunk, off_nbytes + BLOSC2_MAX_OVERHEAD);
  blosc2_free_ctx(cctx);

  free(offsets);
  if (new_off_cbytes < 0) {
    free(off_chunk);
    return NULL;
  }

  int64_t new_cbytes = cbytes;

  int64_t new_frame_len;
  if (frame->sframe) {
    new_frame_len = header_len + 0 + new_off_cbytes + frame->trailer_len;
  }
  else {
    new_frame_len = header_len + new_cbytes + new_off_cbytes + frame->trailer_len;
  }

  // Add the chunk and update meta
  FILE* fp = NULL;
  if (frame->cframe != NULL) {
    uint8_t* framep = frame->cframe;
    /* Make space for the new chunk and copy it */
    frame->cframe = framep = realloc(framep, (size_t)new_frame_len);
    if (framep == NULL) {
      BLOSC_TRACE_ERROR("Cannot realloc space for the frame.");
      return NULL;
    }
    /* Copy the offsets */
    memcpy(framep + header_len + new_cbytes, off_chunk, (size_t)new_off_cbytes);
  } else {
    blosc2_io_cb *io_cb = blosc2_get_io_cb(frame->schunk->storage->io->id);
    if (io_cb == NULL) {
      BLOSC_TRACE_ERROR("Error getting the input/output API");
      return NULL;
    }

    size_t wbytes;
    int64_t io_pos = 0;
    if (frame->sframe) {
      int64_t offset;
      rc = get_coffset(frame, header_len, cbytes, nchunk, nchunks, &offset);
      if (rc < 0) {
        BLOSC_TRACE_ERROR("Unable to get offset to chunk %" PRId64 ".", nchunk);
        return NULL;
      }
      if (offset >= 0){
        // Remove the chunk file only if it is not a special value chunk
        int err = sframe_delete_chunk(frame->urlpath, offset);
        if (err != 0) {
          BLOSC_TRACE_ERROR("Unable to delete chunk!");
          return NULL;
        }
      }
      // Update the offsets chunk in the chunks frame
      fp = sframe_open_index(frame->urlpath, "rb+", frame->schunk->storage->io);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return NULL;
      }
      io_pos = frame->file_offset + header_len + 0;
    }
    else {
      // Regular frame
      fp = io_cb->open(frame->urlpath, "rb+", frame->schunk->storage->io);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return NULL;
      }
      io_pos = frame->file_offset + header_len + cbytes;
    }
    wbytes = io_cb->write(off_chunk, 1, new_off_cbytes, io_pos, fp);  // the new offsets
    io_cb->close(fp);
    if (wbytes != (size_t)new_off_cbytes) {
      BLOSC_TRACE_ERROR("Cannot write the offsets to frame.");
      return NULL;
    }
    // Invalidate the cache for chunk offsets
    if (frame->coffsets != NULL) {
      if (frame->coffsets_needs_free)
        free(frame->coffsets);
      frame->coffsets = NULL;
    }
  }
  free(off_chunk);

  frame->len = new_frame_len;
  rc = frame_update_header(frame, schunk, false);
  if (rc < 0) {
    return NULL;
  }

  rc = frame_update_trailer(frame, schunk);
  if (rc < 0) {
    return NULL;
  }

  return frame;
}


int frame_reorder_offsets(blosc2_frame_s* frame, const int64_t* offsets_order, blosc2_schunk* schunk) {
  // Get header info
  int32_t header_len;
  int64_t frame_len;
  int64_t nbytes;
  int64_t cbytes;
  int32_t blocksize;
  int32_t chunksize;
  int64_t nchunks;
  int ret = get_header_info(frame, &header_len, &frame_len, &nbytes, &cbytes,
                            &blocksize, &chunksize, &nchunks,
                            NULL, NULL, NULL, NULL, NULL, NULL, NULL,
                            frame->schunk->storage->io);
  if (ret < 0) {
      BLOSC_TRACE_ERROR("Cannot get the header info for the frame.");
      return ret;
  }

  // Get the current offsets and add one more
  int32_t off_nbytes = (int32_t) (nchunks * sizeof(int64_t));
  int64_t* offsets = (int64_t *) malloc((size_t)off_nbytes);

  int32_t coffsets_cbytes = 0;
  uint8_t *coffsets = get_coffsets(frame, header_len, cbytes, nchunks, &coffsets_cbytes);
  if (coffsets == NULL) {
    BLOSC_TRACE_ERROR("Cannot get the offsets for the frame.");
    free(offsets);
    return BLOSC2_ERROR_DATA;
  }

  // Decompress offsets
  blosc2_dparams off_dparams = BLOSC2_DPARAMS_DEFAULTS;
  blosc2_context *dctx = blosc2_create_dctx(off_dparams);
  if (dctx == NULL) {
    BLOSC_TRACE_ERROR("Error while creating the decompression context");
    return BLOSC2_ERROR_NULL_POINTER;
  }
  int32_t prev_nbytes = blosc2_decompress_ctx(dctx, coffsets, coffsets_cbytes,
                                              offsets, off_nbytes);
  blosc2_free_ctx(dctx);
  if (prev_nbytes < 0) {
    free(offsets);
    BLOSC_TRACE_ERROR("Cannot decompress the offsets chunk.");
    return prev_nbytes;
  }

  // Make a copy of the chunk offsets and reorder it
  int64_t *offsets_copy = malloc(prev_nbytes);
  memcpy(offsets_copy, offsets, prev_nbytes);

  for (int i = 0; i < nchunks; ++i) {
    offsets[i] = offsets_copy[offsets_order[i]];
  }
  free(offsets_copy);

  // Re-compress the offsets again
  blosc2_cparams cparams = BLOSC2_CPARAMS_DEFAULTS;
  cparams.splitmode = BLOSC_NEVER_SPLIT;
  cparams.typesize = sizeof(int64_t);
  cparams.blocksize = 16 * 1024;  // based on experiments with create_frame.c bench
  cparams.nthreads = 4;  // 4 threads seems a decent default for nowadays CPUs
  cparams.compcode = BLOSC_BLOSCLZ;
  blosc2_context* cctx = blosc2_create_cctx(cparams);
  if (cctx == NULL) {
    BLOSC_TRACE_ERROR("Error while creating the compression context");
    return BLOSC2_ERROR_NULL_POINTER;
  }
  void* off_chunk = malloc((size_t)off_nbytes + BLOSC2_MAX_OVERHEAD);
  int32_t new_off_cbytes = blosc2_compress_ctx(cctx, offsets, off_nbytes,
                                               off_chunk, off_nbytes + BLOSC2_MAX_OVERHEAD);
  blosc2_free_ctx(cctx);

  if (new_off_cbytes < 0) {
    free(offsets);
    free(off_chunk);
    return new_off_cbytes;
  }
  free(offsets);
  int64_t new_frame_len;
  if (frame->sframe) {
    // The chunks are not in the frame
    new_frame_len = header_len + 0 + new_off_cbytes + frame->trailer_len;
  }
  else {
    new_frame_len = header_len + cbytes + new_off_cbytes + frame->trailer_len;
  }

  if (frame->cframe != NULL) {
    uint8_t* framep = frame->cframe;
    /* Make space for the new chunk and copy it */
    frame->cframe = framep = realloc(framep, (size_t)new_frame_len);
    if (framep == NULL) {
      BLOSC_TRACE_ERROR("Cannot realloc space for the frame.");
      return BLOSC2_ERROR_MEMORY_ALLOC;
    }
    /* Copy the offsets */
    memcpy(framep + header_len + cbytes, off_chunk, (size_t)new_off_cbytes);
  }
  else {
    void* fp = NULL;

    blosc2_io_cb *io_cb = blosc2_get_io_cb(frame->schunk->storage->io->id);
    if (io_cb == NULL) {
      BLOSC_TRACE_ERROR("Error getting the input/output API");
      return BLOSC2_ERROR_PLUGIN_IO;
    }

    int64_t io_pos = 0;
    if (frame->sframe) {
      // Update the offsets chunk in the chunks frame
      fp = sframe_open_index(frame->urlpath, "rb+",
                             frame->schunk->storage->io);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return BLOSC2_ERROR_FILE_OPEN;
      }
      io_pos = frame->file_offset + header_len + 0;
    }
    else {
      // Regular frame
      fp = io_cb->open(frame->urlpath, "rb+", frame->schunk->storage->io->params);
      if (fp == NULL) {
        BLOSC_TRACE_ERROR("Error opening file in: %s", frame->urlpath);
        return BLOSC2_ERROR_FILE_OPEN;
      }
      io_pos = frame->file_offset + header_len + cbytes;
    }
    int64_t wbytes = io_cb->write(off_chunk, 1, new_off_cbytes, io_pos, fp);  // the new offsets
    io_cb->close(fp);
    if (wbytes != new_off_cbytes) {
      BLOSC_TRACE_ERROR("Cannot write the offsets to frame.");
      return BLOSC2_ERROR_FILE_WRITE;
    }
  }

  // Invalidate the cache for chunk offsets
  if (frame->coffsets != NULL) {
    if (frame->coffsets_needs_free)
      free(frame->coffsets);
    frame->coffsets = NULL;
  }
  free(off_chunk);

  frame->len = new_frame_len;
  int rc = frame_update_header(frame, schunk, false);
  if (rc < 0) {
    return rc;
  }

  rc = frame_update_trailer(frame, schunk);
  if (rc < 0) {
    return rc;
  }

  return 0;
}


/* Decompress and return a chunk that is part of a frame. */
int frame_decompress_chunk(blosc2_context *dctx, blosc2_frame_s* frame, int64_t nchunk, void *dest, int32_t nbytes) {
  uint8_t* src;
  bool needs_free;
  int32_t chunk_nbytes;
  int32_t chunk_cbytes;
  int rc;

  // Use a lazychunk here in order to do a potential parallel read.
  rc = frame_get_lazychunk(frame, nchunk, &src, &needs_free);
  if (rc < 0) {
    BLOSC_TRACE_ERROR("Cannot get the chunk in position %" PRId64 ".", nchunk);
    goto end;
  }
  chunk_cbytes = rc;
  if (chunk_cbytes < (signed)sizeof(int32_t)) {
    /* Not enough input to read `nbytes` */
    rc = BLOSC2_ERROR_READ_BUFFER;
  }

  rc = blosc2_cbuffer_sizes(src, &chunk_nbytes, &chunk_cbytes, NULL);
  if (rc < 0) {
    goto end;
  }

  /* Create a buffer for destination */
  if (chunk_nbytes > nbytes) {
    BLOSC_TRACE_ERROR("Not enough space for decompressing in dest.");
    rc = BLOSC2_ERROR_WRITE_BUFFER;
    goto end;
  }
  /* And decompress it */
  dctx->header_overhead = BLOSC_EXTENDED_HEADER_LENGTH;
  int chunksize = rc = blosc2_decompress_ctx(dctx, src, chunk_cbytes, dest, nbytes);
  if (chunksize < 0 || chunksize != chunk_nbytes) {
    BLOSC_TRACE_ERROR("Error in decompressing chunk.");
    if (chunksize >= 0)
      rc = BLOSC2_ERROR_FAILURE;
  }
  end:
  if (needs_free) {
    free(src);
  }
  return rc;
}


*/