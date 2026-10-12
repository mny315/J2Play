//! Exercise the private date parser through its production native bridge.

use diagnostics::EmuError;
use natives::{HostServices, NativeRegistry, NativeSignature, NativeValue, VmAccess};

pub(super) fn exercise(input: &[u8]) {
    let Some((clock, text)) = input.split_at_checked(8) else {
        return;
    };
    let mut context = DateContext {
        now_millis: i64::from_le_bytes(clock.try_into().expect("eight clock bytes")),
        text: String::from_utf8_lossy(text).into_owned(),
    };
    let mut registry = NativeRegistry::new();
    gcf::register_natives(&mut registry).expect("empty native registry");
    let _ = registry.invoke(
        &NativeSignature::new(
            "javax/microedition/io/HttpConnectionImpl",
            "parseDate0",
            "(Ljava/lang/String;)J",
        ),
        &mut context,
        &[NativeValue::Reference(Some(1))],
    );
}

struct DateContext {
    now_millis: i64,
    text: String,
}

impl HostServices for DateContext {
    fn monotonic_millis(&self) -> i64 {
        0
    }

    fn wall_clock_millis(&self) -> i64 {
        self.now_millis
    }

    fn system_property(&self, _: &str) -> Option<&str> {
        None
    }

    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, EmuError> {
        Ok(None)
    }
}

impl VmAccess for DateContext {
    fn read_java_string(&self, reference: u64) -> Result<String, EmuError> {
        assert_eq!(reference, 1);
        Ok(self.text.clone())
    }
}
