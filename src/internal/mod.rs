use crate::codecs::blosclz;
use crate::filters;
use std::io::{Read, Write};

// Constants
pub const BLOSC_VERSION_FORMAT: u8 = 2;
pub const BLOSC_MIN_HEADER_LENGTH: usize = 16;

// Compressor codes
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

fn create_header(
    nbytes: usize,
    blocksize: usize,
    cbytes: usize,
    typesize: usize,
    flags: u8,
    compressor: u8,
) -> [u8; 16] {
    let mut header = [0u8; 16];
    header[0] = BLOSC_VERSION_FORMAT;
    header[1] = 1; // versionlz
    header[2] = flags | (compressor << 5);
    header[3] = typesize as u8;
    header[4..8].copy_from_slice(&(nbytes as u32).to_le_bytes());
    header[8..12].copy_from_slice(&(blocksize as u32).to_le_bytes());
    header[12..16].copy_from_slice(&(cbytes as u32).to_le_bytes());
    header
}

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
            let mut decoder = ruzstd::decoding::StreamingDecoder::new(std::io::Cursor::new(src)).map_err(|_| -1)?;
            match decoder.read_exact(dest) {
                Ok(_) => Ok(dest.len()),
                Err(_) => Ok(0),
            }
        },
        _ => Err(-1),
    }
}

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
    println!("Compressing: nbytes={}, typesize={}, doshuffle={}, compressor={}", nbytes, typesize, doshuffle, compressor);
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
    if dest.len() < 16 { return Err(-1); }
    let max_compressed_size = dest.len() - 16;
    let mut compressed_size = 0;
    
    match compressor {
        BLOSC_BLOSCLZ => {
            compressed_size = blosclz::compress(clevel, filtered_src, &mut dest[16..]);
        },
        BLOSC_LZ4 => {
             match lz4_flex::block::compress_into(filtered_src, &mut dest[16..]) {
                 Ok(size) => compressed_size = size,
                 Err(_) => compressed_size = 0,
             }
        },
        BLOSC_SNAPPY => {
            let mut encoder = snap::raw::Encoder::new();
            match encoder.compress(filtered_src, &mut dest[16..]) {
                Ok(size) => compressed_size = size,
                Err(_) => compressed_size = 0,
            }
        },
        BLOSC_ZLIB => {
            let cursor = std::io::Cursor::new(&mut dest[16..]);
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
            // ruzstd does not support compression
            return Err(-1);
        },
        _ => return Err(-1),
    }
    println!("Compressed size: {}", compressed_size);
    
    if compressed_size == 0 || compressed_size >= nbytes {
         // Memcpy
         if nbytes > max_compressed_size { return Err(-1); }
         dest[16..16+nbytes].copy_from_slice(src);
         compressed_size = nbytes;
         flags |= BLOSC_MEMCPYED;
         flags &= !BLOSC_DOSHUFFLE;
         flags &= !BLOSC_DOBITSHUFFLE;
    }

    // 3. Header
    let cbytes = compressed_size + 16;
    let header = create_header(nbytes, blocksize, cbytes, typesize, flags, compressor);
    dest[0..16].copy_from_slice(&header);
    
    Ok(cbytes)
}

pub fn decompress(src: &[u8], dest: &mut [u8]) -> Result<usize, i32> {
    if src.len() < 16 { return Err(-1); }
    
    // Parse header
    let flags = src[2];
    let compressor = (flags >> 5) & 0x7;
    let typesize = src[3] as usize;
    let nbytes = u32::from_le_bytes([src[4], src[5], src[6], src[7]]) as usize;
    let _blocksize = u32::from_le_bytes([src[8], src[9], src[10], src[11]]) as usize;
    let cbytes = u32::from_le_bytes([src[12], src[13], src[14], src[15]]) as usize;
    
    if src.len() < cbytes { return Err(-1); }
    if dest.len() < nbytes { return Err(-1); }
    
    let compressed_data = &src[16..cbytes];
    
    // Handle Memcpy
    if (flags & BLOSC_MEMCPYED) != 0 {
        dest[0..nbytes].copy_from_slice(compressed_data);
        return Ok(nbytes);
    }
    
    // Decompress
    let use_filters = (flags & BLOSC_DOSHUFFLE) != 0 || (flags & BLOSC_DOBITSHUFFLE) != 0;
    
    if use_filters {
        let mut tmp_buf = vec![0u8; nbytes];
        let size = decompress_buffer(compressor, compressed_data, &mut tmp_buf)?;
        if size != nbytes { return Err(-1); }
        
        if (flags & BLOSC_DOSHUFFLE) != 0 {
            filters::unshuffle(typesize, nbytes, &tmp_buf, dest);
        } else if (flags & BLOSC_DOBITSHUFFLE) != 0 {
            filters::bitunshuffle(typesize, nbytes, &tmp_buf, dest).map_err(|_| -1)?;
        }
    } else {
        let size = decompress_buffer(compressor, compressed_data, dest)?;
        if size != nbytes { return Err(-1); }
    }
    
    Ok(nbytes)
}
