%builtins output pedersen range_check ecdsa bitwise ec_op keccak poseidon range_check96 add_mod mul_mod
from starkware.cairo.common.cairo_builtins import (
    BitwiseBuiltin,
    KeccakBuiltin,
    PoseidonBuiltin,
    HashBuiltin,
    ModBuiltin,
)
from starkware.cairo.common.cairo_keccak.keccak import finalize_keccak
from starkware.cairo.common.alloc import alloc
from starkware.cairo.common.math_cmp import is_le

from cairo.src.constants import Parameters
from cairo.src.hashing import generate_hash, compute_leaf_hash, hash_header
from cairo.src.debug import info_felt_hex, info_uint256
from cairo.src.utils import indices_from_minimal
from cairo.src.sha import SHA256
from cairo.src.equihash import EquihashTree
from cairo.src.difficulty import get_nbits, target_from_nbits, verify_difficulty_filter

func main{
    output_ptr: felt*,
    pedersen_ptr: HashBuiltin*,
    range_check_ptr,
    ecdsa_ptr: felt*,
    bitwise_ptr: BitwiseBuiltin*,
    ec_op_ptr: felt*,
    keccak_ptr: felt*,
    poseidon_ptr: PoseidonBuiltin*,
    range_check96_ptr: felt*,
    add_mod_ptr: ModBuiltin*,
    mul_mod_ptr: ModBuiltin*,
}() {
    alloc_locals;
    
    let (sha256_ptr, sha256_ptr_start) = SHA256.init();

    let (header_bytes: felt*) = alloc(); // as 32 bit chunks
    let (solution_bytes: felt*) = alloc(); // minimal solution bytes
    
    %{ WRITE_INPUTS %}
    
    let (nbits) = get_nbits(header_bytes);
    let (target) = target_from_nbits(nbits);
    
    with sha256_ptr {
        let (hash) = hash_header(header_bytes, solution_bytes);
    }

    verify_difficulty_filter(hash, target);

    let (indices_ptr, indices_len) = indices_from_minimal(solution_bytes);

    let (root) = EquihashTree.tree_validator(
        header_pow=header_bytes,
        indices_ptr=indices_ptr,
        indices_len=indices_len,
    );
    
    let (ok) = EquihashTree.node_is_zero(root, Parameters.collision_byte_length);
    assert ok = 1;

    SHA256.finalize(sha256_start_ptr=sha256_ptr_start, sha256_end_ptr=sha256_ptr);


    return();
}