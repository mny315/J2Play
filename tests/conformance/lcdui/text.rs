use crate::support::guest::{MethodCode, Pool, code_method, emit_reference};
use std::time::{Duration, Instant};

#[path = "../../support/text_languages.rs"]
mod languages;

const CLASS: &str = "fixtures/MultilingualText";
const FONT: &str = "javax/microedition/lcdui/Font";
const IMAGE: &str = "javax/microedition/lcdui/Image";
const GRAPHICS: &str = "javax/microedition/lcdui/Graphics";

#[derive(Clone, Copy, Debug)]
enum Draw {
    String,
    Chars,
    Substring,
}

fn int(code: &mut Vec<u8>, value: i32) {
    code.push(0x11); // sipush, all fixture operands are small
    code.extend_from_slice(&i16::try_from(value).unwrap().to_be_bytes());
}

fn character(code: &mut Vec<u8>, pool: &mut Pool, unit: u16) {
    if unit <= i16::MAX.cast_unsigned() {
        int(code, i32::from(unit));
    } else {
        emit_reference(
            code,
            0x13,
            pool.push(classfile::Constant::Integer(i32::from(unit))),
        );
    }
}

fn call(
    code: &mut Vec<u8>,
    pool: &mut Pool,
    opcode: u8,
    owner: &str,
    name: &str,
    descriptor: &str,
) {
    emit_reference(code, opcode, pool.method(owner, name, descriptor));
}

fn string(code: &mut Vec<u8>, pool: &mut Pool, value: &str) {
    emit_reference(code, 0x13, pool.string(value));
}

fn draw_reference(code: &mut Vec<u8>, pool: &mut Pool, visual: &str) {
    code.extend_from_slice(&[0x07, 0x36, 0x05, 0x07, 0x36, 0x06]); // pen and base x
    for unit in visual.encode_utf16() {
        // These marks have zero advance in the independent reference drawing.
        let mark = matches!(
            unit,
            0x0301 | 0x05b8 | 0x05b9 | 0x05c1 | 0x064e | 0x094d | 0x0bcd
        );
        if !mark {
            code.extend_from_slice(&[0x15, 0x05, 0x36, 0x06]);
        }
        code.extend_from_slice(&[0x19, 0x04]);
        character(code, pool, unit);
        code.extend_from_slice(&[0x15, 0x06, 0x06, 0x10, 0x14]);
        call(code, pool, 0xb6, GRAPHICS, "drawChar", "(CIII)V");
        if !mark {
            code.extend_from_slice(&[0x15, 0x05, 0x2a]);
            character(code, pool, unit);
            call(code, pool, 0xb6, FONT, "charWidth", "(C)I");
            code.extend_from_slice(&[0x60, 0x36, 0x05]);
        }
    }
}

fn prepare_images(code: &mut Vec<u8>, pool: &mut Pool) {
    code.extend_from_slice(&[0x03, 0x03, 0x10, 0x08]); // face, style, SMALL
    call(
        code,
        pool,
        0xb8,
        FONT,
        "getFont",
        "(III)Ljavax/microedition/lcdui/Font;",
    );
    code.push(0x4b); // font = local 0
    for (image, graphics) in [(1, 2), (3, 4)] {
        int(code, 240);
        int(code, 32);
        call(
            code,
            pool,
            0xb8,
            IMAGE,
            "createImage",
            "(II)Ljavax/microedition/lcdui/Image;",
        );
        code.extend_from_slice(&[0x3a, image, 0x19, image]);
        call(
            code,
            pool,
            0xb6,
            IMAGE,
            "getGraphics",
            "()Ljavax/microedition/lcdui/Graphics;",
        );
        code.extend_from_slice(&[0x3a, graphics, 0x19, graphics, 0x2a]);
        call(
            code,
            pool,
            0xb6,
            GRAPHICS,
            "setFont",
            "(Ljavax/microedition/lcdui/Font;)V",
        );
    }
}

