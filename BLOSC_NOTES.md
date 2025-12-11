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
- In `blusc`, ensuring `split_block` returns `false` for `BITSHUFFLE` improved compression from ~6200 bytes to ~3329 bytes in `test_roundtrip_large_bitshuffle`.

### BLOSClz Compression

Reference: `c-blosc2/blosc/blosclz.c` lines 422-650

The BLOSClz compressor uses several key optimizations:

1. **ipshift and minlen**: Set to 4 for optimal compression with bitshuffle and small typesizes
   - `ipshift = 4`: Shifts back in the match by 4 bytes to find longer runs
   - `minlen = 4`: Minimum match length to encode

2. **Match Finding**: Uses hash table to find potential matches
   - Hash function: `(seq * 2654435761) >> (32 - hashlog)`
   - Matches must be at least 4 bytes initially
   - **Level 9 Optimization**: For `clevel=9`, a second hash update is performed at the match boundary using `seq >> 8`. This helps find more matches in highly compressible data.

3. **Match Length Calculation**:
   - C: `ip = get_run_or_match(ip, ip_bound, ref, !distance)` returns pointer AFTER last match byte
   - C: `ip -= ipshift` to bias the position
   - C: `len = ip - anchor` gives biased match length

4. **Match Acceptance Criteria**:
   - Reject if `len < minlen` OR (`len <= 5` AND `distance >= MAX_DISTANCE`)
   - Accept if `len >= minlen` AND (`len > 5` OR `distance < MAX_DISTANCE`)

### Bitshuffle Filter

The `bitshuffle` implementation in `blusc` is currently monolithic (processes the entire buffer at once).
- `c-blosc2` uses a blocked implementation (typically 8KB blocks) for bitshuffle.
- Attempting to implement 8KB blocking in `blusc` (without other changes) resulted in significantly worse compression (~17KB), likely due to boundary effects or incorrect implementation details.
- The current monolithic implementation + `split_block=false` yields ~3329 bytes for the large bitshuffle test.
- The target (C-Blosc2) is ~2188 bytes.
- The remaining discrepancy (~1100 bytes) suggests that the specific data pattern produced by C-Blosc2's blocked bitshuffle is slightly more compressible by `blosclz` than the monolithic output, OR there are subtle differences in how `blosclz` handles the specific patterns generated.

### Compression Ratio Mismatch

- `test_roundtrip_csv_case`: `blusc` (3064) is smaller than `bound` (4444). `blusc` is better.
- `test_roundtrip_large_bitshuffle`: `blusc` (3329) is larger than `bound` (2188). `blusc` is approaching the target but still ~50% larger.

### C-Blosc2 Bitshuffle Implementation Details

Based on analysis of `c-blosc2` source code (specifically `blosc/blosc2.c`, `blosc/stune.c`, and `blosc/bitshuffle-avx2.c`):

1.  **Blosc Block Splitting**:
    - `blosc` splits the input buffer into blocks. The block size is determined by `blosc_stune_next_blocksize` (or `compute_blocksize` in Blosc 1.x).
    - The default block size logic relies on `L1` cache size (defined as 32KB in `stune.h`).
    - For `clevel=1`, block size is `L1 / 2` = 16KB.
    - For `clevel=0` (memcpy), block size is `L1 / 4` = 8KB.
    - For `clevel>=2`, block size is `L1` (32KB) or larger (up to 256KB for `clevel=9`).
    - **Note**: I did not find explicit logic that forces 8KB blocks specifically for `BLOSC_BITSHUFFLE` in `c-blosc2`. If 8KB blocking is observed, it might be due to specific configuration or `clevel`.

2.  **Bitshuffle Application**:
    - Bitshuffle is applied as a filter in `pipeline_forward` (in `blosc2.c`).
    - It operates on the **entire blosc block** (`bsize`).
    - `pipeline_forward` calls `blosc2_bitshuffle`, which dispatches to the hardware-accelerated implementation (e.g., `bshuf_trans_bit_elem_AVX`).

3.  **Bitshuffle Internal Logic**:
    - The AVX2 implementation (`bshuf_trans_bit_elem_AVX` in `bitshuffle-avx2.c`) allocates a temporary buffer of size `size * elem_size` (which matches the blosc block size).
    - It performs three passes:
        1.  `bshuf_trans_byte_elem_sse2`: Transposes bytes within elements.
        2.  `bshuf_trans_bit_byte_AVX`: Transposes bits within bytes (using 32-byte AVX registers).
        3.  `bshuf_trans_bitrow_eight`: Transposes bit rows.
    - **Blocking**: The implementation does *not* appear to have an internal blocking loop (e.g., processing 8KB chunks) within these functions. It processes the full buffer passed to it.
    - Therefore, the "blocking" behavior is primarily controlled by the **blosc block size**.

4.  **Implication for Rust Port**:
    - To match `c-blosc2` behavior, `blusc` should ensure that the bitshuffle filter is applied to the whole block *before* any potential stream splitting (though bitshuffle usually disables stream splitting in `blosc`).
    - If `c-blosc2` is achieving better compression with what looks like "8KB blocking", it might be that `c-blosc2` is choosing a smaller block size (e.g. 16KB or 32KB) than `blusc` is currently using, or `blusc`'s bitshuffle implementation has subtle differences in the bit manipulation order compared to `c-blosc2`.
