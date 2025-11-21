
from starkware.cairo.common.alloc import alloc
from starkware.cairo.common.math import unsigned_div_rem
from starkware.cairo.common.cairo_builtins import BitwiseBuiltin
from cairo.src.constants import Parameters
from starkware.cairo.common.registers import get_fp_and_pc, get_label_location
from cairo.src.debug import info_felt_hex, info_string
from cairo.src.bitwise_utils import extract_bit_from_byte, shift_chunk_and_add_bit

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

func construct_header_chunks{range_check_ptr}(header_pow: felt*, solution: felt*) -> (header_chunks: felt*) {
    alloc_locals;

    // The header_pow array is expected to be large enough to hold the result (35 + 1 + 336 words)
    // We start writing at index 35.
    // The structure is: [header (140B)] [len (3B)] [solution (1344B)]
    // Word 35 will contain: len (3B) || solution[0].byte0 (1B)
    
    // 0xfd4005 is the CompactSize encoding for 1344 bytes (0x0540) -> fd 40 05

    let (sol_byte0, _) = unsigned_div_rem([solution], 16777216); // 2^24
    assert [header_pow + 35] = 0xfd4005 * 256 + sol_byte0;

    construct_header_chunks_inner(header_pow, solution, 0);

    return (header_pow,);
}

func construct_header_chunks_inner{range_check_ptr}(header_pow: felt*, solution: felt*, index: felt) -> (header_chunks: felt*) {
    alloc_locals;

    if (index == 335) {
        // Last chunk (index 335)
        // We take the remaining 3 bytes of solution[335] and shift them to the left (padding with 0 at the end)
        let (_, rem) = unsigned_div_rem([solution + index], 16777216); // 2^24
        assert [header_pow + 36 + index] = rem;
        return (header_pow,);
    }

    // Current chunk: take lower 3 bytes
    let (_, rem) = unsigned_div_rem([solution + index], 16777216); // 2^24
    
    // Next chunk: take top byte
    let (next_byte, _) = unsigned_div_rem([solution + index + 1], 16777216); // 2^24

    // Combine: (rem << 8) | next_byte
    assert [header_pow + 36 + index] = rem * 256 + next_byte;
    // let value = rem * 256 + next_byte;
    // info_felt_hex(value);

    return construct_header_chunks_inner(header_pow, solution, index + 1);
}

// Unpack big-endian u32 solution words into a contiguous byte array.
func unpack_solution_words_to_bytes{range_check_ptr}(
    solution_words: felt*, num_words: felt
) -> (bytes_ptr: felt*) {
    alloc_locals;

    let (bytes_ptr: felt*) = alloc();

    unpack_solution_words_to_bytes_inner(
        solution_words, num_words, bytes_ptr, 0, 0
    );

    return (bytes_ptr,);
}

func unpack_solution_words_to_bytes_inner{range_check_ptr}(
    solution_words: felt*, num_words: felt,
    bytes_ptr: felt*,
    word_idx: felt, byte_offset: felt,
) {
    if (word_idx == num_words) {
        return ();
    }

    let word = [solution_words + word_idx];

    // Split u32 word into 4 big-endian bytes.
    let (b0, rem0) = unsigned_div_rem(word, 16777216);  // 2^24
    let (b1, rem1) = unsigned_div_rem(rem0, 65536);     // 2^16
    let (b2, b3) = unsigned_div_rem(rem1, 256);         // 2^8

    assert [bytes_ptr + byte_offset] = b0;
    assert [bytes_ptr + byte_offset + 1] = b1;
    assert [bytes_ptr + byte_offset + 2] = b2;
    assert [bytes_ptr + byte_offset + 3] = b3;

    let next_word_idx = word_idx + 1;
    let next_byte_offset = byte_offset + 4;
    return unpack_solution_words_to_bytes_inner(
        solution_words, num_words, bytes_ptr, next_word_idx, next_byte_offset
    );
}

// Read one Equihash index (digit_bit_length bits) from a big-endian byte stream.
func read_index_from_bytes{range_check_ptr, bitwise_ptr: BitwiseBuiltin*}(
    bytes_ptr: felt*,
    byte_idx: felt, bit_idx: felt,
    bit_count: felt, acc: felt,
) -> (new_byte_idx: felt, new_bit_idx: felt, value: felt) {
    if (bit_count == Parameters.digit_bit_length) {
        return (byte_idx, bit_idx, acc);
    }

    let byte = [bytes_ptr + byte_idx];
    let (bit) = extract_bit_from_byte(byte, bit_idx);

    let new_acc = acc + acc + bit;
    let next_bit_count = bit_count + 1;

    if (bit_idx == 7) {
        let next_bit_idx = 0;
        let next_byte_idx = byte_idx + 1;
        return read_index_from_bytes(
            bytes_ptr,
            next_byte_idx, next_bit_idx,
            next_bit_count, new_acc,
        );
    } else {
        let next_bit_idx = bit_idx + 1;
        let next_byte_idx = byte_idx;
        return read_index_from_bytes(
            bytes_ptr,
            next_byte_idx, next_bit_idx,
            next_bit_count, new_acc,
        );
    }
}

func indices_from_minimal_inner{range_check_ptr, bitwise_ptr: BitwiseBuiltin*}(
    bytes_ptr: felt*,
    indices_ptr: felt*,
    idx: felt,
    byte_idx: felt,
    bit_idx: felt,
) -> (final_byte_idx: felt, final_bit_idx: felt) {
    if (idx == Parameters.num_indices) {
        return (byte_idx, bit_idx);
    }

    let (next_byte_idx, next_bit_idx, value) = read_index_from_bytes(
        bytes_ptr,
        byte_idx, bit_idx,
        0, 0,
    );

    assert [indices_ptr + idx] = value;

    let next_idx = idx + 1;
    return indices_from_minimal_inner(
        bytes_ptr,
        indices_ptr,
        next_idx,
        next_byte_idx,
        next_bit_idx,
    );
}

// Decode minimal Equihash solution bytes into big-endian u32 indices.
func indices_from_minimal{range_check_ptr, bitwise_ptr: BitwiseBuiltin*}(
    solution_words: felt*
) -> (indices_ptr: felt*, indices_len: felt) {
    alloc_locals;

    // Minimal solution is 1344 bytes = 336 u32 words.
    let num_words = 336;
    let (minimal_bytes_ptr: felt*) = unpack_solution_words_to_bytes(
        solution_words, num_words
    );

    let (indices_ptr: felt*) = alloc();
    let indices_len = Parameters.num_indices;

    let (final_byte_idx, final_bit_idx) = indices_from_minimal_inner(
        minimal_bytes_ptr,
        indices_ptr,
        0,
        0,
        0,
    );

    // All bits should be consumed exactly.
    assert final_byte_idx = Parameters.minimal_solution_bytes;
    assert final_bit_idx = 0;

    return (indices_ptr, indices_len);
}
