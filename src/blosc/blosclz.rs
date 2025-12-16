// Corresponds to c-blosc2/blosc/blosclz.c (and .h)

use crate::blosc::context::Blosc2Context;
// use crate::blosc::fastcopy::fastcopy; // Not used directly, we use copy_match or slice copy
// use crate::blosc::fastcopy::copy_match; // Implemented locally to handle overlap

const MAX_COPY: usize = 32;
const MAX_DISTANCE: usize = 8191;
const MAX_FARDISTANCE: usize = 65535 + MAX_DISTANCE - 1;
const HASH_LOG: usize = 14;

// Helper functions

#[inline]
fn get_run(input: &[u8], mut ip: usize, ip_bound: usize, mut ref_pos: usize) -> usize {
    let x = input[ip - 1];
    while ip < (ip_bound - 8) {
        let val_ref = u64::from_ne_bytes(input[ref_pos..ref_pos+8].try_into().unwrap());
        let val_x = u64::from_ne_bytes([x; 8]);
        
        if val_x != val_ref {
            while input[ref_pos] == x {
                ref_pos += 1;
                ip += 1;
            }
            return ip;
        } else {
            ip += 8;
            ref_pos += 8;
        }
    }
    while ip < ip_bound && input[ref_pos] == x {
        ref_pos += 1;
        ip += 1;
    }
    ip
}

#[inline]
fn get_match(input: &[u8], mut ip: usize, ip_bound: usize, mut ref_pos: usize) -> usize {
    while ip < (ip_bound - 8) {
        let val_ip = u64::from_ne_bytes(input[ip..ip+8].try_into().unwrap());
        let val_ref = u64::from_ne_bytes(input[ref_pos..ref_pos+8].try_into().unwrap());
        
        if val_ref != val_ip {
            while input[ref_pos] == input[ip] {
                ref_pos += 1;
                ip += 1;
            }
            return ip;
        } else {
            ip += 8;
            ref_pos += 8;
        }
    }
    while ip < ip_bound && input[ref_pos] == input[ip] {
        ref_pos += 1;
        ip += 1;
    }
    ip
}

#[inline]
fn get_run_or_match(input: &[u8], ip: usize, ip_bound: usize, ref_pos: usize, run: bool) -> usize {
    if run {
        get_run(input, ip, ip_bound, ref_pos)
    } else {
        get_match(input, ip, ip_bound, ref_pos)
    }
}

#[inline]
fn hash_function(v: u32, hashlog: u8) -> usize {
    ((v.wrapping_mul(2654435761)) >> (32 - hashlog)) as usize
}

