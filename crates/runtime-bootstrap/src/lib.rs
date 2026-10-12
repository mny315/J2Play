//! Canonical Java ME bootstrap class inventory and profile specialization.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::missing_errors_doc,
    clippy::too_many_lines
)]

mod bootstrap;
mod bytecode_builder;
mod generated_bootstrap;
mod suite_policy;

pub use suite_policy::{class_requests_jsr239, profile_supports_suite_bootstrap};

use crate::generated_bootstrap::generated_bootstrap_classes;
use bootstrap::{
    augment_core_compatibility_surface, com_nokia_mid_sound_sound, com_nokia_mid_ui_device_control,
    com_nokia_mid_ui_direct_graphics, com_nokia_mid_ui_direct_utils, com_nokia_mid_ui_full_canvas,
    com_samsung_util_audio_clip, com_siemens_mp_game_light, com_siemens_mp_game_vibrator,
    com_siemens_mp_media_classes, find_method_ref, java_io_input_stream_reader, java_io_reader,
    java_lang_class, java_lang_math, java_lang_number, java_lang_object, java_lang_ref_reference,
    java_lang_ref_weak_reference, java_lang_runnable, java_lang_runtime, java_lang_system,
    java_lang_throwable, java_lang_throwable_subclass, java_nio_buffer,
    java_nio_buffer_overflow_exception, java_nio_byte_buffer, java_nio_float_buffer,
    java_nio_int_buffer, javax_bluetooth_bluetooth_state_exception, javax_bluetooth_local_device,
    javax_bluetooth_uuid, javax_microedition_khronos_egl_context,
    javax_microedition_khronos_egl_egl, javax_microedition_khronos_egl_egl_impl,
    javax_microedition_khronos_egl_egl11, javax_microedition_khronos_egl_surface,
    javax_microedition_khronos_opengles_gl, javax_microedition_khronos_opengles_gl_impl,
    javax_microedition_khronos_opengles_gl11, javax_microedition_lcdui_display_transition,
    javax_microedition_sensor_data, javax_microedition_sensor_data_listener,
    javax_microedition_sensor_sensor_connection, javax_microedition_sensor_sensor_info,
    javax_microedition_sensor_sensor_manager, javax_wireless_messaging_binary_message,
    javax_wireless_messaging_message, javax_wireless_messaging_message_connection,
    javax_wireless_messaging_message_listener, javax_wireless_messaging_text_message,
    jsr239_handle_class, zh_system_canvas, zh_system_fonts, zh_system_game_midlet,
    zh_system_input_target, zh_system_scene,
};
use classfile::{Attribute, ClassFile, CodeAttribute, Constant, Member};
use device_profile::{JavaCapability, java_capability_for_class};

const ACC_PUBLIC: u16 = 0x0001;
const ACC_PRIVATE: u16 = 0x0002;
const ACC_PROTECTED: u16 = 0x0004;
const ACC_STATIC: u16 = 0x0008;
const ACC_FINAL: u16 = 0x0010;
const ACC_SUPER: u16 = 0x0020;
const ACC_SYNCHRONIZED: u16 = 0x0020;
const ACC_NATIVE: u16 = 0x0100;
const ACC_INTERFACE: u16 = 0x0200;
const ACC_ABSTRACT: u16 = 0x0400;

// The raw LCDUI templates carry deliberately impossible display dimensions.
// Profile specialization replaces only these sentinels, so an unrelated
// numeric literal that happens to equal a common 240x320 display is untouched.
const LCDUI_WIDTH_MARKER: i16 = i16::MIN;
const LCDUI_HEIGHT_MARKER: i16 = i16::MIN + 1;
const LCDUI_WIDTH_MARKER_BYTES: [u8; 2] = LCDUI_WIDTH_MARKER.to_be_bytes();
const LCDUI_HEIGHT_MARKER_BYTES: [u8; 2] = LCDUI_HEIGHT_MARKER.to_be_bytes();

