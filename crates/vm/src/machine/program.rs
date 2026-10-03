//! Class linking, hierarchy validation and bounded runtime metadata.

use super::{
    Arc, Attribute, Class, ClassFile, CountedArrayFill, EmuError, Field, FieldToken, HashMap,
    HashSet, Instruction, Limits, Method, MethodKey, NativeRegistry, NoArgConstructor, Program,
    RuntimeInstructionMeta, build_instruction_index, build_runtime_instructions,
    constant_field_value, decode, default_value, estimate_constant_pool, field_key, fnv1a64,
    link_counted_array_fills, parse_descriptor_type, parse_method_descriptor, vm_error,
};

impl Program {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Mutable registry populated by the selected bootstrap/API libraries.
    pub fn native_registry_mut(&mut self) -> &mut NativeRegistry {
        &mut self.natives
    }

    /// Reports whether a class has already been linked into this program.
    #[must_use]
    pub fn contains_class(&self, class: &str) -> bool {
        self.classes.contains_key(class)
    }

    /// Reports whether `class` is the named class, its subclass, or an
    /// implementation of the named interface.
    #[must_use]
    pub fn is_assignable_to(&self, class: &str, target: &str) -> bool {
        let mut current = class;
        let mut remaining = self.classes.len();
        loop {
            if current == target {
                return true;
            }
            let Some(linked) = self.classes.get(current) else {
                return false;
            };
            // A simple superclass chain needs neither a work list nor a
            // visited set. Allocate graph traversal state only at a branch.
            if !linked.interfaces.is_empty() {
                return self.is_assignable_through_interfaces(current, linked, target);
            }
            let Some(parent) = linked.super_name.as_deref() else {
                return false;
            };
            if remaining == 0 {
                return false;
            }
            remaining -= 1;
            current = parent;
        }
    }

    fn is_assignable_through_interfaces(&self, class: &str, first: &Class, target: &str) -> bool {
        let mut linked = first;
        let mut pending = Vec::new();
        let mut seen = HashSet::from([class]);
        loop {
            for parent in linked
                .interfaces
                .iter()
                .map(String::as_str)
                .chain(linked.super_name.as_deref())
            {
                if parent == target {
                    return true;
                }
                pending.push(parent);
            }
            linked = loop {
                let Some(parent) = pending.pop() else {
                    return false;
                };
                if seen.insert(parent)
                    && let Some(linked) = self.classes.get(parent)
                {
                    break linked;
                }
            };
        }
    }

    /// Reports whether a class is public, concrete, and exposes the public
    /// zero-argument constructor required for host-owned instantiation.
    #[must_use]
    pub fn is_publicly_instantiable(&self, class: &str) -> bool {
        self.classes.get(class).is_some_and(|linked| {
            linked.is_public
                && !linked.is_abstract
                && linked.no_arg_constructor == NoArgConstructor::Public
        })
    }

