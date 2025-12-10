use std::cmp;
use crate::internal::constants::*;

const MAX_COPY: usize = 32;
const MAX_DISTANCE: usize = 8191;
const MAX_FARDISTANCE: usize = 65535 + MAX_DISTANCE - 1;
const HASH_LOG: usize = 14;

pub fn decompress(input: &[u8], output: &mut [u8]) -> usize {
    if input.is_empty() {
        println!("blosclz decompress: input is empty");
        return 0;
    }
    let mut ip = 0;
    let mut op = 0;
    let ip_limit = input.len();
    let op_limit = output.len();
    
    println!("blosclz decompress: input.len()={}, output.len()={}", ip_limit, op_limit);
    
    let mut ctrl = (input[ip] & 31) as u32;
    ip += 1;
    
    println!("Initial ctrl = {}", ctrl);
    
    loop {
        println!("Loop: ip={}, op={}, ctrl={}", ip, op, ctrl);
        if ctrl >= 32 {
            println!("  Match branch");
            let mut len = (ctrl >> 5) as i32 - 1;
            let mut ofs = (ctrl & 31) as i32 * 256;
            
            if len == 6 {
                loop {
                    if ip >= ip_limit { 
                        println!("  Error: ip >= ip_limit in len==6 loop");
                        return 0; 
                    }
                    let code = input[ip];
                    ip += 1;
                    len += code as i32;
                    if code != 255 { break; }
                }
            }
            
            if ip >= ip_limit { 
                println!("  Error: ip >= ip_limit before reading code");
                return 0; 
            }
            let code = input[ip];
            ip += 1;
            len += 3;
            
            let mut distance = ofs + code as i32;
            
            if code == 255 {
                if ofs == 31 * 256 {
                    if ip + 1 >= ip_limit { 
                        println!("  Error: ip + 1 >= ip_limit in code==255");
                        return 0; 
                    }
                    let ofs_new = (input[ip] as i32) << 8 | (input[ip+1] as i32);
                    ip += 2;
                    distance = ofs_new + MAX_DISTANCE as i32;
                }
            }
            
            distance += 1;
            
            if op + len as usize > op_limit { 
                println!("  Error: op + len > op_limit: {} + {} > {}", op, len, op_limit);
                return 0; 
            }
            if (op as i32) < distance { 
                println!("  Error: op < distance: {} < {}", op, distance);
                return 0; 
            }
            
            let ref_pos = op - distance as usize;
            
            if ip >= ip_limit { println!("Break at ip >= ip_limit after match"); break; }
            ctrl = input[ip] as u32;
            ip += 1;
            
            if distance == 1 {
                let val = output[ref_pos];
                for i in 0..len as usize {
                    output[op + i] = val;
                }
            } else {
                if distance as usize >= len as usize {
                    output.copy_within(ref_pos..ref_pos + len as usize, op);
                } else {
                    for i in 0..len as usize {
                        output[op + i] = output[ref_pos + i];
                    }
                }
            }
            op += len as usize;
        } else {
            println!("  Literal branch");
            ctrl += 1;
            if op + ctrl as usize > op_limit { 
                println!("  Error: op + ctrl > op_limit: {} + {} > {}", op, ctrl, op_limit);
                return 0; 
            }
            if ip + ctrl as usize > ip_limit { 
                println!("  Error: ip + ctrl > ip_limit: {} + {} > {}", ip, ctrl, ip_limit);
                return 0; 
            }
            
            output[op..op + ctrl as usize].copy_from_slice(&input[ip..ip + ctrl as usize]);
            op += ctrl as usize;
            ip += ctrl as usize;
            
            if ip >= ip_limit { println!("Break at ip >= ip_limit after literal"); break; }
            ctrl = input[ip] as u32;
            ip += 1;
        }
    }
    
    op
}

pub fn compress(clevel: i32, input: &[u8], output: &mut [u8]) -> usize {
    let length = input.len();
    let maxout = output.len();
    if length < 16 || maxout < 66 { return 0; }

    let mut ip = 0;
    let ip_bound = length - 1;
    let ip_limit = length - 12;
    let mut op = 0;
    
    let hashlog = match clevel {
        0 => 0,
        1 => HASH_LOG - 2,
        2 => HASH_LOG - 1,
        _ => HASH_LOG,
    };
    let hash_size = 1 << hashlog;
    let mut htab = vec![0usize; hash_size];
    
    output[op] = (MAX_COPY - 1) as u8; op += 1;
    output[op] = input[ip]; op += 1; ip += 1;
    output[op] = input[ip]; op += 1; ip += 1;
    output[op] = input[ip]; op += 1; ip += 1;
    output[op] = input[ip]; op += 1; ip += 1;
    
    let mut copy = 4;
    
    while ip < ip_limit {
        let seq = u32::from_le_bytes(input[ip..ip+4].try_into().unwrap());
        let shift = 32 - hashlog;
        let hval = if shift >= 32 { 0 } else { (seq.wrapping_mul(2654435761) >> shift) as usize };
        
        let ref_pos = htab[hval];
        htab[hval] = ip;
        
        let distance = ip - ref_pos;
        
        let mut match_found = false;
        if distance > 0 && distance < MAX_FARDISTANCE {
            if ref_pos < length && input[ref_pos..].len() >= 4 && input[ip..].len() >= 4 {
                 if input[ref_pos..ref_pos+4] == input[ip..ip+4] {
                     match_found = true;
                 }
            }
        }
        
        if !match_found {
            output[op] = input[ip]; op += 1; ip += 1;
            copy += 1;
            if copy == MAX_COPY {
                copy = 0;
                output[op] = (MAX_COPY - 1) as u8; op += 1;
            }
            continue;
        }
        
        // Match found
        let mut len = 4;
        let mut ref_ptr = ref_pos + 4;
        let _anchor = ip;
        ip += 4;
        
        while ip < ip_bound && ref_ptr < length && input[ip] == input[ref_ptr] {
            ip += 1;
            ref_ptr += 1;
            len += 1;
        }
        
        // Reset copy count
        if copy > 0 {
            output[op - copy - 1] = (copy - 1) as u8;
        } else {
            op -= 1;
        }
        copy = 0;
        
        // Encode match
        let dist = distance - 1; // biased
        if distance < MAX_DISTANCE {
            if len < 7 {
                output[op] = ((len as u8) << 5) + ((dist >> 8) as u8); op += 1;
                output[op] = (dist & 255) as u8; op += 1;
            } else {
                output[op] = (7 << 5) + ((dist >> 8) as u8); op += 1;
                let mut l = len - 7;
                while l >= 255 {
                    output[op] = 255; op += 1;
                    l -= 255;
                }
                output[op] = l as u8; op += 1;
                output[op] = (dist & 255) as u8; op += 1;
            }
        } else {
            let dist_far = distance - MAX_DISTANCE;
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
        
        // Update hash
        if ip + 4 <= length {
            let seq = u32::from_le_bytes(input[ip..ip+4].try_into().unwrap());
            let hval = (seq.wrapping_mul(2654435761) >> (32 - hashlog)) as usize;
            htab[hval] = ip;
        }
        ip += 1;
        
        output[op] = (MAX_COPY - 1) as u8; op += 1;
    }
    
    while ip <= ip_bound {
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
    
    output[0] |= 1 << 5;
    
    op
}