/// Rust crate responsible for a production bootstrap class and its native
/// bindings. The class metadata and executable bytecode remain canonical in
/// this crate; ownership identifies the host-side implementation boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapOwner {
    Cldc,
    Midp,
    Rms,
    Mmapi,
    Gcf,
    Bluetooth,
    M3g,
    Micro3d,
}

/// Profile capability required before a bootstrap class becomes visible to a `MIDlet`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapRequirement {
    Core,
    Jsr(&'static str),
    VendorApi(&'static str),
}

/// One machine-readable production bootstrap entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapClass {
    pub owner: BootstrapOwner,
    pub requirement: BootstrapRequirement,
    pub class: ClassFile,
}

/// Returns the single canonical production bootstrap inventory.
///
/// Class flags, inheritance, interfaces, fields, methods, native flags and
/// executable bodies all live in the contained `ClassFile`. `owner` records
/// which Rust crate supplies the associated host-side behavior.
#[must_use]
pub fn production_bootstrap_inventory() -> Vec<BootstrapClass> {
    production_bootstrap_inventory_for_display(240, 320)
}

fn production_bootstrap_template() -> Vec<BootstrapClass> {
    let mut classes = vec![
        java_lang_object(),
        java_lang_runnable(),
        java_lang_number(),
        java_lang_throwable(),
    ];
    classes.extend(
        JAVA_LANG_THROWABLE_SUBCLASSES
            .iter()
            .map(|spec| java_lang_throwable_subclass(*spec)),
    );
    classes.extend([
        java_lang_class(),
        java_lang_ref_reference(),
        java_lang_ref_weak_reference(),
        java_lang_runtime(),
        java_lang_system(),
        java_lang_math(),
        java_io_reader(),
        java_io_input_stream_reader(),
        java_nio_buffer(),
        java_nio_buffer_overflow_exception(),
        java_nio_byte_buffer(),
        java_nio_float_buffer(),
        java_nio_int_buffer(),
        zh_system_game_midlet(),
        zh_system_input_target(),
        zh_system_scene(),
        zh_system_fonts(),
        zh_system_canvas(),
        com_siemens_mp_game_light(),
        com_siemens_mp_game_vibrator(),
        com_nokia_mid_ui_device_control(),
        com_nokia_mid_ui_full_canvas(),
        com_nokia_mid_ui_direct_graphics(),
        com_nokia_mid_ui_direct_utils(),
        com_nokia_mid_sound_sound(),
        com_samsung_util_audio_clip(),
        javax_microedition_sensor_data(),
        javax_microedition_sensor_data_listener(),
        javax_microedition_sensor_sensor_connection(),
        javax_microedition_sensor_sensor_info(),
        javax_microedition_sensor_sensor_manager(),
        javax_bluetooth_bluetooth_state_exception(),
        javax_bluetooth_uuid(),
        javax_bluetooth_local_device(),
        javax_wireless_messaging_message(),
        javax_wireless_messaging_binary_message(),
        javax_wireless_messaging_text_message(),
        javax_wireless_messaging_message_listener(),
        javax_wireless_messaging_message_connection(),
        javax_microedition_khronos_egl_egl(),
        javax_microedition_khronos_egl_egl11(),
        javax_microedition_khronos_egl_egl_impl(),
        javax_microedition_khronos_egl_context(),
        jsr239_handle_class("javax/microedition/khronos/egl/EGLDisplay"),
        jsr239_handle_class("javax/microedition/khronos/egl/EGLConfig"),
        javax_microedition_khronos_egl_surface(),
        javax_microedition_khronos_opengles_gl(),
        javax_microedition_khronos_opengles_gl11(),
        javax_microedition_khronos_opengles_gl_impl(),
    ]);
    let mut generated = generated_bootstrap_classes();
    augment_core_compatibility_surface(&mut generated);
    let siemens_media = com_siemens_mp_media_classes(&generated);
    generated.extend(siemens_media);
    generated.push(javax_microedition_lcdui_display_transition());
    classes.extend(generated);
    classes.extend(m3g::bootstrap_classes());
    classes.extend(micro3d::bootstrap_classes());
    classes
        .into_iter()
        .map(|class| BootstrapClass {
            owner: bootstrap_owner(&class),
            requirement: bootstrap_requirement(&class),
            class,
        })
        .collect()
}

