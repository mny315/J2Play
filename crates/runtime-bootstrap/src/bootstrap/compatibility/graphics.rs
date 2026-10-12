use super::{
    ACC_PUBLIC, ACC_STATIC, Attribute, ClassFile, Constant, Member, append_class,
    append_code_method, append_constant, append_method_ref, append_native_method, find_field_ref,
    find_method_ref, replace_method_with_native,
};
use crate::bytecode_builder::Code;

pub(crate) fn append_image_compatibility_surface(class: &mut ClassFile) {
    replace_method_with_native(
        class,
        "createImage",
        "([BII)Ljavax/microedition/lcdui/Image;",
    );
    append_native_method(
        class,
        ACC_PUBLIC | ACC_STATIC,
        "createImage",
        "(Ljavax/microedition/lcdui/Image;IIIII)Ljavax/microedition/lcdui/Image;",
    );
    append_native_method(
        class,
        ACC_PUBLIC | ACC_STATIC,
        "createImage",
        "(Ljavax/microedition/lcdui/Image;)Ljavax/microedition/lcdui/Image;",
    );
    let output_init = append_method_ref(class, "java/io/ByteArrayOutputStream", "<init>", "()V");
    let input_read = append_method_ref(class, "java/io/InputStream", "read", "([BII)I");
    let output_write =
        append_method_ref(class, "java/io/ByteArrayOutputStream", "write", "([BII)V");
    let output_bytes = append_method_ref(
        class,
        "java/io/ByteArrayOutputStream",
        "toByteArray",
        "()[B",
    );
    let create_bytes = append_method_ref(
        class,
        "javax/microedition/lcdui/Image",
        "createImage",
        "([BII)Ljavax/microedition/lcdui/Image;",
    );
    let output_class = append_class(class, "java/io/ByteArrayOutputStream");
    let mut code = Code::default();
    code.reference(0xbb, output_class)
        .emit(&[0x59])
        .reference(0xb7, output_init)
        .emit(&[0x4c, 0x11, 0x04, 0x00, 0xbc, 0x08, 0x4d]); // output and byte[1024]
    code.label("read")
        .emit(&[0x2a, 0x2c, 0x03, 0x11, 0x04, 0x00])
        .reference(0xb6, input_read)
        .emit(&[0x59, 0x3e, 0x02])
        .jump(0x9f, "done");
    code.emit(&[0x2b, 0x2c, 0x03, 0x1d])
        .reference(0xb6, output_write)
        .jump(0xa7, "read");
    code.label("done")
        .emit(&[0x2b])
        .reference(0xb6, output_bytes)
        .emit(&[0x3a, 0x04, 0x19, 0x04, 0x03, 0x19, 0x04, 0xbe])
        .reference(0xb8, create_bytes)
        .emit(&[0xb0]);
    append_code_method(
        class,
        ACC_PUBLIC | ACC_STATIC,
        "createImage",
        "(Ljava/io/InputStream;)Ljavax/microedition/lcdui/Image;",
        4,
        5,
        code.finish(),
    );
}

pub(crate) fn append_graphics_draw_substring(class: &mut ClassFile) {
    let substring = append_method_ref(
        class,
        "java/lang/String",
        "substring",
        "(II)Ljava/lang/String;",
    );
    let draw_string = append_method_ref(
        class,
        "javax/microedition/lcdui/Graphics",
        "drawString",
        "(Ljava/lang/String;III)V",
    );
    let mut code = Code::default();
    code.emit(&[0x2a, 0x2b, 0x1c, 0x1c, 0x1d, 0x60]) // this, text, offset, offset + length
        .reference(0xb6, substring)
        .emit(&[0x15, 0x04, 0x15, 0x05, 0x15, 0x06]) // x, y, anchor
        .reference(0xb6, draw_string)
        .emit(&[0xb1]);
    append_code_method(
        class,
        ACC_PUBLIC,
        "drawSubstring",
        "(Ljava/lang/String;IIIII)V",
        5,
        7,
        code.finish(),
    );
}

pub(crate) fn append_nokia_direct_graphics_surface(class: &mut ClassFile) {
    let interface = append_class(class, "com/nokia/mid/ui/DirectGraphics");
    class.interfaces.push(interface);
    for (name, descriptor) in nokia_direct_graphics_methods() {
        append_native_method(class, ACC_PUBLIC, name, descriptor);
    }
}

pub(crate) fn nokia_direct_graphics_methods() -> &'static [(&'static str, &'static str)] {
    &[
        ("drawImage", "(Ljavax/microedition/lcdui/Image;IIII)V"),
        ("setARGBColor", "(I)V"),
        ("getAlphaComponent", "()I"),
        ("getNativePixelFormat", "()I"),
        ("drawPolygon", "([II[IIII)V"),
        ("fillPolygon", "([II[IIII)V"),
        ("drawTriangle", "(IIIIIII)V"),
        ("fillTriangle", "(IIIIIII)V"),
        ("drawPixels", "([IZIIIIIIII)V"),
        ("drawPixels", "([SZIIIIIIII)V"),
        ("getPixels", "([IIIIIIII)V"),
        ("getPixels", "([SIIIIIII)V"),
    ]
}

