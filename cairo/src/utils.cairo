

from starkware.cairo.common.alloc import alloc
from starkware.cairo.common.math import unsigned_div_rem
from starkware.cairo.common.cairo_builtins import BitwiseBuiltin
from cairo.src.constants import Parameters


// Expand a 25-byte Equihash digest slice into 10 × 20-bit chunks (30 bytes).
func expand_array{range_check_ptr, bitwise_ptr: BitwiseBuiltin*}(
    vin: felt*, bit_len: felt, byte_pad: felt
) -> (vout: felt*) {
    alloc_locals;

    // Specialised for Equihash (n = 200, k = 9) leaf expansion.
    assert bit_len = Parameters.collision_bit_length;
    assert byte_pad = 0;

    let vin_len = Parameters.digest_slice_bytes;
    let out_width = Parameters.collision_byte_length;
    let out_len = Parameters.leaf_hash_bytes;
    let num_chunks = out_len / out_width;

    let (vout: felt*) = alloc();

    let byte_idx = 0;
    let bit_idx = 0;
    let (_, _) = expand_array_inner(
        vin, vin_len, vout, num_chunks, out_width, byte_idx, bit_idx, 0
    );

    return (vout,);
}

func expand_array_inner{range_check_ptr, bitwise_ptr: BitwiseBuiltin*}(
    vin: felt*, vin_len: felt,
    vout: felt*,
    num_chunks: felt, out_width: felt,
    byte_idx: felt, bit_idx: felt,
    chunk_idx: felt,
) -> (new_byte_idx: felt, new_bit_idx: felt) {
    if (chunk_idx == num_chunks) {
        return (byte_idx, bit_idx);
    }

    let base = chunk_idx * out_width;

    // Build one 3-byte chunk starting from zero state.
    let (b_idx1, bit_idx1, b0, b1, b2) = fill_chunk(
        vin, vin_len, byte_idx, bit_idx, 0, 0, 0, 0
    );

    // Write the final chunk bytes.
    assert [vout + base] = b0;
    assert [vout + base + 1] = b1;
    assert [vout + base + 2] = b2;

    let next_chunk_idx = chunk_idx + 1;
    return expand_array_inner(
        vin, vin_len, vout,
        num_chunks, out_width,
        b_idx1, bit_idx1, next_chunk_idx
    );
}

// Consume 20 bits into one 3-byte big-endian chunk.
func fill_chunk{range_check_ptr, bitwise_ptr: BitwiseBuiltin*}(
    vin: felt*, vin_len: felt,
    byte_idx: felt, bit_idx: felt,
    bit_count: felt,
    b0: felt, b1: felt, b2: felt,
) -> (new_byte_idx: felt, new_bit_idx: felt, out_b0: felt, out_b1: felt, out_b2: felt) {
    if (bit_count == Parameters.collision_bit_length) {
        return (byte_idx, bit_idx, b0, b1, b2);
    }

    // Read current bit from vin[byte_idx], big-endian within the byte.
    let byte = [vin + byte_idx];
    let (bit) = extract_bit_from_byte(byte, bit_idx);

    let (nb0, nb1, nb2) = shift_chunk_and_add_bit(b0, b1, b2, bit);

    let next_bit_count = bit_count + 1;
    let tmp_next_bit = bit_idx + 1;
    if (tmp_next_bit == 8) {
        let next_bit_idx = 0;
        let next_byte_idx = byte_idx + 1;
        return fill_chunk(
            vin, vin_len, next_byte_idx, next_bit_idx, next_bit_count, nb0, nb1, nb2
        );
    } else {
        let next_bit_idx = tmp_next_bit;
        let next_byte_idx = byte_idx;
        return fill_chunk(
            vin, vin_len, next_byte_idx, next_bit_idx, next_bit_count, nb0, nb1, nb2
        );
    }
}

func extract_bit_from_byte{range_check_ptr, bitwise_ptr: BitwiseBuiltin*}(
    byte: felt, bit_idx: felt
) -> (bit: felt) {
    if (bit_idx == 0) {
        let (bit) = extract_bit_div(byte, 128);
        return (bit,);
    }
    if (bit_idx == 1) {
        let (bit) = extract_bit_div(byte, 64);
        return (bit,);
    }
    if (bit_idx == 2) {
        let (bit) = extract_bit_div(byte, 32);
        return (bit,);
    }
    if (bit_idx == 3) {
        let (bit) = extract_bit_div(byte, 16);
        return (bit,);
    }
    if (bit_idx == 4) {
        let (bit) = extract_bit_div(byte, 8);
        return (bit,);
    }
    if (bit_idx == 5) {
        let (bit) = extract_bit_div(byte, 4);
        return (bit,);
    }
    if (bit_idx == 6) {
        let (bit) = extract_bit_div(byte, 2);
        return (bit,);
    }
    // bit_idx == 7
    let (bit) = extract_bit_div(byte, 1);
    return (bit,);
}

func extract_bit_div{range_check_ptr, bitwise_ptr: BitwiseBuiltin*}(
    byte: felt, divisor: felt
) -> (bit: felt) {
    let (q1, _) = unsigned_div_rem(byte, divisor);
    let (_, bit) = unsigned_div_rem(q1, 2);
    return (bit,);
}

func shift_chunk_and_add_bit{range_check_ptr, bitwise_ptr: BitwiseBuiltin*}(
    b0: felt, b1: felt, b2: felt, bit: felt
) -> (nb0: felt, nb1: felt, nb2: felt) {
    alloc_locals;
    let (nb2, carry2) = shift_byte(b2, bit);
    let (nb1, carry1) = shift_byte(b1, carry2);
    let (nb0, _) = shift_byte(b0, carry1);
    return (nb0, nb1, nb2);
}

func shift_byte{range_check_ptr, bitwise_ptr: BitwiseBuiltin*}(
    byte: felt, carry: felt
) -> (new_byte: felt, new_carry: felt) {
    let sum = byte + byte + carry;
    let (quot, rem) = unsigned_div_rem(sum, 256);
    let new_byte = rem;
    let new_carry = quot;
    return (new_byte, new_carry);
}

// Simple wrapper so hashing.cairo can import a div/mod helper.
func bitwise_divmod{range_check_ptr}(a: felt, b: felt) -> (q: felt, r: felt) {
    let (q, r) = unsigned_div_rem(a, b);
    return (q, r);
}