use crate::codecs::blosclz;
use crate::filters;
use std::io::{Read, Write};
use crate::internal::constants::*;

pub mod constants;

fn create_header_blosc1(
    nbytes: usize,
    blocksize: usize,
    cbytes: usize,
    typesize: usize,
    flags: u8,
    compressor: u8,
) -> [u8; 16] {
    let mut header = [0u8; 16];
    header[0] = BLOSC1_VERSION_FORMAT;
    header[1] = 1; // Version for compressor (e.g., 1 for blosclz)
    header[2] = flags | (compressor << 5);
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
) -> [u8; 32] {
    let mut header = [0u8; 32];
    // First 16 bytes: standard Blosc header
    header[0] = BLOSC2_VERSION_FORMAT_STABLE;
    header[1] = 1; // Version for compressor (e.g., 1 for blosclz)
    header[2] = flags | (compressor << 5);
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
    compress_internal(clevel, doshuffle, typesize, src, dest, compressor, false, &[BLOSC_NOFILTER as u8; 6], &[0; 6])
}

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
    compress_internal(clevel, doshuffle, typesize, src, dest, compressor, true, filters, filters_meta)
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
    let blocksize = nbytes; // Single block
    let nblocks = 1; // Single block for now
    
    let header_len = if extended_header { BLOSC_EXTENDED_HEADER_LENGTH } else { BLOSC_MIN_HEADER_LENGTH };
    
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
    
    // For extended headers, set both DOSHUFFLE and DOBITSHUFFLE as a marker
    // (This is how Blosc2 indicates an extended header - both flags set is invalid for actual filtering)
    if extended_header {
        flags |= BLOSC_DOSHUFFLE | BLOSC_DOBITSHUFFLE;
    }

    // Calculate data start offset (header + bstarts array for extended non-memcpy)
    let mut data_offset = header_len;
    let will_add_bstarts = extended_header && (clevel > 0) && (nbytes >= BLOSC_MIN_BUFFERSIZE);
    if will_add_bstarts {
        data_offset += nblocks * 4; // Each block start is an i32 (4 bytes)
    }

    // 2. Compress
    if dest.len() < data_offset { return Err(-1); }
    let max_compressed_size = dest.len() - data_offset;
    let mut compressed_size;
    
    match compressor {
        BLOSC_BLOSCLZ => {
            compressed_size = blosclz::compress(clevel, filtered_src, &mut dest[data_offset..]);
        },
        BLOSC_LZ4 => {
             match lz4_flex::block::compress_into(filtered_src, &mut dest[data_offset..]) {
                 Ok(size) => compressed_size = size,
                 Err(_) => compressed_size = 0,
             }
        },
        BLOSC_SNAPPY => {
            let mut encoder = snap::raw::Encoder::new();
            match encoder.compress(filtered_src, &mut dest[data_offset..]) {
                Ok(size) => compressed_size = size,
                Err(_) => compressed_size = 0,
            }
        },
        BLOSC_ZLIB => {
            let cursor = std::io::Cursor::new(&mut dest[data_offset..]);
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
            let cursor = std::io::Cursor::new(&mut dest[data_offset..]);
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
    
    let mut actual_data_offset = header_len;
    if compressed_size == 0 || compressed_size >= nbytes {
         // Memcpy
         if nbytes > max_compressed_size { return Err(-1); }
         dest[header_len..header_len+nbytes].copy_from_slice(src);
         compressed_size = nbytes;
         flags |= BLOSC_MEMCPYED;
         // When memcpyed, clear the extended header marker flags
         if extended_header {
             flags &= !(BLOSC_DOSHUFFLE | BLOSC_DOBITSHUFFLE);
         }
         // No bstarts for memcpy
         actual_data_offset = header_len;
    } else if will_add_bstarts {
        // Write bstarts array (offset where compressed data begins for single block)
        let bstart_offset = header_len;
        dest[bstart_offset..bstart_offset+4].copy_from_slice(&(data_offset as u32).to_le_bytes());
        actual_data_offset = data_offset;
    }

    // 3. Header
    let cbytes = compressed_size + actual_data_offset;
    if extended_header {
        let header = create_header_blosc2(nbytes, blocksize, cbytes, typesize, flags, compressor, filters, filters_meta);
        dest[0..32].copy_from_slice(&header);
    } else {
        let header = create_header_blosc1(nbytes, blocksize, cbytes, typesize, flags, compressor);
        dest[0..16].copy_from_slice(&header);
    }
    
    Ok(cbytes)
}

pub fn decompress(src: &[u8], dest: &mut [u8]) -> Result<usize, Box<dyn std::error::Error>> {
    if src.len() < BLOSC_MIN_HEADER_LENGTH {
        return Err("Source buffer too small for header".into());
    }

    println!("DEBUG: decompress called. src.len()={}", src.len());
    println!("DEBUG: src[0..32]={:?}", &src[0..std::cmp::min(32, src.len())]);

    // Read version to determine header size
    let version = src[0];
    let header_len = if version == BLOSC2_VERSION_FORMAT_STABLE || version == BLOSC2_VERSION_FORMAT_BETA1 || version == BLOSC2_VERSION_FORMAT_ALPHA {
        BLOSC_EXTENDED_HEADER_LENGTH
    } else {
        BLOSC_MIN_HEADER_LENGTH
    };
    
    if src.len() < header_len {
        return Err("Source buffer too small for extended header".into());
    }

    let (nbytes, cbytes, blocksize) = crate::api::blosc2_cbuffer_sizes(src);
    println!("DEBUG: nbytes={}, cbytes={}, blocksize={}, version={}, header_len={}", nbytes, cbytes, blocksize, version, header_len);

    if dest.len() < nbytes {
        return Err("Destination buffer too small".into());
    }

    let flags = src[2];
    let compressor = (flags >> 5) & 0x7;
    let typesize = src[3] as usize;

    println!("DEBUG: flags={:b}, compressor={}, typesize={}", flags, compressor, typesize);

    if (flags & BLOSC_MEMCPYED) != 0 {
        println!("DEBUG: MEMCPYED path taken");
        // Copy from after header to dest
        dest[0..nbytes].copy_from_slice(&src[header_len..header_len+nbytes]);
        return Ok(nbytes);
    }
    
    // Calculate number of blocks
    let nblocks = if blocksize == 0 {
        0
    } else {
        (nbytes + blocksize - 1) / blocksize
    };

    println!("Decompressing: nbytes={}, blocksize={}, cbytes={}, nblocks={}", nbytes, blocksize, cbytes, nblocks);

    // For extended headers with actual compression (not memcpy), read bstarts array
    let mut bstarts = Vec::new();
    
    if nblocks > 1 {
        // Multiple blocks: read bstarts array
        if src.len() < header_len + nblocks * 4 {
            return Err("Buffer too small for bstarts array".into());
        }
        
        for i in 0..nblocks {
            let offset = header_len + i * 4;
            let bstart = u32::from_le_bytes([src[offset], src[offset+1], src[offset+2], src[offset+3]]) as usize;
            bstarts.push(bstart);
        }
        
        println!("DEBUG: bstarts = {:?}", bstarts);
    } else if nblocks == 1 {
        // Single block handling depends on header type
        if header_len == BLOSC_EXTENDED_HEADER_LENGTH {
            // Extended header (Blosc2): Always has bstarts array, even for single block
            if src.len() < header_len + 4 {
                return Err("Buffer too small for bstarts array".into());
            }
            let bstart = u32::from_le_bytes([src[header_len], src[header_len+1], src[header_len+2], src[header_len+3]]) as usize;
            bstarts.push(bstart);
            println!("DEBUG: single block bstart (extended header) = {}", bstart);
        } else {
            // Blosc1 format (16-byte header): No bstarts, data starts immediately after header
            bstarts.push(header_len);
            println!("DEBUG: single block, no bstarts (Blosc1), data starts at {}", header_len);
        }
    }

    // Decompress each block
    let mut dest_offset = 0;
    
    for i in 0..nblocks {
        let src_offset = bstarts[i];
        
        // Determine block size in compressed buffer
        let block_cbytes = if i + 1 < nblocks {
            bstarts[i + 1] - src_offset
        } else {
            cbytes - src_offset
        };
        
        // Determine uncompressed block size
        let block_nbytes = if i == nblocks - 1 && nbytes % blocksize != 0 {
            nbytes % blocksize  // Last block may be smaller
        } else {
            blocksize
        };

        println!("Block {}: src_offset={}, block_cbytes={}, block_nbytes={}", i, src_offset, block_cbytes, block_nbytes);

        if src_offset + block_cbytes > src.len() {
             return Err("Compressed data is truncated".into());
        }

        let content = &src[src_offset..src_offset + block_cbytes];
        println!("Content start: {:02x?}", &content[0..std::cmp::min(16, content.len())]);

        let decompressed_size = match compressor {
            BLOSC_BLOSCLZ => {
                blosclz::decompress(content, &mut dest[dest_offset..dest_offset + block_nbytes])
            },
            _ => return Err("Unsupported compressor".into()),
        };
        
        println!("Decompressed size: {}", decompressed_size);
        
        if decompressed_size != block_nbytes {
            return Err(format!("Block {} decompression size mismatch: expected {}, got {}", i, block_nbytes, decompressed_size).into());
        }

        dest_offset += decompressed_size;
    }
    
    Ok(nbytes)
}

pub fn getitem(src: &[u8], start: usize, nitems: usize, dest: &mut [u8]) -> Result<usize, i32> {
    if src.len() < 16 { return Err(-1); }
    
    // Check version to determine header size
    let version = src[0];
    let header_len = if version == BLOSC2_VERSION_FORMAT_STABLE || version == BLOSC2_VERSION_FORMAT_BETA1 || version == BLOSC2_VERSION_FORMAT_ALPHA {
        BLOSC_EXTENDED_HEADER_LENGTH
    } else {
        BLOSC_MIN_HEADER_LENGTH
    };
    
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
        dest.copy_from_slice(&src[header_len + start_byte .. header_len + end_byte]);
        return Ok(end_byte - start_byte);
    }
    
    let nblocks = if nbytes == 0 { 0 } else { (nbytes + blocksize - 1) / blocksize };
    
    let start_block = start_byte / blocksize;
    let end_block = (end_byte - 1) / blocksize;
    
    let mut src_offset = header_len;
    let mut block_sizes = Vec::new();
    if nblocks > 1 {
        src_offset += nblocks * 4;
        for i in 0..nblocks {
            let bs = u32::from_le_bytes(src[header_len + i*4 .. header_len + i*4 + 4].try_into().unwrap()) as usize;
            block_sizes.push(bs);
        }
    } else {
        block_sizes.push(cbytes - header_len);
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
