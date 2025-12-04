use crate::internal;
use std::os::raw::{c_char, c_int, c_void};
use std::slice;

// Constants
pub const BLOSC_NOSHUFFLE: c_int = 0;
pub const BLOSC_SHUFFLE: c_int = 1;
pub const BLOSC_BITSHUFFLE: c_int = 2;

pub const BLOSC_MAX_OVERHEAD: c_int = 16; // TODO: Verify this value
pub const BLOSC2_MAX_OVERHEAD: c_int = 32;

#[repr(C)]
pub struct blosc2_context {
    pub cparams: blosc2_cparams,
    pub dparams: blosc2_dparams,
}

// Structs (Placeholders for now)
#[repr(C)]
pub struct blosc2_cparams {
    // TODO: Add fields
    pub compcode: c_int,
    pub clevel: c_int,
    pub use_dict: c_int,
    pub typesize: c_int,
    pub nthreads: c_int,
    pub blocksize: c_int,
    pub splitmode: c_int,
    pub schunk: *mut c_void,
    pub filters: [c_int; 6],
    pub filters_meta: [c_int; 6],
    pub compcode_meta: c_int,
    pub delta: c_int,
}

#[repr(C)]
pub struct blosc2_dparams {
    // TODO: Add fields
    pub nthreads: c_int,
    pub schunk: *mut c_void,
}

// Default constants
pub const BLOSC2_CPARAMS_DEFAULTS: blosc2_cparams = blosc2_cparams {
    compcode: 1, // BLOSC_BLOSCLZ
    clevel: 5,
    use_dict: 0,
    typesize: 8,
    nthreads: 1,
    blocksize: 0,
    splitmode: 1, // BLOSC_FORWARD_COMPAT_SPLIT
    schunk: std::ptr::null_mut(),
    filters: [0; 6],
    filters_meta: [0; 6],
    compcode_meta: 0,
    delta: 0,
};

pub const BLOSC2_DPARAMS_DEFAULTS: blosc2_dparams = blosc2_dparams {
    nthreads: 1,
    schunk: std::ptr::null_mut(),
};


// Function signatures

#[no_mangle]
pub unsafe extern "C" fn blosc1_cbuffer_metainfo(
    cbuffer: *const c_void,
    typesize: *mut usize,
    flags: *mut c_int,
) {
    let src = slice::from_raw_parts(cbuffer as *const u8, 16);
    if src.len() < 16 { return; }
    let ts = src[3] as usize;
    let fl = src[2] as c_int;
    
    if !typesize.is_null() { *typesize = ts; }
    if !flags.is_null() { *flags = fl; }
}

#[no_mangle]
pub unsafe extern "C" fn blosc1_cbuffer_validate(
    cbuffer: *const c_void,
    cbytes: usize,
    nbytes: *mut usize,
) -> c_int {
    let src = slice::from_raw_parts(cbuffer as *const u8, 16);
    if src.len() < 16 { return -1; }
    
    let cb = u32::from_le_bytes([src[12], src[13], src[14], src[15]]) as usize;
    if cbytes != cb { return -1; }
    
    let nb = u32::from_le_bytes([src[4], src[5], src[6], src[7]]) as usize;
    if !nbytes.is_null() { *nbytes = nb; }
    
    0
}

#[no_mangle]
pub unsafe extern "C" fn blosc1_cbuffer_sizes(
    cbuffer: *const c_void,
    nbytes: *mut usize,
    cbytes: *mut usize,
    blocksize: *mut usize,
) {
    blosc2_cbuffer_sizes(cbuffer, nbytes, cbytes, blocksize);
}

#[no_mangle]
pub unsafe extern "C" fn blosc1_getitem(
    _cbuffer: *const c_void,
    _start: c_int,
    _nitems: c_int,
    _dest: *mut c_void,
) -> c_int {
    // TODO
    0
}

#[no_mangle]
pub unsafe extern "C" fn blosc2_get_complib_info(
    _compcode: *const c_char,
    _complib: *mut *mut c_char,
    _version: *mut *mut c_char,
) -> c_int {
    // TODO
    0
}

