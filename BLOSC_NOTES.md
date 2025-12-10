# My current understanding of the BLOSC algorithm

## Compression

### Block Splitting

Reference: `c-blosc2/blosc/stune.c` lines 185-215

The `split_block()` function determines whether to split a block into multiple streams (one per typesize byte). Splitting is only done for:
- Fast codecs: BLOSCLZ, LZ4, or ZSTD with clevel <= 5
- When byte-shuffle (BLOSC_SHUFFLE) is enabled (NOT bitshuffle!)
- When typesize <= MAX_STREAMS (16)
- When blocksize / typesize >= BLOSC_MIN_BUFFERSIZE

**Important**: Bitshuffle (BLOSC_BITSHUFFLE) does NOT enable block splitting. Only byte-shuffle does.

### BLOSClz Compression

Reference: `c-blosc2/blosc/blosclz.c` lines 422-650

The BLOSClz compressor uses several key optimizations:

1. **ipshift and minlen**: Set to 4 for optimal compression with bitshuffle and small typesizes
   - `ipshift = 4`: Shifts back in the match by 4 bytes to find longer runs
   - `minlen = 4`: Minimum match length to encode

2. **Match Finding**: Uses hash table to find potential matches
   - Hash function: `(seq * 2654435761) >> (32 - hashlog)`
   - Matches must be at least 4 bytes initially

3. **Match Length Calculation**: Critical bug was in the Rust implementation
   - C: `ip = get_run_or_match(ip, ip_bound, ref, !distance)` returns pointer AFTER last match byte
   - C: `ip -= ipshift` to bias the position
   - C: `len = ip - anchor` gives biased match length
   - **Bug in Rust**: Was checking `ref_ptr < ip_limit` which prevented finding long matches
   - **Bug in Rust**: Was advancing `ip` twice (once in match finding, once after)
   - **Fix**: Remove bound check on ref_ptr, and update ip correctly: `ip = ip + len - ipshift`

4. **Match Acceptance Criteria**:
   - Reject if `len < minlen` OR (`len <= 5` AND `distance >= MAX_DISTANCE`)
   - Accept if `len >= minlen` AND (`len > 5` OR `distance < MAX_DISTANCE`)

5. **Encoding**:
   - Length is biased by -2 for encoding
   - Short matches (len < 7): 2 bytes overhead
   - Long matches (len >= 7): 2+ bytes overhead (additional bytes for extended length)
   - Far matches (distance >= MAX_DISTANCE): +2 bytes overhead

### Bitshuffle Filter

Reference: `c-blosc2/blosc/bitshuffle-generic.c`

The bitshuffle filter performs three transpose operations:
1. **Transpose bytes within elements** (`bshuf_trans_byte_elem_scal`)
2. **Transpose bits within bytes** (`bshuf_trans_bit_byte_scal`)  
3. **Transpose bitrows** (`bshuf_trans_bitrow_eight`)

Requirements:
- Buffer size must be divisible by 8 elements
- Works best with data that has patterns at the bit level

## Decompression

## Extended Header Flags

