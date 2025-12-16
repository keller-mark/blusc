use crate::include::blosc2_include::*;
use crate::blosc::blosc_private::is_little_endian;
use crate::blosc::context::{Blosc2Context, Blosc2Filter, BLOSC2_MAX_UDFILTERS};
use std::ptr;

// Use constants from include, cast to usize for array sizes
const MAX_FILTERS: usize = BLOSC2_MAX_FILTERS as usize;

#[derive(Debug, Default, Clone, Copy)]
pub struct BloscHeader {
    pub version: u8,
    pub versionlz: u8,
    pub flags: u8,
    pub typesize: u8,
    pub nbytes: i32,
    pub blocksize: i32,
    pub cbytes: i32,
    pub filters: [u8; MAX_FILTERS],
    pub udcompcode: u8,
    pub compcode_meta: u8,
    pub filters_meta: [u8; MAX_FILTERS],
    pub reserved2: u8,
    pub blosc2_flags: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Blosc2Cparams {
    pub compcode: u8,
    pub compcode_meta: u8,
    pub clevel: u8,
    pub use_dict: i32,
    pub typesize: i32,
    pub nthreads: i16,
    pub blocksize: i32,
    pub splitmode: i32,
    pub schunk: *mut std::ffi::c_void,
    pub filters: [u8; MAX_FILTERS],
    pub filters_meta: [u8; MAX_FILTERS],
    pub prefilter: *mut std::ffi::c_void,
    pub preparams: *mut std::ffi::c_void,
    pub tuner_params: *mut std::ffi::c_void,
    pub tuner_id: i32,
    pub instr_codec: bool,
    pub codec_params: *mut std::ffi::c_void,
    pub filter_params: [*mut std::ffi::c_void; MAX_FILTERS],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Blosc2Dparams {
    pub nthreads: i16,
    pub schunk: *mut std::ffi::c_void,
    pub postfilter: *mut std::ffi::c_void,
    pub postparams: *mut std::ffi::c_void,
}

pub const BLOSC2_CPARAMS_DEFAULTS: Blosc2Cparams = Blosc2Cparams {
    compcode: 1, // BLOSC_BLOSCLZ
    compcode_meta: 0,
    clevel: 5,
    use_dict: 0,
    typesize: 0,
    nthreads: 1,
    blocksize: 0,
    splitmode: 1, // BLOSC_ALWAYS_SPLIT
    schunk: ptr::null_mut(),
    filters: [0; MAX_FILTERS],
    filters_meta: [0; MAX_FILTERS],
    prefilter: ptr::null_mut(),
    preparams: ptr::null_mut(),
    tuner_params: ptr::null_mut(),
    tuner_id: 0,
    instr_codec: false,
    codec_params: ptr::null_mut(),
    filter_params: [ptr::null_mut(); MAX_FILTERS],
};

pub const BLOSC2_DPARAMS_DEFAULTS: Blosc2Dparams = Blosc2Dparams {
    nthreads: 1,
    schunk: ptr::null_mut(),
    postfilter: ptr::null_mut(),
    postparams: ptr::null_mut(),
};

pub fn read_chunk_header(src: &[u8], extended_header: bool) -> Result<BloscHeader, i32> {
    if src.len() < BLOSC_MIN_HEADER_LENGTH {
        return Err(BLOSC2_ERROR_READ_BUFFER);
    }

    let mut header = BloscHeader::default();
    
    header.version = src[0];
    header.versionlz = src[1];
    header.flags = src[2];
    header.typesize = src[3];
    
    let nbytes_bytes: [u8; 4] = src[4..8].try_into().unwrap();
    let blocksize_bytes: [u8; 4] = src[8..12].try_into().unwrap();
    let cbytes_bytes: [u8; 4] = src[12..16].try_into().unwrap();

    header.nbytes = i32::from_le_bytes(nbytes_bytes);
    header.blocksize = i32::from_le_bytes(blocksize_bytes);
    header.cbytes = i32::from_le_bytes(cbytes_bytes);

    if header.version > BLOSC2_VERSION_FORMAT {
        return Err(BLOSC2_ERROR_VERSION_SUPPORT);
    }
    if header.cbytes < BLOSC_MIN_HEADER_LENGTH as i32 {
        return Err(BLOSC2_ERROR_INVALID_HEADER);
    }
    if header.blocksize <= 0 || (header.nbytes > 0 && header.blocksize > header.nbytes) {
        return Err(BLOSC2_ERROR_INVALID_HEADER);
    }
    if header.blocksize as usize > BLOSC_MAX_BLOCKSIZE as usize {
        return Err(BLOSC2_ERROR_INVALID_HEADER);
    }
    if header.typesize == 0 {
        return Err(BLOSC2_ERROR_INVALID_HEADER);
    }

    if extended_header && (header.flags & BLOSC_DOSHUFFLE != 0) && (header.flags & BLOSC_DOBITSHUFFLE != 0) {
        if header.cbytes < BLOSC_EXTENDED_HEADER_LENGTH as i32 {
             return Err(BLOSC2_ERROR_INVALID_HEADER);
        }
        if src.len() < BLOSC_EXTENDED_HEADER_LENGTH {
             return Err(BLOSC2_ERROR_READ_BUFFER);
        }
        
        header.filters.copy_from_slice(&src[16..16+MAX_FILTERS]);
        header.udcompcode = src[16+MAX_FILTERS];
        header.compcode_meta = src[16+MAX_FILTERS + 1];
        header.filters_meta.copy_from_slice(&src[16+MAX_FILTERS + 2 .. 16+MAX_FILTERS + 2 + MAX_FILTERS]);
        header.reserved2 = src[16+MAX_FILTERS + 2 + MAX_FILTERS];
        header.blosc2_flags = src[16+MAX_FILTERS + 2 + MAX_FILTERS + 1];
    }

    Ok(header)
}

pub fn blosc2_cbuffer_sizes(cbuffer: &[u8]) -> (usize, usize, usize) {
    match read_chunk_header(cbuffer, false) {
        Ok(header) => (header.nbytes as usize, header.cbytes as usize, header.blocksize as usize),
        Err(_) => (0, 0, 0),
    }
}

pub fn blosc1_cbuffer_sizes(cbuffer: &[u8]) -> (usize, usize, usize) {
    blosc2_cbuffer_sizes(cbuffer)
}

pub fn blosc1_cbuffer_validate(cbuffer: &[u8], cbytes: usize) -> Result<usize, i32> {
    if cbytes < BLOSC_MIN_HEADER_LENGTH {
        return Err(BLOSC2_ERROR_WRITE_BUFFER);
    }
    
    let header = read_chunk_header(cbuffer, false)?;
    
    if header.cbytes as usize != cbytes {
        return Err(BLOSC2_ERROR_INVALID_HEADER);
    }
    
    if header.nbytes as usize > BLOSC2_MAX_BUFFERSIZE {
        return Err(BLOSC2_ERROR_MEMORY_ALLOC);
    }
    
    Ok(header.nbytes as usize)
}

pub fn blosc1_cbuffer_metainfo(cbuffer: &[u8]) -> Option<(usize, i32)> {
    match read_chunk_header(cbuffer, false) {
        Ok(header) => Some((header.typesize as usize, header.flags as i32)),
        Err(_) => None,
    }
}

pub fn blosc2_get_complib_info(_compcode: &str) -> Option<(&'static str, &'static str, i32)> {
    // TODO: Implement
    None
}

pub fn blosc2_compress(
    clevel: i32,
    doshuffle: i32,
    typesize: usize,
    src: &[u8],
    dest: &mut [u8],
) -> i32 {
    let mut cparams = BLOSC2_CPARAMS_DEFAULTS;
    cparams.clevel = clevel as u8;
    cparams.typesize = typesize as i32;
    if BLOSC2_MAX_FILTERS > 0 {
        cparams.filters[BLOSC2_MAX_FILTERS as usize - 1] = doshuffle as u8;
    }
    
    let mut ctx = blosc2_create_cctx(cparams);
    blosc2_compress_ctx(&mut ctx, src, dest)
}

pub fn blosc2_decompress(
    src: &[u8],
    dest: &mut [u8],
) -> i32 {
    let dparams = BLOSC2_DPARAMS_DEFAULTS;
    let ctx = blosc2_create_dctx(dparams);
    blosc2_decompress_ctx(&ctx, src, dest)
}

pub fn blosc2_create_cctx(cparams: Blosc2Cparams) -> Blosc2Context {
    Blosc2Context {
        src: ptr::null(),
        dest: ptr::null_mut(),
        header_flags: 0,
        blosc2_flags: 0,
        sourcesize: 0,
        header_overhead: 0,
        nblocks: 0,
        leftover: 0,
        blocksize: cparams.blocksize,
        splitmode: cparams.splitmode,
        output_bytes: 0,
        srcsize: 0,
        destsize: 0,
        typesize: cparams.typesize,
        bstarts: ptr::null_mut(),
        special_type: 0,
        compcode: cparams.compcode as i32,
        compcode_meta: cparams.compcode_meta,
        clevel: cparams.clevel as i32,
        use_dict: cparams.use_dict,
        dict_buffer: ptr::null_mut(),
        dict_size: 0,
        dict_cdict: ptr::null_mut(),
        dict_ddict: ptr::null_mut(),
        filter_flags: 0,
        filters: cparams.filters,
        filters_meta: cparams.filters_meta,
        urfilters: [Blosc2Filter { _placeholder: 0 }; BLOSC2_MAX_UDFILTERS],
        prefilter: cparams.prefilter as *mut _,
        postfilter: ptr::null_mut(),
        preparams: cparams.preparams as *mut _,
        postparams: ptr::null_mut(),
        block_maskout: ptr::null_mut(),
        block_maskout_nitems: 0,
        schunk: cparams.schunk as *mut _,
        serial_context: ptr::null_mut(),
        do_compress: 1,
        tuner_params: cparams.tuner_params as *mut _,
        tuner_id: cparams.tuner_id,
        codec_params: cparams.codec_params as *mut _,
        filter_params: unsafe { std::mem::transmute(cparams.filter_params) },
        nthreads: cparams.nthreads,
        new_nthreads: cparams.nthreads,
        threads_started: 0,
        end_threads: 0,
        threads: ptr::null_mut(),
        thread_contexts: ptr::null_mut(),
        thread_giveup_code: 0,
        thread_nblock: 0,
        dref_not_init: 0,
    }
}

pub fn blosc2_compress_ctx(
    context: &mut Blosc2Context,
    src: &[u8],
    dest: &mut [u8],
) -> i32 {
    context.src = src.as_ptr();
    context.srcsize = src.len() as i32;
    context.dest = dest.as_mut_ptr();
    context.destsize = dest.len() as i32;
    
    if context.clevel == 0 {
        // Memcpy
        let header_len = BLOSC_EXTENDED_HEADER_LENGTH as usize;
        if (context.destsize as usize) < context.srcsize as usize + header_len {
            return BLOSC2_ERROR_WRITE_BUFFER;
        }
        
        // Write header (simplified)
        let mut header = vec![0u8; header_len];
        header[0] = BLOSC2_VERSION_FORMAT; // Version
        header[1] = 1; // TODO: Why is this 1 in C-Blosc2?
        header[2] = BLOSC_MEMCPYED | BLOSC_DOSHUFFLE | BLOSC_DOBITSHUFFLE; // Flags (Extended header implies DOSHUFFLE | DOBITSHUFFLE)
        header[3] = context.typesize as u8;
        
        let nbytes = context.srcsize;
        let nbytes_bytes = nbytes.to_le_bytes();
        header[4..8].copy_from_slice(&nbytes_bytes);
        
        let blocksize = nbytes; // Single block
        let blocksize_bytes = blocksize.to_le_bytes();
        header[8..12].copy_from_slice(&blocksize_bytes);
        
        let cbytes = nbytes + header_len as i32;
        let cbytes_bytes = cbytes.to_le_bytes();
        header[12..16].copy_from_slice(&cbytes_bytes);
        
        // Copy header to dest
        unsafe {
            ptr::copy_nonoverlapping(header.as_ptr(), context.dest, header_len);
        }
        
        // Copy data
        unsafe {
            ptr::copy_nonoverlapping(context.src, context.dest.add(header_len), context.srcsize as usize);
        }
        
        return cbytes;
    }
    
    // TODO: Implement compression for clevel > 0
    0
}

pub fn blosc2_create_dctx(dparams: Blosc2Dparams) -> Blosc2Context {
    // TODO: Implement
    Blosc2Context {
        src: ptr::null(),
        dest: ptr::null_mut(),
        header_flags: 0,
        blosc2_flags: 0,
        sourcesize: 0,
        header_overhead: 0,
        nblocks: 0,
        leftover: 0,
        blocksize: 0,
        splitmode: 0,
        output_bytes: 0,
        srcsize: 0,
        destsize: 0,
        typesize: 0,
        bstarts: ptr::null_mut(),
        special_type: 0,
        compcode: 0,
        compcode_meta: 0,
        clevel: 0,
        use_dict: 0,
        dict_buffer: ptr::null_mut(),
        dict_size: 0,
        dict_cdict: ptr::null_mut(),
        dict_ddict: ptr::null_mut(),
        filter_flags: 0,
        filters: [0; crate::blosc::context::BLOSC2_MAX_FILTERS],
        filters_meta: [0; crate::blosc::context::BLOSC2_MAX_FILTERS],
        urfilters: [Blosc2Filter { _placeholder: 0 }; BLOSC2_MAX_UDFILTERS],
        prefilter: ptr::null_mut(),
        postfilter: dparams.postfilter as *mut u8,
        preparams: ptr::null_mut(),
        postparams: dparams.postparams as *mut _,
        block_maskout: ptr::null_mut(),
        block_maskout_nitems: 0,
        schunk: dparams.schunk as *mut _,
        serial_context: ptr::null_mut(),
        do_compress: 0,
        tuner_params: ptr::null_mut(),
        tuner_id: 0,
        codec_params: ptr::null_mut(),
        filter_params: [ptr::null_mut(); crate::blosc::context::BLOSC2_MAX_FILTERS],
        nthreads: dparams.nthreads,
        new_nthreads: dparams.nthreads,
        threads_started: 0,
        end_threads: 0,
        threads: ptr::null_mut(),
        thread_contexts: ptr::null_mut(),
        thread_giveup_code: 0,
        thread_nblock: 0,
        dref_not_init: 0,
    }
}

pub fn blosc2_decompress_ctx(
    _context: &Blosc2Context,
    _src: &[u8],
    _dest: &mut [u8],
) -> i32 {
    // TODO: Implement
    0
}

pub fn blosc1_getitem(
    _cbuffer: &[u8],
    _start: i32,
    _nitems: i32,
    _dest: &mut [u8],
) -> i32 {
    // TODO: Implement
    0
}