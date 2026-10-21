use crate::codecs::blosclz;
use crate::filters;
use crate::internal::constants::*;
use std::io::Write;

pub mod constants;

/// Convert compressor code to compressor format (for header flags byte).
///
/// Blosc1 headers store `BLOSC_SNAPPY_LIB` (2) for snappy, while blosc2 extended
/// headers store the raw compcode in byte 22 and the format field is less critical.
/// Pass `is_blosc1 = true` when writing a 16-byte blosc1 header.
fn compcode_to_compformat(compcode: u8, is_blosc1: bool) -> u8 {
    match compcode {
        BLOSC_BLOSCLZ => BLOSC_BLOSCLZ_FORMAT,
        BLOSC_LZ4 => BLOSC_LZ4_FORMAT,
        BLOSC_LZ4HC => BLOSC_LZ4HC_FORMAT,
        BLOSC_SNAPPY => {
            if is_blosc1 {
                BLOSC_SNAPPY_FORMAT // 2 (BLOSC_SNAPPY_LIB in blosc1)
            } else {
                BLOSC_SNAPPY // 3 (no separate lib code in blosc2)
            }
        }
        BLOSC_ZLIB => BLOSC_ZLIB_FORMAT,
        BLOSC_ZSTD => BLOSC_ZSTD_FORMAT,
        _ => compcode,
    }
}

/// Convert compressor format (from header flags byte) back to compressor code.
///
/// Blosc1 headers store the compressor *library* code directly in flags bits 5–7
/// (BLOSC_SNAPPY_LIB=2, BLOSC_ZLIB_LIB=3, …).  Blosc2 extended headers store the
/// same field but disambiguate via the explicit compcode in byte 22, so this
/// function is only needed for the blosc1 path.
fn compformat_to_compcode(compformat: u8, is_blosc1: bool) -> u8 {
    if is_blosc1 {
        match compformat {
            BLOSC_BLOSCLZ_FORMAT => BLOSC_BLOSCLZ, // 0 → 0
            BLOSC_LZ4_FORMAT => BLOSC_LZ4,         // 1 → 1 (also LZ4HC)
            BLOSC_SNAPPY_FORMAT => BLOSC_SNAPPY,   // 2 → 3 (blosc1: SNAPPY_LIB=2)
            BLOSC_ZLIB_FORMAT => BLOSC_ZLIB,       // 3 → 4
            BLOSC_ZSTD_FORMAT => BLOSC_ZSTD,       // 4 → 5
            _ => compformat,
        }
    } else {
        // Blosc2 non-extended headers are not expected in practice (byte 22 is
        // authoritative), but provide a best-effort mapping as a fallback.
        match compformat {
            BLOSC_BLOSCLZ_FORMAT => BLOSC_BLOSCLZ,
            BLOSC_LZ4_FORMAT => BLOSC_LZ4,
            BLOSC_ZLIB_FORMAT => BLOSC_ZLIB,
            BLOSC_ZSTD_FORMAT => BLOSC_ZSTD,
            _ => compformat,
        }
    }
}

fn create_header_blosc1(
    nbytes: usize,
    blocksize: usize,
    cbytes: usize,
    typesize: usize,
    flags: u8,
    compressor: u8,
) -> [u8; BLOSC_MIN_HEADER_LENGTH] {
    let mut header = [0u8; BLOSC_MIN_HEADER_LENGTH];
    header[0] = BLOSC1_VERSION_FORMAT;
    header[1] = 1; // Version for compressor (e.g., 1 for blosclz)
    header[2] = flags | (compcode_to_compformat(compressor, true) << 5);
    header[3] = typesize as u8;
    header[4..8].copy_from_slice(&(nbytes as u32).to_le_bytes());
    header[8..12].copy_from_slice(&(blocksize as u32).to_le_bytes());
    header[12..16].copy_from_slice(&(cbytes as u32).to_le_bytes());
    header
}

