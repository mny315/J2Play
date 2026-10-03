//! Graphics context and resource-loading entry points.

use super::{ACC_FINAL, ACC_PUBLIC, ACC_STATIC, ACC_SUPER, M3gClassSpec, method};

pub(super) const GRAPHICS_3D: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/Graphics3D",
    super_name: "java/lang/Object",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[
        ("ANTIALIAS", 2),
        ("DITHER", 4),
        ("TRUE_COLOR", 8),
        ("OVERWRITE", 16),
    ],
    methods: &[
        method(
            "getInstance",
            "()Ljavax/microedition/m3g/Graphics3D;",
            ACC_PUBLIC | ACC_STATIC | ACC_FINAL,
        ),
        method("bindTarget", "(Ljava/lang/Object;)V", ACC_PUBLIC),
        method("bindTarget", "(Ljava/lang/Object;ZI)V", ACC_PUBLIC),
        method("releaseTarget", "()V", ACC_PUBLIC),
        method("getTarget", "()Ljava/lang/Object;", ACC_PUBLIC),
        method("getHints", "()I", ACC_PUBLIC),
        method("isDepthBufferEnabled", "()Z", ACC_PUBLIC),
        method("setViewport", "(IIII)V", ACC_PUBLIC),
        method("getViewportX", "()I", ACC_PUBLIC),
        method("getViewportY", "()I", ACC_PUBLIC),
        method("getViewportWidth", "()I", ACC_PUBLIC),
        method("getViewportHeight", "()I", ACC_PUBLIC),
        method("setDepthRange", "(FF)V", ACC_PUBLIC),
        method("getDepthRangeNear", "()F", ACC_PUBLIC),
        method("getDepthRangeFar", "()F", ACC_PUBLIC),
        method(
            "clear",
            "(Ljavax/microedition/m3g/Background;)V",
            ACC_PUBLIC,
        ),
        method("render", "(Ljavax/microedition/m3g/World;)V", ACC_PUBLIC),
        method(
            "render",
            "(Ljavax/microedition/m3g/Node;Ljavax/microedition/m3g/Transform;)V",
            ACC_PUBLIC,
        ),
        method(
            "render",
            "(Ljavax/microedition/m3g/VertexBuffer;Ljavax/microedition/m3g/IndexBuffer;Ljavax/microedition/m3g/Appearance;Ljavax/microedition/m3g/Transform;I)V",
            ACC_PUBLIC,
        ),
        method(
            "render",
            "(Ljavax/microedition/m3g/VertexBuffer;Ljavax/microedition/m3g/IndexBuffer;Ljavax/microedition/m3g/Appearance;Ljavax/microedition/m3g/Transform;)V",
            ACC_PUBLIC,
        ),
        method(
            "setCamera",
            "(Ljavax/microedition/m3g/Camera;Ljavax/microedition/m3g/Transform;)V",
            ACC_PUBLIC,
        ),
        method(
            "getCamera",
            "(Ljavax/microedition/m3g/Transform;)Ljavax/microedition/m3g/Camera;",
            ACC_PUBLIC,
        ),
        method(
            "addLight",
            "(Ljavax/microedition/m3g/Light;Ljavax/microedition/m3g/Transform;)I",
            ACC_PUBLIC,
        ),
        method(
            "setLight",
            "(ILjavax/microedition/m3g/Light;Ljavax/microedition/m3g/Transform;)V",
            ACC_PUBLIC,
        ),
        method("resetLights", "()V", ACC_PUBLIC),
        method("getLightCount", "()I", ACC_PUBLIC),
        method(
            "getLight",
            "(ILjavax/microedition/m3g/Transform;)Ljavax/microedition/m3g/Light;",
            ACC_PUBLIC,
        ),
        method(
            "getProperties",
            "()Ljava/util/Hashtable;",
            ACC_PUBLIC | ACC_STATIC | ACC_FINAL,
        ),
    ],
};

pub(super) const LOADER: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/Loader",
    super_name: "java/lang/Object",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[],
    methods: &[
        method(
            "load",
            "(Ljava/lang/String;)[Ljavax/microedition/m3g/Object3D;",
            ACC_PUBLIC | ACC_STATIC,
        ),
        method(
            "load",
            "([BI)[Ljavax/microedition/m3g/Object3D;",
            ACC_PUBLIC | ACC_STATIC,
        ),
    ],
};
