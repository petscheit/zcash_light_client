from starkware.cairo.common.cairo_builtins import BitwiseBuiltin
from starkware.cairo.common.uint256 import Uint256, uint256_mul
from starkware.cairo.common.math import unsigned_div_rem
from starkware.cairo.common.math_cmp import is_le
from starkware.cairo.common.bitwise import bitwise_and

const N_BITS_OFFSET = 26;

// Flip endianness of a 32-bit word.
func flip_u32_endian{range_check_ptr}(x: felt) -> (y: felt) {
    // Decompose into bytes (little-endian in b0..b3).
    let (q0, b0) = unsigned_div_rem(x, 256);
    let (q1, b1) = unsigned_div_rem(q0, 256);
    let (q2, b2) = unsigned_div_rem(q1, 256);
    let (_,  b3) = unsigned_div_rem(q2, 256);

    // Reassemble with reversed byte order.
    let res = b0 * 16777216 + b1 * 65536 + b2 * 256 + b3;
    return (res,);
}

// Returns the nBits field from the Zcash block header, as a little-endian u32.
// Assumes header_pow is an array of 32-bit chunks in big-endian order.
func get_nbits{range_check_ptr}(header_pow: felt*) -> (nbits: felt) {
    let raw_nbits = header_pow[N_BITS_OFFSET];
    let (nbits) = flip_u32_endian(raw_nbits);
    return (nbits,);
}

// Computes 256^exp for small exp.
func pow256{range_check_ptr}(exp: felt) -> (res: felt) {
    if (exp == 0) {
        return (1,);
    }
    let (p) = pow256(exp - 1);
    return (p * 256,);
}

// Converts compact nBits value to a 256-bit target.
// See https://github.com/zcash/librustzcash/blob/master/zcash_crypto/src/difficulty/target.rs
func target_from_nbits{range_check_ptr, bitwise_ptr: BitwiseBuiltin*}(nbits: felt) -> (target: Uint256) {
    alloc_locals;
    
    // mant = nbits & 0x007fffff
    let (mant) = bitwise_and(nbits, 0x7fffff);
    
    // exp = nbits >> 24
    let (exp, _) = unsigned_div_rem(nbits, 0x1000000); // 2^24 = 16777216

    if (mant == 0) {
        return (Uint256(0, 0),);
    }
    
    // Case 1: exp < 3 => shift_bytes < 0.
    // s = -3
    if (exp == 0) { 
         let (res, _) = unsigned_div_rem(mant, 16777216); // 256^3
         return (Uint256(res, 0),);
    }
    // s = -2
    if (exp == 1) { 
         let (res, _) = unsigned_div_rem(mant, 65536); // 256^2
         return (Uint256(res, 0),);
    }
    // s = -1
    if (exp == 2) { 
         let (res, _) = unsigned_div_rem(mant, 256); // 256^1
         return (Uint256(res, 0),);
    }
    
    // Case 2: exp >= 3 => shift_bytes >= 0.
    let s = exp - 3;
    
    // If s >= 32, result is 0 (truncated to 256 bits).
    // Check if 32 <= s
    let s_ge_32 = is_le(32, s);
    if (s_ge_32 == 1) {
         return (Uint256(0, 0),);
    }
    
    let mant_uint = Uint256(mant, 0);
    
    // Construct factor = 256^s.
    // If s < 16: low=256^s, high=0
    // If s >= 16: low=0, high=256^(s-16)
    
    // Check if 16 <= s
    let s_ge_16 = is_le(16, s);
    
    if (s_ge_16 == 1) {
        let (h) = pow256(s - 16);
        let factor_uint = Uint256(0, h);
        let (res_low, _) = uint256_mul(mant_uint, factor_uint);
        return (res_low,);
    } else {
        let (l) = pow256(s);
        let factor_uint = Uint256(l, 0);
        let (res_low, _) = uint256_mul(mant_uint, factor_uint);
        return (res_low,);
    }
}
