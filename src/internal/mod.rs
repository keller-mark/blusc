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
    let mut header = [0u8; 16];
    header[0] = BLOSC1_VERSION_FORMAT;
    header[1] = 1; // BloscLZ version
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
    
    let mut flags = 0; // Single block
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

pub fn decompress(src: &[u8], dest: &mut [u8]) -> Result<usize, Box<dyn std::error::Error>> {
    if src.len() < BLOSC_MIN_HEADER_LENGTH {
        return Err("Source buffer too small for header".into());
    }

    println!("DEBUG: decompress called. src.len()={}", src.len()); // Added debug print
    println!("DEBUG: src[0..32]={:?}", &src[0..std::cmp::min(32, src.len())]); // Added debug print

    let (nbytes, cbytes, blocksize) = crate::api::blosc2_cbuffer_sizes(src);
    println!("DEBUG: nbytes={}, cbytes={}, blocksize={}", nbytes, cbytes, blocksize); // Added debug print

    if dest.len() < nbytes {
        return Err("Destination buffer too small".into());
    }

    let flags = src[2];
    let compressor = (flags >> 5) & 0x7;
    let typesize = src[3] as usize;

    println!("DEBUG: flags={:b}, compressor={}, typesize={}", flags, compressor, typesize); // Added debug print

    if (flags & BLOSC_MEMCPYED) != 0 {
        println!("DEBUG: MEMCPYED path taken"); // Added debug print
        dest[0..nbytes].copy_from_slice(&src[16..16+nbytes]);
        return Ok(nbytes);
    }
    
    let mut src_offset = BLOSC_MIN_HEADER_LENGTH as usize;
    let mut dest_offset = 0;

    println!("Decompressing: nbytes={}, blocksize={}, cbytes={}", nbytes, blocksize, cbytes);

    while src_offset < cbytes {
        let block_cbytes = if nbytes > blocksize {
            // TODO: Handle multi-block properly.
            cbytes - BLOSC_MIN_HEADER_LENGTH as usize
        } else {
            cbytes - BLOSC_MIN_HEADER_LENGTH as usize
        };

        println!("Block: src_offset={}, block_cbytes={}", src_offset, block_cbytes);

        if src_offset + block_cbytes > src.len() {
             return Err("Compressed data is truncated".into());
        }

        let content = &src[src_offset..src_offset + block_cbytes];
        println!("Content start: {:02x?}", &content[0..std::cmp::min(16, content.len())]);

        let decompressed_size = match compressor {
            BLOSC_BLOSCLZ => {
                blosclz::decompress(content, &mut dest[dest_offset..])
            },
            _ => return Err("Unsupported compressor".into()),
        };
        
        println!("Decompressed size: {}", decompressed_size);
        println!("Dest start after: {:02x?}", &dest[dest_offset..std::cmp::min(dest_offset+16, dest.len())]);

        dest_offset += decompressed_size;
        src_offset += block_cbytes;
    }
    
    Ok(nbytes)
}

pub fn getitem(src: &[u8], start: usize, nitems: usize, dest: &mut [u8]) -> Result<usize, i32> {
    if src.len() < 16 { return Err(-1); }
    
    let flags = src[2];
    let compressor = (flags >> 5) & 0x7;
    let typesize = src[3] as usize;
    let nbytes = u32::from_le_bytes(src[4..8].try_into().unwrap()) as usize;
    let blocksize = u32::from_le_bytes(src[8..12].try_into().unwrap()) as usize;
    let cbytes = u32::from_le_bytes(src[12..16].try_into().unwrap()) as usize;
    
    if src.len() < cbytes { return Err(-1); }
    
    let start_byte = start * typesize;
    let end_byte = (start + nitems) * typesize;
    if end_byte > nbytes { return Err(-1); }
    if dest.len() < end_byte - start_byte { return Err(-1); }
    
    if (flags & BLOSC_MEMCPYED) != 0 {
        dest.copy_from_slice(&src[16 + start_byte .. 16 + end_byte]);
        return Ok(end_byte - start_byte);
    }
    
    let nblocks = if nbytes == 0 { 0 } else { (nbytes + blocksize - 1) / blocksize };
    
    let start_block = start_byte / blocksize;
    let end_block = (end_byte - 1) / blocksize;
    
    let mut src_offset = 16;
    let mut block_sizes = Vec::new();
    if nblocks > 1 {
        src_offset += nblocks * 4;
        for i in 0..nblocks {
            let bs = u32::from_le_bytes(src[16 + i*4 .. 16 + i*4 + 4].try_into().unwrap()) as usize;
            block_sizes.push(bs);
        }
    } else {
        block_sizes.push(cbytes - 16);
    }
    
    // Skip blocks before start_block
    for i in 0..start_block {
        src_offset += block_sizes[i];
    }
    
    let mut dest_offset = 0;
    
    for i in start_block..=end_block {
        let cblock_size = block_sizes[i];
        let current_block_size = if i == nblocks - 1 {
            nbytes - (i * blocksize)
        } else {
            blocksize
        };
        
        let src_block = &src[src_offset .. src_offset + cblock_size];
        
        // We need to decompress the whole block to extract items
        let mut block_buf = vec![0u8; current_block_size];
        
        let mut temp_buf = if (flags & (BLOSC_DOSHUFFLE | BLOSC_DOBITSHUFFLE)) != 0 {
            vec![0u8; current_block_size]
        } else {
            Vec::new()
        };
        
        if temp_buf.is_empty() {
             let decompressed_size = decompress_buffer(compressor, src_block, &mut block_buf)?;
             if decompressed_size != current_block_size { return Err(-1); }
        } else {
             let decompressed_size = decompress_buffer(compressor, src_block, &mut temp_buf)?;
             if decompressed_size != current_block_size { return Err(-1); }
             
             if (flags & BLOSC_DOSHUFFLE) != 0 {
                filters::unshuffle(typesize, current_block_size, &temp_buf, &mut block_buf);
             } else if (flags & BLOSC_DOBITSHUFFLE) != 0 {
                filters::bitunshuffle(typesize, current_block_size, &temp_buf, &mut block_buf).map_err(|_| -1)?;
             }
        }
        
        // Copy relevant part
        let block_start = i * blocksize;
        let block_end = block_start + current_block_size;
        
        let copy_start = std::cmp::max(start_byte, block_start);
        let copy_end = std::cmp::min(end_byte, block_end);
        
        let local_start = copy_start - block_start;
        let local_end = copy_end - block_start;
        let len = local_end - local_start;
        
        dest[dest_offset .. dest_offset + len].copy_from_slice(&block_buf[local_start..local_end]);
        
        src_offset += cblock_size;
        dest_offset += len;
    }
    
    Ok(dest_offset)
}
