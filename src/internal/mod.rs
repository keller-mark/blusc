use crate::codecs::blosclz;
use crate::filters;
use std::io::{Read, Write};

// Constants
// Blosc2 stable format version (corresponds to BLOSC2_VERSION_FORMAT_STABLE in C)
pub const BLOSC_VERSION_FORMAT: u8 = 5;
pub const BLOSC_MIN_HEADER_LENGTH: usize = 16;

// Compressor codes
/*
:codec_flags:
    (``uint8``) Compressor enumeration (defaults for all the chunks in storage).

    :``0`` to ``3``:
        Enumerated for codecs (up to 16).
    :``0``:
        ``blosclz``
    :``1``:
        ``lz4`` or ``lz4hc``
    :``2``:
        reserved (slot previously occupied by ``snappy`` and free now)
    :``3``:
        ``zlib``
    :``4``:
        ``zstd``
    :``5``:
        reserved
    :``6``:
        The compressor is defined in the user-defined codec slot (see below).
    :``7 to 15``:
        Reserved
 */
pub const BLOSC_BLOSCLZ: u8 = 0;
pub const BLOSC_LZ4: u8 = 1;
pub const BLOSC_LZ4HC: u8 = 1;
pub const BLOSC_SNAPPY: u8 = 2;
pub const BLOSC_ZLIB: u8 = 3;
pub const BLOSC_ZSTD: u8 = 4;

// Flags
pub const BLOSC_DOSHUFFLE: u8 = 0x1;
pub const BLOSC_MEMCPYED: u8 = 0x2;
pub const BLOSC_DOBITSHUFFLE: u8 = 0x4;
pub const BLOSC_DODELTA: u8 = 0x8;
pub const BLOSC_NOT_SPLIT: u8 = 0x10;

// Internal blosc2 compression function

fn decompress_buffer(compressor: u8, src: &[u8], dest: &mut [u8]) -> Result<usize, i32> {
    match compressor {
        BLOSC_BLOSCLZ => {
            Ok(blosclz::decompress(src, dest))
        },
        BLOSC_LZ4 => {
             match lz4_flex::block::decompress_into(src, dest) {
                 Ok(size) => Ok(size),
                 Err(_) => Ok(0),
             }
        },
        BLOSC_SNAPPY => {
            let mut decoder = snap::raw::Decoder::new();
            match decoder.decompress(src, dest) {
                Ok(size) => Ok(size),
                Err(_) => Ok(0),
            }
        },
        BLOSC_ZLIB => {
            let mut decoder = flate2::read::ZlibDecoder::new(src);
            match decoder.read_exact(dest) {
                Ok(_) => Ok(dest.len()), // Assuming exact read
                Err(_) => Ok(0),
            }
        },
        BLOSC_ZSTD => {
            let mut decoder = zstd::stream::read::Decoder::new(std::io::Cursor::new(src)).map_err(|_| -1)?;
            match decoder.read_exact(dest) {
                Ok(_) => Ok(dest.len()),
                Err(_) => Ok(0),
            }
        },
        _ => Err(-1),
    }
}