pub(crate) fn apply_dark_lcdui_theme(class: &mut ClassFile, name: &str) {
    if name == "javax/microedition/lcdui/Screen" {
        for constant in class.constant_pool.iter_mut().flatten() {
            if matches!(
                constant,
                Constant::Integer(16_777_215 | 15_132_390 | 14_079_702)
            ) {
                *constant = Constant::Integer(0);
            }
        }
    }
    if !matches!(
        name,
        "javax/microedition/lcdui/Alert"
            | "javax/microedition/lcdui/ChoiceGroup"
            | "javax/microedition/lcdui/DateField"
            | "javax/microedition/lcdui/Gauge"
            | "javax/microedition/lcdui/ImageItem"
            | "javax/microedition/lcdui/Item"
            | "javax/microedition/lcdui/List"
            | "javax/microedition/lcdui/Screen"
            | "javax/microedition/lcdui/StringItem"
            | "javax/microedition/lcdui/TextField"
    ) {
        return;
    }
    let Some(set_color) = find_method_ref(
        class,
        "javax/microedition/lcdui/Graphics",
        "setColor",
        "(I)V",
    ) else {
        return;
    };
    let operand = set_color.to_be_bytes();
    for method in &mut class.methods {
        for attribute in &mut method.attributes {
            let Attribute::Code(code) = attribute else {
                continue;
            };
            for offset in 0..code.code.len().saturating_sub(3) {
                if code.code[offset..offset + 4] == [0x03, 0xb6, operand[0], operand[1]] {
                    // iconst_m1 is RGB white after Graphics masks to 24 bits,
                    // and has the same byte length as iconst_0.
                    code.code[offset] = 0x02;
                }
            }
        }
    }
}

pub(crate) fn append_graphics_canvas_support(class: &mut ClassFile) {
    let name_index = append_constant(class, Constant::Utf8("canvas".to_owned()));
    let descriptor_index = append_constant(
        class,
        Constant::Utf8("Ljavax/microedition/lcdui/Canvas;".to_owned()),
    );
    class.fields.push(Member {
        access_flags: 0,
        name_index,
        descriptor_index,
        attributes: Vec::new(),
    });
    let default_font = find_method_ref(
        class,
        "javax/microedition/lcdui/Font",
        "getDefaultFont",
        "()Ljavax/microedition/lcdui/Font;",
    )
    .expect("Graphics constant pool contains Font.getDefaultFont");
    let set_color = find_method_ref(
        class,
        "javax/microedition/lcdui/Graphics",
        "setColor",
        "(I)V",
    )
    .expect("Graphics constant pool contains Graphics.setColor");
    let font = find_field_ref(
        class,
        "javax/microedition/lcdui/Graphics",
        "font",
        "Ljavax/microedition/lcdui/Font;",
    )
    .expect("Graphics constant pool contains Graphics.font");
    let tx = find_field_ref(class, "javax/microedition/lcdui/Graphics", "tx", "I")
        .expect("Graphics constant pool contains Graphics.tx");
    let ty = find_field_ref(class, "javax/microedition/lcdui/Graphics", "ty", "I")
        .expect("Graphics constant pool contains Graphics.ty");
    let stroke = find_field_ref(class, "javax/microedition/lcdui/Graphics", "stroke", "I")
        .expect("Graphics constant pool contains Graphics.stroke");
    let target = find_field_ref(
        class,
        "javax/microedition/lcdui/Graphics",
        "target",
        "Ljavax/microedition/lcdui/Image;",
    )
    .expect("Graphics constant pool contains Graphics.target");
    let image_width = append_method_ref(class, "javax/microedition/lcdui/Image", "getWidth", "()I");
    let image_height =
        append_method_ref(class, "javax/microedition/lcdui/Image", "getHeight", "()I");
    let set_clip = append_method_ref(
        class,
        "javax/microedition/lcdui/Graphics",
        "setClip",
        "(IIII)V",
    );
    let mut code = Code::default();
    code.emit(&[0x2a])
        .reference(0xb8, default_font)
        .reference(0xb5, font);
    code.emit(&[0x2a, 0x03]).reference(0xb6, set_color); // MIDP black
    for field in [tx, ty, stroke] {
        code.emit(&[0x2a, 0x03]).reference(0xb5, field);
    }
    code.emit(&[0x2a, 0x03, 0x03, 0x2a])
        .reference(0xb4, target)
        .reference(0xb6, image_width)
        .emit(&[0x2a])
        .reference(0xb4, target)
        .reference(0xb6, image_height)
        .reference(0xb6, set_clip)
        .emit(&[0x2a, 0xb0]);
    append_code_method(
        class,
        0,
        "__resetForPaint",
        "()Ljavax/microedition/lcdui/Graphics;",
        5,
        1,
        code.finish(),
    );
}
