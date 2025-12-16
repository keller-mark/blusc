
pub mod api;
pub mod blosc;
pub mod include;

pub use api::*;
pub use crate::include::blosc2_include::*;

pub mod codecs {
    pub mod blosclz {
        use crate::blosc::blosclz;
        use crate::blosc::context::Blosc2Context;

        pub fn compress(clevel: i32, input: &[u8], output: &mut [u8]) -> usize {
            // Create a dummy context since it is unused in blosclz_compress
            let ctx = unsafe { std::mem::zeroed::<Blosc2Context>() };
            let maxout = output.len();
            let res = blosclz::blosclz_compress(clevel, input, output, maxout, &ctx);
            if res < 0 {
                0
            } else {
                res as usize
            }
        }

        pub fn decompress(input: &[u8], output: &mut [u8]) -> usize {
             let length = input.len();
             let maxout = output.len();
             let res = blosclz::blosclz_decompress(input, length, output, maxout);
             if res < 0 {
                 0
             } else {
                 res as usize
             }
        }
    }
}

pub mod filters {
    use crate::blosc::shuffle;

    pub fn bitshuffle(typesize: usize, blocksize: usize, src: &[u8], dest: &mut [u8]) -> Result<usize, i32> {
        let res = shuffle::blosc2_bitshuffle(typesize as i32, blocksize as i32, src, dest);
        if res < 0 {
            Err(res)
        } else {
            Ok(res as usize)
        }
    }

    pub fn bitunshuffle(typesize: usize, blocksize: usize, src: &[u8], dest: &mut [u8]) -> Result<usize, i32> {
        let res = shuffle::blosc2_bitunshuffle(typesize as i32, blocksize as i32, src, dest);
        if res < 0 {
            Err(res)
        } else {
            Ok(res as usize)
        }
    }
}