/// Compress data using Blosc2 version 5 format.
///
/// ## Blosc2 Version 5 Chunk Format
///
/// The compressed output follows this structure:
///
/// ### For Compressed Data:
/// ```text
/// +--------+----------+-------+-------------+
/// | header | bstarts  | csize | compressed  |
/// +--------+----------+-------+-------------+
///   32 bytes  4*nblocks  4 bytes    data
/// ```
///
/// ### For MEMCPY (uncompressed) Data:
/// ```text
/// +--------+------+
/// | header | data |
/// +--------+------+
///   32 bytes  nbytes
/// ```
///
/// ## Header Structure (32 bytes)
///
/// ### Basic Header (bytes 0-15):
/// - `[0]`: version (5 for Blosc2 version 5 format)
/// - `[1]`: versionlz (codec format version, typically 1)
/// - `[2]`: flags (see below)
/// - `[3]`: typesize (size of each element in bytes)
/// - `[4-7]`: nbytes (uncompressed data size, little-endian uint32)
/// - `[8-11]`: blocksize (block size for chunking, little-endian uint32)
/// - `[12-15]`: cbytes (total compressed size including all metadata, little-endian uint32)
///
/// ### Extended Header (bytes 16-31):
/// Contains filter pipeline information, codec metadata, and blosc2-specific flags.
/// Currently zeroed out as advanced features are not yet implemented.
///
/// ### Flags Byte (byte 2):
/// - `0x01` (BLOSC_DOSHUFFLE): Byte-shuffle filter applied
/// - `0x02` (BLOSC_MEMCPYED): Data stored uncompressed
/// - `0x04` (BLOSC_DOBITSHUFFLE): Bit-shuffle filter applied  
/// - `0x10` (BLOSC_NOT_SPLIT): Data not split into type-sized streams
/// - `0xE0` (bits 5-7): Compressor enumeration (0=BloscLZ, 1=LZ4, etc.)
///
/// **Important**: When using extended header (version 5), both DOSHUFFLE and DOBITSHUFFLE
/// flags MUST be set to indicate the presence of the extended header, even if these filters
/// are not actually used. Exception: MEMCPY mode clears these flags after setting.
///
/// ## Block Structure
///
/// ### bstarts Array:
/// An array of `nblocks` int32 values (little-endian) that point to the absolute byte offset
/// from the start of the chunk where each block's compressed stream begins. For example,
/// with 1 block and 32-byte header + 4-byte bstarts, the first bstart value would be 36,
/// pointing to the csize field.
///
/// ### csize Field:
/// A 4-byte little-endian uint32 containing the size of the compressed data stream that follows.
/// The C decompressor reads this value to know how many bytes to decompress.
///
/// ### Compressed Data:
/// The actual compressed bytes produced by the codec (BloscLZ, LZ4, etc.).
///
/// ## Parameters
/// - `clevel`: Compression level (0-9, where 0 may trigger MEMCPY for incompressible data)
/// - `doshuffle`: Whether to apply shuffle filter (not yet implemented)
/// - `typesize`: Size of each data element in bytes  
/// - `src`: Input data to compress
/// - `dest`: Output buffer for compressed data (must be large enough)
/// - `compressor`: Codec to use (BLOSC_BLOSCLZ=0, BLOSC_LZ4=1, etc.)
///
/// ## Returns
/// - `Ok(cbytes)`: Total size of compressed chunk including all metadata
/// - `Err(-1)`: Compression failed
///
/// ## Reference
/// Based on C-Blosc2 implementation in `c-blosc2/blosc/blosc2.c`
pub fn compress(
    clevel: i32,
    doshuffle: i32,
    typesize: usize,
    src: &[u8],
    dest: &mut [u8],
    compressor: u8,
) -> Result<usize, i32> {
    let nbytes = src.len();
    let blocksize = nbytes; // Single block
    
    // 1. Filter
    let mut filtered_buf = vec![0u8; nbytes];
    let mut filtered_src = src;
    
    let mut flags = BLOSC_NOT_SPLIT; // Single block
    if doshuffle == 1 { // Shuffle
        filters::shuffle(typesize, nbytes, src, &mut filtered_buf);
        filtered_src = &filtered_buf;
        flags |= BLOSC_DOSHUFFLE;
    } else if doshuffle == 2 { // Bitshuffle
        filters::bitshuffle(typesize, nbytes, src, &mut filtered_buf).map_err(|_| -1)?;
        filtered_src = &filtered_buf;
        flags |= BLOSC_DOBITSHUFFLE;
    }

    // 2. Compress
    // Blosc2 version 5 format layout:
    // +--------+----------+-------+-------------+
    // | header | bstarts  | csize | compressed  |
    // +--------+----------+-------+-------------+
    //   32 bytes  4*nblocks  4 bytes    data
    //
    // Note: For version 5, header is always 32 bytes (16 basic + 16 extended)
    // to match C implementation behavior
    
    let header_size = 32;  // Extended header for version 5
    
    // Calculate number of blocks
    let nblocks = if blocksize > 0 {
        (nbytes + blocksize - 1) / blocksize
    } else {
        1
    };
    
    // Reserve space: header (32) + bstarts (4 * nblocks) + csize (4) + compressed data
    let bstarts_size = 4 * nblocks;
    let csize_size = 4;  // 4-byte csize field before compressed data
    let data_start = header_size + bstarts_size + csize_size;
    
    if dest.len() < data_start { return Err(-1); }
    
    // Write compressed data after: header + bstarts + csize
    let compressed_size;
    
    match compressor {
        BLOSC_BLOSCLZ => {
            compressed_size = blosclz::compress(clevel, filtered_src, &mut dest[data_start..]);
        },
        BLOSC_LZ4 => {
             match lz4_flex::block::compress_into(filtered_src, &mut dest[data_start..]) {
                 Ok(size) => compressed_size = size,
                 Err(_) => compressed_size = 0,
             }
        },
        BLOSC_SNAPPY => {
            let mut encoder = snap::raw::Encoder::new();
            match encoder.compress(filtered_src, &mut dest[data_start..]) {
                Ok(size) => compressed_size = size,
                Err(_) => compressed_size = 0,
            }
        },
        BLOSC_ZLIB => {
            let cursor = std::io::Cursor::new(&mut dest[data_start..]);
            let mut encoder = flate2::write::ZlibEncoder::new(cursor, flate2::Compression::new(clevel as u32));
            if encoder.write_all(filtered_src).is_ok() {
                 match encoder.finish() {
                     Ok(cursor) => {
                         compressed_size = cursor.position() as usize;
                     }
                     Err(_) => compressed_size = 0,
                 }
            } else {
                 compressed_size = 0;
            }
        },
        BLOSC_ZSTD => {
            let cursor = std::io::Cursor::new(&mut dest[data_start..]);
            let mut encoder = zstd::stream::write::Encoder::new(cursor, clevel).map_err(|_| -1)?;
            if encoder.write_all(filtered_src).is_ok() {
                 match encoder.finish() {
                     Ok(cursor) => {
                         compressed_size = cursor.position() as usize;
                     }
                     Err(_) => compressed_size = 0,
                 }
            } else {
                 compressed_size = 0;
            }
        },
        _ => return Err(-1),
    }
    
    if compressed_size == 0 || compressed_size >= nbytes {
         // Memcpy - uncompressed data
         // For MEMCPY mode in Blosc2, format is: header(32) | data(nbytes)
         // No bstarts array, no csize field
         if nbytes > dest.len() - header_size { return Err(-1); }
         
         // Copy data immediately after header
         dest[header_size..header_size+nbytes].copy_from_slice(src);
         
         // Set MEMCPY flag and clear shuffle flags
         flags |= BLOSC_MEMCPYED;
         flags &= !BLOSC_DOSHUFFLE;
         flags &= !BLOSC_DOBITSHUFFLE;
         
         // For MEMCPY, cbytes = header + data (no bstarts, no csize)
         let cbytes = header_size + nbytes;
         
         // For Blosc2 with extended header (32 bytes), we must set both DOSHUFFLE and DOBITSHUFFLE flags
         // to indicate extended header is present (even if not actually using these filters)
         // This is required by the C implementation (see blosc2.c line 2410-2411)
         // Exception: MEMCPY mode clears these flags (see above)
         flags |= BLOSC_DOSHUFFLE | BLOSC_DOBITSHUFFLE;
         
         // Write basic header (first 16 bytes)
         dest[0] = BLOSC_VERSION_FORMAT;  // version = 5
         dest[1] = 1;  // versionlz
         dest[2] = flags;
         dest[3] = typesize as u8;
         dest[4] = (nbytes & 0xFF) as u8;
         dest[5] = ((nbytes >> 8) & 0xFF) as u8;
         dest[6] = ((nbytes >> 16) & 0xFF) as u8;
         dest[7] = ((nbytes >> 24) & 0xFF) as u8;
         dest[8] = (blocksize & 0xFF) as u8;
         dest[9] = ((blocksize >> 8) & 0xFF) as u8;
         dest[10] = ((blocksize >> 16) & 0xFF) as u8;
         dest[11] = ((blocksize >> 24) & 0xFF) as u8;
         dest[12] = (cbytes & 0xFF) as u8;
         dest[13] = ((cbytes >> 8) & 0xFF) as u8;
         dest[14] = ((cbytes >> 16) & 0xFF) as u8;
         dest[15] = ((cbytes >> 24) & 0xFF) as u8;
         
         // Zero out extended header portion (bytes 16-31)
         for i in 16..32 {
             dest[i] = 0;
         }
         
         return Ok(cbytes);
    }

    // 3. Write bstarts array after header
    // Each bstart points to where that block's csize field starts (absolute offset from chunk start)
    // The csize comes before the actual compressed data
    let csize_pos = header_size + bstarts_size;
    let bstart_value = csize_pos as u32;
    for i in 0..nblocks {
        let pos = header_size + i * 4;
        dest[pos..pos+4].copy_from_slice(&bstart_value.to_le_bytes());
    }
    
    // 4. Write csize (size of compressed stream) after bstarts
    dest[csize_pos..csize_pos+4].copy_from_slice(&(compressed_size as u32).to_le_bytes());

    // 5. Write header
    // cbytes includes: header (32) + bstarts (4*nblocks) + csize (4) + compressed data
    let cbytes = data_start + compressed_size;
    
    // For Blosc2 with extended header (32 bytes), we must set both DOSHUFFLE and DOBITSHUFFLE flags
    // to indicate extended header is present (even if not actually using these filters)
    // This is required by the C implementation (see blosc2.c line 2410-2411)
    flags |= BLOSC_DOSHUFFLE | BLOSC_DOBITSHUFFLE;
    
    // Write basic header (first 16 bytes)
    dest[0] = BLOSC_VERSION_FORMAT;  // version = 5
    dest[1] = 1;  // versionlz
    dest[2] = flags;
    dest[3] = typesize as u8;
    dest[4] = (nbytes & 0xFF) as u8;
    dest[5] = ((nbytes >> 8) & 0xFF) as u8;
    dest[6] = ((nbytes >> 16) & 0xFF) as u8;
    dest[7] = ((nbytes >> 24) & 0xFF) as u8;
    dest[8] = (blocksize & 0xFF) as u8;
    dest[9] = ((blocksize >> 8) & 0xFF) as u8;
    dest[10] = ((blocksize >> 16) & 0xFF) as u8;
    dest[11] = ((blocksize >> 24) & 0xFF) as u8;
    dest[12] = (cbytes & 0xFF) as u8;
    dest[13] = ((cbytes >> 8) & 0xFF) as u8;
    dest[14] = ((cbytes >> 16) & 0xFF) as u8;
    dest[15] = ((cbytes >> 24) & 0xFF) as u8;
    
    // Zero out extended header portion (bytes 16-31)
    // This space is reserved for filters, compcode_meta, and blosc2_flags
    // but we're not using those features yet
    for i in 16..32 {
        dest[i] = 0;
    }
    
    Ok(cbytes)
}

