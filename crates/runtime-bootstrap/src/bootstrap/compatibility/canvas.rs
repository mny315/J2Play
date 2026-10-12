use super::{
    ACC_FINAL, ACC_PRIVATE, Attribute, ClassFile, CodeAttribute, Constant, Member,
    append_code_method, append_constant, append_field_ref, append_method_ref, find_field_ref,
    find_method_ref, method_code_mut, method_mut,
};
use crate::bytecode_builder::Code;

pub(crate) fn stabilize_canvas_paint_graphics(class: &mut ClassFile) {
    let graphics_descriptor = "Ljavax/microedition/lcdui/Graphics;";
    let field_descriptor = append_constant(class, Constant::Utf8(graphics_descriptor.to_owned()));
    for name in ["paintGraphics", "gameGraphics"] {
        let name_index = append_constant(class, Constant::Utf8(name.to_owned()));
        class.fields.push(Member {
            access_flags: ACC_PRIVATE | ACC_FINAL,
            name_index,
            descriptor_index: field_descriptor,
            attributes: Vec::new(),
        });
    }

    let framebuffer = find_field_ref(
        class,
        "javax/microedition/lcdui/Canvas",
        "framebuffer",
        "Ljavax/microedition/lcdui/Image;",
    )
    .expect("Canvas constant pool contains Canvas.framebuffer");
    let get_graphics = find_method_ref(
        class,
        "javax/microedition/lcdui/Image",
        "getGraphics",
        "()Ljavax/microedition/lcdui/Graphics;",
    )
    .expect("Canvas constant pool contains Image.getGraphics");
    let paint_graphics = append_field_ref(
        class,
        "javax/microedition/lcdui/Canvas",
        "paintGraphics",
        graphics_descriptor,
    );
    let game_graphics = append_field_ref(
        class,
        "javax/microedition/lcdui/Canvas",
        "gameGraphics",
        graphics_descriptor,
    );
    let reset_graphics = append_method_ref(
        class,
        "javax/microedition/lcdui/Graphics",
        "__resetForPaint",
        "()Ljavax/microedition/lcdui/Graphics;",
    );
    let graphics_canvas = append_field_ref(
        class,
        "javax/microedition/lcdui/Graphics",
        "canvas",
        "Ljavax/microedition/lcdui/Canvas;",
    );

    let constructor = method_code_mut(class, "<init>", "()V");
    assert_eq!(
        constructor.code.pop(),
        Some(0xb1),
        "Canvas constructor must end with return"
    );
    let mut code = Code::default();
    for field in [paint_graphics, game_graphics] {
        code.emit(&[0x2a, 0x2a])
            .reference(0xb4, framebuffer)
            .reference(0xb6, get_graphics)
            .reference(0xb5, field);
        code.emit(&[0x2a])
            .reference(0xb4, field)
            .emit(&[0x2a])
            .reference(0xb5, graphics_canvas);
    }
    code.emit(&[0xb1]);
    constructor.code.extend(code.finish());
    constructor.max_stack = constructor.max_stack.max(2);

    let game_graphics_method = method_code_mut(
        class,
        "__getGameGraphics",
        "()Ljavax/microedition/lcdui/Graphics;",
    );
    game_graphics_method.max_stack = 1;
    game_graphics_method.max_locals = 1;
    // Keep one retargetable context for Canvas mode changes, but expose the
    // initial Graphics state required for every GameCanvas.getGraphics call.
    let mut code = Code::default();
    code.emit(&[0x2a])
        .reference(0xb4, game_graphics)
        .reference(0xb6, reset_graphics)
        .emit(&[0xb0]);
    game_graphics_method.code = code.finish();
    game_graphics_method.exception_table.clear();
    game_graphics_method.attributes.clear();

    let service = method_code_mut(class, "__serviceOneRepaint", "()Z");
    let framebuffer = framebuffer.to_be_bytes();
    let get_graphics = get_graphics.to_be_bytes();
    let paint_graphics = paint_graphics.to_be_bytes();
    let reset_graphics = reset_graphics.to_be_bytes();
    let old = [
        0x2a,
        0xb4,
        framebuffer[0],
        framebuffer[1],
        0xb6,
        get_graphics[0],
        get_graphics[1],
    ];
    let mut patched = 0;
    for offset in 0..service.code.len().saturating_sub(old.len() - 1) {
        if service.code[offset..offset + old.len()] == old {
            service.code[offset..offset + old.len()].copy_from_slice(&[
                0x2a,
                0xb4,
                paint_graphics[0],
                paint_graphics[1],
                0xb6,
                reset_graphics[0],
                reset_graphics[1],
            ]);
            patched += 1;
        }
    }
    assert_eq!(
        patched, 1,
        "Canvas.__serviceOneRepaint must acquire one framebuffer Graphics"
    );
}

