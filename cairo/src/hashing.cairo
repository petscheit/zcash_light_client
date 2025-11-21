from starkware.cairo.common.alloc import alloc
from starkware.cairo.common.cairo_builtins import BitwiseBuiltin
from starkware.cairo.common.uint256 import uint256_reverse_endian, Uint256

from cairo.src.constants import Parameters
from cairo.src.utils import expand_array, construct_header_chunks
from cairo.src.bitwise_utils import bitwise_divmod, pow2alloc128
from cairo.src.sha import SHA256, HashUtils
from cairo.src.debug import info_segment_hex, info_string, info_felt_hex, info_uint256


func generate_hash(header_pow: felt*, index: felt) -> (felt*) {
    alloc_locals;

    let (hash_bytes: felt*) = alloc();

    %{ CREATE_BLAKE2B_HASH %}

    return (hash_bytes,);
}

// Compute the Equihash leaf hash for a given solution index.
// Returns a pointer to 30 bytes (10 × 20-bit chunks) matching Rust's Node::new.
func compute_leaf_hash{range_check_ptr, bitwise_ptr: BitwiseBuiltin*}(
    header_pow: felt*, index: felt
) -> felt* {
    alloc_locals;

    // Split into group index (which BLAKE2b digest) and position within that group.
    let (group_index, inner) = bitwise_divmod(
        index, Parameters.indicies_per_hash_output
    );

    // Each digest corresponds to two indices; generate its 50-byte output.
    let (digest_bytes: felt*) = generate_hash(header_pow, group_index);

    // Select the 25-byte slice for this index (first or second half of the digest).
    let (slice_bytes: felt*) = select_digest_slice(digest_bytes, inner);

    // Expand 25-byte slice into 30-byte leaf hash (10 × 20-bit chunks).
    let (leaf_hash: felt*) = expand_array(
        slice_bytes, Parameters.collision_bit_length, 0
    );

    return leaf_hash;
}

func select_digest_slice{range_check_ptr, bitwise_ptr: BitwiseBuiltin*}(
    digest_bytes: felt*, inner: felt
) -> (felt*) {
    alloc_locals;

    let offset = inner * Parameters.digest_slice_bytes;
    let slice_ptr = digest_bytes + offset;
    return (slice_ptr,);
}


/// double sha header bytes here please
func hash_header{range_check_ptr, bitwise_ptr: BitwiseBuiltin*, sha256_ptr: felt*}(
    header_pow: felt*, solution: felt*
) -> (hash: Uint256) {
    alloc_locals;

    let (pow2_array) = pow2alloc128();

    // 1. Construct the header chunks (header + solution length + solution)
    // Total length: 140 bytes (header) + 3 bytes (len) + 1344 bytes (solution) = 1487 bytes
    let (header_chunks) = construct_header_chunks(header_pow, solution);

    // 2. First SHA256 hash (1487 bytes)
    let (hash1) = SHA256.hash_bytes{pow2_array=pow2_array}(header_chunks, 1487);

    // 3. Second SHA256 hash (32 bytes)
    let (hash2) = SHA256.hash_bytes{pow2_array=pow2_array}(hash1, 32);

    // 4. Convert to Uint256
    let hash_uint256 = HashUtils.chunks_to_uint256{pow2_array=pow2_array}(hash2);
    
    // 5. Convert to little endian
    let (hash_le) = uint256_reverse_endian(hash_uint256);

    return (hash_le,);
}