fn decompress_blocks(
    compressor: u8, 
    compressed_data: &[u8], 
    dest: &mut [u8], 
    blocksize: usize, 
    nbytes: usize
) -> Result<(), i32> {
    let mut src_pos = 0;
    let mut dest_pos = 0;
    
    while src_pos < compressed_data.len() && dest_pos < nbytes {
        if src_pos + 4 > compressed_data.len() { return Err(-1); }
        let cblock_size = u32::from_le_bytes([
            compressed_data[src_pos],
            compressed_data[src_pos+1],
            compressed_data[src_pos+2],
            compressed_data[src_pos+3]
        ]) as usize;
        src_pos += 4;
        
        if cblock_size == 0 {
            continue;
        }
        
        if src_pos + cblock_size > compressed_data.len() { return Err(-1); }
        let cblock = &compressed_data[src_pos .. src_pos + cblock_size];
        src_pos += cblock_size;
        
        let current_blocksize = std::cmp::min(blocksize, nbytes - dest_pos);
        let dest_slice = &mut dest[dest_pos .. dest_pos + current_blocksize];
        
        let size = decompress_buffer(compressor, cblock, dest_slice)?;
        if size != current_blocksize { return Err(-1); }
        dest_pos += current_blocksize;
    }
    Ok(())
}

