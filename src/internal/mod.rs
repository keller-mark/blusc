use crate::codecs::blosclz;
use crate::filters;
use std::io::{Read, Write};
use crate::internal::constants::*;

pub mod constants;

fn create_header(
    nbytes: usize,
    blocksize: usize,
    cbytes: usize,
    typesize: usize,
    flags: u8,
    compressor: u8,
) -> [u8; 16] {
    // TODO
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
            let mut decoder = zstd::stream::read::Decoder::new(std::io::Cursor::new(src)).map_err(|_| -1)?;
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
    
    let mut flags = ; // Single block // TODO: fix
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
    let mut compressed_size;
    
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
            let cursor = std::io::Cursor::new(&mut dest[16..]);
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

fn decompress_blocks(
    compressor: u8, 
    compressed_data: &[u8], 
    dest: &mut [u8], 
    blocksize: usize, 
    nbytes: usize
) -> Result<(), i32> {
    // TODO
}

pub fn decompress(src: &[u8], dest: &mut [u8]) -> Result<usize, i32> {
    // TODO
}

pub fn getitem(src: &[u8], start: usize, nitems: usize, dest: &mut [u8]) -> Result<usize, i32> {
    // TODO
}
