// Corresponds to c-blosc2/include/blosc2/codecs-registry.h

// Simple Lempel-Ziv compressor for NDim data. Experimental, mainly for teaching purposes.
pub const BLOSC_CODEC_NDLZ: u32 = 32;

// ZFP compressor for fixed accuracy mode. The desired accuracy is set in `compcode_meta`.
// See https://github.com/Blosc/c-blosc2/blob/main/plugins/codecs/zfp/README.md
pub const BLOSC_CODEC_ZFP_FIXED_ACCURACY: u32 = 33;

// ZFP compressor for fixed precision. The desired precision is set in `compcode_meta`.
// See https://github.com/Blosc/c-blosc2/blob/main/plugins/codecs/zfp/README.md
pub const BLOSC_CODEC_ZFP_FIXED_PRECISION: u32 = 34;
// ZFP compressor for fixed precision. The desired rate is set in `compcode_meta`.
// See https://github.com/Blosc/c-blosc2/blob/main/plugins/codecs/zfp/README.md
pub const BLOSC_CODEC_ZFP_FIXED_RATE: u32 = 35;

// OpenHTJ2K compressor for JPEG 2000 HT.
// See https://github.com/Blosc/blosc2_openhtj2k
pub const BLOSC_CODEC_OPENHTJ2K: u32 = 36;

// Grok compressor for JPEG 2000.
// See https://github.com/Blosc/blosc2_grok
pub const BLOSC_CODEC_GROK: u32 = 37;
