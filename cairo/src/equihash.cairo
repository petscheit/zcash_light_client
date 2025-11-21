from starkware.cairo.common.builtin_poseidon.poseidon import poseidon_hash
from starkware.cairo.common.cairo_builtins import PoseidonBuiltin, BitwiseBuiltin
from starkware.cairo.common.alloc import alloc
from starkware.cairo.common.memcpy import memcpy
from starkware.cairo.common.bitwise import bitwise_and
from starkware.cairo.common.math_cmp import is_le

from cairo.src.constants import Parameters
from cairo.src.hashing import compute_leaf_hash
from cairo.src.bitwise_utils import bitwise_divmod, pow2alloc128

// Node used for the Equihash merge tree. It mirrors the Rust `Node`:
// - `hash_ptr` points to a byte array (as felts) of length `hash_len`.
// - `indices_ptr` points to a u32 array (as felts) of length `indices_len`.
struct EquihashNode {
    hash_ptr: felt*,
    hash_len: felt,
    indices_ptr: felt*,
    indices_len: felt,
}

namespace EquihashTree {
    // Construct a leaf node: compute the Equihash leaf hash and attach a single index.
    func new_leaf{range_check_ptr, bitwise_ptr: BitwiseBuiltin*}(
        header_pow: felt*, index: felt
    ) -> (node: EquihashNode) {
        alloc_locals;

        let leaf_hash: felt* = compute_leaf_hash(header_pow, index);

        let (indices_ptr: felt*) = alloc();
        assert [indices_ptr] = index;

        let node = EquihashNode(
            hash_ptr=leaf_hash,
            hash_len=Parameters.leaf_hash_bytes,
            indices_ptr=indices_ptr,
            indices_len=1,
        );
        return (node,);
    }

    // Return 1 if `a.indices[0] < b.indices[0]`, 0 otherwise.
    func indices_before{range_check_ptr}(a: EquihashNode, b: EquihashNode) -> (res: felt) {
        let a_first = [a.indices_ptr];
        let b_first = [b.indices_ptr];

        let a_le_b = is_le(a_first, b_first);
        let b_le_a = is_le(b_first, a_first);

        let res = a_le_b * (1 - b_le_a);
        return (res,);
    }

    // Copy `src_len` indices from `src` into `dst` starting at `dst_offset`.
    func copy_indices_segment(
        src: felt*, src_len: felt,
        dst: felt*, dst_offset: felt,
        idx: felt,
    ) {
        if (idx == src_len) {
            return ();
        }

        assert [dst + dst_offset + idx] = [src + idx];

        let next_idx = idx + 1;
        return copy_indices_segment(
            src=src, src_len=src_len, dst=dst, dst_offset=dst_offset, idx=next_idx
        );
    }

    // XOR the suffix of length `parent_len` starting at offset `trim` in the children.
    func xor_suffix{range_check_ptr, bitwise_ptr: BitwiseBuiltin*}(
        a_hash: felt*, b_hash: felt*,
        trim: felt, parent_len: felt,
        out_ptr: felt*, out_idx: felt,
    ) {
        if (out_idx == parent_len) {
            return ();
        }

        let child_idx = trim + out_idx;
        let a_byte = [a_hash + child_idx];
        let b_byte = [b_hash + child_idx];

        let (and_ab) = bitwise_and(a_byte, b_byte);
        let double_and = and_ab + and_ab;
        let xor_byte = a_byte + b_byte - double_and;

        assert [out_ptr + out_idx] = xor_byte;

        let next_out_idx = out_idx + 1;
        return xor_suffix(
            a_hash=a_hash,
            b_hash=b_hash,
            trim=trim,
            parent_len=parent_len,
            out_ptr=out_ptr,
            out_idx=next_out_idx,
        );
    }

    // Combine siblings by XORing post-collision bytes and concatenating indices
    // with the lexicographically earlier subtree first.
    func from_children{range_check_ptr, bitwise_ptr: BitwiseBuiltin*}(
        a: EquihashNode, b: EquihashNode, trim: felt
    ) -> (node: EquihashNode) {
        alloc_locals;

        // Hash lengths must match.
        assert a.hash_len = b.hash_len;

        let child_len = a.hash_len;
        let parent_len = child_len - trim;

        let (parent_hash: felt*) = alloc();
        xor_suffix(
            a_hash=a.hash_ptr,
            b_hash=b.hash_ptr,
            trim=trim,
            parent_len=parent_len,
            out_ptr=parent_hash,
            out_idx=0,
        );

        let (new_indices: felt*) = alloc();
        let new_indices_len = a.indices_len + b.indices_len;

        let (a_before_b) = indices_before(a, b);
        if (a_before_b == 1) {
            copy_indices_segment(
                src=a.indices_ptr,
                src_len=a.indices_len,
                dst=new_indices,
                dst_offset=0,
                idx=0,
            );
            copy_indices_segment(
                src=b.indices_ptr,
                src_len=b.indices_len,
                dst=new_indices,
                dst_offset=a.indices_len,
                idx=0,
            );
        } else {
            copy_indices_segment(
                src=b.indices_ptr,
                src_len=b.indices_len,
                dst=new_indices,
                dst_offset=0,
                idx=0,
            );
            copy_indices_segment(
                src=a.indices_ptr,
                src_len=a.indices_len,
                dst=new_indices,
                dst_offset=b.indices_len,
                idx=0,
            );
        }

        let node = EquihashNode(
            hash_ptr=parent_hash,
            hash_len=parent_len,
            indices_ptr=new_indices,
            indices_len=new_indices_len,
        );
        return (node,);
    }

