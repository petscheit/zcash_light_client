// use zcash_crypto::verify_equihash_solution_with_params;
//
// // Create the types expected by the original test vector files so we can include them 1:1.
// mod params {
//     #[derive(Clone, Copy)]
//     pub(crate) struct Params {
//         pub(crate) n: u32,
//         pub(crate) k: u32,
//     }
// }
//
// // Provide a local Kind enum so the invalid fixtures can be included unchanged.
// mod verify {
//     #[derive(Debug, PartialEq)]
//     #[allow(dead_code)]
//     pub(crate) enum Kind {
//         InvalidParams,
//         Collision,
//         OutOfOrder,
//         DuplicateIdxs,
//         NonZeroRootHash,
//     }
// }
//
// // Include the valid test vectors unchanged.
// mod vectors_valid {
//     include!("../../equihash/src/test_vectors/valid.rs");
// }
//
// // Include the invalid test vectors unchanged.
// mod vectors_invalid {
//     include!("../../equihash/src/test_vectors/invalid.rs");
// }
//
// fn compress_array(array: &[u8], bit_len: usize, byte_pad: usize) -> Vec<u8> {
//     let in_width: usize = (bit_len + 7) / 8 + byte_pad;
//     let out_len = bit_len * array.len() / (8 * in_width);
//     let mut out = Vec::with_capacity(out_len);
//     let bit_len_mask: u32 = (1 << (bit_len as u32)) - 1;
//     let mut acc_bits: usize = 0;
//     let mut acc_value: u32 = 0;
//     let mut j: usize = 0;
//     for _ in 0..out_len {
//         if acc_bits < 8 {
//             acc_value <<= bit_len;
//             for x in byte_pad..in_width {
//                 acc_value |= ((array[j + x] & ((bit_len_mask >> (8 * (in_width - x - 1))) as u8))
//                     as u32)
//                     .wrapping_shl(8 * (in_width - x - 1) as u32);
//             }
//             j += in_width;
//             acc_bits += bit_len;
//         }
//         acc_bits -= 8;
//         out.push((acc_value >> acc_bits) as u8);
//     }
//     out
// }
//
// fn minimal_from_indices(n: u32, k: u32, indices: &[u32]) -> Vec<u8> {
//     let array: Vec<u8> = indices.iter().flat_map(|i| i.to_be_bytes()).collect();
//     let c_bit_len = (n / (k + 1)) as usize;
//     let digit_bytes = ((c_bit_len + 1) + 7) / 8;
//     let byte_pad = core::mem::size_of::<u32>() - digit_bytes;
//     compress_array(&array, c_bit_len + 1, byte_pad)
// }
//
// fn powheader(input: &[u8], nonce: [u8; 32]) -> Vec<u8> {
//     let mut out = input.to_vec();
//     out.extend_from_slice(&nonce);
//     out
// }
//
// #[test]
// fn valid_vectors_all_params() {
//     for tv in vectors_valid::VALID_TEST_VECTORS {
//         let pow = powheader(tv.input, tv.nonce);
//         for sol in tv.solutions {
//             let minimal = minimal_from_indices(tv.params.n, tv.params.k, sol);
//             verify_equihash_solution_with_params(tv.params.n, tv.params.k, &pow, &minimal).unwrap();
//         }
//     }
// }
//
// #[test]
// fn invalid_vectors_all_params() {
//     for tv in vectors_invalid::INVALID_TEST_VECTORS {
//         let pow = powheader(tv.input, tv.nonce);
//         let minimal = minimal_from_indices(tv.params.n, tv.params.k, tv.solution);
//         assert!(
//             verify_equihash_solution_with_params(tv.params.n, tv.params.k, &pow, &minimal).is_err()
//         );
//     }
// }