fn create_header_blosc2(
    nbytes: usize,
    blocksize: usize,
    cbytes: usize,
    typesize: usize,
    flags: u8,
    compressor: u8,
    filters: &[u8; 6],
    filters_meta: &[u8; 6],
) -> [u8; BLOSC_EXTENDED_HEADER_LENGTH] {
    let mut header = [0u8; BLOSC_EXTENDED_HEADER_LENGTH];
    // First 16 bytes: standard Blosc header
    header[0] = BLOSC2_VERSION_FORMAT_STABLE;
    header[1] = 1; // Version for compressor (e.g., 1 for blosclz)
    header[2] = flags | (compcode_to_compformat(compressor, false) << 5);
    header[3] = typesize as u8;
    header[4..8].copy_from_slice(&(nbytes as u32).to_le_bytes());
    header[8..12].copy_from_slice(&(blocksize as u32).to_le_bytes());
    header[12..16].copy_from_slice(&(cbytes as u32).to_le_bytes());

    // Extended header (bytes 16-31)
    // Bytes 16-21: filters[0..6]
    header[16..22].copy_from_slice(filters);
    // Byte 22: compressor code (same as bits 5-7 of flags byte)
    header[22] = compressor;
    // Byte 23: compressor metadata
    header[23] = 0;
    // Bytes 24-29: filters_meta[0..6]
    header[24..30].copy_from_slice(filters_meta);
    // Byte 30: reserved
    header[30] = 0;
    // Byte 31: blosc2 flags
    header[31] = 0;

    header
}

/// Compresses `src` into `dest` using the given codec and a default filter pipeline.
///
/// This is the lower-level entry point used by [`crate::blosc2_compress`]. It handles
/// block sizing, header construction, filter application, and per-block codec invocation.
///
/// Returns the total number of bytes written to `dest` (including the header).
pub fn compress(
    clevel: i32,
    doshuffle: i32,
    typesize: usize,
    src: &[u8],
    dest: &mut [u8],
    compressor: u8,
) -> Result<usize, i32> {
    let mut filters = [BLOSC_NOFILTER; 6];
    if doshuffle == BLOSC_SHUFFLE as i32 && typesize > 1 {
        filters[5] = BLOSC_SHUFFLE;
    } else if doshuffle == BLOSC_BITSHUFFLE as i32 {
        filters[5] = BLOSC_BITSHUFFLE;
    }

    compress_internal(
        clevel,
        doshuffle,
        typesize,
        src,
        dest,
        compressor,
        false,
        &filters,
        &[0; 6],
    )
}

/// Like [`compress`], but accepts an explicit filter pipeline and filter metadata.
///
/// Used by [`crate::blosc2_compress_ctx`] to support custom filter configurations
/// beyond the simple shuffle modes.
pub fn compress_extended(
    clevel: i32,
    doshuffle: i32,
    typesize: usize,
    src: &[u8],
    dest: &mut [u8],
    compressor: u8,
    filters: &[u8; 6],
    filters_meta: &[u8; 6],
) -> Result<usize, i32> {
    compress_internal(
        clevel,
        doshuffle,
        typesize,
        src,
        dest,
        compressor,
        true,
        filters,
        filters_meta,
    )
}

fn compute_blocksize(
    clevel: i32,
    typesize: usize,
    nbytes: usize,
    compressor: u8,
    filter_flags: u8,
    extended_header: bool,
) -> usize {
    if nbytes < typesize {
        return nbytes.max(1);
    }

    let mut blocksize = nbytes;

    // Check splitmode using the initial blocksize (= nbytes), matching C behavior
    let splitmode = split_block(compressor, clevel, typesize, blocksize, filter_flags, extended_header);

    if nbytes >= L1 {
        blocksize = L1;

        let is_hcr = match compressor {
            BLOSC_LZ4HC | BLOSC_ZLIB | BLOSC_ZSTD => true,
            _ => false,
        };

        if is_hcr {
            blocksize *= 2;
        }

        match clevel {
            0 => blocksize /= 4,
            1 => blocksize /= 2,
            2 => blocksize *= 1,
            3 => blocksize *= 2,
            4 | 5 => blocksize *= 4,
            6 | 7 | 8 => blocksize *= 8,
            9 => {
                blocksize *= 8;
                if is_hcr {
                    blocksize *= 2;
                }
            }
            _ => {}
        }
    }

    // Override blocksize for splittable codecs (matching C stune.c)
    if clevel > 0 && splitmode {
        blocksize = match clevel {
            1 | 2 | 3 => 32 * 1024,
            4 | 5 | 6 => 64 * 1024,
            7 => 128 * 1024,
            8 => 256 * 1024,
            _ => 512 * 1024, // clevel 9 and above
        };
        // Multiply by typesize to get proper split sizes
        blocksize *= typesize;
        // But do not exceed 4 MB
        if blocksize > 4 * 1024 * 1024 {
            blocksize = 4 * 1024 * 1024;
        }
        if blocksize < 32 * 1024 {
            // Do not use a too small blocksize (< 32 KB) when typesize is small
            blocksize = 32 * 1024;
        }
    }

    if blocksize > nbytes {
        blocksize = nbytes;
    }

    // blocksize must be a multiple of typesize (matching C's stune.c)
    if typesize > 0 && blocksize > typesize {
        blocksize = (blocksize / typesize) * typesize;
    }

    blocksize
}