/// Decompress data from Blosc2 version 5 format.
///
/// ## Supported Formats
///
/// This function handles both compressed and MEMCPY (uncompressed) data:
///
/// ### Compressed Data Format:
/// ```text
/// +--------+----------+-------+-------------+
/// | header | bstarts  | csize | compressed  |
/// +--------+----------+-------+-------------+
///   32 bytes  4*nblocks  4 bytes    data
/// ```
///
/// ### MEMCPY Format (uncompressed):
/// ```text
/// +--------+------+
/// | header | data |
/// +--------+------+
///   32 bytes  nbytes
/// ```
///
/// ## Header Parsing
///
/// The function first reads the 16-byte basic header to extract:
/// - Version number (must be >= 5 for this implementation)
/// - Flags (indicates compression method, filters, and format details)
/// - Type size, nbytes (uncompressed size), blocksize, and cbytes (compressed size)
///
/// For version 5, the header is always 32 bytes (16 basic + 16 extended), regardless
/// of whether the extended features are actually used.
///
/// ## Decompression Process
///
/// 1. **Detect MEMCPY mode**: Check if BLOSC_MEMCPYED flag (0x02) is set
///    - If yes: Data starts immediately after 32-byte header (no bstarts, no csize)
///    - If no: Continue to step 2
///
/// 2. **Read bstarts**: For compressed data, read the first bstart value from byte 32
///    - This value points to where the compressed stream begins
///    - Allows skipping optional dictionary section if present
///
/// 3. **Read csize**: At the bstart offset, read 4-byte compressed size
///
/// 4. **Decompress**: Use the appropriate codec to decompress the data
///
/// 5. **Apply inverse filters**: If shuffle or bitshuffle was used, reverse it
///
/// ## Parameters
/// - `src`: Compressed data buffer (must contain full Blosc2 chunk)
/// - `dest`: Output buffer for decompressed data (must be at least nbytes in size)
///
/// ## Returns
/// - `Ok(nbytes)`: Number of bytes decompressed
/// - `Err(-1)`: Decompression failed (invalid format, corrupted data, etc.)
///
/// ## Validation
///
/// The function performs several safety checks:
/// - Buffer size validation (src must be >= cbytes, dest must be >= nbytes)
/// - bstart value validation (must be within reasonable range)
/// - Bounds checking on all array accesses
///
/// ## Reference
/// Based on C-Blosc2 implementation in `c-blosc2/blosc/blosc2.c`
pub fn decompress(src: &[u8], dest: &mut [u8]) -> Result<usize, i32> {
    if src.len() < 16 { return Err(-1); }
    
    // Parse basic header (first 16 bytes)
    // Format: version | versionlz | flags | typesize | nbytes(4) | blocksize(4) | cbytes(4)
    let version = src[0];
    let flags = src[2];
    let compressor = (flags >> 5) & 0x7;
    let typesize = src[3] as usize;
    let nbytes = u32::from_le_bytes([src[4], src[5], src[6], src[7]]) as usize;
    let blocksize = u32::from_le_bytes([src[8], src[9], src[10], src[11]]) as usize;
    let cbytes = u32::from_le_bytes([src[12], src[13], src[14], src[15]]) as usize;
    
    // For Blosc2 (version >= 5), header is ALWAYS 32 bytes (extended header)
    // regardless of flags. The C implementation always writes 32-byte headers
    // unless BLOSC_BLOSC1_COMPAT environment variable is set.
    // The extended header contains: filters(8) | compcode_meta(1) | reserved(1) | 
    // filters_meta(8) | blosc2_flags(1) | reserved(3)
    let header_len = if version >= 5 { 32 } else { 16 };
    let extended_header = version >= 5;

    if src.len() < cbytes { return Err(-1); }
    if dest.len() < nbytes { return Err(-1); }
    
    // Check if data is MEMCPY (uncompressed) FIRST, as MEMCPY has a different format
    let is_memcpy = (flags & BLOSC_MEMCPYED) != 0;
    
    // Blosc2 Format (version >= 5):
    //
    // For compressed data:
    // +--------+----------+-----------------+---------+
    // | header | bstarts  | [optional dict] | streams |
    // +--------+----------+-----------------+---------+
    //   32 bytes  4*nblocks   4+dsize          csize+data per block
    //
    // For MEMCPY (uncompressed):
    // +--------+------+
    // | header | data |
    // +--------+------+
    //   32 bytes  nbytes
    //
    // Key differences:
    // - header: always 32 bytes for version 5 (16 basic + 16 extended)
    // - MEMCPY: data starts immediately after header (NO bstarts array!)
    // - Compressed: bstarts array of int32 offsets follows header
    // - Compressed streams: for each block, csize(4 bytes) + compressed_data
    
    if is_memcpy {
        // For MEMCPY mode, data starts right after header (no bstarts, no csize)
        let data_start = header_len;
        if src.len() < data_start + nbytes { return Err(-1); }
        dest[0..nbytes].copy_from_slice(&src[data_start..data_start + nbytes]);
        return Ok(nbytes);
    }
    
    // For compressed data, calculate block information
    let _nblocks = if blocksize > 0 {
        (nbytes + blocksize - 1) / blocksize
    } else {
        1
    };
    
    let data_start = if version >= 5 {
        // Read the first bstart value to find where streams actually start
        let bstart_pos = header_len;
        if src.len() < bstart_pos + 4 { return Err(-1); }
        let first_bstart = u32::from_le_bytes([
            src[bstart_pos], src[bstart_pos+1], src[bstart_pos+2], src[bstart_pos+3]
        ]) as usize;
        
        // Validate bstart value is reasonable
        if first_bstart < header_len || first_bstart >= cbytes || first_bstart >= src.len() {
            return Err(-1);
        }
        
        first_bstart
    } else {
        // For older versions (Blosc1), data starts right after 16-byte header
        header_len
    };
    
    // For compressed data, there's a csize field before the data
    let csize_size = if version >= 5 { 4 } else { 0 };
    let compressed_data_start = data_start + csize_size;
    
    // Sanity check
    if compressed_data_start > cbytes || compressed_data_start >= src.len() {
        return Err(-1);
    }
    
    let data_len = cbytes - compressed_data_start;
    if src.len() < compressed_data_start + data_len { return Err(-1); }
    let compressed_data = &src[compressed_data_start..compressed_data_start + data_len];
    
    // Decompress (MEMCPY already handled above)
    let mut use_filters = (flags & BLOSC_DOSHUFFLE) != 0 || (flags & BLOSC_DOBITSHUFFLE) != 0;
    let mut do_shuffle = (flags & BLOSC_DOSHUFFLE) != 0;
    let mut do_bitshuffle = (flags & BLOSC_DOBITSHUFFLE) != 0;
    
    if extended_header {
        do_shuffle = false;
        do_bitshuffle = false;
        use_filters = false;
        
        for i in 0..8 {
            let filter = src[16 + i];
            if filter == 1 { do_shuffle = true; use_filters = true; }
            if filter == 2 { do_bitshuffle = true; use_filters = true; }
        }
    }

    let not_split = (flags & BLOSC_NOT_SPLIT) != 0;
    
    if use_filters {
        let mut tmp_buf = vec![0u8; nbytes];
        
        if not_split {
             let size = decompress_buffer(compressor, compressed_data, &mut tmp_buf)?;
             if size != nbytes { return Err(-1); }
        } else {
             decompress_blocks(compressor, compressed_data, &mut tmp_buf, blocksize, nbytes)?;
        }
        
        if do_shuffle {
            filters::unshuffle(typesize, nbytes, &tmp_buf, dest);
        } else if do_bitshuffle {
            filters::bitunshuffle(typesize, nbytes, &tmp_buf, dest).map_err(|_| -1)?;
        }
    } else {
        if not_split {
             let size = decompress_buffer(compressor, compressed_data, dest)?;
             if size != nbytes { return Err(-1); }
        } else {
             decompress_blocks(compressor, compressed_data, dest, blocksize, nbytes)?;
        }
    }
    
    Ok(nbytes)
}

