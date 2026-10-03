//! Build-only Android callback declarations. All method implementations are Rust.
//! Emits class files directly; no Java source compiler or embedded Java source.

use std::fs;
use std::io;
use std::path::Path;

const PACKAGE: &str = "io/github/mny315/j2play";
const OWNER: &str = "Lio/github/mny315/j2play/J2PlayActivity;";

#[derive(Default)]
struct Class {
    pool: Vec<Vec<u8>>,
    fields: Vec<Vec<u8>>,
    methods: Vec<Vec<u8>>,
}

fn u16_bytes(value: usize) -> [u8; 2] {
    u16::try_from(value)
        .expect("bounded callback class")
        .to_be_bytes()
}

impl Class {
    fn constant(&mut self, bytes: Vec<u8>) -> u16 {
        if let Some(index) = self.pool.iter().position(|entry| *entry == bytes) {
            return u16::try_from(index + 1).expect("bounded constant pool");
        }
        self.pool.push(bytes);
        u16::try_from(self.pool.len()).expect("bounded constant pool")
    }

    fn utf8(&mut self, value: &str) -> u16 {
        assert!(value.is_ascii(), "callback descriptors are ASCII");
        let mut bytes = vec![1];
        bytes.extend(u16_bytes(value.len()));
        bytes.extend(value.as_bytes());
        self.constant(bytes)
    }

    fn reference(&mut self, tag: u8, left: u16, right: Option<u16>) -> u16 {
        let mut bytes = vec![tag];
        bytes.extend(left.to_be_bytes());
        if let Some(right) = right {
            bytes.extend(right.to_be_bytes());
        }
        self.constant(bytes)
    }

    fn class(&mut self, name: &str) -> u16 {
        let name = self.utf8(name);
        self.reference(7, name, None)
    }

    fn member(&mut self, tag: u8, class: &str, name: &str, descriptor: &str) -> u16 {
        let class = self.class(class);
        let name = self.utf8(name);
        let descriptor = self.utf8(descriptor);
        let pair = self.reference(12, name, Some(descriptor));
        self.reference(tag, class, Some(pair))
    }

    fn method(&mut self, name: &str, descriptor: &str, code: Option<(u16, Vec<u8>)>) {
        let flags: u16 = if name == "<clinit>" {
            0x0008
        } else if code.is_some() {
            0x0001
        } else {
            0x0101
        };
        let mut bytes = flags.to_be_bytes().to_vec();
        bytes.extend(self.utf8(name).to_be_bytes());
        bytes.extend(self.utf8(descriptor).to_be_bytes());
        bytes.extend(u16::from(code.is_some()).to_be_bytes());
        if let Some((locals, code)) = code {
            bytes.extend(self.utf8("Code").to_be_bytes());
            bytes.extend(u32::try_from(code.len() + 12).unwrap().to_be_bytes());
            bytes.extend(4_u16.to_be_bytes());
            bytes.extend(locals.to_be_bytes());
            bytes.extend(u32::try_from(code.len()).unwrap().to_be_bytes());
            bytes.extend(code);
            bytes.extend([0; 4]); // no exception handlers or nested attributes
        }
        self.methods.push(bytes);
    }

    fn write(
        mut self,
        root: &Path,
        name: &str,
        parent: &str,
        interfaces: &[&str],
    ) -> io::Result<()> {
        let name = format!("{PACKAGE}/{name}");
        let this = self.class(&name);
        let parent = self.class(parent);
        let interfaces: Vec<_> = interfaces.iter().map(|name| self.class(name)).collect();
        let mut bytes = vec![0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 49];
        bytes.extend(u16_bytes(self.pool.len() + 1));
        for entry in self.pool {
            bytes.extend(entry);
        }
        bytes.extend(0x0031_u16.to_be_bytes()); // public final, ACC_SUPER
        bytes.extend(this.to_be_bytes());
        bytes.extend(parent.to_be_bytes());
        bytes.extend(u16_bytes(interfaces.len()));
        for interface in interfaces {
            bytes.extend(interface.to_be_bytes());
        }
        bytes.extend(u16_bytes(self.fields.len()));
        for field in self.fields {
            bytes.extend(field);
        }
        bytes.extend(u16_bytes(self.methods.len()));
        for method in self.methods {
            bytes.extend(method);
        }
        bytes.extend([0; 2]);
        let path = root.join(format!("{name}.class"));
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, bytes)
    }
}