/// Compute filter_flags from the filters array, matching C's filters_to_flags().
fn filters_to_flags(filters: &[u8; 6]) -> u8 {
    let mut flags = 0u8;
    for &f in filters.iter() {
        match f {
            BLOSC_SHUFFLE => flags |= BLOSC_DOSHUFFLE,
            BLOSC_BITSHUFFLE => flags |= BLOSC_DOBITSHUFFLE,
            _ => {}
        }
    }
    flags
}

/// Blosc2 split logic (c-blosc2 stune.c): only split for byte shuffle, not bitshuffle.
fn split_block_blosc2(
    compressor: u8,
    clevel: i32,
    typesize: usize,
    blocksize: usize,
    filter_flags: u8,
) -> bool {
    if (filter_flags & BLOSC_DOSHUFFLE) == 0 {
        return false;
    }

    let split = match compressor {
        BLOSC_BLOSCLZ | BLOSC_LZ4 => true,
        BLOSC_ZSTD if clevel <= 5 => true,
        _ => false,
    };

    split && (typesize <= 16) && (blocksize / typesize >= BLOSC_MIN_BUFFERSIZE)
}

/// Blosc1 split logic (c-blosc blosc.c): FORWARD_COMPAT_SPLIT mode, no shuffle check.
fn split_block_blosc1(
    compressor: u8,
    typesize: usize,
    blocksize: usize,
) -> bool {
    (compressor != BLOSC_ZSTD)
        && (typesize <= 16)
        && (blocksize / typesize >= BLOSC_MIN_BUFFERSIZE)
}

fn split_block(
    compressor: u8,
    clevel: i32,
    typesize: usize,
    blocksize: usize,
    filter_flags: u8,
    extended_header: bool,
) -> bool {
    if extended_header {
        split_block_blosc2(compressor, clevel, typesize, blocksize, filter_flags)
    } else {
        split_block_blosc1(compressor, typesize, blocksize)
    }
}