In Blosc2, if the header is an extended header (32 bytes), the `flags` byte (byte 2) may have both `BLOSC_DOSHUFFLE` (0x1) and `BLOSC_DOBITSHUFFLE` (0x2) set. This combination (0x3) is invalid for actual filtering (you can't do both byte shuffle and bit shuffle at the same time) and serves as a marker for the extended header.

When this marker is present, the actual shuffle settings should be read from the `filters` array in the extended header (bytes 16-21).

- `filters[i] == 1` (BLOSC_SHUFFLE) -> Do byte shuffle.
- `filters[i] == 2` (BLOSC_BITSHUFFLE) -> Do bit shuffle.

The `decompress` function in `src/internal/mod.rs` has been updated to handle this: if both flags are set in the header, it ignores them and checks the `filters` array instead.





## blosclz.c Analysis

This file implements the BloscLZ codec, which is heavily based on FastLZ.

### Macros

*   `BLOSCLZ_LIKELY(c)`, `BLOSCLZ_UNLIKELY(c)`: Compiler hints for branch prediction.
*   `MAX_COPY`: `32U`. The maximum number of literals that can be encoded in a single run before a control byte reset is needed.
*   `MAX_DISTANCE`: `8191`. The maximum backward distance for a standard match.
*   `MAX_FARDISTANCE`: `(65535 + MAX_DISTANCE - 1)`. The maximum backward distance for a "far" match.
*   `BLOSCLZ_READU16(p)`, `BLOSCLZ_READU32(p)`: Reads 16-bit or 32-bit unsigned integers from memory. Handles alignment if `BLOSC_STRICT_ALIGN` is defined.
*   `HASH_LOG`: `14U`. The log base 2 of the hash table size (16384 entries).
*   `HASH_FUNCTION(v, s, h)`: Computes a hash value `v` from a 32-bit sequence `s` and hash log `h`. Uses a multiplicative hash constant `2654435761U`.
*   `LITERAL(ip, op, op_limit, anchor, copy)`: Encodes a literal byte.
    *   Checks output bounds.
    *   Copies byte from `anchor` to `op`.
    *   Updates `ip` to `anchor`.
    *   Increments `copy` counter.
    *   If `copy` reaches `MAX_COPY`, resets it and emits a new token.
*   `LITERAL2(ip, anchor, copy)`: A simplified version of `LITERAL` used in `get_cratio` or when skipping matches. It updates pointers and counters but doesn't write to output (or writes minimally).
*   `MATCH_SHORT(op, op_limit, len, distance)`: Encodes a short match (len < 7) with a standard distance.
    *   Format: 2 bytes.
    *   Byte 1: `(len << 5) + (distance >> 8)`
    *   Byte 2: `distance & 255`
*   `MATCH_LONG(op, op_limit, len, distance)`: Encodes a long match (len >= 7) with a standard distance.
    *   Format: Variable length.
    *   Byte 1: `(7 << 5) + (distance >> 8)`
    *   Sequence of `255` for every 255 bytes of length.
    *   Byte N: Remaining length.
    *   Byte N+1: `distance & 255`
*   `MATCH_SHORT_FAR(op, op_limit, len, distance)`: Encodes a short match with a far distance.
    *   Format: 4 bytes.
    *   Byte 1: `(len << 5) + 31` (31 indicates far distance)
    *   Byte 2: `255`
    *   Byte 3: `distance >> 8`
    *   Byte 4: `distance & 255`
*   `MATCH_LONG_FAR(op, op_limit, len, distance)`: Encodes a long match with a far distance.
    *   Similar to `MATCH_LONG` but with the far distance marker.
*   `ALIGNED_(x)`, `ALIGNED_TYPE_(t, x)`: Helper macros for defining aligned variables (used for AVX2 optimization).

### Functions

*   `get_run_32(ip, ip_bound, ref)` (AVX2):
    *   Finds the length of a run of identical bytes starting at `ip` compared to `ref` (which is `ip - 1`).
    *   Uses AVX2 `_mm256_cmpeq_epi64` to compare 32 bytes at a time.
*   `get_run_16(ip, ip_bound, ref)` (SSE2):
    *   SSE2 version of `get_run`, compares 16 bytes at a time.
*   `get_run(ip, ip_bound, ref)`:
    *   Scalar version of `get_run`.
    *   Compares 8 bytes at a time using `int64_t` casts (or `memcpy` for strict alignment).
*   `get_match(ip, ip_bound, ref)`:
    *   Finds the length of a match between sequence at `ip` and sequence at `ref`.
    *   Compares 8 bytes at a time.
*   `get_match_16(ip, ip_bound, ref)` (SSE2):
    *   SSE2 version of `get_match`, compares 16 bytes at a time.
*   `get_match_32(ip, ip_bound, ref)` (AVX2):
    *   AVX2 version of `get_match`, compares 32 bytes at a time.
*   `get_run_or_match(ip, ip_bound, ref, run)`:
    *   Dispatcher function.
    *   If `run` is true (distance is 0/1), calls `get_run` variants.
    *   Otherwise calls `get_match` variants.
    *   Selects AVX2, SSE2, or scalar implementation based on compilation flags.
*   `get_cratio(ibase, maxlen, minlen, ipshift, htab, hashlog)`:
    *   Estimates compression ratio by performing a simplified compression pass on a subset of the data.
    *   Used for "entropy probing" to decide if full compression should be attempted.
    *   Returns `compressed_size / original_size`.
*   `blosclz_compress(clevel, input, length, output, maxout, ctx)`:
    *   Main compression function.
    *   **Entropy Probing**: Calls `get_cratio` to check if data is compressible. If not, returns 0 (not compressible).
    *   **Initialization**: Sets up hash table `htab`.
    *   **Main Loop**:
        *   Reads 4 bytes at `ip`.
        *   Computes hash and looks up `ref` in `htab`.
        *   Updates `htab` with current position.
        *   Checks if `ref` matches `ip`.
        *   **Match Found**:
            *   Calls `get_run_or_match` to find match length.
            *   Encodes match using `MATCH_*` macros.
        *   **No Match**:
            *   Encodes literal using `LITERAL` macro.
    *   **Finalization**: Handles remaining bytes as literals.
    *   Returns compressed size.
*   `copy_match_16(op, match, len)` (AVX2):
    *   Optimized memory copy for matches using AVX2.
    *   Uses `_mm_shuffle_epi8` with a mask table to handle overlapping copies (where `op` and `match` are close).
*   `wild_copy(out, from, end)`:
    *   Fast copy that writes 8 bytes at a time.
    *   May overwrite up to 7 bytes beyond `end`, which is safe due to buffer padding requirements.
*   `blosclz_decompress(input, length, output, maxout)`:
    *   Main decompression function.
    *   Reads control token `ctrl`.
    *   **Main Loop**:
        *   **Match**: If `ctrl >= 32`.
            *   Decodes length and distance.
            *   Handles long matches (reading more bytes if code is 255).
            *   Copies data from `ref` (output - distance) to `op`.
            *   Uses `wild_copy` or `copy_match_16` (AVX2) or `memset` (for runs).
        *   **Literal**: If `ctrl < 32`.
            *   `ctrl` is the length of the literal run.
            *   Copies `ctrl` bytes from input to output using `memcpy` (or `fastcopy`).
    *   Returns decompressed size.