    /// Links a parsed class and its executable methods within the runtime limits.
    ///
    /// # Errors
    ///
    /// Returns a VM diagnostic when linking exceeds a configured limit or a
    /// runtime method cannot be represented consistently.
    pub fn add_class(&mut self, class: &ClassFile, limits: &Limits) -> Result<(), EmuError> {
        let class_name = class
            .class_name(class.this_class)
            .ok_or_else(|| vm_error("invalid-class", "class has no runtime name"))?
            .to_owned();
        if self.classes.contains_key(&class_name) {
            return Err(vm_error("duplicate-class", "class is already linked"));
        }
        let constant_pool_id = self.constant_pool_count;
        let next_constant_pool_count = constant_pool_id
            .checked_add(1)
            .ok_or_else(|| vm_error("memory-limit", "constant-pool count overflow"))?;
        let mut staged = HashMap::new();
        let mut staged_stack_keys = Vec::new();
        let mut staged_counted_array_fills = Vec::new();
        let mut staged_bytes = estimate_constant_pool(&class.constant_pool)?
            .checked_add(class_name.len())
            .and_then(|bytes| {
                bytes.checked_add(
                    std::mem::size_of::<(String, Class)>()
                        + std::mem::size_of::<Vec<Option<super::Constant>>>()
                        + 2 * std::mem::size_of::<usize>(),
                )
            })
            .ok_or_else(|| vm_error("memory-limit", "runtime size overflow"))?;
        self.checked_runtime_size(staged_bytes, limits)?;
        let constants = Arc::new(class.constant_pool.clone());
        for member in &class.methods {
            let code = member.attributes.iter().find_map(|a| {
                if let Attribute::Code(c) = a {
                    Some(c)
                } else {
                    None
                }
            });
            let is_native = member.access_flags & 0x0100 != 0;
            // Abstract declarations still participate in symbolic resolution;
            // dispatch selects an implementation before execution.
            if code.is_none() && !is_native && member.access_flags & 0x0400 == 0 {
                continue;
            }
            let name = class
                .utf8(member.name_index)
                .ok_or_else(|| vm_error("invalid-method", "method has no name"))?
                .to_owned();
            let descriptor = class
                .utf8(member.descriptor_index)
                .ok_or_else(|| vm_error("invalid-method", "method has no descriptor"))?
                .to_owned();
            let parsed_descriptor = parse_method_descriptor(&descriptor)?;
            if member.access_flags & 0x0008 == 0 && parsed_descriptor.parameter_slots == u8::MAX {
                return Err(vm_error(
                    "invalid-descriptor",
                    "instance method parameters exceed 255 slots including the receiver",
                ));
            }
            let key = MethodKey {
                class: class_name.clone(),
                name,
                descriptor,
            };
            let instructions = code.map_or_else(|| Ok(Vec::new()), |code| decode(&code.code))?;
            let readonly_leaf = super::interpreter_batch::is_readonly_leaf(&instructions);
            let readonly_call_tree = super::interpreter_batch::is_readonly_call_tree(&instructions);
            let code_bytes = code.map_or_else(Vec::new, |code| code.code.clone());
            let instruction_index = build_instruction_index(code_bytes.len(), &instructions);
            let mut runtime_instructions =
                build_runtime_instructions(&instructions, &instruction_index)?;
            let first_fill = self
                .counted_array_fills
                .len()
                .checked_add(staged_counted_array_fills.len())
                .ok_or_else(|| vm_error("memory-limit", "array-fill count overflow"))?;
            let fills = link_counted_array_fills(
                &instructions,
                &constants,
                &mut runtime_instructions,
                first_fill,
            );
            staged_bytes = staged_bytes
                .checked_add(
                    fills
                        .len()
                        .checked_mul(std::mem::size_of::<CountedArrayFill>())
                        .ok_or_else(|| vm_error("memory-limit", "runtime size overflow"))?,
                )
                .ok_or_else(|| vm_error("memory-limit", "runtime size overflow"))?;
            staged_counted_array_fills.extend(fills);
            // A method owns three symbolic keys (map, method and stack trace),
            // five shared vectors, decoded operands and its parsed descriptor.
            // Counting only names and bytecode omitted the records themselves.
            let storage = [
                (code_bytes.capacity(), 1),
                (instructions.capacity(), std::mem::size_of::<Instruction>()),
                (instruction_index.capacity(), std::mem::size_of::<u16>()),
                (
                    runtime_instructions.capacity(),
                    std::mem::size_of::<RuntimeInstructionMeta>(),
                ),
                (
                    code.map_or(0, |code| code.exception_table.len()),
                    std::mem::size_of::<super::ExceptionHandler>(),
                ),
                (
                    parsed_descriptor.parameters.capacity(),
                    std::mem::size_of::<super::ValueKind>(),
                ),
                (3, key.class.len()),
                (3, key.name.len()),
                (3, key.descriptor.len()),
                (1, std::mem::size_of::<Method>()),
                (2, std::mem::size_of::<MethodKey>()),
                (1, std::mem::size_of::<Arc<MethodKey>>()),
                (5, std::mem::size_of::<Vec<u8>>()),
                // Two reference counters for each shared vector and stack key.
                (6 * 2, std::mem::size_of::<usize>()),
            ];
            let bytes = storage
                .into_iter()
                .chain(
                    instructions
                        .iter()
                        .map(|instruction| (instruction.operands.len(), 1)),
                )
                .try_fold(0_usize, |bytes, (count, size)| {
                    count
                        .checked_mul(size)
                        .and_then(|size| bytes.checked_add(size))
                        .ok_or_else(|| vm_error("memory-limit", "runtime size overflow"))
                })?;
            staged_bytes = staged_bytes
                .checked_add(bytes)
                .ok_or_else(|| vm_error("memory-limit", "runtime size overflow"))?;
            self.checked_runtime_size(staged_bytes, limits)?;
            let stack_key = Arc::new(key.clone());
            let stack_key_id = self
                .stack_keys
                .len()
                .checked_add(staged_stack_keys.len())
                .ok_or_else(|| vm_error("memory-limit", "method key count overflow"))?;
            staged_stack_keys.push(Arc::clone(&stack_key));
            let linked_method = Method {
                stack_key,
                stack_key_id: Some(stack_key_id),
                key: key.clone(),
                constant_pool_id: Some(constant_pool_id),
                code_fingerprint: fnv1a64(&code_bytes),
                max_stack: code.map_or(0, |code| usize::from(code.max_stack)),
                max_locals: code.map_or(
                    parsed_descriptor.parameters.len()
                        + usize::from(member.access_flags & 0x0008 == 0),
                    |code| usize::from(code.max_locals),
                ),
                code: Arc::new(code_bytes),
                instructions: Arc::new(instructions),
                runtime_instructions: Arc::new(runtime_instructions),
                instruction_index: Arc::new(instruction_index),
                constants: Arc::clone(&constants),
                descriptor: parsed_descriptor,
                is_static: member.access_flags & 0x0008 != 0,
                is_synchronized: member.access_flags & 0x0020 != 0,
                exception_table: Arc::new(
                    code.map_or_else(Vec::new, |code| code.exception_table.clone()),
                ),
                is_native,
                compiled_integer: None,
                readonly_leaf,
                readonly_call_tree,
                compatibility_candidate: None,
            };
            if self.methods.contains_key(&key) || staged.insert(key, linked_method).is_some() {
                return Err(vm_error("duplicate-method", "duplicate runtime method"));
            }
        }
        for method in staged.values_mut() {
            method.compatibility_candidate = Some(
                super::compatibility::compatibility_intrinsic_candidate(method),
            );
        }
        let super_name = if class.super_class == 0 {
            None
        } else {
            Some(
                class
                    .class_name(class.super_class)
                    .ok_or_else(|| vm_error("invalid-class", "invalid superclass"))?
                    .to_owned(),
            )
        };
        if let Some(parent) = super_name.as_deref() {
            staged_bytes = staged_bytes
                .checked_add(parent.len())
                .ok_or_else(|| vm_error("memory-limit", "runtime size overflow"))?;
            self.checked_runtime_size(staged_bytes, limits)?;
        }
        let mut interfaces = Vec::new();
        for index in &class.interfaces {
            let interface = class
                .class_name(*index)
                .ok_or_else(|| vm_error("invalid-class", "invalid interface"))?;
            staged_bytes = staged_bytes
                .checked_add(interface.len())
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<String>()))
                .ok_or_else(|| vm_error("memory-limit", "runtime size overflow"))?;
            self.checked_runtime_size(staged_bytes, limits)?;
            interfaces.push(interface.to_owned());
        }
        let mut fields = Vec::new();
        let mut field_names = HashSet::new();
        for member in &class.fields {
            let name = class
                .utf8(member.name_index)
                .ok_or_else(|| vm_error("invalid-field", "field has no name"))?;
            let descriptor = class
                .utf8(member.descriptor_index)
                .ok_or_else(|| vm_error("invalid-field", "field has no descriptor"))?;
            if !field_names.insert((name, descriptor)) {
                return Err(vm_error(
                    "duplicate-field",
                    format!("duplicate runtime field {class_name}.{name}:{descriptor}"),
                ));
            }
            let mut position = 0;
            let kind = parse_descriptor_type(descriptor.as_bytes(), &mut position)?;
            if position != descriptor.len() {
                return Err(vm_error("invalid-descriptor", descriptor));
            }
            let is_static = member.access_flags & 0x0008 != 0;
            let (initial, constant_string) = if is_static {
                constant_field_value(class, member, kind)?
            } else {
                (default_value(kind), None)
            };
            let key = field_key(&class_name, name, descriptor);
            staged_bytes = staged_bytes
                .checked_add(key.len())
                .and_then(|bytes| bytes.checked_add(class_name.len()))
                .and_then(|bytes| {
                    bytes.checked_add(
                        std::mem::size_of::<Field>() + 4 * std::mem::size_of::<usize>(),
                    )
                })
                .and_then(|bytes| {
                    constant_string
                        .as_ref()
                        .map_or(0, Vec::len)
                        .checked_mul(std::mem::size_of::<u16>())
                        .and_then(|string_bytes| bytes.checked_add(string_bytes))
                })
                .ok_or_else(|| vm_error("memory-limit", "runtime size overflow"))?;
            self.checked_runtime_size(staged_bytes, limits)?;
            fields.push(Field {
                key: key.into(),
                declaring_class: class_name.clone(),
                kind,
                is_static,
                field_token: FieldToken::new(),
                instance_slot: None,
                initial,
                constant_string,
            });
        }
        let no_arg_constructor = class
            .methods
            .iter()
            .find(|member| {
                class.utf8(member.name_index) == Some("<init>")
                    && class.utf8(member.descriptor_index) == Some("()V")
            })
            .map_or(NoArgConstructor::Missing, |member| {
                if member.access_flags & 0x0001 != 0 {
                    NoArgConstructor::Public
                } else {
                    NoArgConstructor::NonPublic
                }
            });
        let linked = Class {
            super_name,
            interfaces,
            fields,
            is_public: class.access_flags & 0x0001 != 0,
            is_abstract: class.access_flags & (0x0400 | 0x0200) != 0,
            is_interface: class.access_flags & 0x0200 != 0,
            no_arg_constructor,
        };
        // Forward references are legal, but the class that closes an
        // inheritance cycle must fail before any staged state is committed.
        self.validate_inheritance(&class_name, &linked)?;
        self.runtime_bytes = self.checked_runtime_size(staged_bytes, limits)?;
        self.constant_pool_count = next_constant_pool_count;
        self.stack_keys.extend(staged_stack_keys);
        self.counted_array_fills.extend(staged_counted_array_fills);
        self.methods.extend(staged);
        self.classes.insert(class_name, linked);
        Ok(())
    }

    fn checked_runtime_size(
        &self,
        staged_bytes: usize,
        limits: &Limits,
    ) -> Result<usize, EmuError> {
        let projected = self
            .runtime_bytes
            .checked_add(staged_bytes)
            .ok_or_else(|| vm_error("memory-limit", "runtime size overflow"))?;
        if projected > limits.max_runtime_bytes {
            return Err(vm_error(
                "memory-limit",
                format!(
                    "runtime representation exceeds {} bytes",
                    limits.max_runtime_bytes
                ),
            ));
        }
        Ok(projected)
    }

    fn validate_inheritance(&self, name: &str, class: &Class) -> Result<(), EmuError> {
        let mut pending = Vec::new();
        let mut seen = HashSet::new();
        if let Some(parent) = class.super_name.as_deref() {
            pending.push(parent);
        }
        pending.extend(class.interfaces.iter().map(String::as_str));
        while let Some(parent) = pending.pop() {
            if parent == name {
                return Err(vm_error("class-circularity", name));
            }
            if !seen.insert(parent) {
                continue;
            }
            if let Some(parent) = self.classes.get(parent) {
                pending.extend(parent.super_name.as_deref());
                pending.extend(parent.interfaces.iter().map(String::as_str));
            }
        }
        Ok(())
    }
}