/// Returns the canonical inventory with LCDUI classes specialized to the
/// selected profile's full-screen Canvas dimensions.
///
/// # Panics
///
/// Panics if dimensions exceed the profile validator's signed 16-bit bound or
/// if the project-owned bootstrap bytecode is malformed.
#[must_use]
pub fn production_bootstrap_inventory_for_display(width: u32, height: u32) -> Vec<BootstrapClass> {
    production_bootstrap_inventory_for_display_modes((width, height), (width, height))
}

/// Returns the canonical inventory with LCDUI classes specialized to the
/// selected profile's full-screen and ordinary Canvas dimensions.
///
/// `Screen` remains tied to the physical/full-screen dimensions. A `Canvas`
/// starts in the ordinary mode and can switch to the full-screen dimensions at
/// runtime through `setFullScreenMode`.
///
/// # Panics
///
/// Panics if dimensions exceed the profile validator's signed 16-bit bound or
/// if the project-owned bootstrap bytecode is malformed.
#[must_use]
pub fn production_bootstrap_inventory_for_display_modes(
    (fullscreen_width, fullscreen_height): (u32, u32),
    (normal_width, normal_height): (u32, u32),
) -> Vec<BootstrapClass> {
    let fullscreen_width =
        i16::try_from(fullscreen_width).expect("validated profile width fits JVM sipush");
    let fullscreen_height =
        i16::try_from(fullscreen_height).expect("validated profile height fits JVM sipush");
    let normal_width = i16::try_from(normal_width).expect("validated Canvas width fits JVM sipush");
    let normal_height =
        i16::try_from(normal_height).expect("validated Canvas height fits JVM sipush");
    let mut inventory = production_bootstrap_template();
    for entry in &mut inventory {
        let name = entry
            .class
            .class_name(entry.class.this_class)
            .expect("Rust bootstrap classes always have a valid name");
        match name {
            "javax/microedition/lcdui/Canvas" => {
                specialize_lcdui_dimensions(&mut entry.class, normal_width, normal_height);
            }
            "javax/microedition/lcdui/Screen" | "javax/microedition/lcdui/Displayable" => {
                specialize_lcdui_dimensions(&mut entry.class, fullscreen_width, fullscreen_height);
            }
            _ => {}
        }
    }
    inventory
}

/// Returns whether a suite class is the standard constructor-only Nokia
/// `FullCanvas` adapter commonly bundled by cross-vendor `MIDlets`.
///
/// Such an adapter adds no suite behavior of its own, so the runtime may use
/// the canonical implementation to preserve Nokia selection-key semantics on
/// a non-Nokia device profile. The deliberately strict shape check prevents a
/// suite-defined class with custom fields, methods or constructor behavior
/// from being replaced.
#[must_use]
pub fn is_bundled_nokia_full_canvas_adapter(class: &ClassFile) -> bool {
    if class.class_name(class.this_class) != Some("com/nokia/mid/ui/FullCanvas")
        || class.class_name(class.super_class) != Some("javax/microedition/lcdui/Canvas")
        || class.access_flags != ACC_PUBLIC | ACC_SUPER | ACC_ABSTRACT
        || !class.interfaces.is_empty()
        || !class.fields.is_empty()
        || class.methods.len() != 1
    {
        return false;
    }

    let constructor = &class.methods[0];
    if constructor.access_flags != ACC_PROTECTED
        || class.utf8(constructor.name_index) != Some("<init>")
        || class.utf8(constructor.descriptor_index) != Some("()V")
    {
        return false;
    }
    let Some(code) = constructor.attributes.iter().find_map(|attribute| {
        let Attribute::Code(code) = attribute else {
            return None;
        };
        Some(code)
    }) else {
        return false;
    };
    let Some(canvas_init) =
        find_method_ref(class, "javax/microedition/lcdui/Canvas", "<init>", "()V")
    else {
        return false;
    };
    let canvas_init = canvas_init.to_be_bytes();
    code.max_stack == 1
        && code.max_locals == 1
        && code.exception_table.is_empty()
        && code.code == [0x2a, 0xb7, canvas_init[0], canvas_init[1], 0xb1]
}