fn compress_internal(
    clevel: i32,
    doshuffle: i32,
    typesize: usize,
    src: &[u8],
    dest: &mut [u8],
    compressor: u8,
    extended_header: bool,
    filters: &[u8; 6],
    filters_meta: &[u8; 6],
) -> Result<usize, i32> {
    let nbytes = src.len();

    // Compute actual filter flags from the filters array (matching C's filters_to_flags)
    // Must be computed before blocksize since split_block depends on it.
    let filter_flags = filters_to_flags(filters);

    let blocksize = compute_blocksize(clevel, typesize, nbytes, compressor, filter_flags, extended_header);
    let nblocks = if nbytes == 0 {
        0
    } else {
        (nbytes + blocksize - 1) / blocksize
    };

    let header_len = if extended_header {
        BLOSC_EXTENDED_HEADER_LENGTH
    } else {
        BLOSC_MIN_HEADER_LENGTH
    };

    // Header flags: for extended header, both DOSHUFFLE and DOBITSHUFFLE are set as a marker
    let mut flags = filter_flags;
    if extended_header {
        flags |= BLOSC_DOSHUFFLE | BLOSC_DOBITSHUFFLE;
    }

    // Use actual filter_flags (not header flags) for split decision, matching C behavior
    let split = split_block(compressor, clevel, typesize, blocksize, filter_flags, extended_header);

    if !split {
        // For blosc2, only set dont_split when clevel > 0 (matching C blosc2 stune.c)
        // For blosc1, always set dont_split when not splitting (matching C blosc1 write_compression_header)
        if !extended_header || (clevel != 0 && nbytes >= BLOSC_MIN_BUFFERSIZE) {
            flags |= 0x10;
        }
    }

    // Calculate data start offset
    let mut data_offset = header_len;
    // Always add bstarts if nblocks > 0 (Blosc1 behavior)
    if nblocks > 0 {
        data_offset += nblocks * 4;
    }

    let mut current_dest_offset = data_offset;
    let mut incompressible = false;

    let mut bstarts = vec![0usize; nblocks];

    for i in 0..nblocks {
        let start = i * blocksize;
        let end = std::cmp::min(start + blocksize, nbytes);
        let block_len = end - start;
        let src_block = &src[start..end];

        bstarts[i] = current_dest_offset;

        let mut filtered_buf = if doshuffle != BLOSC_NOSHUFFLE as i32 {
            vec![0u8; block_len]
        } else {
            Vec::new()
        };

        let mut filtered_src = src_block;

        if doshuffle == BLOSC_SHUFFLE as i32 {
            filters::shuffle(typesize, block_len, src_block, &mut filtered_buf);
            filtered_src = &filtered_buf;
        } else if doshuffle == BLOSC_BITSHUFFLE as i32 {
            filters::bitshuffle(typesize, block_len, src_block, &mut filtered_buf)
                .map_err(|_| -1)?;
            filtered_src = &filtered_buf;
        }

        // C does not split the leftover (last partial) block
        let leftoverblock = i == nblocks - 1 && (nbytes % blocksize) != 0;
        let block_split =
            !leftoverblock && split_block(compressor, clevel, typesize, block_len, filter_flags, extended_header);
        let nstreams = if block_split { typesize } else { 1 };
        let neblock = block_len / nstreams;

        for j in 0..nstreams {
            let stream_offset = j * neblock;
            let stream_src = &filtered_src[stream_offset..stream_offset + neblock];

            if current_dest_offset + 4 > dest.len() {
                incompressible = true;
                break;
            }

            let stream_csize;

            match compressor {
                BLOSC_BLOSCLZ => {
                    stream_csize =
                        blosclz::compress(clevel, stream_src, &mut dest[current_dest_offset + 4..]);
                }
                BLOSC_LZ4 | BLOSC_LZ4HC => {
                    match lz4_flex::block::compress_into(
                        stream_src,
                        &mut dest[current_dest_offset + 4..],
                    ) {
                        Ok(size) => stream_csize = size,
                        Err(_) => stream_csize = 0,
                    }
                }
                BLOSC_SNAPPY => {
                    let mut encoder = snap::raw::Encoder::new();
                    match encoder.compress(stream_src, &mut dest[current_dest_offset + 4..]) {
                        Ok(size) => stream_csize = size,
                        Err(_) => stream_csize = 0,
                    }
                }
                BLOSC_ZLIB => {
                    let cursor = std::io::Cursor::new(&mut dest[current_dest_offset + 4..]);
                    let mut encoder = flate2::write::ZlibEncoder::new(
                        cursor,
                        flate2::Compression::new(clevel as u32),
                    );
                    if encoder.write_all(stream_src).is_ok() {
                        match encoder.finish() {
                            Ok(cursor) => {
                                stream_csize = cursor.position() as usize;
                            }
                            Err(_) => stream_csize = 0,
                        }
                    } else {
                        stream_csize = 0;
                    }
                }
                BLOSC_ZSTD => {
                    let cursor = std::io::Cursor::new(&mut dest[current_dest_offset + 4..]);
                    let mut encoder =
                        zstd::stream::write::Encoder::new(cursor, clevel).map_err(|_| -1)?;
                    if encoder.write_all(stream_src).is_ok() {
                        match encoder.finish() {
                            Ok(cursor) => {
                                stream_csize = cursor.position() as usize;
                            }
                            Err(_) => stream_csize = 0,
                        }
                    } else {
                        stream_csize = 0;
                    }
                }
                _ => return Err(-1),
            }

            if stream_csize == 0 || stream_csize >= neblock {
                incompressible = true;
                break;
            }

            dest[current_dest_offset..current_dest_offset + 4]
                .copy_from_slice(&(stream_csize as u32).to_le_bytes());
            current_dest_offset += 4 + stream_csize;
        }

        if incompressible {
            break;
        }
    }

    let compressed_size = current_dest_offset - data_offset;

    if incompressible || compressed_size >= nbytes {
        if nbytes > dest.len() - header_len {
            return Err(-1);
        }
        dest[header_len..header_len + nbytes].copy_from_slice(src);
        flags |= BLOSC_MEMCPYED;
        current_dest_offset = header_len + nbytes;
    } else {
        if nblocks > 0 {
            for i in 0..nblocks {
                let offset = header_len + i * 4;
                dest[offset..offset + 4].copy_from_slice(&(bstarts[i] as u32).to_le_bytes());
            }
        }
    }

    let cbytes = current_dest_offset;
    if extended_header {
        let header = create_header_blosc2(
            nbytes,
            blocksize,
            cbytes,
            typesize,
            flags,
            compressor,
            filters,
            filters_meta,
        );
        dest[0..BLOSC_EXTENDED_HEADER_LENGTH].copy_from_slice(&header);
    } else {
        let header = create_header_blosc1(nbytes, blocksize, cbytes, typesize, flags, compressor);
        dest[0..BLOSC_MIN_HEADER_LENGTH].copy_from_slice(&header);
    }

    Ok(cbytes)
}

