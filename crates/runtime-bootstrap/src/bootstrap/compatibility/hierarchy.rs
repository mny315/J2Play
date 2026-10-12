use super::{
    ACC_PRIVATE, ACC_STATIC, Attribute, ClassFile, Constant, LCDUI_HEIGHT_MARKER_BYTES,
    LCDUI_WIDTH_MARKER_BYTES, Member, append_constant, append_field_ref, find_field_ref,
    find_method_ref,
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
