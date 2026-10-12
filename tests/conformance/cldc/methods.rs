use diagnostics::EmuError;
use natives::HostServices;
use std::path::PathBuf;

#[test]
fn cldc_methods_execute_across_builtin_families() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "cldc_methods_execute_across_builtin_families"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for profile in ["se-featurephone", "nokia-featurephone"] {
        let output =
            crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
                .profile(profile)
                .run_static("fixtures/CldcConsole", "stringSurfaceProbe", "()I");
        assert!(output.success(), "{profile}: {}", output.diagnostics);
        let diagnostics = &output.diagnostics;
        assert_eq!(output.int_value(), Some(28), "{profile}: {diagnostics}");
    }
}

#[test]
fn every_public_method_family_checkpoint_executes_on_the_rust_vm() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "every_public_method_family_checkpoint_executes_on_the_rust_vm"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let checkpoints = [
        ("ioProbe", 16),
        ("calendarProbe", 32),
        ("timerProbe", 34),
        ("collectionsProbe", 30),
        ("stringBufferProbe", 27),
        ("resourceProbe", 29),
        ("arrayCopyProbe", 39),
        ("exceptionProbe", 7),
        ("wrappersProbe", 31),
    ];
    for (method, expected) in checkpoints {
        let output =
            crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
                .profile(repository.join("profiles/sony-ericsson/featurephone.json"))
                .run_static("fixtures/CldcConsole", method, "()I");
        assert!(output.success(), "{method}: {}", output.diagnostics);
        let diagnostics = &output.diagnostics;
        assert_eq!(
            output.int_value(),
            Some(expected),
            "{method}: {diagnostics}"
        );
    }
}

#[test]
fn every_public_bootstrap_method_has_a_direct_executable_fixture() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "every_public_bootstrap_method_has_a_direct_executable_fixture"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let profile = device_profile::DeviceProfile::from_reader(
        std::fs::File::open(repository.join("profiles/sony-ericsson/featurephone.json")).unwrap(),
    )
    .unwrap();
    let mut limits = vm::Limits::default();
    if let Some(heap_bytes) = profile.limits().heap_bytes().value().copied() {
        limits.max_heap_bytes = usize::try_from(heap_bytes).unwrap();
    }
    let mut program = vm::Program::new();
    for class in runtime_bootstrap::production_bootstrap_classes() {
        program.add_class(&class, &limits).unwrap();
    }
    for resource in
        jar::read_class_entries_path(repository.join("tests/fixtures/java-me/conformance.jar"))
            .unwrap()
    {
        let class = classfile::parse(&resource.bytes).unwrap();
        let class_name = class.class_name(class.this_class).unwrap();
        if !program.contains_class(class_name) {
            program.add_class(&class, &limits).unwrap();
        }
    }
    cldc::register_core_natives(program.native_registry_mut()).unwrap();
    let mut context = MethodFixtureContext {
        properties: cldc::SystemProperties::from_profile(&profile),
        resources: jar::ResourceArchive::open(
            repository.join("tests/fixtures/java-me/conformance.jar"),
        )
        .unwrap(),
    };

    let inventory: MethodFixtureInventory = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/java-me/cldc-methods.json"
    )))
    .unwrap();
    assert!(!inventory.fixtures.is_empty());
    for fixture in inventory.fixtures {
        let (class, method) = fixture.entrypoint.split_once("::").unwrap();
        let method = method.strip_suffix("()I").unwrap();
        let execution = program
            .execute_with_context(class, method, "()I", limits.clone(), false, &mut context)
            .unwrap_or_else(|error| panic!("{method}: {error}"));
        assert_eq!(
            execution.value,
            Some(vm::Value::Int(fixture.expected)),
            "{method}"
        );
        assert_eq!(execution.thread_failure_count, 0, "{method}");
    }
}

#[derive(serde::Deserialize)]
struct MethodFixtureInventory {
    fixtures: Vec<MethodFixture>,
}

#[derive(serde::Deserialize)]
struct MethodFixture {
    entrypoint: String,
    expected: i32,
}

struct MethodFixtureContext {
    properties: cldc::SystemProperties,
    resources: jar::ResourceArchive,
}

impl HostServices for MethodFixtureContext {
    fn monotonic_millis(&self) -> i64 {
        0
    }

    fn wall_clock_millis(&self) -> i64 {
        1_650_000_000_000
    }

    fn system_property(&self, name: &str) -> Option<&str> {
        self.properties.get(name)
    }

    fn read_resource(&self, name: &str) -> Result<Option<Vec<u8>>, EmuError> {
        self.resources.read(name)
    }

    fn write_console_error(&mut self, _: &str) -> Result<(), EmuError> {
        Ok(())
    }

    fn write_console_output(&mut self, _: &str, _: bool) -> Result<(), EmuError> {
        Ok(())
    }
}

#[test]
fn core_class_contracts_execute_with_rust_bootstrap() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::core_class_contracts_execute_with_rust_bootstrap"
    )) {
        return;
    }
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (method, expected) in [
        ("numberDefaults", 3),
        ("throwableHierarchy", 4),
        ("mathEdgeCases", 5),
    ] {
        let output =
            crate::support::Fixture::new(repository.join("tests/fixtures/java-me/conformance.jar"))
                .run_static("fixtures/BootstrapMigrationFixtures", method, "()I");
        assert!(output.success(), "{method}: {}", output.diagnostics);
        assert_eq!(output.int_value(), Some(expected), "{method}");
    }
}
