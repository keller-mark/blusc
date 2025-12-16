use crate::include;
use crate::include::blosc2_include::*;
use std::os::raw::c_void;
pub use crate::blosc::context::Blosc2Context;
pub use crate::blosc::blosc2::{Blosc2Cparams, Blosc2Dparams, BLOSC2_CPARAMS_DEFAULTS, BLOSC2_DPARAMS_DEFAULTS};



// Function signatures
pub fn blosc1_cbuffer_metainfo(
    cbuffer: &[u8],
) -> Option<(usize, i32)> {
    crate::blosc::blosc2::blosc1_cbuffer_metainfo(cbuffer)
}

pub fn blosc1_cbuffer_validate(
    cbuffer: &[u8],
    cbytes: usize,
) -> Result<usize, ()> {
    match crate::blosc::blosc2::blosc1_cbuffer_validate(cbuffer, cbytes) {
        Ok(nbytes) => Ok(nbytes),
        Err(_) => Err(()),
    }
}

pub fn blosc1_cbuffer_sizes(
    cbuffer: &[u8],
) -> (usize, usize, usize) {
    crate::blosc::blosc2::blosc1_cbuffer_sizes(cbuffer)
}

pub fn blosc1_getitem(
    cbuffer: &[u8],
    start: i32,
    nitems: i32,
    dest: &mut [u8],
) -> i32 {
    crate::blosc::blosc2::blosc1_getitem(cbuffer, start, nitems, dest)
}

pub fn blosc2_get_complib_info(
    compcode: &str,
) -> Option<(&'static str, &'static str, i32)> {
    crate::blosc::blosc2::blosc2_get_complib_info(compcode)
}

pub fn blosc2_compress(
    clevel: i32,
    doshuffle: i32,
    typesize: usize,
    src: &[u8],
    dest: &mut [u8],
) -> i32 {
    crate::blosc::blosc2::blosc2_compress(clevel, doshuffle, typesize, src, dest)
}

pub fn blosc2_decompress(
    src: &[u8],
    dest: &mut [u8],
) -> i32 {
    crate::blosc::blosc2::blosc2_decompress(src, dest)
}

pub fn blosc2_create_cctx(cparams: Blosc2Cparams) -> Blosc2Context {
    crate::blosc::blosc2::blosc2_create_cctx(cparams)
}

pub fn blosc2_compress_ctx(
    context: &mut Blosc2Context,
    src: &[u8],
    dest: &mut [u8],
) -> i32 {
    crate::blosc::blosc2::blosc2_compress_ctx(context, src, dest)
}

pub fn blosc2_cbuffer_sizes(
    cbuffer: &[u8],
) -> (usize, usize, usize) {
    crate::blosc::blosc2::blosc2_cbuffer_sizes(cbuffer)
}

pub fn blosc2_create_dctx(dparams: Blosc2Dparams) -> Blosc2Context {
    crate::blosc::blosc2::blosc2_create_dctx(dparams)
}

pub fn blosc2_decompress_ctx(
    context: &Blosc2Context,
    src: &[u8],
    dest: &mut [u8],
) -> i32 {
    crate::blosc::blosc2::blosc2_decompress_ctx(context, src, dest)
}

