# My current understanding of the BLOSC algorithm

## Compression

The blosc2 compression pipeline works as follows:

1. **Block sizing** (`compute_blocksize` / C `blosc_stune_next_blocksize`):
   - Determines blocksize based on clevel, typesize, nbytes, and compressor.
   - Base is L1 cache size (32KB), scaled up by clevel.
   - HCR codecs (LZ4HC, ZLIB, ZSTD) get 2x blocksize.
   - When splitting is active (byte-shuffle + compatible codec), a separate blocksize
     table is used (32KB-512KB * typesize, capped at 4MB).

2. **Header creation**: 16-byte (blosc1) or 32-byte (blosc2 extended) header.
   - Byte 0: version format
   - Byte 1: compressor version
   - Byte 2: flags (shuffle bits, memcpy bit, codec in bits 5-7)
   - Byte 3: typesize
   - Bytes 4-7: nbytes (uncompressed size, LE u32)
   - Bytes 8-11: blocksize (LE u32)
   - Bytes 12-15: cbytes (compressed size, LE u32)
   - Extended header bytes 16-21: filter codes
   - Byte 22: compressor code
   - Bytes 24-29: filter metadata

3. **Split decision** (`split_block` / C `stune.c:split_block`):
   - Only splits for byte-shuffle (BLOSC_DOSHUFFLE), NOT bitshuffle.
   - Only for BLOSCLZ, LZ4, or ZSTD (clevel <= 5).
   - typesize must be <= 16 (MAX_STREAMS).
   - blocksize/typesize must be >= BLOSC_MIN_BUFFERSIZE (32).

4. **Per-block processing**:
   - Apply filter (shuffle or bitshuffle).
   - If splitting: compress each typesize-stream separately.
   - If not splitting: compress entire block as one stream.
   - Each stream is preceded by a 4-byte LE u32 compressed size.

5. **Incompressible fallback**: If any stream fails to compress or total compressed
   exceeds original, fall back to memcpy (BLOSC_MEMCPYED flag).

## Decompression

1. Parse header to get nbytes, cbytes, blocksize, flags, compressor, typesize.
2. Detect extended header (both DOSHUFFLE and DOBITSHUFFLE flags set = extended marker).
3. If MEMCPYED flag: direct copy from after header.
4. Otherwise: read bstarts array, decompress each block's streams, apply inverse filter.

### Rust implementation structure (`src/internal/mod.rs`)

Mirrors C `blosc2.c`: `read_chunk_header` -> `blosc_d` (per block) -> `pipeline_backward`.

- `ChunkHeader::read`: parses/validates the header once (C `read_chunk_header`).
  Rejects `blocksize == 0` / `typesize == 0` for non-memcpyed chunks and checks
  that the bstarts array and `cbytes` fit in `src`.
- `ChunkHeader::block_content`: bstarts are read directly from `src` (no `Vec`).
  Like C `blosc_d`, a block's streams are bounded by `cbytes`, not the next
  bstart (stream sizes are self-delimiting).
- `decode_block`: the per-stream loop of C `blosc_d`. Stream csize (LE i32):
  `0` = zeros, `< 0` = run of byte `-csize` (+1 token byte), `== neblock` = raw,
  otherwise codec data. `nstreams = typesize` unless `dont_split` (flags bit 4)
  or the leftover block.
- `blosc_d`: decode + inverse filter, using ONE reusable block-sized scratch buffer:
  - shuffle: decode -> tmp, `unshuffle(tmp -> dest)`
  - bitshuffle: decode -> dest, `bitunshuffle_inplace(dest, tmp)`
  - none: decode -> dest
- `Decoders`: zstd `Decompressor` and zlib `flate2::Decompress` are created
  lazily once per call and reused across all streams/blocks (C keeps
  `zstd_dctx` in its thread context). zlib inflates one-shot into the output
  (C uses `uncompress`).
- `getitem`: only blocks overlapping the range are decoded. Fully covered blocks go
  through `blosc_d` straight into `dest`; partial blocks are decoded into scratch and
  only the requested bytes are unshuffled (`filters::unshuffle_partial`).

### Filters (`src/filters/mod.rs`)

- `unshuffle` is `dest[i*ts + j] = src[j*quot + i]`, trailing `blocksize % ts`
  bytes copied as-is. Implemented with 16-element tiles specialized for typesize
  2/4/8/16 so LLVM auto-vectorizes. No `std::arch` intrinsics, so this stays
  portable to WASM.
