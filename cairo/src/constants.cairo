

namespace Parameters {
    const n = 200;
    const k = 9;

    // Zcash-style Equihash parameters for (n = 200, k = 9).
    // These are fixed small integers; we spell them out to avoid field-division issues.
    const indicies_per_hash_output = 2;
    const hash_output = 50;              // 2 * 200 / 8

    // Collision length in bits and bytes.
    const collision_bit_length = 20;     // 200 / (9 + 1)
    const collision_byte_length = 3;     // ceil(20 / 8)

    // Digest slice length used per leaf (n bits).
    const digest_slice_bytes = 25;       // 200 / 8

    // Number of bytes in a leaf hash after expand_array.
    const leaf_hash_bytes = 30;          // 8 * 3 * 25 / 20
}