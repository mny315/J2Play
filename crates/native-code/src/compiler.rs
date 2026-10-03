use crate::executable::Image;
use crate::verify::{Verified, verify};
use cranelift_codegen::ir::{
    AbiParam, InstBuilder, MemFlagsData, Signature, Value, condcodes::IntCC, types,
};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};
use vm::acceleration::{CompiledIntegerMethod, IntegerMethodCompiler, IntegerMethodSpec};

#[derive(Clone, Copy, Debug, Default)]
pub struct PreparationStats {
    pub candidates: usize,
    pub compiled: usize,
    pub cache_hits: usize,
    pub rejected: usize,
    pub code_bytes: usize,
    pub elapsed_millis: u128,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RunStats {
    pub attempts: u64,
    pub calls: u64,
    pub instructions: u64,
    pub bailouts: u64,
}

#[derive(Default)]
pub struct Compiler {
    preparation: PreparationStats,
    execution: Rc<Cell<RunStats>>,
    cache_root: Option<PathBuf>,
}

impl Compiler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Uses host-owned private storage outside all guest-accessible file roots.
    #[must_use]
    pub fn with_cache(root: PathBuf) -> Self {
        Self {
            cache_root: Some(root),
            ..Self::default()
        }
    }

    #[must_use]
    pub const fn preparation_stats(&self) -> PreparationStats {
        self.preparation
    }

    #[must_use]
    pub fn run_stats(&self) -> RunStats {
        self.execution.get()
    }

    #[allow(clippy::too_many_lines)]
    fn prepare(
        &mut self,
        methods: &[IntegerMethodSpec],
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<Option<Rc<dyn CompiledIntegerMethod>>>, String> {
        let started = Instant::now();
        let mut flags = settings::builder();
        flags.set("opt_level", "speed").map_err(|e| e.to_string())?;
        flags.set("is_pic", "false").map_err(|e| e.to_string())?;
        let isa = cranelift_native::builder()
            .map_err(str::to_owned)?
            .finish(settings::Flags::new(flags))
            .map_err(|e| e.to_string())?;
        let target = format!(
            "{}:{}:{}",
            isa.triple(),
            isa.flags(),
            isa.isa_flags()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        );
        let mut cache = self.cache_root.as_ref().and_then(|root| {
            match crate::cache::Cache::open(root, &target) {
                Ok(cache) => Some(cache),
                Err(error) => {
                    eprintln!("J2Play AOT cache unavailable: {error}");
                    None
                }
            }
        });
        let module = JITModule::new(JITBuilder::with_isa(
            isa,
            cranelift_module::default_libcall_names(),
        ));
        let mut image = Image::new(module, Rc::clone(&self.execution));
        let module = image.module_mut();
        let mut context = module.make_context();
        let mut builder_context = FunctionBuilderContext::new();
        let signature = numeric_signature(module);
        let mut ids = vec![None; methods.len().min(256)];
        self.preparation = PreparationStats {
            candidates: ids.len(),
            ..PreparationStats::default()
        };
        for (index, spec) in methods.iter().take(256).enumerate() {
            if cancelled()
                || started.elapsed() >= Duration::from_secs(4)
                || self.preparation.code_bytes >= 4 * 1_024 * 1_024
            {
                break;
            }
            let Some(verified) = verify(spec) else {
                self.preparation.rejected += 1;
                continue;
            };
            let id = module
                .declare_function(&format!("numeric_{index}"), Linkage::Local, &signature)
                .map_err(|e| e.to_string())?;
            let identity = cache.as_ref().map(|cache| cache.identity(spec));
            if let Some(code) = identity
                .as_ref()
                .and_then(|identity| cache.as_ref()?.load(identity))
            {
                module
                    .define_function_bytes(id, 16, &code, &[])
                    .map_err(|e| e.to_string())?;
                self.preparation.cache_hits += 1;
                self.preparation.code_bytes += code.len();
                ids[index] = Some(id);
                continue;
            }
            module.clear_context(&mut context);
            context.func.signature = signature.clone();
            translate(
                spec,
                &verified,
                &mut context.func,
                &mut builder_context,
                module.target_config(),
            );
            if let Err(error) = module.define_function(id, &mut context) {
                self.preparation.rejected += 1;
                eprintln!("J2Play AOT: numeric compilation declined: {error}");
                continue;
            }
            let bytes = context
                .compiled_code()
                .map_or(0, |code| code.code_buffer().len());
            if let Some(code) = context.compiled_code()
                && code.buffer.relocs().is_empty()
                && let (Some(cache), Some(identity)) = (&mut cache, &identity)
                && let Err(error) = cache.store(identity, code.code_buffer())
            {
                eprintln!("J2Play AOT cache write skipped: {error}");
            }
            self.preparation.code_bytes += bytes;
            self.preparation.compiled += 1;
            ids[index] = Some(id);
        }
        module.finalize_definitions().map_err(|e| e.to_string())?;
        let image = Rc::new(image);
        let compiled = ids
            .into_iter()
            .zip(methods)
            .map(|(id, spec)| id.map(|id| image.method(id, spec.parameter_slots.len())))
            .collect();
        self.preparation.elapsed_millis = started.elapsed().as_millis();
        Ok(compiled)
    }
}

impl IntegerMethodCompiler for Compiler {
    fn compile(
        &mut self,
        methods: &[IntegerMethodSpec],
        cancelled: &dyn Fn() -> bool,
    ) -> Vec<Option<Rc<dyn CompiledIntegerMethod>>> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.prepare(methods, cancelled)
        })) {
            Ok(Ok(methods)) => methods,
            Ok(Err(error)) => {
                eprintln!("J2Play AOT unavailable: {error}");
                vec![]
            }
            Err(_) => {
                eprintln!("J2Play AOT unavailable: compiler failed; retaining interpreter");
                vec![]
            }
        }
    }
}

