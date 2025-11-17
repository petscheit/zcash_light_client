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
from cairo.src.debug import info_felt_hex

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

    let result = compute_leaf_hash(header_bytes, 4755);

    let r0 = result[0];
    let r1 = result[1];
    let r2 = result[2];
    let r3 = result[3];
    let r4 = result[4];
    let r5 = result[5];
    let r6 = result[6];
    let r7 = result[7];
    let r8 = result[8];

    info_felt_hex(r0);
    info_felt_hex(r1);
    info_felt_hex(r2);
    info_felt_hex(r3);
    info_felt_hex(r4);
    info_felt_hex(r5);
    info_felt_hex(r6);
    info_felt_hex(r7);
    info_felt_hex(r8);


    



    return();
}