fn specialize_lcdui_dimensions(class: &mut ClassFile, width: i16, height: i16) {
    let mut width_markers = 0;
    let mut height_markers = 0;
    for constant in class.constant_pool.iter_mut().flatten() {
        match constant {
            Constant::Long(value) if *value == i64::from(LCDUI_WIDTH_MARKER) => {
                *constant = Constant::Long(i64::from(width));
                width_markers += 1;
            }
            Constant::Long(value) if *value == i64::from(LCDUI_HEIGHT_MARKER) => {
                *constant = Constant::Long(i64::from(height));
                height_markers += 1;
            }
            _ => {}
        }
    }
    for method in &mut class.methods {
        for attribute in &mut method.attributes {
            let Attribute::Code(code) = attribute else {
                continue;
            };
            let instructions = bytecode::decode(&code.code)
                .expect("generated LCDUI bootstrap bytecode must remain valid");
            for instruction in instructions {
                if instruction.opcode != 0x11 || instruction.operands.len() != 2 {
                    continue;
                }
                let value = i16::from_be_bytes([instruction.operands[0], instruction.operands[1]]);
                let replacement = match value {
                    LCDUI_WIDTH_MARKER => {
                        width_markers += 1;
                        width
                    }
                    LCDUI_HEIGHT_MARKER => {
                        height_markers += 1;
                        height
                    }
                    _ => continue,
                }
                .to_be_bytes();
                code.code[instruction.offset + 1..instruction.offset + 3]
                    .copy_from_slice(&replacement);
            }
        }
    }
    assert!(
        width_markers > 0 && height_markers > 0,
        "generated LCDUI class must contain explicit width and height markers"
    );
}

fn bootstrap_requirement(class: &ClassFile) -> BootstrapRequirement {
    let name = class
        .class_name(class.this_class)
        .expect("Rust bootstrap classes always have a valid name");
    match java_capability_for_class(name) {
        Some(JavaCapability::Jsr(jsr)) => BootstrapRequirement::Jsr(jsr),
        Some(JavaCapability::VendorApi(api)) => BootstrapRequirement::VendorApi(api),
        None => BootstrapRequirement::Core,
    }
}

/// Returns all Rust-owned production bootstrap class models.
#[must_use]
pub fn production_bootstrap_classes() -> Vec<ClassFile> {
    production_bootstrap_inventory()
        .into_iter()
        .map(|entry| entry.class)
        .collect()
}

fn bootstrap_owner(class: &ClassFile) -> BootstrapOwner {
    let name = class
        .class_name(class.this_class)
        .expect("Rust bootstrap classes always have a valid name");
    if name.starts_with("javax/bluetooth/") {
        BootstrapOwner::Bluetooth
    } else if name.starts_with("com/mascotcapsule/micro3d/v3/") {
        BootstrapOwner::Micro3d
    } else if name.starts_with("javax/microedition/m3g/") {
        BootstrapOwner::M3g
    } else if name.starts_with("javax/microedition/rms/") {
        BootstrapOwner::Rms
    } else if name.starts_with("javax/microedition/media/")
        || name.starts_with("com/siemens/mp/media/")
        || matches!(
            name,
            "com/nokia/mid/sound/Sound" | "com/samsung/util/AudioClip"
        )
    {
        BootstrapOwner::Mmapi
    } else if name.starts_with("javax/microedition/io/") {
        BootstrapOwner::Gcf
    } else if name.starts_with("zh/system/") || name.starts_with("javax/microedition/") {
        BootstrapOwner::Midp
    } else {
        BootstrapOwner::Cldc
    }
}