// Get a guess for the compressed size of a buffer
fn get_cratio(
    input: &[u8],
    ibase: usize,
    maxlen: usize,
    minlen: usize,
    ipshift: usize,
    htab: &mut [u32],
    hashlog: u8,
) -> f64 {
    let mut ip = ibase;
    let mut oc = 0;
    let hashlen = 1 << hashlog;
    let mut copy = 4;
    oc += 5;
    
    let limit = if maxlen > hashlen { hashlen } else { maxlen };
    let ip_bound = ibase + limit - 1;
    let ip_limit = ibase + limit - 12;
    
    // Initialize hash table
    htab.fill(0);
    
    while ip < ip_limit {
        let mut anchor = ip;
        let seq = u32::from_ne_bytes(input[ip..ip+4].try_into().unwrap());
        let hval = hash_function(seq, hashlog);
        let mut ref_pos = ibase + htab[hval] as usize;
        
        let distance = anchor - ref_pos;
        htab[hval] = (anchor - ibase) as u32;
        
        if distance == 0 || distance >= MAX_FARDISTANCE {
            // LITERAL2
            oc += 1;
            anchor += 1;
            ip = anchor;
            copy += 1;
            if copy == MAX_COPY {
                copy = 0;
                oc += 1;
            }
            continue;
        }
        
        if u32::from_ne_bytes(input[ref_pos..ref_pos+4].try_into().unwrap()) == u32::from_ne_bytes(input[ip..ip+4].try_into().unwrap()) {
            ref_pos += 4;
        } else {
            // LITERAL2
            oc += 1;
            anchor += 1;
            ip = anchor;
            copy += 1;
            if copy == MAX_COPY {
                copy = 0;
                oc += 1;
            }
            continue;
        }
        
        ip = anchor + 4;
        let distance_biased = distance - 1;
        
        ip = get_run_or_match(input, ip, ip_bound, ref_pos, distance_biased == 0);
        
        ip -= ipshift;
        let len = ip - anchor;
        
        if len < minlen {
            // LITERAL2
            oc += 1;
            anchor += 1;
            ip = anchor;
            copy += 1;
            if copy == MAX_COPY {
                copy = 0;
                oc += 1;
            }
            continue;
        }
        
        if copy == 0 {
            oc -= 1;
        }
        copy = 0;
        
        if distance < MAX_DISTANCE {
            if len >= 7 {
                oc += ((len - 7) / 255) + 1;
            }
            oc += 2;
        } else {
            if len >= 7 {
                oc += ((len - 7) / 255) + 1;
            }
            oc += 4;
        }
        
        let seq = u32::from_ne_bytes(input[ip..ip+4].try_into().unwrap());
        let hval = hash_function(seq, hashlog);
        htab[hval] = (ip - ibase) as u32;
        ip += 1;
        oc += 1;
    }
    
    let ic = (ip - ibase) as f64;
    ic / (oc as f64)
}

