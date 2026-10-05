//! Synthetic bootstrap functions, never guest code. Unsafe is confined here.
use cranelift_codegen::ir::{AbiParam, InstBuilder, types};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module, default_libcall_names};

extern "C" fn add_seven(value: u64) -> u64 {
    value.wrapping_add(7)
}

fn builder() -> Result<JITBuilder, String> {
    let mut builder = JITBuilder::with_flags(
        &[("opt_level", "none"), ("enable_verifier", "true")],
        default_libcall_names(),
    )
    .map_err(|e| e.to_string())?;
    builder.clear_symbol_lookup_functions();
    Ok(builder)
}

/// Effect: creates and invokes synthetic native code; no guest source/effects.
pub fn native_smoke(helper: bool) -> Result<u64, String> {
    let mut builder = builder()?;
    builder.symbol("lrr_bootstrap_add_seven", add_seven as *const u8);
    let mut module = JITModule::new(builder);
    let result = (|| {
        let mut signature = module.make_signature();
        signature.params.push(AbiParam::new(types::I64));
        signature.returns.push(AbiParam::new(types::I64));
        let target = if helper {
            Some(
                module
                    .declare_function("lrr_bootstrap_add_seven", Linkage::Import, &signature)
                    .map_err(|e| e.to_string())?,
            )
        } else {
            None
        };
        let function = module
            .declare_function("lrr_bootstrap", Linkage::Local, &signature)
            .map_err(|e| e.to_string())?;
        let mut ctx = module.make_context();
        ctx.func.signature = signature;
        let mut function_ctx = FunctionBuilderContext::new();
        {
            let mut f = FunctionBuilder::new(&mut ctx.func, &mut function_ctx);
            let block = f.create_block();
            f.append_block_params_for_function_params(block);
            f.switch_to_block(block);
            let input = f.block_params(block)[0];
            let output = if let Some(target) = target {
                let imported = module.declare_func_in_func(target, f.func);
                let call = f.ins().call(imported, &[input]);
                f.inst_results(call)[0]
            } else {
                f.ins().iadd_imm_s(input, 7)
            };
            f.ins().return_(&[output]);
            f.seal_all_blocks();
            f.finalize(module.target_config());
        }
        module
            .define_function(function, &mut ctx)
            .map_err(|e| e.to_string())?;
        module.finalize_definitions().map_err(|e| e.to_string())?;
        // SAFETY: finalized definition has this exact signature and the live
        // module owns its executable allocation through the entire invocation.
        let output = unsafe {
            let function: extern "C" fn(u64) -> u64 =
                std::mem::transmute(module.get_finalized_function(function));
            function(35)
        }; // The saved function pointer is out of scope before memory retirement.
        Ok(output)
    })();
    // SAFETY: no function pointers escape; every invocation has completed.
    unsafe {
        module.free_memory();
    }
    result
}

/// Effect: attempts finalization of an ambient libc import in a child process.
/// Upstream may abort/panic; caller must supervise this synthetic smoke probe.
pub fn ambient_import_probe() -> Result<(), String> {
    let mut module = JITModule::new(builder()?);
    let mut signature = module.make_signature();
    signature.returns.push(AbiParam::new(types::I32));
    let imported = module
        .declare_function("getpid", Linkage::Import, &signature)
        .map_err(|e| e.to_string())?;
    let function = module
        .declare_function("lrr_ambient_probe", Linkage::Local, &signature)
        .map_err(|e| e.to_string())?;
    let mut ctx = module.make_context();
    ctx.func.signature = signature;
    let mut function_ctx = FunctionBuilderContext::new();
    {
        let mut f = FunctionBuilder::new(&mut ctx.func, &mut function_ctx);
        let block = f.create_block();
        f.switch_to_block(block);
        let imported = module.declare_func_in_func(imported, f.func);
        let call = f.ins().call(imported, &[]);
        let output = f.inst_results(call)[0];
        f.ins().return_(&[output]);
        f.seal_all_blocks();
        f.finalize(module.target_config());
    }
    module
        .define_function(function, &mut ctx)
        .map_err(|e| e.to_string())?;
    let result = module.finalize_definitions().map_err(|e| e.to_string());
    // SAFETY: no executable pointer was obtained.
    unsafe {
        module.free_memory();
    }
    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn jit_adds_seven() {
        assert_eq!(super::native_smoke(false).unwrap(), 42);
    }
    #[test]
    fn jit_calls_registered_helper() {
        assert_eq!(super::native_smoke(true).unwrap(), 42);
    }
}
