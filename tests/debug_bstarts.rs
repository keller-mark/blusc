/// Debug test to understand bstarts layout from C
#[test]
fn debug_c_bstarts() {
    // From the test output, C compressed 85 bytes to 117 bytes
    // Header shows: blocksize=85, cbytes=117, so we have 1 block
    // nblocks = (85 + 85 - 1) / 85 = 1
    // bstarts should have 1 entry = 4 bytes
    
    // Bytes 16-19 are: [00, 00, 00, 00] = 0 ???
    // That can't be right - bstart can't be 0 (that's the header)
    
    // Wait, let me re-read the format:
    // Header is 16 bytes (no extended header since flags=0x01)
    // Then comes bstarts
    // nblocks = 1, so bstarts is 4 bytes
    // bstart points to where the block stream starts
    // 
    // So layout is:
    // 0-15: header
    // 16-19: bstart[0] = offset to first block
    // 20+: first block data
    
    // If bstart[0] = 0x00000000, that would mean block starts at byte 0???
    // That doesn't make sense.
    
    // Let me check if extended header is ALWAYS present in version 5...
    
    println!("Testing bstart interpretation");
    
    // According to the hex dump:
    // [00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00, 00]
    // This is 16 bytes of zeros at positions 16-31
    
    // If header is 16 bytes and we have 1 block:
    // - bstart[0] should be at byte 16, value should point to byte ~20
    // - But we see 0x00000000
    
    // Unless... in version 5, the header is ALWAYS 32 bytes?
    // That would explain bytes 16-31 being the extended header
    // And bstarts starting at byte 32
    
    println!("Hypothesis: Version 5 always uses 32-byte header");
}
