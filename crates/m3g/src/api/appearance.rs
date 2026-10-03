//! Appearance state, materials, textures and image resources.

use super::{ACC_PUBLIC, ACC_SUPER, M3gClassSpec, method};

pub(super) const APPEARANCE: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/Appearance",
    super_name: "javax/microedition/m3g/Object3D",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[],
    methods: &[
        method("<init>", "()V", ACC_PUBLIC),
        method("setLayer", "(I)V", ACC_PUBLIC),
        method("getLayer", "()I", ACC_PUBLIC),
        method("setFog", "(Ljavax/microedition/m3g/Fog;)V", ACC_PUBLIC),
        method("getFog", "()Ljavax/microedition/m3g/Fog;", ACC_PUBLIC),
        method(
            "setPolygonMode",
            "(Ljavax/microedition/m3g/PolygonMode;)V",
            ACC_PUBLIC,
        ),
        method(
            "getPolygonMode",
            "()Ljavax/microedition/m3g/PolygonMode;",
            ACC_PUBLIC,
        ),
        method(
            "setCompositingMode",
            "(Ljavax/microedition/m3g/CompositingMode;)V",
            ACC_PUBLIC,
        ),
        method(
            "getCompositingMode",
            "()Ljavax/microedition/m3g/CompositingMode;",
            ACC_PUBLIC,
        ),
        method(
            "setTexture",
            "(ILjavax/microedition/m3g/Texture2D;)V",
            ACC_PUBLIC,
        ),
        method(
            "getTexture",
            "(I)Ljavax/microedition/m3g/Texture2D;",
            ACC_PUBLIC,
        ),
        method(
            "setMaterial",
            "(Ljavax/microedition/m3g/Material;)V",
            ACC_PUBLIC,
        ),
        method(
            "getMaterial",
            "()Ljavax/microedition/m3g/Material;",
            ACC_PUBLIC,
        ),
    ],
};

pub(super) const BACKGROUND: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/Background",
    super_name: "javax/microedition/m3g/Object3D",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[("BORDER", 32), ("REPEAT", 33)],
    methods: &[
        method("<init>", "()V", ACC_PUBLIC),
        method("setColorClearEnable", "(Z)V", ACC_PUBLIC),
        method("isColorClearEnabled", "()Z", ACC_PUBLIC),
        method("setDepthClearEnable", "(Z)V", ACC_PUBLIC),
        method("isDepthClearEnabled", "()Z", ACC_PUBLIC),
        method("setColor", "(I)V", ACC_PUBLIC),
        method("getColor", "()I", ACC_PUBLIC),
        method(
            "setImage",
            "(Ljavax/microedition/m3g/Image2D;)V",
            ACC_PUBLIC,
        ),
        method("getImage", "()Ljavax/microedition/m3g/Image2D;", ACC_PUBLIC),
        method("setImageMode", "(II)V", ACC_PUBLIC),
        method("getImageModeX", "()I", ACC_PUBLIC),
        method("getImageModeY", "()I", ACC_PUBLIC),
        method("setCrop", "(IIII)V", ACC_PUBLIC),
        method("getCropX", "()I", ACC_PUBLIC),
        method("getCropY", "()I", ACC_PUBLIC),
        method("getCropWidth", "()I", ACC_PUBLIC),
        method("getCropHeight", "()I", ACC_PUBLIC),
    ],
};

pub(super) const COMPOSITING_MODE: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/CompositingMode",
    super_name: "javax/microedition/m3g/Object3D",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[
        ("ALPHA", 64),
        ("ALPHA_ADD", 65),
        ("MODULATE", 66),
        ("MODULATE_X2", 67),
        ("REPLACE", 68),
    ],
    methods: &[
        method("<init>", "()V", ACC_PUBLIC),
        method("setBlending", "(I)V", ACC_PUBLIC),
        method("getBlending", "()I", ACC_PUBLIC),
        method("setAlphaThreshold", "(F)V", ACC_PUBLIC),
        method("getAlphaThreshold", "()F", ACC_PUBLIC),
        method("setAlphaWriteEnable", "(Z)V", ACC_PUBLIC),
        method("isAlphaWriteEnabled", "()Z", ACC_PUBLIC),
        method("setColorWriteEnable", "(Z)V", ACC_PUBLIC),
        method("isColorWriteEnabled", "()Z", ACC_PUBLIC),
        method("setDepthWriteEnable", "(Z)V", ACC_PUBLIC),
        method("isDepthWriteEnabled", "()Z", ACC_PUBLIC),
        method("setDepthTestEnable", "(Z)V", ACC_PUBLIC),
        method("isDepthTestEnabled", "()Z", ACC_PUBLIC),
        method("setDepthOffset", "(FF)V", ACC_PUBLIC),
        method("getDepthOffsetFactor", "()F", ACC_PUBLIC),
        method("getDepthOffsetUnits", "()F", ACC_PUBLIC),
    ],
};

