use super::{
    ACC_ABSTRACT, ACC_FINAL, ACC_INTERFACE, ACC_PRIVATE, ACC_PROTECTED, ACC_PUBLIC, ACC_STATIC,
    ACC_SUPER, ClassFile, ConstantPool, abstract_method,
    append_nokia_full_canvas_callback_key_adapter, code_method, forwarding_constructor,
    interface_class, interface_class_with_fields, interface_class_with_string_fields, native_class,
    native_method, nokia_direct_graphics_methods,
};

pub(crate) fn java_nio_buffer_overflow_exception() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("java/nio/BufferOverflowException");
    let super_class = pool.class("java/lang/RuntimeException");
    let code_name = pool.utf8("Code");
    let [high, low] = pool
        .method_ref("java/lang/RuntimeException", "<init>", "()V")
        .to_be_bytes();
    let constructor = forwarding_constructor(
        &mut pool,
        code_name,
        "()V",
        1,
        vec![0x2a, 0xb7, high, low, 0xb1],
    );
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_SUPER,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods: vec![constructor],
        attributes: Vec::new(),
    }
}

pub(crate) fn java_nio_buffer() -> ClassFile {
    native_class(
        "java/nio/Buffer",
        "java/lang/Object",
        &[],
        &[
            (ACC_PRIVATE, "position", "I"),
            (ACC_PRIVATE, "capacity", "I"),
            (ACC_PRIVATE, "bytes", "[B"),
            (ACC_PRIVATE, "byteOffset", "I"),
        ],
        &[
            (ACC_PUBLIC, "remaining", "()I"),
            (ACC_PUBLIC, "rewind", "()Ljava/nio/Buffer;"),
        ],
    )
}

pub(crate) fn java_nio_byte_buffer() -> ClassFile {
    native_class(
        "java/nio/ByteBuffer",
        "java/nio/Buffer",
        &[],
        &[],
        &[
            (
                ACC_PUBLIC | ACC_STATIC,
                "allocateDirect",
                "(I)Ljava/nio/ByteBuffer;",
            ),
            (ACC_PUBLIC, "asFloatBuffer", "()Ljava/nio/FloatBuffer;"),
            (ACC_PUBLIC, "asIntBuffer", "()Ljava/nio/IntBuffer;"),
        ],
    )
}

pub(crate) fn java_nio_float_buffer() -> ClassFile {
    native_class(
        "java/nio/FloatBuffer",
        "java/nio/Buffer",
        &[],
        &[],
        &[(ACC_PUBLIC, "put", "([F)Ljava/nio/FloatBuffer;")],
    )
}

pub(crate) fn java_nio_int_buffer() -> ClassFile {
    native_class(
        "java/nio/IntBuffer",
        "java/nio/Buffer",
        &[],
        &[],
        &[(ACC_PUBLIC, "put", "([I)Ljava/nio/IntBuffer;")],
    )
}

pub(crate) fn javax_microedition_khronos_egl_egl() -> ClassFile {
    interface_class("javax/microedition/khronos/egl/EGL", &[], &[])
}

pub(crate) const EGL11_METHODS: &[(&str, &str)] = &[
    (
        "eglGetDisplay",
        "(Ljava/lang/Object;)Ljavax/microedition/khronos/egl/EGLDisplay;",
    ),
    (
        "eglInitialize",
        "(Ljavax/microedition/khronos/egl/EGLDisplay;[I)Z",
    ),
    (
        "eglGetConfigs",
        "(Ljavax/microedition/khronos/egl/EGLDisplay;[Ljavax/microedition/khronos/egl/EGLConfig;I[I)Z",
    ),
    (
        "eglChooseConfig",
        "(Ljavax/microedition/khronos/egl/EGLDisplay;[I[Ljavax/microedition/khronos/egl/EGLConfig;I[I)Z",
    ),
    (
        "eglCreateContext",
        "(Ljavax/microedition/khronos/egl/EGLDisplay;Ljavax/microedition/khronos/egl/EGLConfig;Ljavax/microedition/khronos/egl/EGLContext;[I)Ljavax/microedition/khronos/egl/EGLContext;",
    ),
    (
        "eglCreateWindowSurface",
        "(Ljavax/microedition/khronos/egl/EGLDisplay;Ljavax/microedition/khronos/egl/EGLConfig;Ljava/lang/Object;[I)Ljavax/microedition/khronos/egl/EGLSurface;",
    ),
    (
        "eglMakeCurrent",
        "(Ljavax/microedition/khronos/egl/EGLDisplay;Ljavax/microedition/khronos/egl/EGLSurface;Ljavax/microedition/khronos/egl/EGLSurface;Ljavax/microedition/khronos/egl/EGLContext;)Z",
    ),
    (
        "eglTerminate",
        "(Ljavax/microedition/khronos/egl/EGLDisplay;)Z",
    ),
    ("eglWaitGL", "()Z"),
    ("eglWaitNative", "(ILjava/lang/Object;)Z"),
];

