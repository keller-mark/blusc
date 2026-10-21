//! zstd support via `zrip`, a pure-Rust (`#![forbid(unsafe_code)]`) zstd codec.
//!
//! Mirrors c-blosc2 blosc2.c `zstd_wrap_compress` / `zstd_wrap_decompress`, which
//! lazily create one `ZSTD_CCtx` / `ZSTD_DCtx` per thread context and reuse it.
//!
//! zrip decodes frames from any zstd level, but its encoder only supports levels
//! -7..=4, so Blosc clevels mapping to higher zstd levels are clamped to 4.

/// Highest zstd level supported by the zrip encoder.
const ZRIP_MAX_LEVEL: i32 = 4;

/// Maps a Blosc clevel (0-9) to a zstd compression level supported by zrip.
///
/// C (c-blosc2 blosc2.c `zstd_wrap_compress`, c-blosc blosc.c `zstd_wrap_compress`):
/// `clevel = (clevel < 9) ? clevel * 2 - 1 : ZSTD_maxCLevel();`
/// (The following `if (clevel == 8)` in C is unreachable, since mapped levels < 9 are odd.)
/// The result is then clamped to zrip's maximum level (C would use up to 22).
pub fn blosc_clevel_to_zstd(clevel: i32) -> i32 {
    if clevel < 9 {
        (clevel * 2 - 1).min(ZRIP_MAX_LEVEL)
    } else {
        ZRIP_MAX_LEVEL
    }
}

/// Reusable zstd compression context (C: `thread_context->zstd_cctx`).
pub struct Compressor(zrip::CompressContext);

impl Compressor {
    /// Creates a context compressing at zstd level `level` (see [`blosc_clevel_to_zstd`]).
    pub fn new(level: i32) -> Result<Self, String> {
        let mut ctx = zrip::CompressContext::new(level).map_err(|e| e.to_string())?;
        // C `ZSTD_compressCCtx` does not write a content checksum by default; skip it
        // to match (Blosc chunks are not checksummed either).
        ctx.set_content_checksum(false);
        Ok(Compressor(ctx))
    }

    /// Compresses `src` into `dst` as a single zstd frame. Returns the compressed
    /// size, or an error if the frame does not fit in `dst`.
    pub fn compress(&mut self, src: &[u8], dst: &mut [u8]) -> Result<usize, String> {
        self.0.compress_into(src, dst).map_err(|e| e.to_string())
    }
}

/// Reusable zstd decompression context (C: `thread_context->zstd_dctx`).
pub struct Decompressor {
    ctx: zrip::DecompressContext,
    /// zrip only decodes into a `Vec`; this buffer is reused across streams so it
    /// is allocated once, and its contents are copied into the caller's slice.
    buf: Vec<u8>,
}

impl Decompressor {
    pub fn new() -> Self {
        Decompressor {
            ctx: zrip::DecompressContext::new(),
            buf: Vec::new(),
        }
    }

    /// Decompresses the zstd frame(s) in `src` into `dst`, returning the decompressed
    /// size. Output larger than `dst` is an error.
    pub fn decompress(&mut self, src: &[u8], dst: &mut [u8]) -> Result<usize, String> {
        self.buf.clear();
        let n = self
            .ctx
            .decompress_into_with_limit(src, &mut self.buf, dst.len())
            .map_err(|e| e.to_string())?;
        dst[..n].copy_from_slice(&self.buf[..n]);
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zstd_roundtrip() {
        let src: Vec<u8> = (0..100_000u32).map(|i| (i / 7) as u8).collect();
        let mut compressed = vec![0u8; src.len()];
        let mut cctx = Compressor::new(3).unwrap();
        let csize = cctx.compress(&src, &mut compressed).unwrap();
        assert!(csize < src.len());

        let mut out = vec![0u8; src.len()];
        let mut dctx = Decompressor::new();
        assert_eq!(dctx.decompress(&compressed[..csize], &mut out).unwrap(), src.len());
        assert_eq!(out, src);

        // Output buffer too small is an error, not a panic or overflow.
        assert!(dctx.decompress(&compressed[..csize], &mut out[..10]).is_err());
        assert!(cctx.compress(&src, &mut compressed[..4]).is_err());
    }

    #[test]
    fn test_clevel_mapping() {
        assert_eq!(blosc_clevel_to_zstd(0), -1);
        assert_eq!(blosc_clevel_to_zstd(1), 1);
        assert_eq!(blosc_clevel_to_zstd(2), 3);
        assert_eq!(blosc_clevel_to_zstd(5), 4);
        assert_eq!(blosc_clevel_to_zstd(9), 4);
        for clevel in 0..=9 {
            assert!(Compressor::new(blosc_clevel_to_zstd(clevel)).is_ok());
        }
    }
}