pub fn blosclz_compress(
    clevel: i32,
    input: &[u8],
    output: &mut [u8],
    maxout: usize,
    _ctx: &Blosc2Context,
) -> i32 {
    let length = input.len();
    let ibase = 0;
    let mut htab = vec![0u32; 1 << HASH_LOG];
    
    let ipshift = 4;
    let minlen = 4;
    
    let hashlog_arr = [0, HASH_LOG - 2, HASH_LOG - 1, HASH_LOG, HASH_LOG,
                          HASH_LOG, HASH_LOG, HASH_LOG, HASH_LOG, HASH_LOG];
    let hashlog = hashlog_arr[clevel as usize] as u8;
    
    let mut maxlen = length;
    if clevel < 2 {
        maxlen /= 8;
    } else if clevel < 4 {
        maxlen /= 4;
    } else if clevel < 7 {
        maxlen /= 2;
    }
    
    let shift = length - maxlen;
    let cratio = get_cratio(input, ibase + shift, maxlen, minlen, ipshift, &mut htab, hashlog);
    
    let cratio_arr = [0.0, 2.0, 1.5, 1.2, 1.2, 1.2, 1.2, 1.15, 1.1, 1.0];
    if cratio < cratio_arr[clevel as usize] {
        return 0;
    }
    
    let mut ip = ibase;
    let ip_bound = ibase + length - 1;
    let ip_limit = ibase + length - 12;
    let mut op = 0;
    let op_limit = maxout;
    
    if length < 16 || maxout < 66 {
        return 0;
    }
    
    htab.fill(0);
    
    let mut copy = 4;
    output[op] = (MAX_COPY - 1) as u8; op += 1;
    output[op] = input[ip]; op += 1; ip += 1;
    output[op] = input[ip]; op += 1; ip += 1;
    output[op] = input[ip]; op += 1; ip += 1;
    output[op] = input[ip]; op += 1; ip += 1;
    
    while ip < ip_limit {
        let mut anchor = ip;
        let seq = u32::from_ne_bytes(input[ip..ip+4].try_into().unwrap());
        let hval = hash_function(seq, hashlog);
        let mut ref_pos = ibase + htab[hval] as usize;
        
        let mut distance = anchor - ref_pos;
        htab[hval] = (anchor - ibase) as u32;
        
        if distance == 0 || distance >= MAX_FARDISTANCE {
            // LITERAL
            if op + 2 > op_limit { return 0; }
            output[op] = input[anchor]; op += 1; anchor += 1;
            ip = anchor;
            copy += 1;
            if copy == MAX_COPY {
                copy = 0;
                output[op] = (MAX_COPY - 1) as u8; op += 1;
            }
            continue;
        }
        
        if u32::from_ne_bytes(input[ref_pos..ref_pos+4].try_into().unwrap()) == u32::from_ne_bytes(input[ip..ip+4].try_into().unwrap()) {
            ref_pos += 4;
        } else {
            // LITERAL
            if op + 2 > op_limit { return 0; }
            output[op] = input[anchor]; op += 1; anchor += 1;
            ip = anchor;
            copy += 1;
            if copy == MAX_COPY {
                copy = 0;
                output[op] = (MAX_COPY - 1) as u8; op += 1;
            }
            continue;
        }
        
        ip = anchor + 4;
        distance -= 1;
        
        ip = get_run_or_match(input, ip, ip_bound, ref_pos, distance == 0);
        
        ip -= ipshift;
        let len = ip - anchor;
        
        if len < minlen || (len <= 5 && distance >= MAX_DISTANCE) {
            // LITERAL
            if op + 2 > op_limit { return 0; }
            output[op] = input[anchor]; op += 1; anchor += 1;
            ip = anchor;
            copy += 1;
            if copy == MAX_COPY {
                copy = 0;
                output[op] = (MAX_COPY - 1) as u8; op += 1;
            }
            continue;
        }
        
        if copy != 0 {
            output[op - copy - 1] = (copy - 1) as u8;
        } else {
            op -= 1;
        }
        copy = 0;
        
        if distance < MAX_DISTANCE {
            if len < 7 {
                // MATCH_SHORT
                if op + 2 > op_limit { return 0; }
                output[op] = ((len << 5) + (distance >> 8)) as u8; op += 1;
                output[op] = (distance & 255) as u8; op += 1;
            } else {
                // MATCH_LONG
                if op + 1 > op_limit { return 0; }
                output[op] = ((7 << 5) + (distance >> 8)) as u8; op += 1;
                let mut l = len - 7;
                while l >= 255 {
                    if op + 1 > op_limit { return 0; }
                    output[op] = 255; op += 1;
                    l -= 255;
                }
                if op + 2 > op_limit { return 0; }
                output[op] = l as u8; op += 1;
                output[op] = (distance & 255) as u8; op += 1;
            }
        } else {
            distance -= MAX_DISTANCE;
            if len < 7 {
                // MATCH_SHORT_FAR
                if op + 4 > op_limit { return 0; }
                output[op] = ((len << 5) + 31) as u8; op += 1;
                output[op] = 255; op += 1;
                output[op] = (distance >> 8) as u8; op += 1;
                output[op] = (distance & 255) as u8; op += 1;
            } else {
                // MATCH_LONG_FAR
                if op + 1 > op_limit { return 0; }
                output[op] = ((7 << 5) + 31) as u8; op += 1;
                let mut l = len - 7;
                while l >= 255 {
                    if op + 1 > op_limit { return 0; }
                    output[op] = 255; op += 1;
                    l -= 255;
                }
                if op + 4 > op_limit { return 0; }
                output[op] = l as u8; op += 1;
                output[op] = 255; op += 1;
                output[op] = (distance >> 8) as u8; op += 1;
                output[op] = (distance & 255) as u8; op += 1;
            }
        }
        
        let seq = u32::from_ne_bytes(input[ip..ip+4].try_into().unwrap());
        let hval = hash_function(seq, hashlog);
        htab[hval] = (ip - ibase) as u32;
        
        if clevel == 9 {
            let seq2 = seq >> 8;
            let hval2 = hash_function(seq2, hashlog);
            htab[hval2] = (ip + 1 - ibase) as u32;
            ip += 1;
        } else {
            ip += 1;
        }
        
        if op + 1 > op_limit { return 0; }
        output[op] = (MAX_COPY - 1) as u8; op += 1;
    }
    
    while ip <= ip_bound {
        if op + 2 > op_limit { return 0; }
        output[op] = input[ip]; op += 1; ip += 1;
        copy += 1;
        if copy == MAX_COPY {
            copy = 0;
            output[op] = (MAX_COPY - 1) as u8; op += 1;
        }
    }
    
    if copy != 0 {
        output[op - copy - 1] = (copy - 1) as u8;
    } else {
        op -= 1;
    }
    
    output[0] |= 1 << 5;
    
    op as i32
}

