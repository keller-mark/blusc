use crate::internal;
use crate::internal::constants::*;
use std::os::raw::c_void;


#[repr(C)]
pub struct Blosc2Context {
    pub cparams: Blosc2Cparams,
    pub dparams: Blosc2Dparams,
}

// Structs
// Note: these may need to be adjusted so that the implementation works correctly,
// but we want to use the proper Rust idioms when porting the structs.
#[repr(C)]
pub struct Blosc2Cparams {
    // Reference: context.h in the original blosc2 codebase
    pub compcode: u8,
    pub compcode_meta: u8,
    pub clevel: u8,
    pub use_dict: i32,
    pub typesize: i32,
    pub nthreads: i16,
    pub blocksize: i32,
    pub splitmode: i32,
    pub schunk: *mut c_void,
    pub filters: [u8; BLOSC2_MAX_FILTERS as usize],
    pub filters_meta: [u8; BLOSC2_MAX_FILTERS as usize],
    pub prefilter: *mut c_void,
    pub preparams: *mut c_void,
    pub tuner_params: *mut c_void,
    pub tuner_id: i32,
    pub instr_codec: bool,
    pub codec_params: *mut c_void,
    pub filter_params: [*mut c_void; BLOSC2_MAX_FILTERS as usize],
}

#[repr(C)]
pub struct Blosc2Dparams {
    pub nthreads: i16,
    pub schunk: *mut c_void,
    pub postfilter: *mut c_void,
    pub postparams: *mut c_void,
}

// Default constants
pub const BLOSC2_CPARAMS_DEFAULTS: Blosc2Cparams = Blosc2Cparams {
    compcode: BLOSC_BLOSCLZ,
    compcode_meta: 0,
    clevel: 5,
    use_dict: 0,
    typesize: 8,
    nthreads: 1,
    blocksize: 0,
    splitmode: BLOSC_FORWARD_COMPAT_SPLIT as i32,
    schunk: std::ptr::null_mut(),
    filters: [BLOSC_NOFILTER; BLOSC2_MAX_FILTERS as usize],
    filters_meta: [0; BLOSC2_MAX_FILTERS as usize],
    prefilter: std::ptr::null_mut(),
    preparams: std::ptr::null_mut(),
    tuner_params: std::ptr::null_mut(),
    tuner_id: 0,
    instr_codec: false,
    codec_params: std::ptr::null_mut(),
    filter_params: [std::ptr::null_mut(); BLOSC2_MAX_FILTERS as usize],
};

pub const BLOSC2_DPARAMS_DEFAULTS: Blosc2Dparams = Blosc2Dparams {
    nthreads: 1,
    schunk: std::ptr::null_mut(),
    postfilter: std::ptr::null_mut(),
    postparams: std::ptr::null_mut(),
};


// Function signatures
pub fn blosc1_cbuffer_metainfo(
    cbuffer: &[u8],
) -> Option<(usize, i32)> {
    if cbuffer.len() < 16 { return None; }
    let ts = cbuffer[3] as usize;
    let fl = cbuffer[2] as i32;
    
    Some((ts, fl))
}

pub fn blosc1_cbuffer_validate(
    cbuffer: &[u8],
    cbytes: usize,
) -> Result<usize, ()> {
    if cbuffer.len() < 16 { return Err(()); }
    
    let cb = u32::from_le_bytes([cbuffer[12], cbuffer[13], cbuffer[14], cbuffer[15]]) as usize;
    if cbytes != cb { return Err(()); }
    
    let nb = u32::from_le_bytes([cbuffer[4], cbuffer[5], cbuffer[6], cbuffer[7]]) as usize;
    Ok(nb)
}

pub fn blosc1_cbuffer_sizes(
    cbuffer: &[u8],
) -> (usize, usize, usize) {
    blosc2_cbuffer_sizes(cbuffer)
}

pub fn blosc1_getitem(
    cbuffer: &[u8],
    start: i32,
    nitems: i32,
    dest: &mut [u8],
) -> i32 {
    let cbytes = u32::from_le_bytes([cbuffer[12], cbuffer[13], cbuffer[14], cbuffer[15]]) as usize;
    
    let src_slice = if cbuffer.len() >= cbytes {
        &cbuffer[..cbytes]
    } else {
        cbuffer
    };
    
    match internal::getitem(src_slice, start as usize, nitems as usize, dest) {
        Ok(size) => size as i32,
        Err(_) => 0,
    }
}