pub(super) const FOG: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/Fog",
    super_name: "javax/microedition/m3g/Object3D",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[("EXPONENTIAL", 80), ("LINEAR", 81)],
    methods: &[
        method("<init>", "()V", ACC_PUBLIC),
        method("setMode", "(I)V", ACC_PUBLIC),
        method("getMode", "()I", ACC_PUBLIC),
        method("setLinear", "(FF)V", ACC_PUBLIC),
        method("getNearDistance", "()F", ACC_PUBLIC),
        method("getFarDistance", "()F", ACC_PUBLIC),
        method("setDensity", "(F)V", ACC_PUBLIC),
        method("getDensity", "()F", ACC_PUBLIC),
        method("setColor", "(I)V", ACC_PUBLIC),
        method("getColor", "()I", ACC_PUBLIC),
    ],
};

pub(super) const IMAGE_2D: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/Image2D",
    super_name: "javax/microedition/m3g/Object3D",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[
        ("ALPHA", 96),
        ("LUMINANCE", 97),
        ("LUMINANCE_ALPHA", 98),
        ("RGB", 99),
        ("RGBA", 100),
    ],
    methods: &[
        method("<init>", "(ILjava/lang/Object;)V", ACC_PUBLIC),
        method("<init>", "(III[B)V", ACC_PUBLIC),
        method("<init>", "(III[B[B)V", ACC_PUBLIC),
        method("<init>", "(III)V", ACC_PUBLIC),
        method("set", "(IIII[B)V", ACC_PUBLIC),
        method("isMutable", "()Z", ACC_PUBLIC),
        method("getFormat", "()I", ACC_PUBLIC),
        method("getWidth", "()I", ACC_PUBLIC),
        method("getHeight", "()I", ACC_PUBLIC),
    ],
};

pub(super) const MATERIAL: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/Material",
    super_name: "javax/microedition/m3g/Object3D",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[
        ("AMBIENT", 1024),
        ("DIFFUSE", 2048),
        ("EMISSIVE", 4096),
        ("SPECULAR", 8192),
    ],
    methods: &[
        method("<init>", "()V", ACC_PUBLIC),
        method("setColor", "(II)V", ACC_PUBLIC),
        method("getColor", "(I)I", ACC_PUBLIC),
        method("setShininess", "(F)V", ACC_PUBLIC),
        method("getShininess", "()F", ACC_PUBLIC),
        method("setVertexColorTrackingEnable", "(Z)V", ACC_PUBLIC),
        method("isVertexColorTrackingEnabled", "()Z", ACC_PUBLIC),
    ],
};

pub(super) const POLYGON_MODE: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/PolygonMode",
    super_name: "javax/microedition/m3g/Object3D",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[
        ("CULL_BACK", 160),
        ("CULL_FRONT", 161),
        ("CULL_NONE", 162),
        ("SHADE_FLAT", 164),
        ("SHADE_SMOOTH", 165),
        ("WINDING_CCW", 168),
        ("WINDING_CW", 169),
    ],
    methods: &[
        method("<init>", "()V", ACC_PUBLIC),
        method("setCulling", "(I)V", ACC_PUBLIC),
        method("getCulling", "()I", ACC_PUBLIC),
        method("setWinding", "(I)V", ACC_PUBLIC),
        method("getWinding", "()I", ACC_PUBLIC),
        method("setShading", "(I)V", ACC_PUBLIC),
        method("getShading", "()I", ACC_PUBLIC),
        method("setTwoSidedLightingEnable", "(Z)V", ACC_PUBLIC),
        method("isTwoSidedLightingEnabled", "()Z", ACC_PUBLIC),
        method("setLocalCameraLightingEnable", "(Z)V", ACC_PUBLIC),
        method("isLocalCameraLightingEnabled", "()Z", ACC_PUBLIC),
        method("setPerspectiveCorrectionEnable", "(Z)V", ACC_PUBLIC),
        method("isPerspectiveCorrectionEnabled", "()Z", ACC_PUBLIC),
    ],
};

pub(super) const TEXTURE_2D: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/Texture2D",
    super_name: "javax/microedition/m3g/Transformable",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[
        ("FILTER_BASE_LEVEL", 208),
        ("FILTER_LINEAR", 209),
        ("FILTER_NEAREST", 210),
        ("FUNC_ADD", 224),
        ("FUNC_BLEND", 225),
        ("FUNC_DECAL", 226),
        ("FUNC_MODULATE", 227),
        ("FUNC_REPLACE", 228),
        ("WRAP_CLAMP", 240),
        ("WRAP_REPEAT", 241),
    ],
    methods: &[
        method("<init>", "(Ljavax/microedition/m3g/Image2D;)V", ACC_PUBLIC),
        method(
            "setImage",
            "(Ljavax/microedition/m3g/Image2D;)V",
            ACC_PUBLIC,
        ),
        method("getImage", "()Ljavax/microedition/m3g/Image2D;", ACC_PUBLIC),
        method("setFiltering", "(II)V", ACC_PUBLIC),
        method("getLevelFilter", "()I", ACC_PUBLIC),
        method("getImageFilter", "()I", ACC_PUBLIC),
        method("setWrapping", "(II)V", ACC_PUBLIC),
        method("getWrappingS", "()I", ACC_PUBLIC),
        method("getWrappingT", "()I", ACC_PUBLIC),
        method("setBlending", "(I)V", ACC_PUBLIC),
        method("getBlending", "()I", ACC_PUBLIC),
        method("setBlendColor", "(I)V", ACC_PUBLIC),
        method("getBlendColor", "()I", ACC_PUBLIC),
    ],
};
