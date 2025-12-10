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

The Rust implementation in `src/filters/mod.rs` correctly implements these operations.

## Decompression

## Extended Header Flags

In Blosc2, if the header is an extended header (32 bytes), the `flags` byte (byte 2) may have both `BLOSC_DOSHUFFLE` (0x1) and `BLOSC_DOBITSHUFFLE` (0x2) set. This combination (0x3) is invalid for actual filtering (you can't do both byte shuffle and bit shuffle at the same time) and serves as a marker for the extended header.

When this marker is present, the actual shuffle settings should be read from the `filters` array in the extended header (bytes 16-21).

- `filters[i] == 1` (BLOSC_SHUFFLE) -> Do byte shuffle.
- `filters[i] == 2` (BLOSC_BITSHUFFLE) -> Do bit shuffle.

The `decompress` function in `src/internal/mod.rs` has been updated to handle this: if both flags are set in the header, it ignores them and checks the `filters` array instead.

## Key Bug Fixes

### BLOSClz Match Finding (December 2025)

**Location**: `src/codecs/blosclz.rs` lines 150-190

**Problem**: Compression ratio for bitshuffled data was only 7.90:1 instead of expected 36.6:1

**Root Causes**:
1. **Incorrect ref_ptr bound check**: The condition `ref_ptr < ip_limit` was preventing long matches from being found
2. **Double ip advancement**: After finding a match, `ip` was being advanced twice - once inside the match-finding block and once after

**Fix**:
- Removed the `ref_ptr < ip_limit` check since ref_ptr is always behind temp_ip (distance > 0)
- Set `ip = ip + len - ipshift` inside the match-finding block
- Removed the redundant `ip += len` after match found
- Set encoded length correctly: `len = (len_c - 2)` (already biased for encoding)

**Status**: Partially fixed - compression improved from 7.90:1 to 9.10:1, but still not matching C implementation (36.6:1). The C version achieves 2188 bytes compressed vs our 8828 bytes. Additional issues remain:
- Hash table updates at match boundaries may be incorrect
- Entropy probing not implemented (C uses `get_cratio()` to estimate compression ratio)
- Match extension logic may differ in subtle ways

**Next Steps**:
- Review hash table update logic more carefully against C implementation
- Consider implementing entropy probing
- Add more detailed debug output to compare match lengths and positions with C implementation