/// Result type used by the decompression internals.
type DResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Fields of a Blosc1/Blosc2 chunk header needed for decompression.
///
/// C: blosc2.c `read_chunk_header` fills the equivalent `blosc_header` struct, and
/// `blosc_run_decompression_with_context` / `blosc_getitem` consume it.
struct ChunkHeader {
    header_len: usize,
    nbytes: usize,
    cbytes: usize,
    blocksize: usize,
    typesize: usize,
    compressor: u8,
    doshuffle: bool,
    dobitshuffle: bool,
    /// Header flags bit 4: every block is stored as a single stream.
    dont_split: bool,
    memcpyed: bool,
    nblocks: usize,
}

impl ChunkHeader {
    fn read(src: &[u8]) -> DResult<Self> {
        if src.len() < BLOSC_MIN_HEADER_LENGTH {
            return Err("Source buffer too small for header".into());
        }

        // Read version to determine header size
        let version = src[0];
        let header_len = if version == BLOSC2_VERSION_FORMAT_STABLE
            || version == BLOSC2_VERSION_FORMAT_BETA1
            || version == BLOSC2_VERSION_FORMAT_ALPHA
        {
            BLOSC_EXTENDED_HEADER_LENGTH
        } else {
            BLOSC_MIN_HEADER_LENGTH
        };

        if src.len() < header_len {
            return Err("Source buffer too small for extended header".into());
        }

        let (nbytes, cbytes, blocksize) = crate::api::blosc2_cbuffer_sizes(src);

        let flags = src[2];
        let is_blosc1 = header_len == BLOSC_MIN_HEADER_LENGTH;
        // Flags byte bits 5-7 store compformat, not compcode
        let compressor = if !is_blosc1 {
            // Extended header byte 22 has the actual compressor code
            src[22]
        } else {
            compformat_to_compcode((flags >> 5) & 0x7, is_blosc1)
        };
        let typesize = src[3] as usize;

        let mut doshuffle = (flags & BLOSC_DOSHUFFLE) != 0;
        let mut dobitshuffle = (flags & BLOSC_DOBITSHUFFLE) != 0;

        // Check for extended header marker (both flags set)
        if doshuffle && dobitshuffle {
            doshuffle = false;
            dobitshuffle = false;

            if !is_blosc1 {
                for &f in &src[16..22] {
                    if f == BLOSC_SHUFFLE {
                        doshuffle = true;
                    } else if f == BLOSC_BITSHUFFLE {
                        dobitshuffle = true;
                    }
                }
            }
        }

        doshuffle = doshuffle && typesize > 1;
        dobitshuffle = dobitshuffle && blocksize >= typesize;

        let memcpyed = (flags & BLOSC_MEMCPYED) != 0;

        // Memcpyed chunks store the data right after the header: no blocks to decode.
        let nblocks = if nbytes == 0 || memcpyed {
            0
        } else if blocksize == 0 {
            return Err("Invalid blocksize of 0".into());
        } else {
            nbytes.div_ceil(blocksize)
        };

        if nblocks > 0 {
            if typesize == 0 {
                return Err("Invalid typesize of 0".into());
            }
            // The bstarts array (one LE u32 per block) follows the header.
            let bstarts_end = nblocks
                .checked_mul(4)
                .and_then(|n| n.checked_add(header_len))
                .ok_or("Invalid number of blocks")?;
            if src.len() < bstarts_end {
                return Err("Buffer too small for bstarts array".into());
            }
            if src.len() < cbytes {
                return Err("Compressed data is truncated".into());
            }
        }

        Ok(ChunkHeader {
            header_len,
            nbytes,
            cbytes,
            blocksize,
            typesize,
            compressor,
            doshuffle,
            dobitshuffle,
            dont_split: (flags & 0x10) != 0,
            memcpyed,
            nblocks,
        })
    }

    /// Returns true if `nblock` is the last, partial block.
    fn is_leftover_block(&self, nblock: usize) -> bool {
        nblock == self.nblocks - 1 && self.nbytes % self.blocksize != 0
    }

    /// Uncompressed size of block `nblock` (the last block may be smaller).
    fn block_nbytes(&self, nblock: usize) -> usize {
        if self.is_leftover_block(nblock) {
            self.nbytes % self.blocksize
        } else {
            self.blocksize
        }
    }