// LZ4 wildCopy which can reach excellent copy bandwidth (even if insecure)
#[inline]
fn wild_copy(output: &mut [u8], mut op: usize, mut ref_pos: usize, end: usize) {
    while op < end {
        if op >= ref_pos + 8 {
             output.copy_within(ref_pos..ref_pos+8, op);
        } else {
             for i in 0..8 {
                 output[op + i] = output[ref_pos + i];
             }
        }
        op += 8;
        ref_pos += 8;
    }
}

#[inline]
fn copy_match(output: &mut [u8], mut op: usize, mut ref_pos: usize, len: usize) -> usize {
    for _ in 0..len {
        output[op] = output[ref_pos];
        op += 1;
        ref_pos += 1;
    }
    op
}

pub fn blosclz_decompress(
    input: &[u8],
    length: usize,
    output: &mut [u8],
    maxout: usize,
) -> i32 {
    let mut ip = 0;
    let ip_limit = length;
    let mut op = 0;
    let op_limit = maxout;
    
    if length == 0 { return 0; }
    
    let mut ctrl = (input[ip] & 31) as u32;
    ip += 1;
    
    loop {
        if ctrl >= 32 {
            let mut len = ((ctrl >> 5) - 1) as i32;
            let mut ofs = ((ctrl & 31) << 8) as i32;
            let mut code: u8;
            
            if len == 6 {
                loop {
                    if ip + 1 >= ip_limit { return 0; }
                    code = input[ip]; ip += 1;
                    len += code as i32;
                    if code != 255 { break; }
                }
            } else {
                if ip + 1 >= ip_limit { return 0; }
            }
            
            code = input[ip]; ip += 1;
            len += 3;
            let mut ref_pos = (op as i32) - ofs - (code as i32);
            
            if code == 255 {
                if ofs == (31 << 8) {
                    if ip + 1 >= ip_limit { return 0; }
                    ofs = (input[ip] as i32) << 8; ip += 1;
                    ofs += input[ip] as i32; ip += 1;
                    ref_pos = (op as i32) - ofs - MAX_DISTANCE as i32;
                }
            }
            
            if op + (len as usize) > op_limit { return 0; }
            if ref_pos < 0 { return 0; }
            
            if ip >= ip_limit { break; }
            ctrl = input[ip] as u32; ip += 1;
            
            let ref_ptr = ref_pos as usize;
            let len = len as usize;
            
            if ref_ptr == op - 1 {
                // optimized copy for a run
                let val = output[ref_ptr];
                output[op..op+len].fill(val);
                op += len;
            } else if (op - ref_ptr >= 8) && (op_limit - op >= len + 8) {
                wild_copy(output, op, ref_ptr, op + len);
                op += len;
            } else {
                op = copy_match(output, op, ref_ptr, len);
            }
        } else {
            ctrl += 1;
            let len = ctrl as usize;
            if op + len > op_limit { return 0; }
            if ip + len > ip_limit { return 0; }
            
            output[op..op+len].copy_from_slice(&input[ip..ip+len]);
            op += len;
            ip += len;
            
            if ip >= ip_limit { break; }
            ctrl = input[ip] as u32; ip += 1;
        }
    }
    
    op as i32
}
