from starkware.cairo.common.math import unsigned_div_rem
from starkware.cairo.common.cairo_builtins import BitwiseBuiltin
from starkware.cairo.common.registers import get_label_location

// Returns q and r such that:
//  0 <= q < rc_bound, 0 <= r < div and value = q * div + r.
//
// Assumption: 0 < div <= PRIME / rc_bound.
// Prover assumption: value / div < rc_bound.
// Modified version of unsigned_div_rem with inlined range checks.
func felt_divmod{range_check_ptr}(value, div) -> (q: felt, r: felt) {
    let r = [range_check_ptr];
    let q = [range_check_ptr + 1];
    %{
        from starkware.cairo.common.math_utils import assert_integer
        assert_integer(ids.div)
        assert 0 < ids.div <= PRIME // range_check_builtin.bound, \
            f'div={hex(ids.div)} is out of the valid range.'
        ids.q, ids.r = divmod(ids.value, ids.div)
    %}
    assert [range_check_ptr + 2] = div - 1 - r;
    let range_check_ptr = range_check_ptr + 3;

    assert value = q * div + r;
    return (q, r);
}

// Computes x//y and x%y.
// Assumption: y must be a power of 2
// params:
//   x: the dividend.
//   y: the divisor.
// returns:
//   q: the quotient.
//   r: the remainder.
func bitwise_divmod{bitwise_ptr: BitwiseBuiltin*}(x: felt, y: felt) -> (q: felt, r: felt) {
    if (y == 1) {
        let bitwise_ptr = bitwise_ptr;
        return (q=x, r=0);
    } else {
        assert bitwise_ptr.x = x;
        assert bitwise_ptr.y = y - 1;
        let x_and_y = bitwise_ptr.x_and_y;

        let bitwise_ptr = bitwise_ptr + BitwiseBuiltin.SIZE;
        return (q=(x - x_and_y) / y, r=x_and_y);
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

// Utility to get a pointer on an array of 2^i from i = 0 to 128.
func pow2alloc128() -> (array: felt*) {
    let (data_address) = get_label_location(data);
    return (data_address,);

    data:
    dw 0x1;
    dw 0x2;
    dw 0x4;
    dw 0x8;
    dw 0x10;
    dw 0x20;
    dw 0x40;
    dw 0x80;
    dw 0x100;
    dw 0x200;
    dw 0x400;
    dw 0x800;
    dw 0x1000;
    dw 0x2000;
    dw 0x4000;
    dw 0x8000;
    dw 0x10000;
    dw 0x20000;
    dw 0x40000;
    dw 0x80000;
    dw 0x100000;
    dw 0x200000;
    dw 0x400000;
    dw 0x800000;
    dw 0x1000000;
    dw 0x2000000;
    dw 0x4000000;
    dw 0x8000000;
    dw 0x10000000;
    dw 0x20000000;
    dw 0x40000000;
    dw 0x80000000;
    dw 0x100000000;
    dw 0x200000000;
    dw 0x400000000;
    dw 0x800000000;
    dw 0x1000000000;
    dw 0x2000000000;
    dw 0x4000000000;
    dw 0x8000000000;
    dw 0x10000000000;
    dw 0x20000000000;
    dw 0x40000000000;
    dw 0x80000000000;
    dw 0x100000000000;
    dw 0x200000000000;
    dw 0x400000000000;
    dw 0x800000000000;
    dw 0x1000000000000;
    dw 0x2000000000000;
    dw 0x4000000000000;
    dw 0x8000000000000;
    dw 0x10000000000000;
    dw 0x20000000000000;
    dw 0x40000000000000;
    dw 0x80000000000000;
    dw 0x100000000000000;
    dw 0x200000000000000;
    dw 0x400000000000000;
    dw 0x800000000000000;
    dw 0x1000000000000000;
    dw 0x2000000000000000;
    dw 0x4000000000000000;
    dw 0x8000000000000000;
    dw 0x10000000000000000;
    dw 0x20000000000000000;
    dw 0x40000000000000000;
    dw 0x80000000000000000;
    dw 0x100000000000000000;
    dw 0x200000000000000000;
    dw 0x400000000000000000;
    dw 0x800000000000000000;
    dw 0x1000000000000000000;
    dw 0x2000000000000000000;
    dw 0x4000000000000000000;
    dw 0x8000000000000000000;
    dw 0x10000000000000000000;
    dw 0x20000000000000000000;
    dw 0x40000000000000000000;
    dw 0x80000000000000000000;
    dw 0x100000000000000000000;
    dw 0x200000000000000000000;
    dw 0x400000000000000000000;
    dw 0x800000000000000000000;
    dw 0x1000000000000000000000;
    dw 0x2000000000000000000000;
    dw 0x4000000000000000000000;
    dw 0x8000000000000000000000;
    dw 0x10000000000000000000000;
    dw 0x20000000000000000000000;
    dw 0x40000000000000000000000;
    dw 0x80000000000000000000000;
    dw 0x100000000000000000000000;
    dw 0x200000000000000000000000;
    dw 0x400000000000000000000000;
    dw 0x800000000000000000000000;
    dw 0x1000000000000000000000000;
    dw 0x2000000000000000000000000;
    dw 0x4000000000000000000000000;
    dw 0x8000000000000000000000000;
    dw 0x10000000000000000000000000;
    dw 0x20000000000000000000000000;
    dw 0x40000000000000000000000000;
    dw 0x80000000000000000000000000;
    dw 0x100000000000000000000000000;
    dw 0x200000000000000000000000000;
    dw 0x400000000000000000000000000;
    dw 0x800000000000000000000000000;
    dw 0x1000000000000000000000000000;
    dw 0x2000000000000000000000000000;
    dw 0x4000000000000000000000000000;
    dw 0x8000000000000000000000000000;
    dw 0x10000000000000000000000000000;
    dw 0x20000000000000000000000000000;
    dw 0x40000000000000000000000000000;
    dw 0x80000000000000000000000000000;
    dw 0x100000000000000000000000000000;
    dw 0x200000000000000000000000000000;
    dw 0x400000000000000000000000000000;
    dw 0x800000000000000000000000000000;
    dw 0x1000000000000000000000000000000;
    dw 0x2000000000000000000000000000000;
    dw 0x4000000000000000000000000000000;
    dw 0x8000000000000000000000000000000;
    dw 0x10000000000000000000000000000000;
    dw 0x20000000000000000000000000000000;
    dw 0x40000000000000000000000000000000;
    dw 0x80000000000000000000000000000000;
    dw 0x100000000000000000000000000000000;
}