    /// Compressed streams of block `nblock`, from its bstart up to `cbytes`.
    ///
    /// Like C `blosc_d`, the end is bounded by the chunk size rather than the next
    /// block's bstart: stream sizes are self-delimiting, so blocks need not be ordered.
    /// Reading bstarts directly from `src` avoids allocating a `Vec` of all of them.
    fn block_content<'a>(&self, src: &'a [u8], nblock: usize) -> DResult<&'a [u8]> {
        let offset = self.header_len + nblock * 4;
        let bstart = u32::from_le_bytes(src[offset..offset + 4].try_into().unwrap()) as usize;
        if bstart > self.cbytes {
            return Err("Compressed data is truncated".into());
        }
        Ok(&src[bstart..self.cbytes])
    }
}

/// Codec decoder state reused across all streams and blocks of a decompression call.
///
/// Creating a zstd `DCtx` or a zlib inflate state is expensive (each is a heap
/// allocation of tens to hundreds of KB). C blosc2 likewise keeps `zstd_dctx` in its
/// thread context (blosc2.c `zstd_wrap_decompress`) instead of recreating it per stream.
struct Decoders {
    zstd: Option<zstd::bulk::Decompressor<'static>>,
    zlib: Option<flate2::Decompress>,
    snappy: snap::raw::Decoder,
}

impl Decoders {
    fn new() -> Self {
        Decoders {
            zstd: None,
            zlib: None,
            snappy: snap::raw::Decoder::new(),
        }
    }

    /// Decompresses one codec stream into `output`, returning the number of bytes written.
    fn decode(&mut self, compressor: u8, input: &[u8], output: &mut [u8]) -> DResult<usize> {
        Ok(match compressor {
            BLOSC_BLOSCLZ => blosclz::decompress(input, output),
            BLOSC_LZ4 | BLOSC_LZ4HC => lz4_flex::decompress_into(input, output)
                .map_err(|e| format!("LZ4 error: {}", e))?,
            BLOSC_SNAPPY => self
                .snappy
                .decompress(input, output)
                .map_err(|e| format!("Snappy error: {}", e))?,
            BLOSC_ZLIB => {
                // One-shot inflate directly into `output` (C uses zlib `uncompress`),
                // avoiding the intermediate buffer of a `Read` + `io::copy` pipeline.
                let zlib = self.zlib.get_or_insert_with(|| flate2::Decompress::new(true));
                zlib.reset(true);
                let status = zlib
                    .decompress(input, output, flate2::FlushDecompress::Finish)
                    .map_err(|e| format!("Zlib error: {}", e))?;
                if status != flate2::Status::StreamEnd {
                    return Err("Zlib error: incomplete stream".into());
                }
                zlib.total_out() as usize
            }
            BLOSC_ZSTD => {
                let zstd = match &mut self.zstd {
                    Some(zstd) => zstd,
                    slot @ None => slot.insert(
                        zstd::bulk::Decompressor::new()
                            .map_err(|e| format!("Zstd error: {}", e))?,
                    ),
                };
                zstd.decompress_to_buffer(input, output)
                    .map_err(|e| format!("Zstd error: {}", e))?
            }
            _ => return Err(format!("Unsupported compressor: {}", compressor).into()),
        })
    }
}

/// Grows `buf` to at least `len` bytes. Scratch buffers are reused across blocks,
/// so this allocates at most once per call (the first block is the largest).
fn ensure_len(buf: &mut Vec<u8>, len: usize) {
    if buf.len() < len {
        buf.resize(len, 0);
    }
}

