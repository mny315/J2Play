use super::*;

mod application_bytecode;
mod array_access;
mod byte_streams;
mod font_mapping;
mod graphics_scalars;
mod short_tables;
mod static_tables;
mod strings;

// Install the independently assembled application methods and their field
// declarations. They execute through ordinary bytecode and the generic leaf
// engine, never an algorithm-specific replacement.
fn bytecode_program(methods: &mut [&mut Method]) -> Program {
    let mut program = program_with_core_natives();
    for method in methods {
        method.readonly_leaf =
            crate::machine::interpreter_batch::is_readonly_leaf(&method.instructions);
        method.readonly_call_tree =
            crate::machine::interpreter_batch::is_readonly_call_tree(&method.instructions);
        method.stack_key_id = Some(program.stack_keys.len());
        program.stack_keys.push(Arc::clone(&method.stack_key));
        method.constant_pool_id = Some(program.constant_pool_count);
        program.constant_pool_count += 1;
        program
            .classes
            .entry(method.key.class.clone())
            .or_insert_with(|| test_class_definition(None));
        for instruction in method.instructions.iter() {
            if !matches!(instruction.opcode, 0xb2..=0xb5) {
                continue;
            }
            let index = u16::from_be_bytes(instruction.operands[..2].try_into().unwrap());
            let field = resolve_field(&method.constants, index).unwrap();
            let key: Arc<str> =
                format!("{}.{}:{}", field.class, field.name, field.descriptor).into();
            let class = program
                .classes
                .entry(field.class.clone())
                .or_insert_with(|| test_class_definition(None));
            if class.fields.iter().any(|existing| existing.key == key) {
                continue;
            }
            let mut offset = 0;
            let kind = parse_descriptor_type(field.descriptor.as_bytes(), &mut offset).unwrap();
            class.fields.push(Field {
                key,
                declaring_class: field.class,
                kind,
                is_static: matches!(instruction.opcode, 0xb2 | 0xb3),
                field_token: FieldToken::new(),
                instance_slot: None,
                initial: default_value(kind),
                constant_string: None,
            });
        }
        program
            .methods
            .insert(method.key.clone(), (**method).clone());
    }
    program
}

fn push_constant(constants: &mut Vec<Option<Constant>>, value: Constant) -> u16 {
    let index = u16::try_from(constants.len()).unwrap();
    constants.push(Some(value));
    index
}

fn push_class(constants: &mut Vec<Option<Constant>>, name: &str) -> u16 {
    let name_index = push_constant(constants, Constant::Utf8(name.to_owned()));
    push_constant(constants, Constant::Class { name_index })
}

fn push_name_and_type(constants: &mut Vec<Option<Constant>>, name: &str, descriptor: &str) -> u16 {
    let name_index = push_constant(constants, Constant::Utf8(name.to_owned()));
    let descriptor_index = push_constant(constants, Constant::Utf8(descriptor.to_owned()));
    push_constant(
        constants,
        Constant::NameAndType {
            name_index,
            descriptor_index,
        },
    )
}

fn reference(code: &mut Vec<u8>, opcode: u8, index: u16) {
    code.push(opcode);
    code.extend_from_slice(&index.to_be_bytes());
}
