
#[cfg(test)]
mod tests {
    use crate::filters::{bitshuffle, bitunshuffle};

    #[test]
    fn test_bitshuffle_roundtrip() {
        let size = 128;
        let typesize = 4;
        let blocksize = size * typesize;
        let mut src = vec![0u8; blocksize];
        for i in 0..blocksize {
            src[i] = (i % 255) as u8;
        }
        let mut dest = vec![0u8; blocksize];
        let mut recovered = vec![0u8; blocksize];

        bitshuffle(typesize, blocksize, &src, &mut dest).unwrap();
        bitunshuffle(typesize, blocksize, &dest, &mut recovered).unwrap();

        assert_eq!(src, recovered);
    }
}
