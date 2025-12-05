// BloscLZ codec implementation
// Maximum number of literals that can be encoded in a single run
const MAX_COPY: usize = 32;

// Maximum distance for short matches (13-bit, 0-8191)
const MAX_DISTANCE: usize = 8191;

// Maximum distance for far matches (16-bit distance + MAX_DISTANCE)
const MAX_FARDISTANCE: usize = 65535 + MAX_DISTANCE - 1;

// Hash table size (2^14 = 16384 entries)
const HASH_LOG: usize = 14;

/// Hash function for building the hash table during compression.
/// Uses a multiplication-based hash similar to LZ4.
/// The constant 2654435761 is a large prime that provides good distribution.
fn hash_function(v: u32, h: usize) -> usize {
    ((v.wrapping_mul(2654435761)) >> (32 - h)) as usize
}

/// Compress input data using the BloscLZ algorithm.
/// 
/// This is a fast LZ77-based compression algorithm that:
/// - Uses a hash table (2^14 = 16384 entries) to find matching sequences
/// - Encodes matches with (length, distance) pairs
/// - Falls back to literal bytes when matches aren't beneficial
///
/// # Arguments
/// * `clevel` - Compression level (0-9, affects hash log and minimum match length)
/// * `input` - Input data to compress
/// * `output` - Output buffer for compressed data
///
/// # Returns
/// Number of bytes written to output buffer, or 0 if compression failed
///
/// # Format
/// The output format alternates between:
/// - Literal runs: A count byte followed by literal bytes
/// - Matches: Encoded as (length << 5) | distance_high, distance_low
///   - For distances < 8192 (MAX_DISTANCE): 2 bytes base + extensions
///   - For distances >= 8192: 4 bytes base + extensions
///
/// # Reference
/// Based on c-blosc2/blosc/blosclz.c blosclz_compress function
pub fn compress(clevel: i32, input: &[u8], output: &mut [u8]) -> usize {
    let length = input.len();
    let maxout = output.len();
    
    // Minimum input size is 16 bytes, minimum output size is 66 bytes
    if length < 16 || maxout < 66 {
        return 0;
    }

    // Adjust hash_log and ipshift based on compression level
    // Higher levels use more hash table entries for better compression
    let hashlog_table = [0, HASH_LOG - 2, HASH_LOG - 1, HASH_LOG, HASH_LOG,
                         HASH_LOG, HASH_LOG, HASH_LOG, HASH_LOG, HASH_LOG];
    let hashlog = hashlog_table[clevel.clamp(0, 9) as usize];
    
    // ipshift: how much we shift back when finding matches
    // Affects compression ratio vs speed tradeoff
    let ipshift = 4;
    let minlen = 4;

    // Hash table: maps hash values to positions in the input
    // Use dynamic size based on hashlog
    let hash_size = 1 << hashlog;
    let mut htab = vec![0u32; hash_size];
    
    // Current position in input
    let mut ip = 0;
    // Current position in output
    let mut op = 0;
    // Stop searching for matches 12 bytes before the end
    let ip_bound = length - 1;
    let ip_limit = if length > 12 { length - 12 } else { 0 };

    // We start with a literal copy of 4 bytes
    // The first byte is a control byte that will be updated if needed
    let mut copy = 4;
    output[op] = (MAX_COPY - 1) as u8; 
    op += 1;
    output[op] = input[ip]; op += 1; ip += 1;
    output[op] = input[ip]; op += 1; ip += 1;
    output[op] = input[ip]; op += 1; ip += 1;
    output[op] = input[ip]; op += 1; ip += 1;

    // Main compression loop
    while ip < ip_limit {
        let anchor = ip;
        
        // Read 4 bytes for hashing
        let seq = u32::from_le_bytes([input[ip], input[ip+1], input[ip+2], input[ip+3]]);
        let hval = hash_function(seq, hashlog);
        
        // Look up this sequence in the hash table
        let ref_pos = htab[hval] as usize;
        
        // Calculate distance to the potential match
        let distance = anchor - ref_pos;
        
        // Update hash table with current position AFTER looking up
        htab[hval] = anchor as u32;

        // Check if this is a valid match:
        // - distance must be non-zero (not same position)
        // - distance must be within range
        if distance == 0 || distance >= MAX_FARDISTANCE {
             // No valid match, output as literal
             if op + 2 > maxout { return 0; }
             output[op] = input[anchor]; op += 1;
             ip = anchor + 1;
             copy += 1;
             if copy == MAX_COPY {
                 // Max literals reached, start a new literal run
                 copy = 0;
                 output[op] = (MAX_COPY - 1) as u8; op += 1;
             }
             continue;
        }

        // Check if the first 4 bytes actually match
        let ref_seq = u32::from_le_bytes([input[ref_pos], input[ref_pos+1], 
                                          input[ref_pos+2], input[ref_pos+3]]);
        if ref_seq != seq {
             // No match, output as literal
             if op + 2 > maxout { return 0; }
             output[op] = input[anchor]; op += 1;
             ip = anchor + 1;
             copy += 1;
             if copy == MAX_COPY {
                 copy = 0;
                 output[op] = (MAX_COPY - 1) as u8; op += 1;
             }
             continue;
        }

        // We have a match! Now find how long it extends
        ip = anchor + 4;
        let mut ref_ptr = ref_pos + 4;
        
        // Find additional matching bytes beyond the first 4
        // This is like get_run_or_match in C, but simplified
        while ip < ip_bound && input[ip] == input[ref_ptr] {
            ip += 1;
            ref_ptr += 1;
        }
        
        // Apply ipshift: shift back to account for compression heuristics
        if ip >= ipshift {
            ip -= ipshift;
        }

        // Total match length
        let len = ip - anchor;

        // Short matches are expensive to encode, especially far matches
        // Only encode if the match is worth it
        if len < minlen || (len <= 5 && distance >= MAX_DISTANCE) {
             // Not worth encoding, backtrack and treat as literal
             if op + 2 > maxout { return 0; }
             output[op] = input[anchor]; op += 1;
             ip = anchor + 1;
             copy += 1;

             if copy == MAX_COPY {
                 copy = 0;
                 output[op] = (MAX_COPY - 1) as u8; op += 1;
             }
             continue;
        }

        // If we have copied literals, finalize the literal count
        if copy > 0 {
            // Update the control byte for the literal run (biased by -1)
            output[op - copy - 1] = (copy - 1) as u8;
        } else {
            // No literals, back up to overwrite the control byte
            op -= 1;
        }
        copy = 0;


        // Distance is biased: subtract 1 for encoding
        let distance = distance - 1;

        // Encode the match based on distance and length
        if distance < MAX_DISTANCE {
            // Near match (distance fits in 13 bits)
            if len < 7 {
                // Short match: len (3 bits) | distance_high (5 bits), distance_low (8 bits)
                if op + 2 > maxout { return 0; }
                output[op] = ((len as u8) << 5) + ((distance >> 8) as u8); op += 1;
                output[op] = (distance & 255) as u8; op += 1;
            } else {
                // Long match: 7 << 5 | distance_high, extra_len_bytes..., distance_low
                if op + 1 > maxout { return 0; }
                output[op] = (7 << 5) + ((distance >> 8) as u8); op += 1;
                let mut l = len - 7;
                while l >= 255 {
                    if op + 1 > maxout { return 0; }
                    output[op] = 255; op += 1;
                    l -= 255;
                }
                if op + 2 > maxout { return 0; }
                output[op] = l as u8; op += 1;
                output[op] = (distance & 255) as u8; op += 1;
            }
        } else {
            // Far match (distance needs 16 bits)
            let dist_far = distance - MAX_DISTANCE;
            if len < 7 {
                // Short far match: len << 5 | 31, 255, distance_high, distance_low
                if op + 4 > maxout { return 0; }
                output[op] = ((len as u8) << 5) + 31; op += 1;
                output[op] = 255; op += 1;
                output[op] = (dist_far >> 8) as u8; op += 1;
                output[op] = (dist_far & 255) as u8; op += 1;
            } else {
                // Long far match: 7 << 5 | 31, extra_len_bytes..., 255, distance_high, distance_low
                if op + 1 > maxout { return 0; }
                output[op] = (7 << 5) + 31; op += 1;
                let mut l = len - 7;
                while l >= 255 {
                    if op + 1 > maxout { return 0; }
                    output[op] = 255; op += 1;
                    l -= 255;
                }
                if op + 4 > maxout { return 0; }
                output[op] = l as u8; op += 1;
                output[op] = 255; op += 1;
                output[op] = (dist_far >> 8) as u8; op += 1;
                output[op] = (dist_far & 255) as u8; op += 1;
            }
        }
        
        // Update hash at match boundary
        if ip < length - 3 {
            let seq = u32::from_le_bytes([input[ip], input[ip+1], input[ip+2], input[ip+3]]);
            let hval = hash_function(seq, hashlog);
            htab[hval] = ip as u32;
            ip += 1;
            
            // For highest compression level, add a second hash
            if clevel == 9 && ip < length - 3 {
                let seq = u32::from_le_bytes([input[ip], input[ip+1], input[ip+2], input[ip+3]]);
                let hval = hash_function(seq, hashlog);
                htab[hval] = ip as u32;
                ip += 1;
            } else {
                ip += 1;
            }
        } else {
            ip += 2;
        }
        
        // Prepare for the next literal run
        if op + 1 > maxout { return 0; }
        output[op] = (MAX_COPY - 1) as u8; op += 1;
    }

    // Handle remaining bytes as literals
    while ip <= ip_bound {
        if op + 2 > maxout { return 0; }
        output[op] = input[ip]; op += 1; ip += 1;
        copy += 1;
        if copy == MAX_COPY {
            copy = 0;
            output[op] = (MAX_COPY - 1) as u8; op += 1;
        }
    }

    // Finalize the last literal run
    if copy > 0 {
        output[op - copy - 1] = (copy - 1) as u8;
    } else {
        op -= 1;
    }

    // Set the marker bit (bit 5 of first byte) to indicate BloscLZ format
    if op > 0 {
        output[0] |= 1 << 5;
    }



    op
}

