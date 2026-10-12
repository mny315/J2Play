use super::*;

fn fixture() -> ClassFile {
    ClassFile {
        minor_version: 0,
        major_version: 50,
        constant_pool: vec![
            None,
            Some(Constant::Class { name_index: 2 }),
            Some(Constant::Utf8("Budget".into())),
            Some(Constant::Utf8("a".into())),
            Some(Constant::Utf8("b".into())),
            Some(Constant::Utf8(format!("L{};", "X".repeat(8_192)))),
            Some(Constant::Class { name_index: 7 }),
            Some(Constant::Utf8(format!("A{}", "X".repeat(8_192)))),
            Some(Constant::Class { name_index: 9 }),
            Some(Constant::Utf8(format!("B{}", "X".repeat(8_192)))),
        ],
        access_flags: 0x0021,
        this_class: 1,
        super_class: 0,
        interfaces: vec![],
        fields: vec![],
        methods: vec![],
        attributes: vec![],
    }
}

#[test]
fn metadata_budget_stops_linking_before_later_members_and_rolls_back() {
    let mut codes = Vec::new();
    for kind in 0..3 {
        let mut class = fixture();
        let cost = |class: &ClassFile| {
            let mut program = Program::new();
            program.add_class(class, &Limits::default()).unwrap();
            program.runtime_bytes
        };
        let base = cost(&class);
        let field = |name_index| classfile::Member {
            access_flags: 0x0001,
            name_index,
            descriptor_index: 5,
            attributes: vec![],
        };
        let budget = match kind {
            0 => {
                class.methods.push(classfile::Member {
                    access_flags: 0x0109,
                    name_index: 0,
                    descriptor_index: 0,
                    attributes: vec![],
                });
                base - 1
            }
            1 => {
                class.interfaces = vec![6];
                let budget = cost(&class);
                class.interfaces = vec![6, 8, 0];
                budget
            }
            _ => {
                class.fields = vec![field(3)];
                let budget = cost(&class);
                class.fields = vec![field(3), field(4), field(0)];
                budget
            }
        };
        let limits = Limits {
            max_runtime_bytes: budget,
            ..Limits::default()
        };
        let mut program = Program::new();
        codes.push(
            program
                .add_class(&class, &limits)
                .unwrap_err()
                .code()
                .to_owned(),
        );
        assert!(program.classes.is_empty());
        assert!(program.methods.is_empty());
        assert!(program.stack_keys.is_empty());
        assert!(program.counted_array_fills.is_empty());
        assert_eq!(program.runtime_bytes, 0);
        assert_eq!(program.constant_pool_count, 0);

        // The exact same boundary accepts a class whose metadata fits. No
        // failed attempt may reserve a pool ID or leave partial linked state.
        class.methods.clear();
        class.interfaces.truncate(1);
        class.fields.truncate(1);
        let limits = Limits {
            max_runtime_bytes: budget.max(base),
            ..Limits::default()
        };
        program.add_class(&class, &limits).unwrap();
        assert!(program.contains_class("Budget"));
        assert_eq!(program.runtime_bytes, limits.max_runtime_bytes);
        assert_eq!(program.constant_pool_count, 1);
    }
    assert_eq!(codes, ["memory-limit", "memory-limit", "memory-limit"]);
}

#[test]
fn runtime_budget_accounts_for_field_and_method_records() {
    for methods in [false, true] {
        let mut class = fixture();
        class.constant_pool[5] = Some(Constant::Utf8(if methods { "()V" } else { "I" }.into()));
        for index in 0..128 {
            let name_index = u16::try_from(class.constant_pool.len()).unwrap();
            class
                .constant_pool
                .push(Some(Constant::Utf8(format!("f{index}"))));
            let member = classfile::Member {
                access_flags: if methods { 0x0109 } else { 0x0001 },
                name_index,
                descriptor_index: 5,
                attributes: vec![],
            };
            if methods {
                class.methods.push(member);
            } else {
                class.fields.push(member);
            }
        }
        let record_bytes = 128
            * if methods {
                std::mem::size_of::<Method>()
            } else {
                std::mem::size_of::<Field>()
            };
        // Even ignoring strings, shared allocation headers and map entries,
        // the owned records alone must be included in the representation limit.
        let minimum = estimate_constant_pool(&class.constant_pool).unwrap() + record_bytes;
        let limits = Limits {
            max_runtime_bytes: minimum,
            ..Limits::default()
        };
        let mut program = Program::new();
        assert_eq!(
            program.add_class(&class, &limits).unwrap_err().code(),
            "memory-limit"
        );
        assert!(program.classes.is_empty());
        assert!(program.methods.is_empty());
        program.add_class(&class, &Limits::default()).unwrap();
        assert!(program.runtime_bytes > minimum);
    }
}
