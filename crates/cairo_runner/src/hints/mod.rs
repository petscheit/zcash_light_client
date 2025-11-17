use std::collections::HashMap;

use cairo_vm_base::vm::cairo_vm::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::HintProcessorData;
use cairo_vm_base::vm::cairo_vm::hint_processor::builtin_hint_processor::hint_utils::get_relocatable_from_var_name;
use cairo_vm_base::vm::cairo_vm::vm::vm_core::VirtualMachine;
use cairo_vm_base::vm::cairo_vm::vm::errors::hint_errors::HintError;
use cairo_vm_base::vm::cairo_vm::types::exec_scope::ExecutionScopes;
use cairo_vm_base::vm::cairo_vm::Felt252;

pub mod hashing;
use crate::types::InputData;

pub const WRITE_INPUTS_HINT: &str = "WRITE_INPUTS";


pub fn write_inputs(
    vm: &mut VirtualMachine,
    exec_scopes: &mut ExecutionScopes,
    hint_data: &HintProcessorData,
    _constants: &HashMap<String, Felt252>,
) -> Result<(), HintError> {
    let inputs: &InputData = exec_scopes.get_ref::<InputData>("input")?;
    let solution_indicies_var_addr = get_relocatable_from_var_name(
        "solution_indicies",
        vm,
        &hint_data.ids_data,
        &hint_data.ap_tracking,
    )?;
    let solution_indicies_ptr = vm.get_relocatable(solution_indicies_var_addr)?;

    // Write each next sync committee branch element
    let mut segment_ptr = solution_indicies_ptr;
    for index in &inputs.solution_indexes {
        vm.insert_value(segment_ptr, Felt252::from(*index as u64))?;
        segment_ptr = (segment_ptr + 1)?;
    }

    let header_bytes_var_addr = get_relocatable_from_var_name(
        "header_bytes",
        vm,
        &hint_data.ids_data,
        &hint_data.ap_tracking,
    )?;
    let header_bytes_ptr = vm.get_relocatable(header_bytes_var_addr)?;

    let mut segment_ptr = header_bytes_ptr;
    for chunk in inputs.header_bytes.clone() {
        vm.insert_value(segment_ptr, Felt252::from(chunk))?;
        segment_ptr = (segment_ptr + 1)?;
    }



    
    
    

    Ok(())
}
