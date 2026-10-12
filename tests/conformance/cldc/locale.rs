use crate::support::guest::{MethodCode, Pool, code_method, emit_reference};

// This synthetic guest executes System.getProperty instead of inspecting host state.
fn locale_probe() -> classfile::ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class("fixtures/GuestLocale");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let mut code = Vec::new();
    emit_reference(&mut code, 0x13, pool.string("microedition.locale"));
    emit_reference(
        &mut code,
        0xb8,
        pool.method(
            "java/lang/System",
            "getProperty",
            "(Ljava/lang/String;)Ljava/lang/String;",
        ),
    );
    emit_reference(
        &mut code,
        0xb6,
        pool.method("java/lang/String", "hashCode", "()I"),
    );
    code.push(0xac);
    let method = code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 0x0009,
            name: "readLocale",
            descriptor: "()I",
            max_stack: 1,
            max_locals: 0,
            code,
        },
    );
    classfile::ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: 0x0021,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods: vec![method],
        attributes: Vec::new(),
    }
}

struct LocaleHost(cldc::SystemProperties);

impl natives::HostServices for LocaleHost {
    fn monotonic_millis(&self) -> i64 {
        0
    }
    fn wall_clock_millis(&self) -> i64 {
        0
    }
    fn system_property(&self, name: &str) -> Option<&str> {
        self.0.get(name)
    }
    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, diagnostics::EmuError> {
        Ok(None)
    }
}

#[test]
fn selected_language_reaches_guest_system_get_property() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::selected_language_reaches_guest_system_get_property"
    )) {
        return;
    }
    let profiles = launch::builtin_device_profiles().unwrap();
    let profile = profiles
        .iter()
        .find(|p| p.profile_id() == "nokia-featurephone")
        .unwrap();
    let base = profile
        .resolve(device_profile::ProfileOverrides::default())
        .unwrap();
    for locale in [
        None,
        Some("he-IL"),
        Some("ru-RU"),
        Some("iw-IL"),
        Some("pt-BR"),
        Some("ru"),
    ] {
        let resolved = locale.map_or(base, |value| base.with_guest_locale(value).unwrap());
        let mut host = LocaleHost(cldc::SystemProperties::from_resolved_profile(&resolved));
        let expected = locale
            .unwrap_or("en-US")
            .encode_utf16()
            .fold(0_i32, |hash, unit| {
                hash.wrapping_mul(31).wrapping_add(i32::from(unit))
            });
        let mut program = vm::Program::new();
        let limits = vm::Limits {
            max_instructions: 100_000,
            ..vm::Limits::default()
        };
        runtime::install_rust_bootstrap(
            &mut program,
            &limits,
            &resolved,
            &std::collections::HashSet::<String>::new(),
            false,
        )
        .unwrap();
        runtime::register_profile_natives(&mut program, profile).unwrap();
        program.add_class(&locale_probe(), &limits).unwrap();
        let execution = program
            .execute_with_context(
                "fixtures/GuestLocale",
                "readLocale",
                "()I",
                limits,
                false,
                &mut host,
            )
            .unwrap();
        assert_eq!(
            execution.value,
            Some(vm::Value::Int(expected)),
            "{locale:?}"
        );
    }
}
