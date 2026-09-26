use super::*;
use crate::*;

#[test]
fn display_specialization_ignores_unmarked_numeric_literals() {
    let mut canvas = production_bootstrap_template()
        .into_iter()
        .find(|entry| {
            entry.class.class_name(entry.class.this_class)
                == Some("javax/microedition/lcdui/Canvas")
        })
        .unwrap()
        .class;
    canvas
        .constant_pool
        .extend([Some(Constant::Long(240)), Some(Constant::Long(320))]);
    append_code_method(
        &mut canvas,
        ACC_PUBLIC | ACC_STATIC,
        "__ordinaryNumericLiterals",
        "()I",
        1,
        0,
        vec![0x11, 0x00, 0xf0, 0x57, 0x11, 0x01, 0x40, 0xac],
    );

    specialize_lcdui_dimensions(&mut canvas, 176, 208);

    assert!(
        canvas
            .constant_pool
            .iter()
            .flatten()
            .any(|constant| constant == &Constant::Long(240))
    );
    assert!(
        canvas
            .constant_pool
            .iter()
            .flatten()
            .any(|constant| constant == &Constant::Long(320))
    );
    let literal_method = canvas
        .methods
        .iter()
        .find(|method| canvas.utf8(method.name_index) == Some("__ordinaryNumericLiterals"))
        .unwrap();
    let Attribute::Code(code) = &literal_method.attributes[0] else {
        panic!("test method must have bytecode");
    };
    let values = bytecode::decode(&code.code)
        .unwrap()
        .into_iter()
        .filter(|instruction| instruction.opcode == 0x11)
        .map(|instruction| i16::from_be_bytes([instruction.operands[0], instruction.operands[1]]))
        .collect::<Vec<_>>();
    assert_eq!(values, [240, 320]);
}

#[test]
fn optional_bootstrap_surfaces_declare_profile_requirements() {
    let inventory = production_bootstrap_inventory();
    let requirement = |name: &str| {
        inventory
            .iter()
            .find(|entry| entry.class.class_name(entry.class.this_class) == Some(name))
            .unwrap()
            .requirement
    };
    assert_eq!(
        requirement("com/nokia/mid/ui/DirectUtils"),
        BootstrapRequirement::VendorApi("nokia-ui")
    );
    assert_eq!(
        requirement("com/samsung/util/AudioClip"),
        BootstrapRequirement::VendorApi("samsung-audioclip")
    );
    assert_eq!(
        requirement("javax/microedition/m3g/Graphics3D"),
        BootstrapRequirement::Jsr("184")
    );
    assert_eq!(
        requirement("javax/microedition/khronos/egl/EGL11"),
        BootstrapRequirement::Jsr("239")
    );
    assert_eq!(
        requirement("javax/microedition/khronos/opengles/GL11"),
        BootstrapRequirement::Jsr("239")
    );
    assert_eq!(
        requirement("java/nio/ByteBuffer"),
        BootstrapRequirement::Jsr("239")
    );
    assert_eq!(
        requirement("javax/microedition/sensor/SensorManager"),
        BootstrapRequirement::VendorApi("sensor-probe")
    );
    assert_eq!(
        requirement("javax/microedition/sensor/SensorConnection"),
        BootstrapRequirement::Jsr("256")
    );
    assert_eq!(
        requirement("javax/microedition/io/file/FileConnection"),
        BootstrapRequirement::Jsr("75")
    );
    assert_eq!(
        requirement("javax/microedition/media/Player"),
        BootstrapRequirement::Jsr("135")
    );
}

#[test]
fn jsr239_bootstrap_exposes_egl_gl_and_nio_entry_points() {
    let inventory = production_bootstrap_inventory();
    let class = |name: &str| {
        &inventory
            .iter()
            .find(|entry| entry.class.class_name(entry.class.this_class) == Some(name))
            .unwrap()
            .class
    };
    let has_method = |class: &ClassFile, name: &str, descriptor: &str| {
        class.methods.iter().any(|method| {
            class.utf8(method.name_index) == Some(name)
                && class.utf8(method.descriptor_index) == Some(descriptor)
                && method.access_flags & ACC_NATIVE != 0
        })
    };

    assert!(has_method(
        class("javax/microedition/khronos/egl/EGLContext"),
        "getEGL",
        "()Ljavax/microedition/khronos/egl/EGL;"
    ));
    assert!(has_method(
        class("javax/microedition/khronos/egl/EGLImpl"),
        "eglCreateWindowSurface",
        "(Ljavax/microedition/khronos/egl/EGLDisplay;Ljavax/microedition/khronos/egl/EGLConfig;Ljava/lang/Object;[I)Ljavax/microedition/khronos/egl/EGLSurface;"
    ));
    assert!(has_method(
        class("javax/microedition/khronos/opengles/GLImpl"),
        "glDrawArrays",
        "(III)V"
    ));
    assert!(has_method(
        class("java/nio/ByteBuffer"),
        "asFloatBuffer",
        "()Ljava/nio/FloatBuffer;"
    ));
}
