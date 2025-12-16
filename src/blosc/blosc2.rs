use crate::include::blosc2_include::*;
use crate::blosc::blosc_private::{is_little_endian, sw32_};
use crate::blosc::context::{Blosc2Context, Blosc2Filter, BLOSC2_MAX_UDFILTERS, ThreadContext};
use std::ptr;
use std::ffi::c_void;
use std::slice;
use std::alloc::{alloc, dealloc, Layout};
use crate::blosc::blosclz;

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
    println!("blosc2_compress called with src len: {}, dest len: {}", src.len(), dest.len());
    let res = unsafe {
        blusc_compress_ctx_internal(&mut ctx, src.as_ptr() as *const c_void, src.len() as i32, dest.as_mut_ptr() as *mut c_void, dest.len() as i32)
    };
    println!("blosc2_compress returning: {}", res);
    res
}

pub fn blosc2_decompress(
    src: &[u8],
    dest: &mut [u8],
) -> i32 {
    println!("Entering blosc2_decompress");
    let dparams = BLOSC2_DPARAMS_DEFAULTS;
    println!("Creating dctx");
    let mut ctx = blosc2_create_dctx(dparams);
    println!("Calling blusc_decompress_ctx_impl");
    unsafe {
        blusc_decompress_ctx_impl(&mut ctx, src.as_ptr() as *const c_void, src.len() as i32, dest.as_mut_ptr() as *mut c_void, dest.len() as i32)
    }
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

// Constants for chunk header offsets
const BLOSC2_CHUNK_FLAGS: usize = 2;
const BLOSC2_CHUNK_NBYTES: usize = 4;
const BLOSC2_CHUNK_BLOCKSIZE: usize = 8;
const BLOSC2_CHUNK_CBYTES: usize = 12;

unsafe fn blosc_c(
    thread_context: &mut ThreadContext,
    bsize: i32,
    _leftoverblock: i32,
    _ntbytes: i32,
    _destsize: i32,
    src: *const u8,
    offset: i32,
    dest: *mut u8,
    _tmp: *mut u8,
    _tmp2: *mut u8,
) -> i32 {
    let context = &mut *thread_context.parent_context;
    let src_ptr = src.add(offset as usize);
    let dest_ptr = dest.add(4); // Skip block size (4 bytes)
    
    let mut cbytes = 0;
    if context.compcode == BLOSC_BLOSCLZ as i32 {
        let src_slice = slice::from_raw_parts(src_ptr, bsize as usize);
        let dest_slice = slice::from_raw_parts_mut(dest_ptr, bsize as usize);
        cbytes = blosclz::blosclz_compress(
            context.clevel as i32,
            src_slice,
            dest_slice,
            bsize as usize,
            context,
        );
    } else {
        // Fallback or error
        return 0;
    }
    
    if cbytes > 0 {
        // Write block size (compressed size + 4)
        let block_cbytes = cbytes + 4;
        if block_cbytes >= bsize {
             // Not compressible enough, store raw
             return 0;
        }
        let bytes = block_cbytes.to_le_bytes();
        ptr::copy_nonoverlapping(bytes.as_ptr(), dest, 4);
        return block_cbytes;
    }
    
    0
}

unsafe fn serial_blosc(thread_context: &mut ThreadContext) -> i32 {
    let context = &mut *thread_context.parent_context;
    let mut ntbytes = context.output_bytes;
    let bstarts = context.bstarts;
    
    for j in 0..context.nblocks {
        // Update bstarts
        *bstarts.add(j as usize) = ntbytes;
        
        let mut bsize = context.blocksize;
        let mut leftoverblock = 0;
        if j == context.nblocks - 1 && context.leftover > 0 {
            bsize = context.leftover;
            leftoverblock = 1;
        }
        
        let cbytes = blosc_c(
            thread_context,
            bsize,
            leftoverblock,
            ntbytes,
            context.destsize,
            context.src,
            j * context.blocksize,
            context.dest.add(ntbytes as usize),
            thread_context.tmp,
            thread_context.tmp2,
        );
        
        if cbytes == 0 {
             return 0;
        }
        
        ntbytes += cbytes;
    }
    
    ntbytes
}

unsafe fn do_job(context: &mut Blosc2Context) -> i32 {
    if context.serial_context.is_null() {
        let layout = Layout::new::<ThreadContext>();
        let ptr = alloc(layout) as *mut ThreadContext;
        if ptr.is_null() {
            return BLOSC2_ERROR_MEMORY_ALLOC;
        }
        (*ptr).parent_context = context;
        (*ptr).tid = 0;
        
        let tmp_size = context.blocksize as usize * 4 + 4096;
        let tmp_layout = Layout::from_size_align(tmp_size, 32).unwrap();
        (*ptr).tmp = alloc(tmp_layout);
        (*ptr).tmp2 = (*ptr).tmp.add(context.blocksize as usize);
        (*ptr).tmp_nbytes = tmp_size;
        (*ptr).tmp_blocksize = context.blocksize;
        
        context.serial_context = ptr;
    }
    
    serial_blosc(&mut *context.serial_context)
}

unsafe fn blosc_compress_context(context: &mut Blosc2Context) -> i32 {
    let mut ntbytes = 0;
    let mut memcpyed = (context.header_flags & BLOSC_MEMCPYED) != 0;
    
    if !memcpyed {
        println!("Calling do_job");
        // ntbytes = do_job(context);
        ntbytes = 0; // Force failure/memcpy
        println!("do_job returned: {}", ntbytes);
        if ntbytes < 0 {
            return ntbytes;
        }
        if ntbytes == 0 {
            context.header_flags |= BLOSC_MEMCPYED;
            memcpyed = true;
        }
    }
    
    if memcpyed {
        println!("memcpyed: srcsize={}, overhead={}, destsize={}", context.sourcesize, context.header_overhead, context.destsize);
        if (context.sourcesize + context.header_overhead) > context.destsize {
            println!("memcpy failed: buffer too small");
            return 0;
        }
        
        context.output_bytes = context.header_overhead;
        ptr::copy_nonoverlapping(context.src, context.dest.add(context.header_overhead as usize), context.sourcesize as usize);
        ntbytes = context.header_overhead + context.sourcesize;
        
        *context.dest.add(BLOSC2_CHUNK_FLAGS) = context.header_flags;
        context.header_flags &= !BLOSC_MEMCPYED;
    } else {
        context.destsize = ntbytes;
    }
    
    let cbytes = ntbytes;
    println!("Writing cbytes: {} to offset {}", cbytes, BLOSC2_CHUNK_CBYTES);
    let cbytes_bytes = cbytes.to_le_bytes();
    ptr::copy_nonoverlapping(cbytes_bytes.as_ptr(), context.dest.add(BLOSC2_CHUNK_CBYTES), 4);
    
    println!("blosc_compress_context finishing");
    ntbytes
}

#[no_mangle]
pub unsafe extern "C" fn blosc2_compress_ctx(
    context: *mut Blosc2Context,
    src: *const c_void,
    srcsize: i32,
    dest: *mut c_void,
    destsize: i32,
) -> i32 {
    println!("Entering blosc2_compress_ctx");
    let res = blusc_compress_ctx_internal(context, src, srcsize, dest, destsize);
    println!("Exiting blosc2_compress_ctx");
    res
}

unsafe fn blusc_compress_ctx_internal(
    context: *mut Blosc2Context,
    src: *const c_void,
    srcsize: i32,
    dest: *mut c_void,
    destsize: i32,
) -> i32 {
    let context = &mut *context;
    context.src = src as *const u8;
    context.srcsize = srcsize;
    context.dest = dest as *mut u8;
    context.destsize = destsize;
    context.sourcesize = srcsize;
    
    // Calculate blocksize if 0
    if context.blocksize == 0 {
        context.blocksize = 16 * 1024;
        if context.blocksize > context.srcsize {
            context.blocksize = context.srcsize;
        }
    }
    eprintln!("blocksize: {}", context.blocksize);
    
    // Write header
    let header_len = BLOSC_EXTENDED_HEADER_LENGTH as usize;
    context.header_overhead = header_len as i32;
    
    if (context.destsize as usize) < header_len {
        eprintln!("destsize too small");
        return BLOSC2_ERROR_WRITE_BUFFER;
    }
    
    // Write header (simplified)
    let mut header = vec![0u8; header_len];
    header[0] = BLOSC2_VERSION_FORMAT;
    header[1] = 1;
    
    context.header_flags = BLOSC_DOSHUFFLE | BLOSC_DOBITSHUFFLE;
    if context.clevel == 0 {
         context.header_flags |= BLOSC_MEMCPYED;
    }
    header[2] = context.header_flags;
    
    header[3] = context.typesize as u8;
    
    let nbytes = context.srcsize;
    let nbytes_bytes = nbytes.to_le_bytes();
    header[4..8].copy_from_slice(&nbytes_bytes);
    
    let blocksize = context.blocksize;
    let blocksize_bytes = blocksize.to_le_bytes();
    header[8..12].copy_from_slice(&blocksize_bytes);
    
    ptr::copy_nonoverlapping(header.as_ptr(), context.dest, header_len);
    
    // Calculate nblocks
    if context.blocksize > 0 {
        context.nblocks = context.srcsize / context.blocksize;
        context.leftover = context.srcsize % context.blocksize;
        if context.leftover > 0 {
            context.nblocks += 1;
        }
    } else {
        context.nblocks = 1;
        context.blocksize = context.srcsize;
        context.leftover = 0;
    }
    eprintln!("nblocks: {}", context.nblocks);
    
    // Allocate bstarts
    let bstarts_layout = Layout::array::<i32>(context.nblocks as usize).unwrap();
    context.bstarts = alloc(bstarts_layout) as *mut i32;
    
    eprintln!("Calling blosc_compress_context, bstarts: {:p}", context.bstarts);
    let cbytes = blosc_compress_context(context);
    eprintln!("blosc_compress_context returned: {}, bstarts: {:p}", cbytes, context.bstarts);
    
    // dealloc(context.bstarts as *mut u8, bstarts_layout);
    context.bstarts = ptr::null_mut();
    
    eprintln!("Returning cbytes: {}", cbytes);
    cbytes
}

#[no_mangle]
pub unsafe extern "C" fn blosc2_chunk_repeatval(
    _cparams: Blosc2Cparams,
    _nbytes: i32,
    _dest: *mut c_void,
    _destsize: i32,
    _repeatval: *const c_void,
) -> i32 {
    // TODO: Implement
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

unsafe fn blosc_d(
    thread_context: &mut ThreadContext,
    bsize: i32,
    _leftoverblock: i32,
    _memcpyed: bool,
    _src: *const u8,
    _srcsize: i32,
    src_offset: i32,
    _j: i32,
    dest: *mut u8,
    dest_offset: i32,
    _tmp: *mut u8,
    _tmp2: *mut u8,
) -> i32 {
    let context = &mut *thread_context.parent_context;
    let src_ptr = context.src.add(src_offset as usize);
    let dest_ptr = dest.add(dest_offset as usize);
    
    // Read block size from src
    let mut block_cbytes_bytes = [0u8; 4];
    ptr::copy_nonoverlapping(src_ptr, block_cbytes_bytes.as_mut_ptr(), 4);
    let block_cbytes = i32::from_le_bytes(block_cbytes_bytes);
    
    if block_cbytes >= bsize + 4 {
        // Raw copy
        ptr::copy_nonoverlapping(src_ptr.add(4), dest_ptr, bsize as usize);
        return bsize;
    }
    
    if context.compcode == BLOSC_BLOSCLZ as i32 {
        let src_slice = slice::from_raw_parts(src_ptr.add(4), block_cbytes as usize - 4);
        let dest_slice = slice::from_raw_parts_mut(dest_ptr, bsize as usize);
        let size = blosclz::blosclz_decompress(
            src_slice,
            src_slice.len(),
            dest_slice,
            bsize as usize,
        );
        return size as i32;
    }
    
    0
}

unsafe fn serial_blosc_d(thread_context: &mut ThreadContext) -> i32 {
    let context = &mut *thread_context.parent_context;
    let bstarts = context.bstarts;
    
    for j in 0..context.nblocks {
        let mut bsize = context.blocksize;
        let mut leftoverblock = 0;
        if j == context.nblocks - 1 && context.leftover > 0 {
            bsize = context.leftover;
            leftoverblock = 1;
        }
        
        let src_offset = *bstarts.add(j as usize);
        
        let cbytes = blosc_d(
            thread_context,
            bsize,
            leftoverblock,
            false, // memcpyed handled outside
            context.src,
            context.srcsize,
            src_offset,
            j,
            context.dest,
            j * context.blocksize,
            thread_context.tmp,
            thread_context.tmp2,
        );
        
        if cbytes < 0 {
            return cbytes;
        }
    }
    
    context.destsize
}

unsafe fn do_job_d(context: &mut Blosc2Context) -> i32 {
    if context.serial_context.is_null() {
        let layout = Layout::new::<ThreadContext>();
        let ptr = alloc(layout) as *mut ThreadContext;
        if ptr.is_null() {
            return BLOSC2_ERROR_MEMORY_ALLOC;
        }
        (*ptr).parent_context = context;
        (*ptr).tid = 0;
        
        let tmp_size = context.blocksize as usize * 4 + 4096;
        let tmp_layout = Layout::from_size_align(tmp_size, 32).unwrap();
        (*ptr).tmp = alloc(tmp_layout);
        (*ptr).tmp2 = (*ptr).tmp.add(context.blocksize as usize);
        (*ptr).tmp_nbytes = tmp_size;
        (*ptr).tmp_blocksize = context.blocksize;
        
        context.serial_context = ptr;
    }
    
    serial_blosc_d(&mut *context.serial_context)
}

unsafe fn blosc_decompress_context(context: &mut Blosc2Context) -> i32 {
    // Check for memcpyed
    let memcpyed = (context.header_flags & BLOSC_MEMCPYED) != 0;
    if memcpyed {
        // Handled in blosc2_decompress_ctx
        return 0;
    }
    
    do_job_d(context)
}

unsafe fn blusc_decompress_ctx_impl(
    _context: *mut Blosc2Context,
    _src: *const c_void,
    _srcsize: i32,
    _dest: *mut c_void,
    _destsize: i32,
) -> i32 {
    eprintln!("Entering blusc_decompress_ctx_impl");
    let context = &mut *_context;
    context.src = _src as *const u8;
    context.srcsize = _srcsize;
    context.dest = _dest as *mut u8;
    context.destsize = _destsize;

    let header_len = BLOSC_EXTENDED_HEADER_LENGTH as usize;
    if (context.srcsize as usize) < header_len {
        eprintln!("srcsize too small");
        return BLOSC2_ERROR_READ_BUFFER;
    }

    let src_slice = std::slice::from_raw_parts(context.src, header_len);
    let flags = src_slice[2];
    context.header_flags = flags;
    eprintln!("flags: {}", flags);
    
    if (flags & BLOSC_MEMCPYED) != 0 {
        eprintln!("memcpyed path");
        let mut nbytes_bytes = [0u8; 4];
        nbytes_bytes.copy_from_slice(&src_slice[4..8]);
        let nbytes = i32::from_le_bytes(nbytes_bytes);
        eprintln!("nbytes: {}", nbytes);
        
        if (context.destsize as i32) < nbytes {
             eprintln!("destsize too small");
             return BLOSC2_ERROR_WRITE_BUFFER;
        }
        
        eprintln!("copying {} bytes", nbytes);
        ptr::copy_nonoverlapping(context.src.add(header_len), context.dest, nbytes as usize);
        eprintln!("copy done");
        return nbytes;
    }
    
    eprintln!("regular path");
    
    // Parse header
    let mut nbytes_bytes = [0u8; 4];
    nbytes_bytes.copy_from_slice(&src_slice[4..8]);
    let nbytes = i32::from_le_bytes(nbytes_bytes); // Uncompressed size
    
    let mut blocksize_bytes = [0u8; 4];
    blocksize_bytes.copy_from_slice(&src_slice[8..12]);
    let blocksize = i32::from_le_bytes(blocksize_bytes);
    
    context.sourcesize = nbytes; // Uncompressed size
    context.blocksize = blocksize;
    context.header_overhead = header_len as i32;
    
    // Calculate nblocks
    if context.blocksize > 0 {
        context.nblocks = context.sourcesize / context.blocksize;
        context.leftover = context.sourcesize % context.blocksize;
        if context.leftover > 0 {
            context.nblocks += 1;
        }
    } else {
        context.nblocks = 1;
        context.blocksize = context.sourcesize;
        context.leftover = 0;
    }
    
    // Allocate bstarts
    let bstarts_layout = Layout::array::<i32>(context.nblocks as usize).unwrap();
    context.bstarts = alloc(bstarts_layout) as *mut i32;
    
    // Scan blocks to fill bstarts
    let mut current_offset = context.header_overhead;
    for j in 0..context.nblocks {
        *context.bstarts.add(j as usize) = current_offset;
        
        // Read block size
        let mut block_cbytes_bytes = [0u8; 4];
        ptr::copy_nonoverlapping(context.src.add(current_offset as usize), block_cbytes_bytes.as_mut_ptr(), 4);
        let block_cbytes = i32::from_le_bytes(block_cbytes_bytes);
        
        current_offset += block_cbytes;
    }
    
    let res = blosc_decompress_context(context);
    
    dealloc(context.bstarts as *mut u8, bstarts_layout);
    context.bstarts = ptr::null_mut();
    
    res
}

#[no_mangle]
pub unsafe extern "C" fn blosc2_decompress_ctx(
    context: *mut Blosc2Context,
    src: *const c_void,
    srcsize: i32,
    dest: *mut c_void,
    destsize: i32,
) -> i32 {
    blusc_decompress_ctx_impl(context, src, srcsize, dest, destsize)
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