fn constructor(class: &mut Class, name: &str, parent: &str, connection: bool) {
    let activity = name == "J2PlayActivity";
    let mut code = vec![0x2a]; // aload_0
    let descriptor = if activity {
        "()V"
    } else if connection {
        code.extend([0x2b, 0x03]); // connection, immutable=false
        "(Landroid/view/inputmethod/InputConnection;Z)V"
    } else if name == "BridgeEditor" {
        code.push(0x2b);
        "(Landroid/content/Context;)V"
    } else {
        "()V"
    };
    code.push(0xb7);
    code.extend(class.member(10, parent, "<init>", descriptor).to_be_bytes());
    let (descriptor, locals) = if activity {
        ("()V".to_owned(), 1)
    } else {
        let mut field = 0x0011_u16.to_be_bytes().to_vec();
        field.extend(class.utf8("owner").to_be_bytes());
        field.extend(class.utf8(OWNER).to_be_bytes());
        field.extend([0; 2]);
        class.fields.push(field);
        code.extend([0x2a, if connection { 0x2c } else { 0x2b }, 0xb5]);
        code.extend(
            class
                .member(9, &format!("{PACKAGE}/{name}"), "owner", OWNER)
                .to_be_bytes(),
        );
        if connection {
            (
                format!("(Landroid/view/inputmethod/InputConnection;{OWNER})V"),
                3,
            )
        } else {
            (format!("({OWNER})V"), 2)
        }
    };
    code.push(0xb1);
    class.method("<init>", &descriptor, Some((locals, code)));
}

fn activity(root: &Path) -> io::Result<()> {
    let mut activity = Class::default();
    constructor(
        &mut activity,
        "J2PlayActivity",
        "android/app/NativeActivity",
        false,
    );
    let library = activity.utf8("j2play_android");
    let library = activity.reference(8, library, None);
    let load = activity.member(
        10,
        "java/lang/System",
        "loadLibrary",
        "(Ljava/lang/String;)V",
    );
    let mut code = vec![0x13]; // ldc_w
    code.extend(library.to_be_bytes());
    code.push(0xb8);
    code.extend(load.to_be_bytes());
    code.push(0xb1);
    activity.method("<clinit>", "()V", Some((0, code)));
    for (name, descriptor) in [
        ("onCreate", "(Landroid/os/Bundle;)V"),
        ("onResume", "()V"),
        ("onPause", "()V"),
        ("onDestroy", "()V"),
        ("onSaveInstanceState", "(Landroid/os/Bundle;)V"),
        ("onWindowFocusChanged", "(Z)V"),
        (
            "onConfigurationChanged",
            "(Landroid/content/res/Configuration;)V",
        ),
        ("onActivityResult", "(IILandroid/content/Intent;)V"),
        ("onNewIntent", "(Landroid/content/Intent;)V"),
        ("onBackPressed", "()V"),
    ] {
        activity.method(name, descriptor, None);
    }
    activity.write(root, "J2PlayActivity", "android/app/NativeActivity", &[])
}

pub fn generate(root: &Path) -> io::Result<()> {
    activity(root)?;
    let mut callbacks = Class::default();
    constructor(&mut callbacks, "Callbacks", "java/lang/Object", false);
    callbacks.method("run", "()V", None);
    for method in [
        "onInputDeviceAdded",
        "onInputDeviceChanged",
        "onInputDeviceRemoved",
    ] {
        callbacks.method(method, "(I)V", None);
    }
    callbacks.method(
        "onApplyWindowInsets",
        "(Landroid/view/View;Landroid/view/WindowInsets;)Landroid/view/WindowInsets;",
        None,
    );
    callbacks.write(
        root,
        "Callbacks",
        "java/lang/Object",
        &[
            "java/lang/Runnable",
            "android/view/View$OnApplyWindowInsetsListener",
            "android/hardware/input/InputManager$InputDeviceListener",
        ],
    )?;

    let mut back = Class::default();
    constructor(&mut back, "BackCallback", "java/lang/Object", false);
    back.method("onBackInvoked", "()V", None);
    back.write(
        root,
        "BackCallback",
        "java/lang/Object",
        &["android/window/OnBackInvokedCallback"],
    )?;

    let mut editor = Class::default();
    constructor(
        &mut editor,
        "BridgeEditor",
        "android/widget/EditText",
        false,
    );
    for name in ["onKeyPreIme", "onKeyDown", "onKeyUp"] {
        editor.method(name, "(ILandroid/view/KeyEvent;)Z", None);
    }
    editor.method(
        "onCreateInputConnection",
        "(Landroid/view/inputmethod/EditorInfo;)Landroid/view/inputmethod/InputConnection;",
        None,
    );
    editor.write(root, "BridgeEditor", "android/widget/EditText", &[])?;

    let mut connection = Class::default();
    constructor(
        &mut connection,
        "EditorConnection",
        "android/view/inputmethod/InputConnectionWrapper",
        true,
    );
    for name in ["setComposingText", "commitText"] {
        connection.method(name, "(Ljava/lang/CharSequence;I)Z", None);
    }
    for name in [
        "setComposingRegion",
        "setSelection",
        "deleteSurroundingText",
        "deleteSurroundingTextInCodePoints",
    ] {
        connection.method(name, "(II)Z", None);
    }
    connection.method("finishComposingText", "()Z", None);
    connection.write(
        root,
        "EditorConnection",
        "android/view/inputmethod/InputConnectionWrapper",
        &[],
    )
}

#[allow(dead_code)]
fn main() -> io::Result<()> {
    let output = std::env::args_os()
        .nth(1)
        .expect("usage: callback-classes OUTPUT_DIRECTORY");
    generate(Path::new(&output))
}

#[cfg(test)]
#[path = "../../../tests/unit/j2play-android/build-support/callback_classes/mod.rs"]
mod tests;