- Bitunshuffle = reverse of bitshuffle steps 3, 2, 1:
  - step 3 reverse (`untrans_bitrow_eight`) is a pure block permutation and is fused
    into the step 2 reverse: for element byte `e`, bit row `kk`, offset `r`,
    the input byte is at `e*8*B + kk*B + r` with `B = size/8`.
  - step 1 reverse (`trans_byte_elem` with swapped args) is exactly
    `unshuffle(typesize, aligned_bytes)`.
  - bytes past the last multiple of 8 elements are not bitshuffled, which enables
    the in-place variant.

### Performance notes
- `lz4_flex` default features (`safe-decode`) are kept on purpose: on shuffled data
  (long runs) the safe decoder was ~5x faster than the unsafe one in
  `examples/bench_decompress.rs`.
- BloscLZ overlapping matches (`distance < len`) are copied in doubling chunks
  starting at multiples of `distance` (`copy_within`), `distance == 1` uses `fill`.
- Benchmark: `cargo run --release --example bench_decompress`.

### zstd (`src/codecs/zstd.rs`)
- Uses `zrip`, a pure-Rust zstd codec (`#![forbid(unsafe_code)]`, builds for wasm32).
  `Compressor` / `Decompressor` wrap `zrip::CompressContext` / `DecompressContext`;
  one of each per (de)compression call, reused across streams (C keeps
  `zstd_cctx`/`zstd_dctx` in the thread context).
- clevel mapping, identical in c-blosc2 `blosc/blosc2.c` `zstd_wrap_compress`
  (lines ~494-522) and c-blosc `blosc/blosc.c` `zstd_wrap_compress` (lines ~499-511):
  `clevel = (clevel < 9) ? clevel * 2 - 1 : ZSTD_maxCLevel();` (the following
  `if (clevel == 8)` is unreachable). zrip's encoder only supports levels -7..=4, so
  the result is clamped to 4: output is larger than C's for clevel >= 3, especially
  clevel 9 (C uses zstd level 22). A zstd error => stream treated as incompressible.
- Content checksums are disabled to match C `ZSTD_compressCCtx` defaults.
- zrip decodes only into a `Vec`, so each stream is decoded into a reused `Vec`
  (limit = stream size) and copied into the block.
- Rejected alternatives: `ruzstd` (0.7 has no encoder; 0.9 only implements the
  `Fastest` level and decoded the zarr test shard ~5x slower); `libzstd-rs-sys`
  0.0.1-prerelease.2 (same speed as zrip, matching C levels, but does not compile
  for wasm32-unknown-unknown because it uses `libc::size_t`/`libc::memcpy`).

### Compression (`blosclz_compress`, line ~430)
- Uses ipshift=4, minlen=4 (constant for all clevels).
- Hash table size: `1 << hashlog` where hashlog depends on clevel
  (0 for clevel 0, HASH_LOG-2 for 1, HASH_LOG-1 for 2, HASH_LOG for 3+).
- **Entropy probing** (line ~455): estimates compression ratio by running a simulated
  compression on a portion of the buffer. Bails early if ratio is too low.
  Probe length: length/8 (clevel<2), length/4 (clevel<4), length/2 (clevel<7), full (clevel>=7).
  Threshold table: [0, 2, 1.5, 1.2, 1.2, 1.2, 1.2, 1.15, 1.1, 1.0].
- Hash table is re-initialized to 0 after entropy probing (line ~485).
- Match detection: first 4 bytes checked, then `get_run_or_match` extends.
- `get_match` returns one-past-end (C post-increment semantics) -> encoded len = ip - anchor.
- `get_run` (for repeated bytes, distance==0 after decrement) does NOT post-increment.
- Encoded match length is biased: decompressor adds 3 to recover actual length.
- After encoding, hash is updated at match boundary position, then ip advances by 2.
- At clevel 9, a second hash entry is stored (shifted by 1 byte).

### Decompression (`blosclz_decompress`, line ~550)
- ctrl byte: if >= 32 -> match, else -> literal copy.
- Match: len = (ctrl >> 5) - 1, with extension for len==6. Then len += 3.
- Far distance (code==255, ofs==31*256): 16-bit distance encoding.
- Optimized copy for runs (distance==1) using memset.

### Critical C implementation detail: `get_match` post-increment
The C `get_match` function (line ~149) uses `while (*ref++ == *ip++) {}`.
The post-increment means ip advances one extra byte on mismatch but NOT when
hitting ip_bound. In the 8-byte fast path (non-STRICT_ALIGN), the inner byte-by-byte
fallback has no ip_bound check, so it always post-increments on mismatch.