pub fn blosc2_get_complib_info(
    compcode: &str,
) -> Option<(&'static str, &'static str, i32)> {
    // TODO: use constants
    match compcode {
        "blosclz" => Some(("BloscLZ", "2.5.1", 0)),
        "lz4" => Some(("LZ4", "1.9.4", 1)),
        "lz4hc" => Some(("LZ4", "1.9.4", 1)),
        "snappy" => Some(("Snappy", "1.1.9", 2)),
        "zlib" => Some(("Zlib", "1.2.11", 3)),
        "zstd" => Some(("Zstd", "1.5.2", 4)),
        _ => None,
    }
}

pub fn blosc2_compress(
    clevel: i32,
    doshuffle: i32,
    typesize: usize,
    src: &[u8],
    dest: &mut [u8],
) -> i32 {
    // Default compressor: BLOSCLZ (0)
    let compressor = BLOSC_BLOSCLZ;
    
    let mut filters = [0u8; 6];
    let filters_meta = [0u8; 6];
    
    if doshuffle == BLOSC_SHUFFLE as i32 {
        filters[5] = BLOSC_SHUFFLE;
    } else if doshuffle == BLOSC_BITSHUFFLE as i32 {
        filters[5] = BLOSC_BITSHUFFLE;
    }
    
    match internal::compress_extended(clevel, doshuffle, typesize, src, dest, compressor, &filters, &filters_meta) {
        Ok(size) => size as i32,
        Err(_) => 0,
    }
}

pub fn blosc2_decompress(
    src: &[u8],
    dest: &mut [u8],
) -> i32 {
    match internal::decompress(src, dest) {
        Ok(size) => size as i32,
        Err(_) => -1,
    }
}

pub fn blosc2_create_cctx(cparams: Blosc2Cparams) -> Blosc2Context {
    Blosc2Context {
        cparams,
        dparams: BLOSC2_DPARAMS_DEFAULTS,
    }
}

pub fn blosc2_compress_ctx(
    context: &Blosc2Context,
    src: &[u8],
    dest: &mut [u8],
) -> i32 {
    let clevel = context.cparams.clevel as i32;
    let typesize = context.cparams.typesize as usize;
    let compressor = context.cparams.compcode;
    
    let mut doshuffle = BLOSC_NOSHUFFLE as i32;
    for &f in context.cparams.filters.iter() {
        if f == BLOSC_SHUFFLE { doshuffle = BLOSC_SHUFFLE as i32; }
        if f == BLOSC_BITSHUFFLE { doshuffle = BLOSC_BITSHUFFLE as i32; }
    }
    
    // Filters are already u8 array
    let filters = context.cparams.filters;
    let filters_meta = context.cparams.filters_meta;
    
    match internal::compress_extended(clevel, doshuffle, typesize, src, dest, compressor, &filters, &filters_meta) {
        Ok(size) => size as i32,
        Err(_) => 0,
    }
}

pub fn blosc2_cbuffer_sizes(
    cbuffer: &[u8],
) -> (usize, usize, usize) {
    if cbuffer.len() < 16 { return (0, 0, 0); }
    let nb = u32::from_le_bytes([cbuffer[4], cbuffer[5], cbuffer[6], cbuffer[7]]) as usize;
    let bs = u32::from_le_bytes([cbuffer[8], cbuffer[9], cbuffer[10], cbuffer[11]]) as usize;
    let cb = u32::from_le_bytes([cbuffer[12], cbuffer[13], cbuffer[14], cbuffer[15]]) as usize;
    
    (nb, cb, bs)
}

pub fn blosc2_create_dctx(dparams: Blosc2Dparams) -> Blosc2Context {
    Blosc2Context {
        cparams: BLOSC2_CPARAMS_DEFAULTS,
        dparams,
    }
}

pub fn blosc2_decompress_ctx(
    _context: &Blosc2Context,
    src: &[u8],
    dest: &mut [u8],
) -> i32 {
    match internal::decompress(src, dest) {
        Ok(size) => size as i32,
        Err(_) => -1,
    }
}