pub(crate) fn javax_microedition_khronos_egl_egl11() -> ClassFile {
    interface_class_with_fields(
        "javax/microedition/khronos/egl/EGL11",
        &["javax/microedition/khronos/egl/EGL"],
        &[
            ("EGL_DEFAULT_DISPLAY", "Ljava/lang/Object;"),
            (
                "EGL_NO_CONTEXT",
                "Ljavax/microedition/khronos/egl/EGLContext;",
            ),
            (
                "EGL_NO_SURFACE",
                "Ljavax/microedition/khronos/egl/EGLSurface;",
            ),
        ],
        EGL11_METHODS,
    )
}

pub(crate) fn javax_microedition_khronos_egl_egl_impl() -> ClassFile {
    let methods = EGL11_METHODS
        .iter()
        .map(|(name, descriptor)| (ACC_PUBLIC, *name, *descriptor))
        .collect::<Vec<_>>();
    native_class(
        "javax/microedition/khronos/egl/EGLImpl",
        "java/lang/Object",
        &["javax/microedition/khronos/egl/EGL11"],
        &[],
        methods.as_slice(),
    )
}

pub(crate) fn javax_microedition_khronos_egl_context() -> ClassFile {
    native_class(
        "javax/microedition/khronos/egl/EGLContext",
        "java/lang/Object",
        &[],
        &[],
        &[
            (
                ACC_PUBLIC | ACC_STATIC,
                "getEGL",
                "()Ljavax/microedition/khronos/egl/EGL;",
            ),
            (
                ACC_PUBLIC,
                "getGL",
                "()Ljavax/microedition/khronos/opengles/GL;",
            ),
        ],
    )
}

pub(crate) fn jsr239_handle_class(name: &str) -> ClassFile {
    native_class(name, "java/lang/Object", &[], &[], &[])
}

pub(crate) fn javax_microedition_khronos_egl_surface() -> ClassFile {
    native_class(
        "javax/microedition/khronos/egl/EGLSurface",
        "java/lang/Object",
        &[],
        &[(ACC_PUBLIC, "target", "Ljava/lang/Object;")],
        &[],
    )
}

pub(crate) fn javax_microedition_khronos_opengles_gl() -> ClassFile {
    interface_class("javax/microedition/khronos/opengles/GL", &[], &[])
}

pub(crate) const GL11_METHODS: &[(&str, &str)] = &[
    ("glBindTexture", "(II)V"),
    ("glBlendFunc", "(II)V"),
    ("glColor4f", "(FFFF)V"),
    ("glDisable", "(I)V"),
    ("glDisableClientState", "(I)V"),
    ("glDrawArrays", "(III)V"),
    ("glEnable", "(I)V"),
    ("glEnableClientState", "(I)V"),
    ("glFrustumf", "(FFFFFF)V"),
    ("glGenTextures", "(I[II)V"),
    ("glLoadIdentity", "()V"),
    ("glMatrixMode", "(I)V"),
    ("glPopMatrix", "()V"),
    ("glPushMatrix", "()V"),
    ("glScalef", "(FFF)V"),
    ("glTexCoordPointer", "(IIILjava/nio/Buffer;)V"),
    ("glTexEnvi", "(III)V"),
    ("glTexImage2D", "(IIIIIIIILjava/nio/Buffer;)V"),
    ("glTexParameteri", "(III)V"),
    ("glTranslatef", "(FFF)V"),
    ("glVertexPointer", "(IIILjava/nio/Buffer;)V"),
];

pub(crate) fn javax_microedition_khronos_opengles_gl11() -> ClassFile {
    interface_class(
        "javax/microedition/khronos/opengles/GL11",
        &["javax/microedition/khronos/opengles/GL"],
        GL11_METHODS,
    )
}

