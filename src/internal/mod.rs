use crate::codecs::blosclz;
use crate::filters;
use crate::internal::constants::*;
use std::io::{Read, Write};

pub mod constants;

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
) -> [u8; BLOSC_EXTENDED_HEADER_LENGTH] {
    let mut header = [0u8; BLOSC_EXTENDED_HEADER_LENGTH];
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
        BLOSC_BLOSCLZ => Ok(blosclz::decompress(src, dest)),
        BLOSC_LZ4 => match lz4_flex::block::decompress_into(src, dest) {
            Ok(size) => Ok(size),
            Err(_) => Ok(0),
        },
        BLOSC_SNAPPY => {
            let mut decoder = snap::raw::Decoder::new();
            match decoder.decompress(src, dest) {
                Ok(size) => Ok(size),
                Err(_) => Ok(0),
            }
        }
        BLOSC_ZLIB => {
            let mut decoder = flate2::read::ZlibDecoder::new(src);
            match decoder.read_exact(dest) {
                Ok(_) => Ok(dest.len()), // Assuming exact read
                Err(_) => Ok(0),
            }
        }
        BLOSC_ZSTD => {
            let mut decoder =
                zstd::stream::read::Decoder::new(std::io::Cursor::new(src)).map_err(|_| -1)?;
            match decoder.read_exact(dest) {
                Ok(_) => Ok(dest.len()),
                Err(_) => Ok(0),
            }
        }
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
    compress_internal(
        clevel,
        doshuffle,
        typesize,
        src,
        dest,
        compressor,
        false,
        &[BLOSC_NOFILTER as u8; 6],
        &[0; 6],
    )
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

fn compute_blocksize(clevel: i32, typesize: usize, nbytes: usize, compressor: u8) -> usize {
    if nbytes < typesize {
        return nbytes.max(1);
    }

    let mut blocksize = nbytes;

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

    if blocksize > nbytes {
        blocksize = nbytes;
    }

    if blocksize > typesize {
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

fn split_block(
    compressor: u8,
    clevel: i32,
    typesize: usize,
    blocksize: usize,
    filter_flags: u8,
) -> bool {
    // Only split for byte shuffle, NOT bitshuffle (as per c-blosc2 stune.c)
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
    let blocksize = compute_blocksize(clevel, typesize, nbytes, compressor);
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

    let mut flags = 0;
    if doshuffle == BLOSC_SHUFFLE as i32 {
        flags |= BLOSC_DOSHUFFLE;
    } else if doshuffle == BLOSC_BITSHUFFLE as i32 {
        flags |= BLOSC_DOBITSHUFFLE;
    }

    if extended_header {
        flags |= BLOSC_DOSHUFFLE | BLOSC_DOBITSHUFFLE;
    }

    let split = split_block(compressor, clevel, typesize, blocksize, flags);

    if !split && clevel != 0 && nbytes >= BLOSC_MIN_BUFFERSIZE {
        flags |= 0x10;
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

        let block_split = split_block(compressor, clevel, typesize, block_len, flags);
        let nstreams = if block_split { typesize } else { 1 };
        let neblock = block_len / nstreams;

        for j in 0..nstreams {
            let stream_offset = j * neblock;
            let stream_src = &filtered_src[stream_offset..stream_offset + neblock];

            if current_dest_offset + 4 > dest.len() {
                incompressible = true;
                break;
            }

            let mut stream_csize;

            match compressor {
                BLOSC_BLOSCLZ => {
                    stream_csize =
                        blosclz::compress(clevel, stream_src, &mut dest[current_dest_offset + 4..]);
                }
                BLOSC_LZ4 => {
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

pub fn decompress(src: &[u8], dest: &mut [u8]) -> Result<usize, Box<dyn std::error::Error>> {
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

    if dest.len() < nbytes {
        return Err("Destination buffer too small".into());
    }

    let flags = src[2];
    let compressor = (flags >> 5) & 0x7;
    let typesize = src[3] as usize;

    let mut doshuffle = (flags & BLOSC_DOSHUFFLE) != 0;
    let mut dobitshuffle = (flags & BLOSC_DOBITSHUFFLE) != 0;

    // Check for extended header marker (both flags set)
    if (flags & BLOSC_DOSHUFFLE) != 0 && (flags & BLOSC_DOBITSHUFFLE) != 0 {
        doshuffle = false;
        dobitshuffle = false;

        if header_len == BLOSC_EXTENDED_HEADER_LENGTH {
            let filters = &src[16..22];
            for &f in filters {
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

    if (flags & BLOSC_MEMCPYED) != 0 {
        // Copy from after header to dest
        dest[0..nbytes].copy_from_slice(&src[header_len..header_len + nbytes]);
        return Ok(nbytes);
    }

    // Calculate number of blocks
    let nblocks = if blocksize == 0 {
        0
    } else {
        (nbytes + blocksize - 1) / blocksize
    };

    // For extended headers with actual compression (not memcpy), read bstarts array
    let mut bstarts = Vec::new();

    if nblocks > 0 {
        // Always read bstarts array if nblocks > 0
        // Note: This assumes Blosc1 format also includes bstarts for nblocks=1.
        // c-blosc implementation suggests it does.
        if src.len() < header_len + nblocks * 4 {
            return Err("Buffer too small for bstarts array".into());
        }

        for i in 0..nblocks {
            let offset = header_len + i * 4;
            let bstart = u32::from_le_bytes([
                src[offset],
                src[offset + 1],
                src[offset + 2],
                src[offset + 3],
            ]) as usize;
            bstarts.push(bstart);
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
            nbytes % blocksize // Last block may be smaller
        } else {
            blocksize
        };

        if src_offset + block_cbytes > src.len() {
            return Err("Compressed data is truncated".into());
        }

        let content = &src[src_offset..src_offset + block_cbytes];

        let mut content_offset = 0;
        let mut block_dest_offset = 0;

        let use_temp = doshuffle || dobitshuffle;
        let mut temp_buf = if use_temp {
            vec![0u8; block_nbytes]
        } else {
            Vec::new()
        };
        {
            let target_slice = if use_temp {
                &mut temp_buf[..]
            } else {
                &mut dest[dest_offset..dest_offset + block_nbytes]
            };

            while content_offset < block_cbytes {
                if content_offset + 4 > block_cbytes {
                    return Err("Block too small for chunk size".into());
                }
                let chunk_cbytes = u32::from_le_bytes([
                    content[content_offset],
                    content[content_offset + 1],
                    content[content_offset + 2],
                    content[content_offset + 3],
                ]) as usize;
                content_offset += 4;

                if content_offset + chunk_cbytes > block_cbytes {
                    return Err("Chunk size exceeds block size".into());
                }

                let chunk_content = &content[content_offset..content_offset + chunk_cbytes];
                content_offset += chunk_cbytes;

                let dest_slice = &mut target_slice[block_dest_offset..block_nbytes];

                let chunk_decompressed_size = match compressor {
                    BLOSC_BLOSCLZ => blosclz::decompress(chunk_content, dest_slice),
                    BLOSC_LZ4 | BLOSC_LZ4HC => lz4_flex::decompress_into(chunk_content, dest_slice)
                        .map_err(|e| format!("LZ4 error: {}", e))?,
                    BLOSC_SNAPPY => {
                        let mut decoder = snap::raw::Decoder::new();
                        decoder
                            .decompress(chunk_content, dest_slice)
                            .map_err(|e| format!("Snappy error: {}", e))?
                    }
                    BLOSC_ZLIB => {
                        let mut decoder = flate2::read::ZlibDecoder::new(chunk_content);
                        let mut writer = std::io::Cursor::new(dest_slice);
                        std::io::copy(&mut decoder, &mut writer)
                            .map_err(|e| format!("Zlib error: {}", e))?
                            as usize
                    }
                    BLOSC_ZSTD => zstd::bulk::decompress_to_buffer(chunk_content, dest_slice)
                        .map_err(|e| format!("Zstd error: {}", e))?,
                    _ => return Err(format!("Unsupported compressor: {}", compressor).into()),
                };

                block_dest_offset += chunk_decompressed_size;
            }

            if block_dest_offset != block_nbytes {
                return Err(format!(
                    "Block {} decompression size mismatch: expected {}, got {}",
                    i, block_nbytes, block_dest_offset
                )
                .into());
            }
        }

        if use_temp {
            if doshuffle {
                filters::unshuffle(
                    typesize,
                    block_nbytes,
                    &temp_buf,
                    &mut dest[dest_offset..dest_offset + block_nbytes],
                );
            } else {
                filters::bitunshuffle(
                    typesize,
                    block_nbytes,
                    &temp_buf,
                    &mut dest[dest_offset..dest_offset + block_nbytes],
                )
                .map_err(|e| format!("Bitunshuffle error: {}", e))?;
            }
        }

        dest_offset += block_dest_offset;
    }

    Ok(nbytes)
}

pub fn getitem(src: &[u8], start: usize, nitems: usize, dest: &mut [u8]) -> Result<usize, i32> {
    if src.len() < BLOSC_MIN_HEADER_LENGTH {
        return Err(-1);
    }

    // Check version to determine header size
    let version = src[0];
    let header_len = if version == BLOSC2_VERSION_FORMAT_STABLE
        || version == BLOSC2_VERSION_FORMAT_BETA1
        || version == BLOSC2_VERSION_FORMAT_ALPHA
    {
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

    if src.len() < cbytes {
        return Err(-1);
    }

    let start_byte = start * typesize;
    let end_byte = (start + nitems) * typesize;
    if end_byte > nbytes {
        return Err(-1);
    }
    if dest.len() < end_byte - start_byte {
        return Err(-1);
    }

    if (flags & BLOSC_MEMCPYED) != 0 {
        dest.copy_from_slice(&src[header_len + start_byte..header_len + end_byte]);
        return Ok(end_byte - start_byte);
    }

    let nblocks = if nbytes == 0 {
        0
    } else {
        (nbytes + blocksize - 1) / blocksize
    };

    let start_block = start_byte / blocksize;
    let end_block = (end_byte - 1) / blocksize;

    let mut src_offset = header_len;
    let mut block_sizes = Vec::new();
    if nblocks > 1 {
        src_offset += nblocks * 4;
        for i in 0..nblocks {
            let bs = u32::from_le_bytes(
                src[header_len + i * 4..header_len + i * 4 + 4]
                    .try_into()
                    .unwrap(),
            ) as usize;
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

        let src_block = &src[src_offset..src_offset + cblock_size];

        // We need to decompress the whole block to extract items
        let mut block_buf = vec![0u8; current_block_size];

        let mut temp_buf = if (flags & (BLOSC_DOSHUFFLE | BLOSC_DOBITSHUFFLE)) != 0 {
            vec![0u8; current_block_size]
        } else {
            Vec::new()
        };

        if temp_buf.is_empty() {
            let decompressed_size = decompress_buffer(compressor, src_block, &mut block_buf)?;
            if decompressed_size != current_block_size {
                return Err(-1);
            }
        } else {
            let decompressed_size = decompress_buffer(compressor, src_block, &mut temp_buf)?;
            if decompressed_size != current_block_size {
                return Err(-1);
            }

            if (flags & BLOSC_DOSHUFFLE) != 0 {
                filters::unshuffle(typesize, current_block_size, &temp_buf, &mut block_buf);
            } else if (flags & BLOSC_DOBITSHUFFLE) != 0 {
                filters::bitunshuffle(typesize, current_block_size, &temp_buf, &mut block_buf)
                    .map_err(|_| -1)?;
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

        dest[dest_offset..dest_offset + len].copy_from_slice(&block_buf[local_start..local_end]);

        src_offset += cblock_size;
        dest_offset += len;
    }

    Ok(dest_offset)
}