/// Decompress data compressed with the BloscLZ algorithm.
///
/// This function reverses the compression process by:
/// - Reading control bytes to determine if next segment is literal or match
/// - For literals: directly copy bytes from input to output
/// - For matches: copy bytes from earlier in the output buffer (back-reference)
///
/// # Arguments
/// * `input` - Compressed data
/// * `output` - Output buffer for decompressed data
///
/// # Returns
/// Number of bytes written to output buffer, or 0 if decompression failed
///
/// # Format
/// The control byte determines the type of the next segment:
/// - If ctrl < 32: Literal run of (ctrl + 1) bytes
/// - If ctrl >= 32: Match with length and distance encoded in following bytes
///   - Length: (ctrl >> 5) - 1, with possible extension bytes
///   - Distance: encoded in 13 or 16 bits depending on the format
pub fn decompress(input: &[u8], output: &mut [u8]) -> usize {
    let length = input.len();
    let maxout = output.len();
    
    if length == 0 { 
        return 0; 
    }

    let mut ip = 0;
    let mut op = 0;
    
    // Read first control byte (bits 0-4 contain control, bit 5 is format marker)
    let mut ctrl = (input[ip] & 31) as usize; 
    ip += 1;

    loop {
        if ctrl >= 32 {
            // This is a match (back-reference)
            
            // Extract initial length from control byte (3 bits)
            // Length encoding in C: stored value = actual_length - 2
            // So: actual_length = stored_value + 2 = (ctrl >> 5) + 2
            // We compute as: len = (ctrl >> 5) - 1, then add 3 later
            let mut len = (ctrl >> 5) - 1;
            
            // Extract initial offset (high 5 bits of distance)
            let mut ofs = (ctrl & 31) << 8;
            
            // Check if we need to read length extension bytes
            // This happens when (ctrl >> 5) == 7, making len == 6 at this point
            let code: usize;
            if len == 7 - 1 {
                // Length >= 9 bytes. Read extension bytes (each 255 adds 255).
                loop {
                    if ip >= length { 
                        return 0; 
                    }
                    let ext = input[ip] as usize; 
                    ip += 1;
                    len += ext;
                    if ext != 255 {
                        break;
                    }
                }
            }
            
            // Always read the distance low byte (after any extension bytes)
            if ip >= length { 
                return 0; 
            }
            code = input[ip] as usize; 
            ip += 1;

            // Add 3 to get actual match length (minimum match is 3 bytes)
            len += 3;

            // Check for far distance encoding (16-bit distance)
            if code == 255 && ofs == (31 << 8) {
                // Far distance: read 2 more bytes for 16-bit distance
                if ip + 1 >= length { 
                    return 0; 
                }
                ofs = (input[ip] as usize) << 8; 
                ip += 1;
                ofs += input[ip] as usize; 
                ip += 1;
                ofs += MAX_DISTANCE;
            } else {
                // Normal distance: combine high bits from ctrl with low byte
                ofs += code; 
            }

            // Add 1 to distance (distances are biased by 1)
            ofs += 1;

            // Bounds check for output
            if op + len > maxout { 
                return 0; 
            }
            
            // Calculate source position for match
            if ofs > op {
                // Invalid: trying to reference before start of output
                return 0;
            }
            
            let start = op - ofs;

            // Copy match bytes
            // Note: We can't use copy_from_within or copy_from_slice when source and 
            // destination overlap in the forward direction, as we may be repeating a 
            // pattern (e.g., "ab" with distance 2 and length 8 becomes "abababab")
            for i in 0..len {
                output[op + i] = output[start + i];
            }
            op += len;

            // Check if we're done
            if ip >= length { 
                break; 
            }
            
            // Read next control byte
            ctrl = input[ip] as usize; 
            ip += 1;
        } else {
            // This is a literal run
            
            // ctrl + 1 is the number of literal bytes to copy
            ctrl += 1;
            
            // Bounds checks
            if op + ctrl > maxout { 
                return 0; 
            }
            if ip + ctrl > length { 
                return 0; 
            }
            
            // Copy literal bytes directly
            output[op..op+ctrl].copy_from_slice(&input[ip..ip+ctrl]);
            op += ctrl;
            ip += ctrl;

            // Check if we're done
            if ip >= length { 
                break; 
            }
            
            // Read next control byte
            ctrl = input[ip] as usize; 
            ip += 1;
        }
    }

    op
}