fn numeric_signature(module: &JITModule) -> Signature {
    let mut signature = module.make_signature();
    signature
        .params
        .push(AbiParam::new(module.target_config().pointer_type()));
    signature.params.push(AbiParam::new(types::I32));
    signature.returns.push(AbiParam::new(types::I64));
    signature
}

#[allow(clippy::too_many_lines)]
fn translate(
    spec: &IntegerMethodSpec,
    verified: &Verified,
    function: &mut cranelift_codegen::ir::Function,
    context: &mut FunctionBuilderContext,
    config: cranelift_codegen::isa::TargetFrontendConfig,
) {
    let mut builder = FunctionBuilder::new(function, context);
    let entry = builder.create_block();
    let failed = builder.create_block();
    let blocks: Vec<_> = verified
        .blocks
        .iter()
        .map(|_| builder.create_block())
        .collect();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    let pointer = builder.block_params(entry)[0];
    let initial_fuel = builder.block_params(entry)[1];
    let fuel = builder.declare_var(types::I32);
    builder.def_var(fuel, initial_fuel);
    let zero = builder.ins().iconst(types::I32, 0);
    let locals: Vec<_> = (0..spec.max_locals)
        .map(|_| {
            let variable = builder.declare_var(types::I32);
            builder.def_var(variable, zero);
            variable
        })
        .collect();
    let stack_vars: Vec<_> = (0..spec.max_stack)
        .map(|_| {
            let variable = builder.declare_var(types::I32);
            builder.def_var(variable, zero);
            variable
        })
        .collect();
    for (index, &slot) in spec.parameter_slots.iter().enumerate() {
        let value = builder.ins().load(
            types::I32,
            MemFlagsData::trusted(),
            pointer,
            i32::try_from(index * 4).expect("bounded parameters"),
        );
        builder.def_var(locals[usize::from(slot)], value);
    }
    builder.ins().jump(blocks[0], &[]);
    builder.switch_to_block(failed);
    let failure = builder.ins().iconst(types::I64, 0);
    builder.ins().return_(&[failure]);
    for (index, block) in verified.blocks.iter().enumerate() {
        let Some(depth) = block.depth else {
            continue;
        };
        builder.switch_to_block(blocks[index]);
        let remaining = builder.use_var(fuel);
        let cost = i64::try_from(block.end - block.start).expect("bounded code");
        let enough = builder
            .ins()
            .icmp_imm_s(IntCC::UnsignedGreaterThanOrEqual, remaining, cost);
        let body = builder.create_block();
        builder.ins().brif(enough, body, &[], failed, &[]);
        builder.switch_to_block(body);
        let remaining = builder.ins().iadd_imm_s(remaining, -cost);
        builder.def_var(fuel, remaining);
        let mut stack: Vec<Value> = stack_vars[..depth]
            .iter()
            .map(|&variable| builder.use_var(variable))
            .collect();
        for op in &verified.ops[block.start..block.end] {
            match op.opcode {
                0x00 => {}
                0x02..=0x08 | 0x10..=0x13 => {
                    stack.push(builder.ins().iconst(types::I32, i64::from(op.immediate)));
                }
                0x15 | 0x1a..=0x1d => stack.push(
                    builder.use_var(locals[usize::try_from(op.immediate).expect("verified local")]),
                ),
                0x36 | 0x3b..=0x3e => {
                    let value = pop(&mut stack);
                    builder.def_var(
                        locals[usize::try_from(op.immediate).expect("verified local")],
                        value,
                    );
                }
                0x84 => {
                    let value = builder.use_var(locals[op.target]);
                    let value = builder.ins().iadd_imm_s(value, i64::from(op.immediate));
                    builder.def_var(locals[op.target], value);
                }
                0x57..=0x5f => stack_operation(&mut stack, op.opcode),
                0x74 | 0x91..=0x93 => {
                    let value = pop(&mut stack);
                    let value = match op.opcode {
                        0x74 => builder.ins().ineg(value),
                        0x91 => {
                            let low = builder.ins().ireduce(types::I8, value);
                            builder.ins().sextend(types::I32, low)
                        }
                        0x92 => builder.ins().band_imm_s(value, 65_535),
                        _ => {
                            let low = builder.ins().ireduce(types::I16, value);
                            builder.ins().sextend(types::I32, low)
                        }
                    };
                    stack.push(value);
                }
                0x99..=0xa4 => {
                    let right = if op.opcode <= 0x9e {
                        builder.ins().iconst(types::I32, 0)
                    } else {
                        pop(&mut stack)
                    };
                    let left = pop(&mut stack);
                    let condition = match (op.opcode - 0x99) % 6 {
                        0 => IntCC::Equal,
                        1 => IntCC::NotEqual,
                        2 => IntCC::SignedLessThan,
                        3 => IntCC::SignedGreaterThanOrEqual,
                        4 => IntCC::SignedGreaterThan,
                        _ => IntCC::SignedLessThanOrEqual,
                    };
                    let condition = builder.ins().icmp(condition, left, right);
                    save_stack(&mut builder, &stack_vars, &stack);
                    builder.ins().brif(
                        condition,
                        blocks[verified.block_at[&op.target]],
                        &[],
                        blocks[verified.block_at[&block.end]],
                        &[],
                    );
                }
                0xa7 => {
                    save_stack(&mut builder, &stack_vars, &stack);
                    builder
                        .ins()
                        .jump(blocks[verified.block_at[&op.target]], &[]);
                }
                0xac => {
                    let value = pop(&mut stack);
                    let remaining = builder.use_var(fuel);
                    let used = builder.ins().isub(initial_fuel, remaining);
                    let used = builder.ins().iadd_imm_s(used, 1);
                    let used = builder.ins().uextend(types::I64, used);
                    let used = builder.ins().ishl_imm_s(used, 32);
                    let value = builder.ins().uextend(types::I64, value);
                    let result = builder.ins().bor(used, value);
                    builder.ins().return_(&[result]);
                }
                _ => {
                    let right = pop(&mut stack);
                    let left = pop(&mut stack);
                    let value = match op.opcode {
                        0x60 => builder.ins().iadd(left, right),
                        0x64 => builder.ins().isub(left, right),
                        0x68 => builder.ins().imul(left, right),
                        0x6c | 0x70 => {
                            let is_zero = builder.ins().icmp_imm_s(IntCC::Equal, right, 0);
                            let safe = builder.create_block();
                            builder.ins().brif(is_zero, failed, &[], safe, &[]);
                            builder.switch_to_block(safe);
                            let is_min =
                                builder
                                    .ins()
                                    .icmp_imm_s(IntCC::Equal, left, i64::from(i32::MIN));
                            let is_minus_one = builder.ins().icmp_imm_s(IntCC::Equal, right, -1);
                            let overflow = builder.ins().band(is_min, is_minus_one);
                            let one = builder.ins().iconst(types::I32, 1);
                            let divisor = builder.ins().select(overflow, one, right);
                            if op.opcode == 0x6c {
                                builder.ins().sdiv(left, divisor)
                            } else {
                                builder.ins().srem(left, divisor)
                            }
                        }
                        0x78 => builder.ins().ishl(left, right),
                        0x7a => builder.ins().sshr(left, right),
                        0x7c => builder.ins().ushr(left, right),
                        0x7e => builder.ins().band(left, right),
                        0x80 => builder.ins().bor(left, right),
                        0x82 => builder.ins().bxor(left, right),
                        _ => unreachable!("verified opcode"),
                    };
                    stack.push(value);
                }
            }
        }
        if !matches!(
            verified.ops[block.end - 1].opcode,
            0x99..=0xa4 | 0xa7 | 0xac
        ) {
            save_stack(&mut builder, &stack_vars, &stack);
            builder
                .ins()
                .jump(blocks[verified.block_at[&block.end]], &[]);
        }
    }
    builder.seal_all_blocks();
    builder.finalize(config);
}

fn pop(stack: &mut Vec<Value>) -> Value {
    stack.pop().expect("verified stack depth")
}

fn save_stack(builder: &mut FunctionBuilder<'_>, variables: &[Variable], values: &[Value]) {
    for (&variable, &value) in variables.iter().zip(values) {
        builder.def_var(variable, value);
    }
}

fn stack_operation(stack: &mut Vec<Value>, opcode: u8) {
    match opcode {
        0x57 => {
            pop(stack);
        }
        0x58 => {
            pop(stack);
            pop(stack);
        }
        0x59..=0x5b => {
            let value = *stack.last().expect("verified stack");
            stack.insert(stack.len() - usize::from(opcode - 0x58), value);
        }
        0x5c..=0x5e => {
            let a = stack[stack.len() - 2];
            let b = stack[stack.len() - 1];
            let at = stack.len() - usize::from(opcode - 0x5a);
            stack.insert(at, b);
            stack.insert(at, a);
        }
        0x5f => {
            let len = stack.len();
            stack.swap(len - 1, len - 2);
        }
        _ => unreachable!("verified stack opcode"),
    }
}