    // Check collision prefix equality for `len` bytes.
    func has_collision{range_check_ptr}(
        a_hash: felt*, b_hash: felt*, len: felt, idx: felt
    ) -> (ok: felt) {
        if (idx == len) {
            return (1,);
        }

        let a_byte = [a_hash + idx];
        let b_byte = [b_hash + idx];
        if (a_byte != b_byte) {
            return (0,);
        }

        let next_idx = idx + 1;
        return has_collision(a_hash=a_hash, b_hash=b_hash, len=len, idx=next_idx);
    }

    // Ensure index sets of `a` and `b` are disjoint (all pairs checked).
    func distinct_indices{range_check_ptr}(
        a_ptr: felt*, a_len: felt,
        b_ptr: felt*, b_len: felt,
        i: felt, j: felt,
    ) -> (ok: felt) {
        return distinct_indices_outer(
            a_ptr=a_ptr, a_len=a_len, b_ptr=b_ptr, b_len=b_len, i=0
        );
    }

    func distinct_indices_outer{range_check_ptr}(
        a_ptr: felt*, a_len: felt,
        b_ptr: felt*, b_len: felt,
        i: felt,
    ) -> (ok: felt) {
        if (i == a_len) {
            return (1,);
        }

        let (row_ok) = distinct_indices_inner(
            a_ptr=a_ptr, b_ptr=b_ptr, b_len=b_len, i=i, j=0
        );
        if (row_ok == 0) {
            return (0,);
        }

        let next_i = i + 1;
        return distinct_indices_outer(
            a_ptr=a_ptr, a_len=a_len, b_ptr=b_ptr, b_len=b_len, i=next_i
        );
    }

    func distinct_indices_inner{range_check_ptr}(
        a_ptr: felt*, b_ptr: felt*, b_len: felt,
        i: felt, j: felt,
    ) -> (ok: felt) {
        if (j == b_len) {
            return (1,);
        }

        let a_val = [a_ptr + i];
        let b_val = [b_ptr + j];

        if (a_val == b_val) {
            return (0,);
        }

        let next_j = j + 1;
        return distinct_indices_inner(
            a_ptr=a_ptr, b_ptr=b_ptr, b_len=b_len, i=i, j=next_j
        );
    }

    // Validate sibling constraints: collision equality, ordering, and distinctness.
    func validate_subtrees{range_check_ptr, bitwise_ptr: BitwiseBuiltin*}(
        a: EquihashNode, b: EquihashNode
    ) {
        let collision_len = Parameters.collision_byte_length;

        let (collision_ok) = has_collision(
            a_hash=a.hash_ptr,
            b_hash=b.hash_ptr,
            len=collision_len,
            idx=0,
        );
        assert collision_ok = 1;

        let (b_before_a) = indices_before(b, a);
        assert b_before_a = 0;

        let (distinct_ok) = distinct_indices(
            a_ptr=a.indices_ptr,
            a_len=a.indices_len,
            b_ptr=b.indices_ptr,
            b_len=b.indices_len,
            i=0,
            j=0,
        );
        assert distinct_ok = 1;

        return ();
    }

    // Recursively build and validate the merge tree; returns the root node.
    func tree_validator{range_check_ptr, bitwise_ptr: BitwiseBuiltin*}(
        header_pow: felt*, indices_ptr: felt*, indices_len: felt
    ) -> (root: EquihashNode) {
        alloc_locals;

        if (indices_len == 1) {
            let index = [indices_ptr];
            let (leaf) = new_leaf(header_pow, index);
            return (leaf,);
        }

        // Split the indices into two halves.
        let (half, rem) = bitwise_divmod(indices_len, 2);
        assert rem = 0;

        let (left) = tree_validator(
            header_pow=header_pow,
            indices_ptr=indices_ptr,
            indices_len=half,
        );

        let right_ptr = indices_ptr + half;
        let right_len = indices_len - half;
        let (right) = tree_validator(
            header_pow=header_pow,
            indices_ptr=right_ptr,
            indices_len=right_len,
        );

        validate_subtrees(left, right);

        let (parent) = from_children(
            a=left,
            b=right,
            trim=Parameters.collision_byte_length,
        );
        return (parent,);
    }

    // Check that the first `len` bytes of the node hash are zero.
    func node_is_zero{range_check_ptr}(node: EquihashNode, len: felt) -> (ok: felt) {
        let (ok) = bytes_zero(node.hash_ptr, len, 0);
        return (ok,);
    }

    func bytes_zero{range_check_ptr}(ptr: felt*, len: felt, idx: felt) -> (ok: felt) {
        if (idx == len) {
            return (1,);
        }

        let v = [ptr + idx];
        if (v != 0) {
            return (0,);
        }

        let next_idx = idx + 1;
        return bytes_zero(ptr=ptr, len=len, idx=next_idx);
    }
}