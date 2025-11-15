use core::cmp::Ordering;

/// 256-bit little-endian target value.
pub type Target = [u8; 32];

/// Compare two 256-bit little-endian integers.
pub fn cmp_target(a: &Target, b: &Target) -> Ordering {
    for i in (0..32).rev() {
        match a[i].cmp(&b[i]) {
            Ordering::Equal => continue,
            non_eq => return non_eq,
        }
    }
    Ordering::Equal
}

/// Convert compact `nBits` to a 256-bit little-endian target.
pub fn target_from_nbits(nbits: u32) -> Target {
    let mant = nbits & 0x007f_ffff;
    let exp = (nbits >> 24) as u8;

    if mant == 0 {
        return [0u8; 32];
    }

    let mut mant_le = [0u8; 32];
    mant_le[0] = (mant & 0xff) as u8;
    mant_le[1] = ((mant >> 8) & 0xff) as u8;
    mant_le[2] = ((mant >> 16) & 0xff) as u8;

    let shift_bytes = exp as i32 - 3;
    if shift_bytes == 0 {
        return mant_le;
    }

    let mut out = [0u8; 32];
    if shift_bytes > 0 {
        let s = shift_bytes as usize;
        if s >= 32 {
            return [0u8; 32];
        }
        for i in 0..(32 - s) {
            out[i + s] = mant_le[i];
        }
    } else {
        let s = (-shift_bytes) as usize;
        if s >= 32 {
            return [0u8; 32];
        }
        for i in 0..(32 - s) {
            out[i] = mant_le[i + s];
        }
    }

    out
}

/// Convert a 256-bit little-endian target to compact `nBits`.
pub fn target_to_nbits(target_le: &Target) -> u32 {
    let mut bytes_be = [0u8; 32];
    for i in 0..32 {
        bytes_be[i] = target_le[31 - i];
    }

    let mut i = 0usize;
    while i < 32 && bytes_be[i] == 0 {
        i += 1;
    }
    if i == 32 {
        return 0;
    }

    let mut size = (32 - i) as u32;
    let mut mant: u32;

    if size <= 3 {
        mant = (bytes_be[i] as u32) << 16;
        if i + 1 < 32 {
            mant |= (bytes_be[i + 1] as u32) << 8;
        }
        if i + 2 < 32 {
            mant |= bytes_be[i + 2] as u32;
        }
        mant <<= 8 * (3 - size);
    } else {
        mant =
            (bytes_be[i] as u32) << 16 | (bytes_be[i + 1] as u32) << 8 | (bytes_be[i + 2] as u32);
    }

    if mant & 0x0080_0000 != 0 {
        mant >>= 8;
        size += 1;
    }

    (size << 24) | (mant & 0x007f_ffff)
}
