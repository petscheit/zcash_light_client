from starkware.cairo.common.alloc import alloc
from starkware.cairo.common.cairo_builtins import BitwiseBuiltin

from cairo.src.constants import Parameters
from cairo.src.utils import expand_array, bitwise_divmod



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