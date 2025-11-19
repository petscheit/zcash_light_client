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

from cairo.src.constants import Parameters
from cairo.src.hashing import generate_hash, compute_leaf_hash
from cairo.src.debug import info_felt_hex, info_uint256
from cairo.src.merkle import EquihashTree
from cairo.src.difficulty import get_nbits, target_from_nbits

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

    let (solution_indicies: felt*) = alloc(); // as u32
    let (header_bytes: felt*) = alloc(); // as 32 bit chunks

    %{ WRITE_INPUTS %}

    let indices_len = 512; // for (n=200, k=9)

    let (nbits) = get_nbits(header_bytes);
    info_felt_hex(nbits);
    let (target) = target_from_nbits(nbits);
    info_uint256(target);

    let (root) = EquihashTree.tree_validator(
        header_pow=header_bytes,
        indices_ptr=solution_indicies,
        indices_len=indices_len,
    );
    let (ok) = EquihashTree.node_is_zero(root, Parameters.collision_byte_length);

    assert ok = 1;

    return();
}