fn method(
    pool: &mut Pool,
    code_name: u16,
    name: &str,
    logical: &str,
    visual: &str,
    draw: Draw,
) -> classfile::Member {
    let mut code = Vec::new();
    prepare_images(&mut code, pool);
    code.push(0x2c); // actual graphics
    let padded = format!("x{logical}x");
    string(
        &mut code,
        pool,
        if matches!(draw, Draw::String) {
            logical
        } else {
            &padded
        },
    );
    match draw {
        Draw::String => {}
        Draw::Chars => {
            call(
                &mut code,
                pool,
                0xb6,
                "java/lang/String",
                "toCharArray",
                "()[C",
            );
            int(&mut code, 1);
            int(
                &mut code,
                i32::try_from(logical.encode_utf16().count()).unwrap(),
            );
        }
        Draw::Substring => {
            int(&mut code, 1);
            int(
                &mut code,
                i32::try_from(logical.encode_utf16().count()).unwrap(),
            );
        }
    }
    code.extend_from_slice(&[0x07, 0x06, 0x10, 0x14]); // x=4, y=3, LEFT|TOP
    let (draw_name, descriptor) = match draw {
        Draw::String => ("drawString", "(Ljava/lang/String;III)V"),
        Draw::Chars => ("drawChars", "([CIIIII)V"),
        Draw::Substring => ("drawSubstring", "(Ljava/lang/String;IIIII)V"),
    };
    call(&mut code, pool, 0xb6, GRAPHICS, draw_name, descriptor);
    draw_reference(&mut code, pool, visual);
    code.extend_from_slice(&[0x2b, 0x2d, 0x2a]); // actual image, reference image, font
    string(&mut code, pool, logical);
    call(
        &mut code,
        pool,
        0xb6,
        FONT,
        "stringWidth",
        "(Ljava/lang/String;)I",
    );
    code.extend_from_slice(&[0x15, 0x05, 0x07, 0x64]); // expected width = pen - 4
    call(
        &mut code,
        pool,
        0xb8,
        CLASS,
        "check",
        "(Ljavax/microedition/lcdui/Image;Ljavax/microedition/lcdui/Image;II)I",
    );
    code.push(0xac);
    code_method(
        pool,
        code_name,
        MethodCode {
            flags: 0x0009,
            name,
            descriptor: "()I",
            max_stack: 7,
            max_locals: 7,
            code,
        },
    )
}

fn fixture(cases: &[(&str, &str, &str)]) -> classfile::ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class(CLASS);
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let mut methods = Vec::new();
    for (index, &(_, logical, visual)) in cases.iter().enumerate() {
        for (kind, draw) in [Draw::String, Draw::Chars, Draw::Substring]
            .into_iter()
            .enumerate()
        {
            methods.push(method(
                &mut pool,
                code_name,
                &format!("case{index}_{kind}"),
                logical,
                visual,
                draw,
            ));
        }
    }
    methods.push(classfile::Member {
        access_flags: 0x0109,
        name_index: pool.utf8("check"),
        descriptor_index: pool
            .utf8("(Ljavax/microedition/lcdui/Image;Ljavax/microedition/lcdui/Image;II)I"),
        attributes: Vec::new(),
    });
    classfile::ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: 0x0021,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods,
        attributes: Vec::new(),
    }
}

struct TextHost {
    properties: cldc::SystemProperties,
    deadline: Instant,
}

impl natives::HostServices for TextHost {
    fn monotonic_millis(&self) -> i64 {
        0
    }
    fn wall_clock_millis(&self) -> i64 {
        0
    }
    fn system_property(&self, name: &str) -> Option<&str> {
        self.properties.get(name)
    }
    fn read_resource(&self, _: &str) -> Result<Option<Vec<u8>>, diagnostics::EmuError> {
        Ok(None)
    }
    fn execution_cancelled(&self) -> bool {
        Instant::now() >= self.deadline
    }
}

