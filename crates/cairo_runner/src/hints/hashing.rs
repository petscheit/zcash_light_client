use std::collections::HashMap;

use cairo_vm_base::vm::cairo_vm::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::HintProcessorData;
use cairo_vm_base::vm::cairo_vm::hint_processor::builtin_hint_processor::hint_utils::get_relocatable_from_var_name;
use cairo_vm_base::vm::cairo_vm::vm::vm_core::VirtualMachine;
use cairo_vm_base::vm::cairo_vm::vm::errors::hint_errors::HintError;
use cairo_vm_base::vm::cairo_vm::types::exec_scope::ExecutionScopes;
use cairo_vm_base::vm::cairo_vm::Felt252;

use blake2b_simd::{Hash as Blake2bHash, Params as Blake2bParams, State as Blake2bState};

use crate::constants::{DIGEST_LEN, K, N};

/// Initialize BLAKE2b with Zcash personalization and the desired digest length.
///
/// Personalization: "ZcashPoW" || LE32(n) || LE32(k).
fn initialise_state(n: u32, k: u32, digest_len: u8) -> Blake2bState {
    // personalization = "ZcashPoW" || LE32(n) || LE32(k)
    let mut personalization: [u8; 16] = *b"ZcashPoW\x00\x00\x00\x00\x00\x00\x00\x00";
    personalization[8..12].copy_from_slice(&n.to_le_bytes());
    personalization[12..16].copy_from_slice(&k.to_le_bytes());
    Blake2bParams::new()
        .hash_length(digest_len as usize)
        .personal(&personalization)
        .to_state()
}

/// Compute the `i`-th group BLAKE2b digest by hashing the 32-bit little-endian counter.
///
/// A digest contains several adjacent `n`-bit slices; leaf construction selects one slice.
fn generate_hash(pow_header: &[u8], i: u32) -> Blake2bHash {
    let base_state = initialise_state(N, K, DIGEST_LEN);

    let mut state = base_state.clone();
    state.update(pow_header);
    state.update(&i.to_le_bytes());
    state.finalize()
}

pub const HINT_GENERATE_HASH: &str = "CREATE_BLAKE2B_HASH";

pub fn generate_hash_hint(
    vm: &mut VirtualMachine,
    _exec_scopes: &mut ExecutionScopes,
    hint_data: &HintProcessorData,
    _constants: &HashMap<String, Felt252>,
) -> Result<(), HintError> {
    let header_bytes_var_addr = get_relocatable_from_var_name(
        "header_pow",
        vm,
        &hint_data.ids_data,
        &hint_data.ap_tracking,
    )?;

    let mut header_felts = vec![];
    let mut header_ptr = vm.get_relocatable(header_bytes_var_addr)?;
    for _i in 0..35 {
        let res = vm.get_integer(header_ptr)?;
        let value: u32 = (*res.as_ref()).try_into().unwrap();
        header_felts.push(value);
        header_ptr = (header_ptr + 1)?;
    }

    let mut pow_header_bytes = Vec::with_capacity(140);
    for val in header_felts {
        pow_header_bytes.extend_from_slice(&val.to_be_bytes());
    }

    assert_eq!(pow_header_bytes.len(), 140, "Header must be 140 bytes long");

    let index_ptr =
        get_relocatable_from_var_name("index", vm, &hint_data.ids_data, &hint_data.ap_tracking)?;
    let index: u32 = (*vm
        .get_integer(index_ptr)?
        .as_ref())
        .try_into()
        .unwrap();

    let hash = generate_hash(&pow_header_bytes, index);

    // Write the 50-byte digest as a contiguous felt array (one byte per felt).
    let hash_bytes_var_addr = get_relocatable_from_var_name(
        "hash_bytes",
        vm,
        &hint_data.ids_data,
        &hint_data.ap_tracking,
    )?;
    let mut hash_ptr = vm.get_relocatable(hash_bytes_var_addr)?;

    for b in hash.as_bytes().iter() {
        vm.insert_value(hash_ptr, Felt252::from(*b as u64))?;
        hash_ptr = (hash_ptr + 1)?;
    }

    Ok(())
}