const JAVA_LANG_THROWABLE_SUBCLASSES: &[(&str, &str, bool)] = &[
    ("java/lang/Error", "java/lang/Throwable", false),
    ("java/lang/LinkageError", "java/lang/Error", false),
    (
        "java/lang/ClassCircularityError",
        "java/lang/LinkageError",
        false,
    ),
    (
        "java/lang/ExceptionInInitializerError",
        "java/lang/LinkageError",
        false,
    ),
    (
        "java/lang/NoClassDefFoundError",
        "java/lang/LinkageError",
        false,
    ),
    (
        "java/lang/NoSuchMethodError",
        "java/lang/LinkageError",
        false,
    ),
    (
        "java/lang/NoSuchFieldError",
        "java/lang/LinkageError",
        false,
    ),
    (
        "java/lang/IncompatibleClassChangeError",
        "java/lang/LinkageError",
        false,
    ),
    (
        "java/lang/AbstractMethodError",
        "java/lang/IncompatibleClassChangeError",
        false,
    ),
    (
        "java/lang/UnsatisfiedLinkError",
        "java/lang/LinkageError",
        false,
    ),
    ("java/lang/VirtualMachineError", "java/lang/Error", false),
    (
        "java/lang/OutOfMemoryError",
        "java/lang/VirtualMachineError",
        false,
    ),
    (
        "java/lang/StackOverflowError",
        "java/lang/VirtualMachineError",
        false,
    ),
    ("java/lang/Exception", "java/lang/Throwable", false),
    ("java/lang/RuntimeException", "java/lang/Exception", false),
    (
        "java/lang/IllegalArgumentException",
        "java/lang/RuntimeException",
        false,
    ),
    (
        "java/lang/IllegalThreadStateException",
        "java/lang/IllegalArgumentException",
        false,
    ),
    (
        "java/lang/IllegalStateException",
        "java/lang/RuntimeException",
        false,
    ),
    (
        "java/lang/SecurityException",
        "java/lang/RuntimeException",
        false,
    ),
    (
        "java/lang/IllegalMonitorStateException",
        "java/lang/RuntimeException",
        false,
    ),
    (
        "java/lang/ArithmeticException",
        "java/lang/RuntimeException",
        false,
    ),
    (
        "java/lang/NullPointerException",
        "java/lang/RuntimeException",
        false,
    ),
    (
        "java/lang/IndexOutOfBoundsException",
        "java/lang/RuntimeException",
        false,
    ),
    (
        "java/lang/ArrayIndexOutOfBoundsException",
        "java/lang/IndexOutOfBoundsException",
        true,
    ),
    (
        "java/lang/StringIndexOutOfBoundsException",
        "java/lang/IndexOutOfBoundsException",
        false,
    ),
    (
        "java/lang/NegativeArraySizeException",
        "java/lang/RuntimeException",
        false,
    ),
    (
        "java/lang/ClassCastException",
        "java/lang/RuntimeException",
        false,
    ),
    (
        "java/lang/ArrayStoreException",
        "java/lang/RuntimeException",
        false,
    ),
    (
        "java/lang/NumberFormatException",
        "java/lang/IllegalArgumentException",
        false,
    ),
    (
        "java/lang/InterruptedException",
        "java/lang/Exception",
        false,
    ),
    (
        "java/lang/ClassNotFoundException",
        "java/lang/Exception",
        false,
    ),
    (
        "java/lang/InstantiationException",
        "java/lang/Exception",
        false,
    ),
    (
        "java/lang/IllegalAccessException",
        "java/lang/Exception",
        false,
    ),
    (
        "java/io/InterruptedIOException",
        "java/io/IOException",
        false,
    ),
];