pub(crate) fn isolate_canvas_paint_callback_exceptions(class: &mut ClassFile) {
    let code = method_code_mut(class, "__serviceOneRepaint", "()Z");
    let handler = code
        .exception_table
        .iter()
        .find(|handler| handler.catch_type == 0)
        .expect("Canvas.__serviceOneRepaint resets paint state through a catch-all handler");
    let return_offset = usize::from(handler.handler_pc) + 7;
    assert_eq!(
        code.code.get(return_offset..return_offset + 3),
        Some([0x19, 0x06, 0xbf].as_slice()),
        "Canvas.__serviceOneRepaint catch-all must end with aload 6; athrow"
    );
    // paint() is called by the implementation-owned LCDUI event dispatcher.
    // Keep a broken or temporarily unready application callback from unwinding
    // through the AMS host driver after the catch-all restores `painting`.
    code.code[return_offset..return_offset + 3].copy_from_slice(&[
        0x03, // iconst_0
        0xac, // ireturn
        0x00, // nop (preserve all following bytecode offsets)
    ]);
}

pub(crate) fn append_canvas_size_change_dispatch(class: &mut ClassFile) {
    let pending_name = append_constant(class, Constant::Utf8("sizeChangePending".to_owned()));
    let boolean_descriptor = append_constant(class, Constant::Utf8("Z".to_owned()));
    class.fields.push(Member {
        access_flags: ACC_PRIVATE,
        name_index: pending_name,
        descriptor_index: boolean_descriptor,
        attributes: Vec::new(),
    });

    let pending = append_field_ref(
        class,
        "javax/microedition/lcdui/Canvas",
        "sizeChangePending",
        "Z",
    );
    let get_width = append_method_ref(class, "javax/microedition/lcdui/Canvas", "getWidth", "()I");
    let get_height =
        append_method_ref(class, "javax/microedition/lcdui/Canvas", "getHeight", "()I");
    let size_changed = append_method_ref(
        class,
        "javax/microedition/lcdui/Canvas",
        "sizeChanged",
        "(II)V",
    );
    let mut code = Code::default();
    code.emit(&[0x2a])
        .reference(0xb4, pending)
        .jump(0x99, "done");
    code.emit(&[0x2a, 0x03])
        .reference(0xb5, pending)
        .emit(&[0x2a, 0x2a])
        .reference(0xb6, get_width)
        .emit(&[0x2a])
        .reference(0xb6, get_height)
        .reference(0xb6, size_changed);
    code.label("done").emit(&[0xb1]);
    append_code_method(
        class,
        ACC_PRIVATE | ACC_FINAL,
        "__notifySizeChanged",
        "()V",
        3,
        1,
        code.finish(),
    );
    let notify = append_method_ref(
        class,
        "javax/microedition/lcdui/Canvas",
        "__notifySizeChanged",
        "()V",
    );
    let show_notify = append_method_ref(
        class,
        "javax/microedition/lcdui/Canvas",
        "showNotify",
        "()V",
    );
    let service_one = append_method_ref(
        class,
        "javax/microedition/lcdui/Canvas",
        "__serviceOneRepaint",
        "()Z",
    );

    for (name, opcode, callback, returns_value) in [
        ("__show", 0xb6, show_notify, false),
        ("__hostIdle", 0xb7, service_one, true),
    ] {
        let mut code = Code::default();
        code.emit(&[0x2a])
            .reference(0xb7, notify)
            .emit(&[0x2a])
            .reference(opcode, callback);
        if returns_value {
            code.emit(&[0x57]); // discard repaint result
        }
        code.emit(&[0xb1]);
        let method = method_mut(class, name, "()V");
        let code_name = method
            .attributes
            .iter()
            .find_map(|attribute| match attribute {
                Attribute::Code(code) => Some(code.name_index),
                Attribute::Raw { .. } => None,
            })
            .unwrap_or_else(|| panic!("Canvas.{name} has Code"));
        method.attributes = vec![Attribute::Code(CodeAttribute {
            name_index: code_name,
            max_stack: 1,
            max_locals: 1,
            code: code.finish(),
            exception_table: Vec::new(),
            attributes: Vec::new(),
        })];
    }
}
