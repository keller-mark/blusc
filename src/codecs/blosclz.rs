use std::cmp;

const MAX_COPY: usize = 32;
const MAX_DISTANCE: usize = 8191;
const MAX_FARDISTANCE: usize = 65535 + MAX_DISTANCE - 1;
const HASH_LOG: usize = 14;
const HASH_SIZE: usize = 1 << HASH_LOG;

fn hash_function(v: u32, h: usize) -> usize {
    ((v.wrapping_mul(2654435761)) >> (32 - h)) as usize
}

fn get_match(ip: &[u8], ref_ptr: &[u8]) -> usize {
    let mut count = 0;
    let len = cmp::min(ip.len(), ref_ptr.len());
    while count < len && ip[count] == ref_ptr[count] {
        count += 1;
    }
    count
}

pub fn compress(_clevel: i32, input: &[u8], output: &mut [u8]) -> usize {
    let length = input.len();
    let maxout = output.len();
    if length < 16 || maxout < 66 {
        return 0;
    }

    let mut htab = [0usize; HASH_SIZE];
    let mut ip = 0;
    let mut op = 0;
    let ip_limit = length - 12;

    // We start with literal copy
    let mut copy = 4;
    output[op] = (MAX_COPY - 1) as u8; op += 1;
    output[op] = input[ip]; op += 1; ip += 1;
    output[op] = input[ip]; op += 1; ip += 1;
    output[op] = input[ip]; op += 1; ip += 1;
    output[op] = input[ip]; op += 1; ip += 1;

    while ip < ip_limit {
        let seq = u32::from_le_bytes([input[ip], input[ip+1], input[ip+2], input[ip+3]]);
        let hval = hash_function(seq, HASH_LOG);
        let ref_pos = htab[hval];
        htab[hval] = ip;

        let distance = ip.wrapping_sub(ref_pos); 
        
        // Check if distance is valid and match is good
        if distance == 0 || distance >= MAX_FARDISTANCE || ref_pos >= ip {
             // Literal
             output[op] = input[ip]; op += 1; ip += 1;
             copy += 1;
             if copy == MAX_COPY {
                 copy = 0;
                 output[op] = (MAX_COPY - 1) as u8; op += 1;
             }
             continue;
        }

        // Check first 4 bytes
        if u32::from_le_bytes([input[ref_pos], input[ref_pos+1], input[ref_pos+2], input[ref_pos+3]]) != seq {
             // Literal
             output[op] = input[ip]; op += 1; ip += 1;
             copy += 1;
             if copy == MAX_COPY {
                 copy = 0;
                 output[op] = (MAX_COPY - 1) as u8; op += 1;
             }
             continue;
        }

        // Match found
        // Last matched byte
        let anchor = ip;
        ip += 4;
        let ref_ptr = ref_pos + 4;
        
        // Distance is biased
        let distance_biased = distance;

        // Get run or match
        let len_inc = get_match(&input[ip..], &input[ref_ptr..]);
        ip += len_inc;

        let len = ip - anchor;

        // Encoding short lengths is expensive
        if len < 4 || (len <= 5 && distance_biased >= MAX_DISTANCE) {
             // Backtrack and treat as literal
             ip = anchor;
             output[op] = input[ip]; op += 1; ip += 1;
             copy += 1;
             if copy == MAX_COPY {
                 copy = 0;
                 output[op] = (MAX_COPY - 1) as u8; op += 1;
             }
             continue;
        }

        // If we have copied something, adjust the copy count
        if copy > 0 {
            output[op - copy - 1] = (copy - 1) as u8;
        } else {
            op -= 1;
        }
        copy = 0;

        // Encode the match
        if distance_biased < MAX_DISTANCE {
            if len < 7 {
                output[op] = ((len as u8) << 5) + ((distance_biased >> 8) as u8); op += 1;
                output[op] = (distance_biased & 255) as u8; op += 1;
            } else {
                output[op] = (7 << 5) + ((distance_biased >> 8) as u8); op += 1;
                let mut l = len - 7;
                while l >= 255 {
                    output[op] = 255; op += 1;
                    l -= 255;
                }
                output[op] = l as u8; op += 1;
                output[op] = (distance_biased & 255) as u8; op += 1;
            }
        } else {
            let dist_far = distance_biased - MAX_DISTANCE;
            if len < 7 {
                output[op] = ((len as u8) << 5) + 31; op += 1;
                output[op] = 255; op += 1;
                output[op] = (dist_far >> 8) as u8; op += 1;
                output[op] = (dist_far & 255) as u8; op += 1;
            } else {
                output[op] = (7 << 5) + 31; op += 1;
                let mut l = len - 7;
                while l >= 255 {
                    output[op] = 255; op += 1;
                    l -= 255;
                }
                output[op] = l as u8; op += 1;
                output[op] = 255; op += 1;
                output[op] = (dist_far >> 8) as u8; op += 1;
                output[op] = (dist_far & 255) as u8; op += 1;
            }
        }
        
        // Assuming literal copy next
        if op < maxout {
            output[op] = (MAX_COPY - 1) as u8; op += 1;
        } else {
            return 0;
        }
    }

    // Left-over as literal copy
    while ip < length {
        if op + 2 > maxout { return 0; }
        output[op] = input[ip]; op += 1; ip += 1;
        copy += 1;
        if copy == MAX_COPY {
            copy = 0;
            output[op] = (MAX_COPY - 1) as u8; op += 1;
        }
    }

    if copy > 0 {
        output[op - copy - 1] = (copy - 1) as u8;
    } else {
        op -= 1;
    }

    // Marker for blosclz
    if op > 0 {
        output[0] |= 1 << 5;
    }

    op
}

