use super::{
    ACC_PRIVATE, ACC_STATIC, Attribute, ClassFile, Constant, LCDUI_HEIGHT_MARKER_BYTES,
    LCDUI_WIDTH_MARKER_BYTES, Member, append_class, append_constant, append_field_ref,
    append_method_ref, find_field_ref, find_method_ref,
};

pub(crate) fn share_screen_framebuffer(class: &mut ClassFile) {
    const IMAGE_DESCRIPTOR: &str = "Ljavax/microedition/lcdui/Image;";

    let displayable_init = find_method_ref(
        class,
        "javax/microedition/lcdui/Displayable",
        "<init>",
        "()V",
    )
    .expect("Screen constant pool contains Displayable.<init>");
    let create_image = find_method_ref(
        class,
        "javax/microedition/lcdui/Image",
        "createImage",
        "(II)Ljavax/microedition/lcdui/Image;",
    )
    .expect("Screen constant pool contains Image.createImage");
    let framebuffer = find_field_ref(
        class,
        "javax/microedition/lcdui/Screen",
        "framebuffer",
        IMAGE_DESCRIPTOR,
    )
    .expect("Screen constant pool contains its framebuffer");

    let shared_name = append_constant(class, Constant::Utf8("__sharedFramebuffer".to_owned()));
    let image_descriptor = append_constant(class, Constant::Utf8(IMAGE_DESCRIPTOR.to_owned()));
    class.fields.push(Member {
        access_flags: ACC_PRIVATE | ACC_STATIC,
        name_index: shared_name,
        descriptor_index: image_descriptor,
        attributes: Vec::new(),
    });
    let shared_framebuffer = append_field_ref(
        class,
        "javax/microedition/lcdui/Screen",
        "__sharedFramebuffer",
        IMAGE_DESCRIPTOR,
    );

    let constructor_index = class
        .methods
        .iter()
        .position(|method| {
            class.utf8(method.name_index) == Some("<init>")
                && class.utf8(method.descriptor_index) == Some("()V")
        })
        .expect("Screen no-argument constructor exists");
    let Attribute::Code(code) = &mut class.methods[constructor_index].attributes[0] else {
        panic!("Screen no-argument constructor must have bytecode");
    };
    let displayable_init = displayable_init.to_be_bytes();
    let create_image = create_image.to_be_bytes();
    let framebuffer = framebuffer.to_be_bytes();
    assert_eq!(
        code.code,
        [
            0x2a,
            0xb7,
            displayable_init[0],
            displayable_init[1], // super()
            0x2a,
            0x11,
            LCDUI_WIDTH_MARKER_BYTES[0],
            LCDUI_WIDTH_MARKER_BYTES[1],
            0x11,
            LCDUI_HEIGHT_MARKER_BYTES[0],
            LCDUI_HEIGHT_MARKER_BYTES[1],
            0xb8,
            create_image[0],
            create_image[1],
            0xb5,
            framebuffer[0],
            framebuffer[1],
            0xb1,
        ],
        "Screen constructor shape changed"
    );

    // High-level LCDUI screens are mutually exclusive display targets. Their
    // framebuffer is an implementation detail, so retaining one full-size
    // int[] per Form/List/Alert needlessly consumes the guest heap when a game
    // constructs its menus up front. Keep one suite-local surface and assign
    // it to every Screen instance.
    let shared_framebuffer = shared_framebuffer.to_be_bytes();
    code.max_stack = 2;
    code.max_locals = 1;
    code.code = vec![
        0x2a,
        0xb7,
        displayable_init[0],
        displayable_init[1], // super()
        0xb2,
        shared_framebuffer[0],
        shared_framebuffer[1], // getstatic shared framebuffer
        0xc7,
        0x00,
        0x0f, // ifnonnull assign
        0x11,
        LCDUI_WIDTH_MARKER_BYTES[0],
        LCDUI_WIDTH_MARKER_BYTES[1], // explicit width specialization point
        0x11,
        LCDUI_HEIGHT_MARKER_BYTES[0],
        LCDUI_HEIGHT_MARKER_BYTES[1], // explicit height specialization point
        0xb8,
        create_image[0],
        create_image[1], // Image.createImage
        0xb3,
        shared_framebuffer[0],
        shared_framebuffer[1], // putstatic shared framebuffer
        0x2a,                  // assign: aload_0
        0xb2,
        shared_framebuffer[0],
        shared_framebuffer[1], // getstatic shared framebuffer
        0xb5,
        framebuffer[0],
        framebuffer[1], // putfield framebuffer
        0xb1,
    ];
    code.exception_table.clear();
    code.attributes.clear();
}

pub(crate) fn repair_print_stream_hierarchy(class: &mut ClassFile) {
    assert_eq!(
        class.class_name(class.super_class),
        Some("java/lang/Object"),
        "migrated PrintStream must initially extend Object"
    );

    class.super_class = append_class(class, "java/io/OutputStream");
    let output_stream_init = append_method_ref(class, "java/io/OutputStream", "<init>", "()V");
    let constructor_index = class
        .methods
        .iter()
        .position(|method| {
            class.utf8(method.name_index) == Some("<init>")
                && class.utf8(method.descriptor_index) == Some("()V")
        })
        .expect("PrintStream no-argument constructor must exist");
    let Attribute::Code(code) = &mut class.methods[constructor_index].attributes[0] else {
        panic!("PrintStream no-argument constructor must have bytecode");
    };
    assert_eq!(
        code.code,
        [0x2a, 0xb7, 0x00, 0x01, 0xb1],
        "migrated PrintStream constructor shape changed"
    );
    let [init_high, init_low] = output_stream_init.to_be_bytes();
    code.code = vec![0x2a, 0xb7, init_high, init_low, 0xb1];
}

pub(crate) fn allow_owned_authmode_any_open(class: &mut ClassFile) {
    let method_index = class
        .methods
        .iter()
        .position(|method| {
            class.utf8(method.name_index) == Some("openRecordStore")
                && class.utf8(method.descriptor_index)
                    == Some("(Ljava/lang/String;ZIZ)Ljavax/microedition/rms/RecordStore;")
        })
        .expect("RecordStore auth-mode open overload exists");
    let Attribute::Code(code) = &mut class.methods[method_index].attributes[0] else {
        panic!("RecordStore auth-mode open overload must have bytecode");
    };
    assert_eq!(
        code.code,
        [
            0x2a, 0xb8, 0x00, 0x1c, // validateName(name)
            0x1c, 0xb8, 0x00, 0x2b, // validateMode(authmode)
            0x1c, 0x04, 0xa0, 0x00, 0x0d, // authmode != AUTHMODE_ANY: open
            0xbb, 0x00, 0x2f, 0x59, 0x12, 0x31, 0xb7, 0x00, 0x33, 0xbf, 0x2a, 0x2a, 0x1b, 0xb8,
            0x00, 0x23, 0xb8, 0x00, 0x27, 0xb0,
        ],
        "RecordStore auth-mode open guard changed"
    );

    // AUTHMODE_ANY declares how another suite may open this store; opening
    // the active suite is not itself cross-suite access. The owner-qualified
    // overload remains deny-by-default at the RMS host boundary.
    code.code = vec![
        0x2a, 0xb8, 0x00, 0x1c, // validateName(name)
        0x1c, 0xb8, 0x00, 0x2b, // validateMode(authmode)
        0x2a, 0x2a, 0x1b, 0xb8, 0x00, 0x23, 0xb8, 0x00, 0x27, 0xb0,
    ];
}