pub fn getitem(src: &[u8], start: usize, nitems: usize, dest: &mut [u8]) -> Result<usize, i32> {
    if src.len() < 16 { return Err(-1); }
    
    // Parse header
    let typesize = src[3] as usize;
    let nbytes = u32::from_le_bytes([src[4], src[5], src[6], src[7]]) as usize;
    
    if typesize == 0 { return Err(-1); }
    
    let start_byte = start * typesize;
    let num_bytes = nitems * typesize;
    
    if start_byte + num_bytes > nbytes { return Err(-1); }
    if dest.len() < num_bytes { return Err(-1); }
    
    // For now, we decompress the whole block. 
    // Optimization: If memcpyed, we can just copy.
    
    let flags = src[2];
    if (flags & BLOSC_MEMCPYED) != 0 {
        let cbytes = u32::from_le_bytes([src[12], src[13], src[14], src[15]]) as usize;
        if src.len() < cbytes { return Err(-1); }
        let compressed_data = &src[16..cbytes];
        dest[0..num_bytes].copy_from_slice(&compressed_data[start_byte..start_byte+num_bytes]);
        return Ok(num_bytes);
    }

    // Full decompression needed
    let mut tmp_buf = vec![0u8; nbytes];
    decompress(src, &mut tmp_buf)?;
    
    dest[0..num_bytes].copy_from_slice(&tmp_buf[start_byte..start_byte+num_bytes]);
    
    Ok(num_bytes)
}