#[no_mangle]
pub unsafe extern "C" fn blosc2_compress(
    clevel: c_int,
    doshuffle: c_int,
    typesize: usize,
    src: *const c_void,
    srcsize: usize,
    dest: *mut c_void,
    destsize: usize,
) -> c_int {
    let src_slice = slice::from_raw_parts(src as *const u8, srcsize);
    let dest_slice = slice::from_raw_parts_mut(dest as *mut u8, destsize);
    
    // Default compressor: BLOSCLZ (0)
    let compressor = internal::BLOSC_BLOSCLZ;
    
    match internal::compress(clevel, doshuffle, typesize, src_slice, dest_slice, compressor) {
        Ok(size) => size as c_int,
        Err(_) => 0,
    }
}

#[no_mangle]
pub unsafe extern "C" fn blosc2_decompress(
    src: *const c_void,
    srcsize: usize,
    dest: *mut c_void,
    destsize: usize,
) -> c_int {
    let src_slice = slice::from_raw_parts(src as *const u8, srcsize);
    let dest_slice = slice::from_raw_parts_mut(dest as *mut u8, destsize);
    
    match internal::decompress(src_slice, dest_slice) {
        Ok(size) => size as c_int,
        Err(_) => -1,
    }
}

#[no_mangle]
pub unsafe extern "C" fn blosc2_create_cctx(cparams: blosc2_cparams) -> *mut blosc2_context {
    let ctx = Box::new(blosc2_context {
        cparams,
        dparams: BLOSC2_DPARAMS_DEFAULTS,
    });
    Box::into_raw(ctx)
}

#[no_mangle]
pub unsafe extern "C" fn blosc2_compress_ctx(
    context: *mut blosc2_context,
    src: *const c_void,
    srcsize: usize,
    dest: *mut c_void,
    destsize: usize,
) -> c_int {
    let ctx = &*context;
    let src_slice = slice::from_raw_parts(src as *const u8, srcsize);
    let dest_slice = slice::from_raw_parts_mut(dest as *mut u8, destsize);
    
    let clevel = ctx.cparams.clevel;
    let typesize = ctx.cparams.typesize as usize;
    let compressor = match ctx.cparams.compcode {
        0 => internal::BLOSC_BLOSCLZ,
        1 => internal::BLOSC_LZ4,
        2 => internal::BLOSC_SNAPPY,
        3 => internal::BLOSC_ZLIB,
        4 => internal::BLOSC_ZSTD,
        _ => internal::BLOSC_BLOSCLZ,
    };
    
    let mut doshuffle = 0;
    for &f in ctx.cparams.filters.iter() {
        if f == 1 { doshuffle = 1; }
        if f == 2 { doshuffle = 2; }
    }
    
    match internal::compress(clevel, doshuffle, typesize, src_slice, dest_slice, compressor as u8) {
        Ok(size) => size as c_int,
        Err(_) => 0,
    }
}

#[no_mangle]
pub unsafe extern "C" fn blosc2_cbuffer_sizes(
    cbuffer: *const c_void,
    nbytes: *mut usize,
    cbytes: *mut usize,
    blocksize: *mut usize,
) {
    let src = slice::from_raw_parts(cbuffer as *const u8, 16);
    if src.len() < 16 { return; }
    let nb = u32::from_le_bytes([src[4], src[5], src[6], src[7]]) as usize;
    let bs = u32::from_le_bytes([src[8], src[9], src[10], src[11]]) as usize;
    let cb = u32::from_le_bytes([src[12], src[13], src[14], src[15]]) as usize;
    
    if !nbytes.is_null() { *nbytes = nb; }
    if !cbytes.is_null() { *cbytes = cb; }
    if !blocksize.is_null() { *blocksize = bs; }
}

#[no_mangle]
pub unsafe extern "C" fn blosc2_create_dctx(dparams: blosc2_dparams) -> *mut blosc2_context {
    let ctx = Box::new(blosc2_context {
        cparams: BLOSC2_CPARAMS_DEFAULTS,
        dparams,
    });
    Box::into_raw(ctx)
}

#[no_mangle]
pub unsafe extern "C" fn blosc2_decompress_ctx(
    context: *mut blosc2_context,
    src: *const c_void,
    srcsize: usize,
    dest: *mut c_void,
    destsize: usize,
) -> c_int {
    let src_slice = slice::from_raw_parts(src as *const u8, srcsize);
    let dest_slice = slice::from_raw_parts_mut(dest as *mut u8, destsize);
    
    match internal::decompress(src_slice, dest_slice) {
        Ok(size) => size as c_int,
        Err(_) => -1,
    }
}
