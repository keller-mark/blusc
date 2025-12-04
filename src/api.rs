use crate::internal;
use std::os::raw::c_void;

// Constants
// Reference: https://github.com/maiteko/blosc2-src-rs/blob/9aa1ba98a9fe7a59c112d691bfea0caed6f00382/src/bindings.rs#L3C1-L21C53
pub const BLOSC2_VERSION_MAJOR: u32 = 2;
pub const BLOSC2_VERSION_MINOR: u32 = 11;
pub const BLOSC2_VERSION_RELEASE: u32 = 0;
pub const BLOSC2_VERSION_STRING: &[u8; 7usize] = b"2.11.0\0";
pub const BLOSC2_VERSION_DATE: &[u8; 22usize] = b"$Date:: 2023-11-04 #$\0";
pub const BLOSC2_MAX_DIM: u32 = 8;
pub const BLOSC_BLOSCLZ_COMPNAME: &[u8; 8usize] = b"blosclz\0";
pub const BLOSC_LZ4_COMPNAME: &[u8; 4usize] = b"lz4\0";
pub const BLOSC_LZ4HC_COMPNAME: &[u8; 6usize] = b"lz4hc\0";
pub const BLOSC_ZLIB_COMPNAME: &[u8; 5usize] = b"zlib\0";
pub const BLOSC_ZSTD_COMPNAME: &[u8; 5usize] = b"zstd\0";
pub const BLOSC_SNAPPY_COMPNAME: &[u8; 7usize] = b"snappy\0";
pub const BLOSC_BLOSCLZ_LIBNAME: &[u8; 8usize] = b"BloscLZ\0";
pub const BLOSC_LZ4_LIBNAME: &[u8; 4usize] = b"LZ4\0";
pub const BLOSC_ZLIB_LIBNAME: &[u8; 5usize] = b"Zlib\0";
pub const BLOSC_ZSTD_LIBNAME: &[u8; 5usize] = b"Zstd\0";
pub const BLOSC2_MAX_METALAYERS: u32 = 16;
pub const BLOSC2_METALAYER_NAME_MAXLEN: u32 = 31;
pub const BLOSC2_MAX_VLMETALAYERS: u32 = 8192;
pub const BLOSC2_VLMETALAYERS_NAME_MAXLEN: u32 = 31;

pub const BLOSC_NOSHUFFLE: i32 = 0;
pub const BLOSC_SHUFFLE: i32 = 1;
pub const BLOSC_BITSHUFFLE: i32 = 2;

pub const BLOSC_MAX_OVERHEAD: i32 = 16; // TODO: Verify this value
pub const BLOSC2_MAX_OVERHEAD: i32 = 32;

#[repr(C)]
pub struct Blosc2Context {
    pub cparams: Blosc2Cparams,
    pub dparams: Blosc2Dparams,
}

// Structs (Placeholders for now)
#[repr(C)]
pub struct Blosc2Cparams {
    // TODO: Add fields
    pub compcode: i32,
    pub clevel: i32,
    pub use_dict: i32,
    pub typesize: i32,
    pub nthreads: i32,
    pub blocksize: i32,
    pub splitmode: i32,
    pub schunk: *mut c_void,
    pub filters: [i32; 6],
    pub filters_meta: [i32; 6],
    pub compcode_meta: i32,
    pub delta: i32,
}

#[repr(C)]
pub struct Blosc2Dparams {
    // TODO: Add fields
    pub nthreads: i32,
    pub schunk: *mut c_void,
}

// Default constants
pub const BLOSC2_CPARAMS_DEFAULTS: Blosc2Cparams = Blosc2Cparams {
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

pub const BLOSC2_DPARAMS_DEFAULTS: Blosc2Dparams = Blosc2Dparams {
    nthreads: 1,
    schunk: std::ptr::null_mut(),
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
    let compressor = internal::BLOSC_BLOSCLZ;
    
    match internal::compress(clevel, doshuffle, typesize, src, dest, compressor) {
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
    let clevel = context.cparams.clevel;
    let typesize = context.cparams.typesize as usize;
    let compressor = match context.cparams.compcode {
        0 => internal::BLOSC_BLOSCLZ,
        1 => internal::BLOSC_LZ4,
        2 => internal::BLOSC_SNAPPY,
        3 => internal::BLOSC_ZLIB,
        4 => internal::BLOSC_ZSTD,
        _ => internal::BLOSC_BLOSCLZ,
    };
    
    let mut doshuffle = 0;
    for &f in context.cparams.filters.iter() {
        if f == 1 { doshuffle = 1; }
        if f == 2 { doshuffle = 2; }
    }
    
    match internal::compress(clevel, doshuffle, typesize, src, dest, compressor as u8) {
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