/// Decodes the codec streams of block `nblock` into `dest` (exactly the block's
/// uncompressed size), *without* applying the inverse filters.
///
/// C: blosc2.c `blosc_d`, the `for (int j = 0; j < nstreams; j++)` loop. Each stream
/// is preceded by its compressed size as a LE i32:
/// - `0`: the stream is all zeros,
/// - negative: a run of the byte `-csize` (followed by a 1-byte token),
/// - `== neblock`: stored uncompressed,
/// - otherwise: codec-compressed data.
fn decode_block(
    hdr: &ChunkHeader,
    src: &[u8],
    nblock: usize,
    dest: &mut [u8],
    decoders: &mut Decoders,
) -> DResult<()> {
    let content = hdr.block_content(src, nblock)?;
    let block_nbytes = dest.len();

    // Determine number of streams (matching C: dont_split or leftoverblock → 1 stream)
    let nstreams = if !hdr.dont_split && !hdr.is_leftover_block(nblock) {
        hdr.typesize
    } else {
        1
    };
    let neblock = block_nbytes / nstreams;
    if neblock * nstreams != block_nbytes {
        return Err(format!(
            "Block {} decompression size mismatch: expected {}, got {}",
            nblock,
            block_nbytes,
            neblock * nstreams
        )
        .into());
    }

    let mut content_offset = 0;

    for out in dest.chunks_exact_mut(neblock.max(1)).take(nstreams) {
        let csize_bytes = content
            .get(content_offset..content_offset + 4)
            .ok_or("Block too small for chunk size")?;
        // Read stream size as signed i32 (C uses sw32_ which returns signed)
        let stream_cbytes = i32::from_le_bytes(csize_bytes.try_into().unwrap());
        content_offset += 4;

        if stream_cbytes == 0 {
            // A run of zeros
            out.fill(0);
        } else if stream_cbytes < 0 {
            // Run-length encoding: negative value encodes the byte value
            let token = *content
                .get(content_offset)
                .ok_or("Not enough input for run-length token")?;
            content_offset += 1;
            if token & 0x1 == 0 {
                return Err("Invalid run-length token".into());
            }
            // A run of a non-zero byte value
            out.fill((-stream_cbytes) as u8);
        } else {
            let sc = stream_cbytes as usize;
            let stream = content
                .get(content_offset..content_offset + sc)
                .ok_or("Chunk size exceeds block size")?;
            content_offset += sc;

            if sc == neblock {
                // Incompressible: raw data stored directly
                out.copy_from_slice(stream);
            } else {
                let n = decoders.decode(hdr.compressor, stream, out)?;
                if n != neblock {
                    return Err(format!(
                        "Block {} decompression size mismatch: expected {}, got {}",
                        nblock, neblock, n
                    )
                    .into());
                }
            }
        }
    }

    Ok(())
}

/// Decompresses block `nblock` and applies the inverse filter, writing the block's
/// uncompressed bytes into `dest` (exactly `block_nbytes` long).
///
/// C: blosc2.c `blosc_d` + `pipeline_backward`. Only one block-sized scratch buffer
/// (`tmp`) is needed:
/// - shuffle: decode into `tmp`, then unshuffle `tmp` -> `dest`;
/// - bitshuffle: decode straight into `dest`, then bitunshuffle in place using `tmp`;
/// - no filter: decode straight into `dest`.
fn blosc_d(
    hdr: &ChunkHeader,
    src: &[u8],
    nblock: usize,
    dest: &mut [u8],
    tmp: &mut Vec<u8>,
    decoders: &mut Decoders,
) -> DResult<()> {
    let block_nbytes = dest.len();
    if hdr.doshuffle {
        ensure_len(tmp, block_nbytes);
        let tmp = &mut tmp[..block_nbytes];
        decode_block(hdr, src, nblock, tmp, decoders)?;
        filters::unshuffle(hdr.typesize, block_nbytes, tmp, dest);
    } else if hdr.dobitshuffle {
        ensure_len(tmp, block_nbytes);
        decode_block(hdr, src, nblock, dest, decoders)?;
        filters::bitunshuffle_inplace(hdr.typesize, block_nbytes, dest, tmp)
            .map_err(|e| format!("Bitunshuffle error: {}", e))?;
    } else {
        decode_block(hdr, src, nblock, dest, decoders)?;
    }
    Ok(())
}

/// Decompresses a Blosc/Blosc2 compressed buffer (including header) into `dest`.
///
/// Automatically detects the format version, codec, and filter pipeline from the
/// header. Supports both Blosc1 (16-byte header) and Blosc2 (32-byte extended header)
/// formats.
///
/// Returns the number of decompressed bytes written to `dest`.
pub fn decompress(src: &[u8], dest: &mut [u8]) -> Result<usize, Box<dyn std::error::Error>> {
    let hdr = ChunkHeader::read(src)?;
    let nbytes = hdr.nbytes;

    if dest.len() < nbytes {
        return Err("Destination buffer too small".into());
    }

    if hdr.memcpyed {
        // Copy from after header to dest
        let data = src
            .get(hdr.header_len..hdr.header_len + nbytes)
            .ok_or("Compressed data is truncated")?;
        dest[..nbytes].copy_from_slice(data);
        return Ok(nbytes);
    }

    if hdr.nblocks == 0 {
        return Ok(nbytes);
    }

    // Scratch buffer and codec state shared by all blocks (C keeps these in the context).
    let mut tmp = Vec::new();
    let mut decoders = Decoders::new();

    for (nblock, block_dest) in dest[..nbytes].chunks_mut(hdr.blocksize).enumerate() {
        blosc_d(&hdr, src, nblock, block_dest, &mut tmp, &mut decoders)?;
    }

    Ok(nbytes)
}