pub fn decompress(input: &[u8], output: &mut [u8]) -> usize {
    let length = input.len();
    let maxout = output.len();
    if length == 0 { return 0; }

    let mut ip = 0;
    let mut op = 0;
    let mut ctrl = (input[ip] & 31) as usize; ip += 1;

    loop {
        if ctrl >= 32 {
            // Match
            let mut len = ctrl >> 5;
            let mut ofs = (ctrl & 31) << 8;
            
            if ip >= length { println!("Fail 1"); return 0; }
            let mut code = input[ip] as usize; ip += 1;
            
            if len == 7 {
                while code == 255 {
                    len += code;
                    if ip >= length { println!("Fail 2"); return 0; }
                    code = input[ip] as usize; ip += 1;
                }
                len += code;
                if ip >= length { println!("Fail 3"); return 0; }
                code = input[ip] as usize; ip += 1;
            }

            if code == 255 && ofs == (31 << 8) {
                if ip + 1 >= length { println!("Fail 4"); return 0; }
                ofs = (input[ip] as usize) << 8; ip += 1;
                ofs += input[ip] as usize; ip += 1;
                ofs += MAX_DISTANCE;
            } else {
                ofs += code; 
            }

            if op + len > maxout { println!("Fail 5: op={} len={} maxout={}", op, len, maxout); return 0; }
            
            // Copy match
            // Rust doesn't allow reading from output while writing to it easily with slices.
            // We need to copy byte by byte or use `copy_within` if available (it is in recent Rust).
            // But `copy_within` might not handle overlapping ranges correctly if we are just extending?
            // Actually `copy_within` handles overlap.
            // But here the source is `op - ofs` and dest is `op`.
            // If `ofs` is small (e.g. 1), we are repeating the last byte.
            // `copy_within` panics if src and dest overlap in a way that is not supported?
            // Documentation says: "The two regions may overlap."
            
            // However, we can't use `copy_within` on `output` because we are borrowing it as mutable.
            // We can use a loop.
            let start = op - ofs;
            if start >= op { println!("Fail 6"); return 0; } // Should not happen if ofs > 0
            
            for i in 0..len {
                output[op + i] = output[start + i];
            }
            op += len;

            if ip >= length { break; }
            ctrl = input[ip] as usize; ip += 1;
        } else {
            // Literal
            ctrl += 1;
            if op + ctrl > maxout { println!("Fail 7"); return 0; }
            if ip + ctrl > length { println!("Fail 8"); return 0; }
            
            output[op..op+ctrl].copy_from_slice(&input[ip..ip+ctrl]);
            op += ctrl;
            ip += ctrl;

            if ip >= length { break; }
            ctrl = input[ip] as usize; ip += 1;
        }
    }

    op
}
