use super::{
    Allocation, EmuError, Field, FieldRef, FieldRuntimeSlots, Handle, HashSet, Machine, Method,
    MethodKey, ResolvedMethodRef, Value, VirtualMethodTarget, array_descriptor, display_key,
    heap_error, reference_component, resolve_any_method, resolve_field, type_error, vm_error,
};
use crate::machine::constant_pool::field_key;
use std::borrow::Cow;

impl Machine<'_, '_> {
    pub(in crate::machine) fn object_class(
        &self,
        handle: Handle,
    ) -> Result<Cow<'_, str>, EmuError> {
        match self.heap.managed.get(handle).map_err(heap_error)? {
            Allocation::Object { class, .. } => Ok(Cow::Borrowed(class)),
            Allocation::Array { kind, .. } => Ok(array_descriptor(kind)),
        }
    }

    pub(in crate::machine) fn is_instance(&self, mut actual: &str, mut target: &str) -> bool {
        if actual == target {
            return true;
        }
        // Peel array dimensions without recursive host frames or comparing the
        // entire remaining descriptor again at every level.
        while let Some(actual_component) = actual.strip_prefix('[') {
            if matches!(
                target,
                "java/lang/Object" | "java/lang/Cloneable" | "java/io/Serializable"
            ) {
                return true;
            }
            let Some(target_component) = target.strip_prefix('[') else {
                return false;
            };
            match (
                reference_component(actual_component),
                reference_component(target_component),
            ) {
                (Some(actual_class), Some(target_class)) => {
                    actual = actual_class;
                    target = target_class;
                }
                _ => return actual_component == target_component,
            }
        }
        target == "java/lang/Object" || self.program.is_assignable_to(actual, target)
    }

    pub(in crate::machine) fn resolve_virtual(
        &self,
        class: &str,
        name: &str,
        descriptor: &str,
    ) -> Result<MethodKey, EmuError> {
        // Every Java array type derives directly from Object. Array classes
        // are represented by descriptors rather than synthetic Class entries,
        // so begin virtual lookup at Object for calls such as array.getClass().
        let mut key = MethodKey {
            class: if class.starts_with('[') {
                "java/lang/Object"
            } else {
                class
            }
            .to_owned(),
            name: name.to_owned(),
            descriptor: descriptor.to_owned(),
        };
        // A superclass chain has at most one edge per linked class. A hop
        // bound retains cycle protection without allocating a visited set.
        let mut remaining = self.program.classes.len();
        loop {
            if self.program.methods.contains_key(&key) {
                return Ok(key);
            }
            let Some(parent) = self
                .program
                .classes
                .get(&key.class)
                .and_then(|class| class.super_name.as_ref())
            else {
                break;
            };
            if remaining == 0 {
                return Err(vm_error("class-circularity", &key.class));
            }
            remaining -= 1;
            key.class.clone_from(parent);
        }
        Err(vm_error(
            "abstract-method",
            format!("{class}::{name}{descriptor}"),
        ))
    }

    pub(in crate::machine) fn resolve_method_ref_cached(
        &mut self,
        caller: &Method,
        index: u16,
    ) -> Result<std::rc::Rc<ResolvedMethodRef>, EmuError> {
        if let Some(constant_pool_id) = caller.constant_pool_id
            && let Some(reference) = self
                .classes
                .method_ref_inline_cache
                .get(constant_pool_id, index)
        {
            return Ok(std::rc::Rc::clone(reference));
        }
        if let Some(reference) = self
            .classes
            .constant_pool_cache(caller)
            .and_then(|cache| cache.resolved_method_refs.get(index))
            .map(std::rc::Rc::clone)
        {
            if let Some(constant_pool_id) = caller.constant_pool_id {
                self.classes.method_ref_inline_cache.insert(
                    constant_pool_id,
                    index,
                    std::rc::Rc::clone(&reference),
                );
            }
            return Ok(reference);
        }
        let symbolic = resolve_any_method(&caller.constants, index)?;
        let resolved = self.resolve_symbolic_method(&symbolic)?;
        let declaration = self
            .program
            .methods
            .get(&resolved)
            .ok_or_else(|| vm_error("method-not-found", display_key(&resolved)))?;
        let reference = std::rc::Rc::new(ResolvedMethodRef {
            symbolic,
            descriptor: declaration.descriptor.clone(),
            is_static: declaration.is_static,
        });
        if let Some(cache) = self.classes.constant_pool_cache_mut(caller) {
            cache
                .resolved_method_refs
                .insert(index, std::rc::Rc::clone(&reference));
        }
        if let Some(constant_pool_id) = caller.constant_pool_id {
            self.classes.method_ref_inline_cache.insert(
                constant_pool_id,
                index,
                std::rc::Rc::clone(&reference),
            );
        }
        Ok(reference)
    }

    pub(in crate::machine) fn fixed_method_inline_cached(
        &self,
        caller: &Method,
        index: u16,
        opcode: u8,
    ) -> Option<(std::rc::Rc<Method>, Option<usize>)> {
        let constant_pool_id = caller.constant_pool_id?;
        self.classes
            .fixed_method_inline_cache
            .get(constant_pool_id, index, opcode)
            .map(|(method, class)| (std::rc::Rc::clone(method), class))
    }

    pub(in crate::machine) fn resolve_fixed_method_cached(
        &mut self,
        caller: &Method,
        index: u16,
        opcode: u8,
        symbolic: &MethodKey,
    ) -> Result<(std::rc::Rc<Method>, Option<usize>), EmuError> {
        if let Some(method) = self.fixed_method_inline_cached(caller, index, opcode) {
            return Ok(method);
        }
        let cached = self.classes.constant_pool_cache(caller).and_then(|cache| {
            let targets = if opcode == 0xb7 {
                &cache.special_method_targets
            } else {
                &cache.static_method_targets
            };
            targets.get(index)
        });
        let method = if let Some(method) = cached {
            std::rc::Rc::clone(method)
        } else {
            let key = self.resolve_symbolic_method(symbolic)?;
            let method = std::rc::Rc::new(
                self.program
                    .methods
                    .get(&key)
                    .cloned()
                    .ok_or_else(|| vm_error("method-not-found", display_key(&key)))?,
            );
            if let Some(cache) = self.classes.constant_pool_cache_mut(caller) {
                let targets = if opcode == 0xb7 {
                    &mut cache.special_method_targets
                } else {
                    &mut cache.static_method_targets
                };
                targets.insert(index, std::rc::Rc::clone(&method));
            }
            method
        };
        let class = self.classes.initialized.slot(&method.key.class);
        if let Some(constant_pool_id) = caller.constant_pool_id {
            self.classes.fixed_method_inline_cache.insert(
                constant_pool_id,
                index,
                opcode,
                std::rc::Rc::clone(&method),
                class,
            );
        }
        Ok((method, class))
    }

    pub(in crate::machine) fn resolve_virtual_method_cached(
        &mut self,
        caller: &Method,
        index: u16,
        receiver: Handle,
        symbolic: &MethodKey,
    ) -> Result<std::rc::Rc<Method>, EmuError> {
        // The overwhelming majority of Java ME call sites are monomorphic.
        // Check the cached receiver class without allocating a String on the
        // hit path. Arrays are rare virtual receivers and use the generic
        // fallback below.
        if let Allocation::Object { class, .. } =
            self.heap.managed.get(receiver).map_err(heap_error)?
            && let Some(cached) = self
                .classes
                .constant_pool_cache(caller)
                .and_then(|cache| cache.virtual_method_targets.get(index))
            && cached.receiver_class.as_str() == class.as_ref()
        {
            return Ok(std::rc::Rc::clone(&cached.callee));
        }

        let receiver_class = self.object_class(receiver)?.into_owned();
        if !self.is_instance(&receiver_class, &symbolic.class) {
            return Err(vm_error(
                "incompatible-class-change",
                format!("{receiver_class} is not assignable to {}", symbolic.class),
            ));
        }
        let target = self.resolve_virtual(&receiver_class, &symbolic.name, &symbolic.descriptor)?;
        let callee = std::rc::Rc::new(
            self.program
                .methods
                .get(&target)
                .cloned()
                .ok_or_else(|| vm_error("method-not-found", display_key(&target)))?,
        );
        if let Some(cache) = self.classes.constant_pool_cache_mut(caller) {
            cache.virtual_method_targets.insert(
                index,
                VirtualMethodTarget {
                    string_equals_ignore_case: receiver_class == "java/lang/String"
                        && callee.key.class == "java/lang/String"
                        && callee.key.name == "equalsIgnoreCase"
                        && callee.key.descriptor == "(Ljava/lang/String;)Z"
                        && !callee.is_static
                        && !callee.is_synchronized,
                    receiver_class,
                    callee: std::rc::Rc::clone(&callee),
                },
            );
        }
        Ok(callee)
    }

    pub(in crate::machine) fn resolve_symbolic_method(
        &self,
        symbolic: &MethodKey,
    ) -> Result<MethodKey, EmuError> {
        // A constructor must be declared on the symbolic owner. Reject the
        // inherited case before caching a target or inspecting the receiver.
        if symbolic.name == "<init>" {
            return self
                .program
                .methods
                .get_key_value(symbolic)
                .map(|(key, _)| key.clone())
                .ok_or_else(|| vm_error("method-not-found", display_key(symbolic)));
        }
        let mut pending = vec![if symbolic.class.starts_with('[') {
            "java/lang/Object"
        } else {
            symbolic.class.as_str()
        }];
        let mut seen = HashSet::new();
        while let Some(owner) = pending.pop() {
            if !seen.insert(owner) {
                continue;
            }
            let key = MethodKey {
                class: owner.to_owned(),
                name: symbolic.name.clone(),
                descriptor: symbolic.descriptor.clone(),
            };
            if self.program.methods.contains_key(&key) {
                return Ok(key);
            }
            if let Some(class) = self.program.classes.get(owner) {
                pending.extend(class.interfaces.iter().map(String::as_str));
                if let Some(parent) = class.super_name.as_deref() {
                    pending.push(parent);
                }
            }
        }
        Err(vm_error("method-not-found", display_key(symbolic)))
    }

    pub(in crate::machine) fn instance_fields(&self, class: &str) -> Result<Vec<&Field>, EmuError> {
        let mut hierarchy = Vec::new();
        let mut current = Some(class);
        while let Some(name) = current {
            if name == "java/lang/Object" && !self.program.classes.contains_key(name) {
                break;
            }
            let definition = self
                .program
                .classes
                .get(name)
                .ok_or_else(|| vm_error("class-not-found", name))?;
            if hierarchy.len() >= self.program.classes.len() {
                return Err(vm_error("class-circularity", name));
            }
            hierarchy.push(definition);
            current = definition.super_name.as_deref();
        }
        let mut fields = Vec::new();
        for definition in hierarchy.into_iter().rev() {
            fields.extend(definition.fields.iter().filter(|field| !field.is_static));
        }
        Ok(fields)
    }

    pub(in crate::machine) fn resolve_field_cached(
        &mut self,
        method: &Method,
        index: u16,
    ) -> Result<(std::rc::Rc<Field>, FieldRuntimeSlots), EmuError> {
        if let Some(constant_pool_id) = method.constant_pool_id
            && let Some((field, runtime_slots)) = self
                .classes
                .field_inline_cache
                .get_resolved(constant_pool_id, index)
        {
            return Ok((std::rc::Rc::clone(field), runtime_slots));
        }
        let cached = self
            .classes
            .constant_pool_cache(method)
            .and_then(|cache| cache.resolved_fields.get(index))
            .map(std::rc::Rc::clone);
        if let Some(field) = cached {
            let runtime_slots = self.classes.runtime_slots(&field);
            if let Some(constant_pool_id) = method.constant_pool_id {
                self.classes.field_inline_cache.insert_resolved(
                    constant_pool_id,
                    index,
                    std::rc::Rc::clone(&field),
                    runtime_slots,
                );
            }
            return Ok((field, runtime_slots));
        }
        let reference = resolve_field(&method.constants, index)?;
        let mut field = self.find_field(&reference)?;
        if !field.is_static {
            field.instance_slot = self
                .instance_fields(&field.declaring_class)?
                .iter()
                .position(|candidate| candidate.key == field.key);
            if field.instance_slot.is_none() {
                return Err(vm_error("field-not-found", reference.id()));
            }
        }
        let field = std::rc::Rc::new(field);
        let runtime_slots = self.classes.runtime_slots(&field);
        if let Some(cache) = self.classes.constant_pool_cache_mut(method) {
            cache
                .resolved_fields
                .insert(index, std::rc::Rc::clone(&field));
        }
        if let Some(constant_pool_id) = method.constant_pool_id {
            self.classes.field_inline_cache.insert_resolved(
                constant_pool_id,
                index,
                std::rc::Rc::clone(&field),
                runtime_slots,
            );
        }
        Ok((field, runtime_slots))
    }

    pub(in crate::machine) fn find_field(&self, reference: &FieldRef) -> Result<Field, EmuError> {
        let suffix = field_key("", &reference.name, &reference.descriptor);
        let mut pending = vec![(reference.class.as_str(), false)];
        let mut active = HashSet::new();
        let mut visited = HashSet::new();
        while let Some((name, leaving)) = pending.pop() {
            if leaving {
                active.remove(name);
                visited.insert(name);
                continue;
            }
            if visited.contains(name) {
                continue;
            }
            if !active.insert(name) {
                return Err(vm_error("class-circularity", name));
            }
            let class = self
                .program
                .classes
                .get(name)
                .ok_or_else(|| vm_error("class-not-found", name))?;
            if let Some(field) = class
                .fields
                .iter()
                .find(|field| field.key.ends_with(&suffix))
            {
                return Ok(field.clone());
            }
            pending.push((name, true));
            if !class.is_interface
                && let Some(super_name) = class.super_name.as_deref()
            {
                pending.push((super_name, false));
            }
            for interface in class.interfaces.iter().rev() {
                pending.push((interface, false));
            }
        }
        Err(vm_error("field-not-found", reference.id()))
    }

    pub(in crate::machine) fn validate_reference_component(
        &self,
        component: &str,
        value: Value,
    ) -> Result<(), EmuError> {
        let Value::Reference(reference) = value else {
            return Err(type_error());
        };
        let Some(value) = reference else {
            return Ok(());
        };
        let actual = self.object_class(value)?;
        if self.is_instance(&actual, component) {
            Ok(())
        } else {
            Err(vm_error(
                "array-store-exception",
                format!("cannot store {actual} in {component}[]"),
            ))
        }
    }
}