/// Decompresses a Blosc/Blosc2 buffer into a newly allocated `Vec`.
///
/// The header is validated before allocating the output, and memcpyed chunks are
/// copied directly without zero-initializing the output first.
pub(crate) fn decompress_to_vec(src: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let hdr = ChunkHeader::read(src)?;
    if hdr.memcpyed {
        let data = src
            .get(hdr.header_len..hdr.header_len + hdr.nbytes)
            .ok_or("Compressed data is truncated")?;
        return Ok(data.to_vec());
    }
    let mut dest = vec![0u8; hdr.nbytes];
    decompress(src, &mut dest)?;
    Ok(dest)
}

/// Partially decompresses a Blosc buffer, extracting `nitems` elements starting
/// at element index `start`.
///
/// This avoids decompressing the entire buffer when only a subset of elements is
/// needed. Returns the number of bytes written to `dest`.
pub fn getitem(src: &[u8], start: usize, nitems: usize, dest: &mut [u8]) -> Result<usize, i32> {
    getitem_inner(src, start, nitems, dest).map_err(|_| -1)
}

/// C: blosc2.c `_blosc_getitem`. Only the blocks overlapping the requested range are
/// decoded. Blocks entirely inside the range are decoded straight into `dest`; for
/// partially covered blocks, only the requested bytes are unshuffled.
fn getitem_inner(src: &[u8], start: usize, nitems: usize, dest: &mut [u8]) -> DResult<usize> {
    let hdr = ChunkHeader::read(src)?;
    if src.len() < hdr.cbytes {
        return Err("Compressed data is truncated".into());
    }

    let typesize = hdr.typesize;
    let start_byte = start.checked_mul(typesize).ok_or("Invalid start")?;
    let end_byte = start
        .checked_add(nitems)
        .and_then(|n| n.checked_mul(typesize))
        .ok_or("Invalid nitems")?;
    if end_byte > hdr.nbytes {
        return Err("Requested items out of range".into());
    }
    let len = end_byte - start_byte;
    if dest.len() < len {
        return Err("Destination buffer too small".into());
    }
    if len == 0 {
        return Ok(0);
    }
    let dest = &mut dest[..len];

    if hdr.memcpyed {
        let data = src
            .get(hdr.header_len + start_byte..hdr.header_len + end_byte)
            .ok_or("Compressed data is truncated")?;
        dest.copy_from_slice(data);
        return Ok(len);
    }

    let blocksize = hdr.blocksize;
    let start_block = start_byte / blocksize;
    let end_block = (end_byte - 1) / blocksize;

    let mut tmp = Vec::new();
    let mut tmp2 = Vec::new();
    let mut decoders = Decoders::new();
    let mut dest_offset = 0;

    for nblock in start_block..=end_block {
        let block_start = nblock * blocksize;
        let block_nbytes = hdr.block_nbytes(nblock);

        // Byte range of the request local to this block.
        let local_start = start_byte.max(block_start) - block_start;
        let local_end = end_byte.min(block_start + block_nbytes) - block_start;
        let out = &mut dest[dest_offset..dest_offset + (local_end - local_start)];

        if local_start == 0 && local_end == block_nbytes {
            // Whole block requested: decode (and unfilter) straight into dest.
            blosc_d(&hdr, src, nblock, out, &mut tmp, &mut decoders)?;
        } else {
            ensure_len(&mut tmp, block_nbytes);
            let raw = &mut tmp[..block_nbytes];
            decode_block(&hdr, src, nblock, raw, &mut decoders)?;

            if hdr.doshuffle {
                // Unshuffle only the requested bytes.
                filters::unshuffle_partial(
                    typesize,
                    block_nbytes,
                    raw,
                    local_start,
                    local_end,
                    out,
                );
            } else {
                if hdr.dobitshuffle {
                    ensure_len(&mut tmp2, block_nbytes);
                    filters::bitunshuffle_inplace(typesize, block_nbytes, raw, &mut tmp2)
                        .map_err(|e| format!("Bitunshuffle error: {}", e))?;
                }
                out.copy_from_slice(&raw[local_start..local_end]);
            }
        }

        dest_offset += local_end - local_start;
    }

    Ok(dest_offset)
}