fn check_cases(cases: &[(&str, &str, &str)]) {
    let profile = launch::builtin_device_profiles()
        .unwrap()
        .iter()
        .find(|profile| profile.profile_id() == "nokia-featurephone")
        .unwrap();
    let resolved = profile
        .resolve(device_profile::ProfileOverrides::default())
        .unwrap();
    let limits = vm::Limits {
        max_instructions: 500_000,
        ..vm::Limits::default()
    };
    let mut program = vm::Program::new();
    runtime::install_rust_bootstrap(
        &mut program,
        &limits,
        &resolved,
        &std::collections::HashSet::new(),
        false,
    )
    .unwrap();
    runtime::register_profile_natives(&mut program, profile).unwrap();
    program.add_class(&fixture(cases), &limits).unwrap();
    program
        .native_registry_mut()
        .register(
            natives::NativeSignature::new(
                CLASS,
                "check",
                "(Ljavax/microedition/lcdui/Image;Ljavax/microedition/lcdui/Image;II)I",
            ),
            |context, args| {
                let [
                    natives::NativeValue::Reference(Some(actual)),
                    natives::NativeValue::Reference(Some(expected)),
                    natives::NativeValue::Int(width),
                    natives::NativeValue::Int(expected_width),
                ] = args
                else {
                    panic!("fixture arguments");
                };
                let pixels = |image| {
                    let reference = context
                        .read_reference_field(image, "javax/microedition/lcdui/Image.pixels:[I")
                        .unwrap()
                        .unwrap();
                    context.read_java_int_array(reference).unwrap()
                };
                let actual = pixels(*actual);
                let expected = pixels(*expected);
                assert_eq!(actual.len(), 240 * 32);
                assert!(expected.contains(&-16_777_216));
                Ok(Some(natives::NativeValue::Int(if actual != expected {
                    2
                } else if width != expected_width {
                    3
                } else {
                    0
                })))
            },
        )
        .unwrap();
    for (index, &(locale, text, _)) in cases.iter().enumerate() {
        for kind in 0..3 {
            let guest_locale = if device_profile::validate_guest_locale(locale).is_ok() {
                locale
            } else {
                "he-IL"
            };
            let mut host = TextHost {
                properties: cldc::SystemProperties::from_resolved_profile(
                    &resolved.with_guest_locale(guest_locale).unwrap(),
                ),
                deadline: Instant::now() + Duration::from_secs(5),
            };
            let result = program
                .execute_with_context(
                    CLASS,
                    &format!("case{index}_{kind}"),
                    "()I",
                    limits.clone(),
                    false,
                    &mut host,
                )
                .unwrap();
            assert_eq!(
                result.value,
                Some(vm::Value::Int(0)),
                "{locale}, {text:?}, draw={kind}"
            );
        }
    }
}

#[test]
fn every_guest_language_renders_in_its_script_direction_through_all_text_apis() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::every_guest_language_renders_in_its_script_direction_through_all_text_apis"
    )) {
        return;
    }
    check_cases(languages::LANGUAGE_SAMPLES);
}

#[test]
fn mixed_rtl_text_keeps_digits_brackets_controls_and_nonspacing_marks() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::mixed_rtl_text_keeps_digits_brackets_controls_and_nonspacing_marks"
    )) {
        return;
    }
    check_cases(&[
        ("he-IL", "שלום (ABC) 123", "123 (ABC) םולש"),
        ("en-US", "ABC שלום 123", "ABC 123 םולש"),
        ("he-IL", "שָׁלוֹם", "םוֹלשָׁ"),
        ("ar-SA", "بَت", "\u{fe96}\u{fe91}\u{064e}"),
        ("en-US", "e\u{0301}", "e\u{0301}"),
        ("he-IL", "\u{202e}ABC\u{202c}", "CBA"),
        ("en-US", "abc \u{2067}אב 123\u{2069} xyz", "abc 123 בא xyz"),
    ]);
}