pub(crate) fn javax_microedition_khronos_opengles_gl_impl() -> ClassFile {
    let methods = GL11_METHODS
        .iter()
        .map(|(name, descriptor)| (ACC_PUBLIC, *name, *descriptor))
        .collect::<Vec<_>>();
    native_class(
        "javax/microedition/khronos/opengles/GLImpl",
        "java/lang/Object",
        &["javax/microedition/khronos/opengles/GL11"],
        &[],
        methods.as_slice(),
    )
}

pub(crate) fn javax_microedition_sensor_data() -> ClassFile {
    interface_class(
        "javax/microedition/sensor/Data",
        &[],
        &[("getIntValues", "()[I")],
    )
}

pub(crate) fn javax_microedition_sensor_data_listener() -> ClassFile {
    interface_class(
        "javax/microedition/sensor/DataListener",
        &[],
        &[(
            "dataReceived",
            "(Ljavax/microedition/sensor/SensorConnection;[Ljavax/microedition/sensor/Data;Z)V",
        )],
    )
}

pub(crate) fn javax_microedition_sensor_sensor_connection() -> ClassFile {
    interface_class(
        "javax/microedition/sensor/SensorConnection",
        &["javax/microedition/io/Connection"],
        &[
            (
                "setDataListener",
                "(Ljavax/microedition/sensor/DataListener;I)V",
            ),
            ("removeDataListener", "()V"),
        ],
    )
}

pub(crate) fn javax_microedition_sensor_sensor_info() -> ClassFile {
    interface_class_with_string_fields(
        "javax/microedition/sensor/SensorInfo",
        &[],
        &[
            ("CONTEXT_TYPE_AMBIENT", "ambient"),
            ("CONTEXT_TYPE_DEVICE", "device"),
            ("CONTEXT_TYPE_USER", "user"),
        ],
        &[("getUrl", "()Ljava/lang/String;")],
    )
}

pub(crate) fn javax_microedition_sensor_sensor_manager() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("javax/microedition/sensor/SensorManager");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let object_init = pool.method_ref("java/lang/Object", "<init>", "()V");
    let sensor_info = pool.class("javax/microedition/sensor/SensorInfo");
    let methods = vec![
        code_method(
            &mut pool,
            code_name,
            ACC_PRIVATE,
            "<init>",
            "()V",
            1,
            1,
            vec![
                0x2a,
                0xb7,
                (object_init >> 8) as u8,
                object_init as u8,
                0xb1,
            ],
        ),
        code_method(
            &mut pool,
            code_name,
            ACC_PUBLIC | ACC_STATIC,
            "findSensors",
            "(Ljava/lang/String;Ljava/lang/String;)[Ljavax/microedition/sensor/SensorInfo;",
            1,
            2,
            vec![
                0x03,
                0xbd,
                (sensor_info >> 8) as u8,
                sensor_info as u8,
                0xb0,
            ],
        ),
    ];
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_FINAL | ACC_SUPER,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods,
        attributes: Vec::new(),
    }
}

pub(crate) fn javax_wireless_messaging_message() -> ClassFile {
    interface_class(
        "javax/wireless/messaging/Message",
        &[],
        &[
            ("getAddress", "()Ljava/lang/String;"),
            ("setAddress", "(Ljava/lang/String;)V"),
            ("getTimestamp", "()Ljava/util/Date;"),
        ],
    )
}

pub(crate) fn javax_wireless_messaging_binary_message() -> ClassFile {
    interface_class(
        "javax/wireless/messaging/BinaryMessage",
        &["javax/wireless/messaging/Message"],
        &[("getPayloadData", "()[B"), ("setPayloadData", "([B)V")],
    )
}

pub(crate) fn javax_wireless_messaging_text_message() -> ClassFile {
    interface_class(
        "javax/wireless/messaging/TextMessage",
        &["javax/wireless/messaging/Message"],
        &[
            ("getPayloadText", "()Ljava/lang/String;"),
            ("setPayloadText", "(Ljava/lang/String;)V"),
        ],
    )
}

pub(crate) fn javax_wireless_messaging_message_listener() -> ClassFile {
    interface_class(
        "javax/wireless/messaging/MessageListener",
        &[],
        &[(
            "notifyIncomingMessage",
            "(Ljavax/wireless/messaging/MessageConnection;)V",
        )],
    )
}

pub(crate) fn javax_wireless_messaging_message_connection() -> ClassFile {
    interface_class_with_string_fields(
        "javax/wireless/messaging/MessageConnection",
        &["javax/microedition/io/Connection"],
        &[
            ("TEXT_MESSAGE", "text"),
            ("BINARY_MESSAGE", "binary"),
            ("MULTIPART_MESSAGE", "multipart"),
        ],
        &[
            (
                "newMessage",
                "(Ljava/lang/String;)Ljavax/wireless/messaging/Message;",
            ),
            (
                "newMessage",
                "(Ljava/lang/String;Ljava/lang/String;)Ljavax/wireless/messaging/Message;",
            ),
            ("numberOfSegments", "(Ljavax/wireless/messaging/Message;)I"),
            ("receive", "()Ljavax/wireless/messaging/Message;"),
            ("send", "(Ljavax/wireless/messaging/Message;)V"),
            (
                "setMessageListener",
                "(Ljavax/wireless/messaging/MessageListener;)V",
            ),
        ],
    )
}

pub(crate) fn com_nokia_mid_ui_direct_graphics() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("com/nokia/mid/ui/DirectGraphics");
    let super_class = pool.class("java/lang/Object");
    let methods = nokia_direct_graphics_methods()
        .iter()
        .map(|(name, descriptor)| abstract_method(&mut pool, ACC_PUBLIC, name, descriptor))
        .collect();

    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_INTERFACE | ACC_ABSTRACT,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods,
        attributes: Vec::new(),
    }
}

pub(crate) fn com_nokia_mid_ui_full_canvas() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("com/nokia/mid/ui/FullCanvas");
    let super_class = pool.class("javax/microedition/lcdui/Canvas");
    let code_name = pool.utf8("Code");
    let canvas_init = pool.method_ref("javax/microedition/lcdui/Canvas", "<init>", "()V");
    let set_fullscreen = pool.method_ref(
        "javax/microedition/lcdui/Canvas",
        "setFullScreenMode",
        "(Z)V",
    );
    let methods = vec![code_method(
        &mut pool,
        code_name,
        ACC_PROTECTED,
        "<init>",
        "()V",
        2,
        1,
        vec![
            0x2a, // aload_0
            0xb7,
            (canvas_init >> 8) as u8,
            canvas_init as u8, // invokespecial Canvas.<init>:()V
            0x2a,              // aload_0
            0x04,              // iconst_1
            0xb6,
            (set_fullscreen >> 8) as u8,
            set_fullscreen as u8, // invokevirtual Canvas.setFullScreenMode:(Z)V
            0xb1,                 // return
        ],
    )];

    let mut class = ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_SUPER | ACC_ABSTRACT,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods,
        attributes: Vec::new(),
    };
    append_nokia_full_canvas_callback_key_adapter(&mut class);
    class
}

pub(crate) fn com_nokia_mid_ui_direct_utils() -> ClassFile {
    let mut pool = ConstantPool::new();
    let this_class = pool.class("com/nokia/mid/ui/DirectUtils");
    let super_class = pool.class("java/lang/Object");
    let code_name = pool.utf8("Code");
    let object_init = pool.method_ref("java/lang/Object", "<init>", "()V");
    let methods = vec![
        code_method(
            &mut pool,
            code_name,
            ACC_PRIVATE,
            "<init>",
            "()V",
            1,
            1,
            vec![
                0x2a, // aload_0
                0xb7,
                (object_init >> 8) as u8,
                object_init as u8, // invokespecial Object.<init>:()V
                0xb1,              // return
            ],
        ),
        native_method(
            &mut pool,
            ACC_PUBLIC | ACC_STATIC,
            "getDirectGraphics",
            "(Ljavax/microedition/lcdui/Graphics;)Lcom/nokia/mid/ui/DirectGraphics;",
        ),
        native_method(
            &mut pool,
            ACC_PUBLIC | ACC_STATIC,
            "createImage",
            "(III)Ljavax/microedition/lcdui/Image;",
        ),
        native_method(
            &mut pool,
            ACC_PUBLIC | ACC_STATIC,
            "createImage",
            "([BII)Ljavax/microedition/lcdui/Image;",
        ),
    ];

    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.finish(),
        access_flags: ACC_PUBLIC | ACC_FINAL | ACC_SUPER,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields: Vec::new(),
        methods,
        attributes: Vec::new(),
    }
}
