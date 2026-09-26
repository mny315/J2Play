//! Scene objects, transforms, hierarchy, cameras and lights.

use super::{ACC_ABSTRACT, ACC_FINAL, ACC_PUBLIC, ACC_SUPER, M3gClassSpec, method};

pub(super) const CAMERA: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/Camera",
    super_name: "javax/microedition/m3g/Node",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[("GENERIC", 48), ("PARALLEL", 49), ("PERSPECTIVE", 50)],
    methods: &[
        method("<init>", "()V", ACC_PUBLIC),
        method("setParallel", "(FFFF)V", ACC_PUBLIC),
        method("setPerspective", "(FFFF)V", ACC_PUBLIC),
        method(
            "setGeneric",
            "(Ljavax/microedition/m3g/Transform;)V",
            ACC_PUBLIC,
        ),
        method(
            "getProjection",
            "(Ljavax/microedition/m3g/Transform;)I",
            ACC_PUBLIC,
        ),
        method("getProjection", "([F)I", ACC_PUBLIC),
    ],
};

pub(super) const GROUP: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/Group",
    super_name: "javax/microedition/m3g/Node",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[],
    methods: &[
        method("<init>", "()V", ACC_PUBLIC),
        method("addChild", "(Ljavax/microedition/m3g/Node;)V", ACC_PUBLIC),
        method(
            "removeChild",
            "(Ljavax/microedition/m3g/Node;)V",
            ACC_PUBLIC,
        ),
        method("getChildCount", "()I", ACC_PUBLIC),
        method("getChild", "(I)Ljavax/microedition/m3g/Node;", ACC_PUBLIC),
        method(
            "pick",
            "(IFFFFFFLjavax/microedition/m3g/RayIntersection;)Z",
            ACC_PUBLIC,
        ),
        method(
            "pick",
            "(IFFLjavax/microedition/m3g/Camera;Ljavax/microedition/m3g/RayIntersection;)Z",
            ACC_PUBLIC,
        ),
    ],
};

pub(super) const LIGHT: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/Light",
    super_name: "javax/microedition/m3g/Node",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[
        ("AMBIENT", 128),
        ("DIRECTIONAL", 129),
        ("OMNI", 130),
        ("SPOT", 131),
    ],
    methods: &[
        method("<init>", "()V", ACC_PUBLIC),
        method("setMode", "(I)V", ACC_PUBLIC),
        method("getMode", "()I", ACC_PUBLIC),
        method("setIntensity", "(F)V", ACC_PUBLIC),
        method("getIntensity", "()F", ACC_PUBLIC),
        method("setColor", "(I)V", ACC_PUBLIC),
        method("getColor", "()I", ACC_PUBLIC),
        method("setSpotAngle", "(F)V", ACC_PUBLIC),
        method("getSpotAngle", "()F", ACC_PUBLIC),
        method("setSpotExponent", "(F)V", ACC_PUBLIC),
        method("getSpotExponent", "()F", ACC_PUBLIC),
        method("setAttenuation", "(FFF)V", ACC_PUBLIC),
        method("getConstantAttenuation", "()F", ACC_PUBLIC),
        method("getLinearAttenuation", "()F", ACC_PUBLIC),
        method("getQuadraticAttenuation", "()F", ACC_PUBLIC),
    ],
};

pub(super) const NODE: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/Node",
    super_name: "javax/microedition/m3g/Transformable",
    flags: ACC_PUBLIC | ACC_SUPER | ACC_ABSTRACT,
    constants: &[
        ("NONE", 144),
        ("ORIGIN", 145),
        ("X_AXIS", 146),
        ("Y_AXIS", 147),
        ("Z_AXIS", 148),
    ],
    methods: &[
        method("setRenderingEnable", "(Z)V", ACC_PUBLIC),
        method("setPickingEnable", "(Z)V", ACC_PUBLIC),
        method("setScope", "(I)V", ACC_PUBLIC),
        method("setAlphaFactor", "(F)V", ACC_PUBLIC),
        method("isRenderingEnabled", "()Z", ACC_PUBLIC),
        method("isPickingEnabled", "()Z", ACC_PUBLIC),
        method("getScope", "()I", ACC_PUBLIC),
        method("getAlphaFactor", "()F", ACC_PUBLIC),
        method("getParent", "()Ljavax/microedition/m3g/Node;", ACC_PUBLIC),
        method(
            "getTransformTo",
            "(Ljavax/microedition/m3g/Node;Ljavax/microedition/m3g/Transform;)Z",
            ACC_PUBLIC,
        ),
        method(
            "align",
            "(Ljavax/microedition/m3g/Node;)V",
            ACC_PUBLIC | ACC_FINAL,
        ),
        method(
            "setAlignment",
            "(Ljavax/microedition/m3g/Node;ILjavax/microedition/m3g/Node;I)V",
            ACC_PUBLIC,
        ),
        method("getAlignmentTarget", "(I)I", ACC_PUBLIC),
        method(
            "getAlignmentReference",
            "(I)Ljavax/microedition/m3g/Node;",
            ACC_PUBLIC,
        ),
    ],
};

pub(super) const OBJECT_3D: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/Object3D",
    super_name: "java/lang/Object",
    flags: ACC_PUBLIC | ACC_SUPER | ACC_ABSTRACT,
    constants: &[],
    methods: &[
        method("animate", "(I)I", ACC_PUBLIC | ACC_FINAL),
        method(
            "duplicate",
            "()Ljavax/microedition/m3g/Object3D;",
            ACC_PUBLIC | ACC_FINAL,
        ),
        method("find", "(I)Ljavax/microedition/m3g/Object3D;", ACC_PUBLIC),
        method(
            "getReferences",
            "([Ljavax/microedition/m3g/Object3D;)I",
            ACC_PUBLIC,
        ),
        method("setUserID", "(I)V", ACC_PUBLIC),
        method("getUserID", "()I", ACC_PUBLIC),
        method("setUserObject", "(Ljava/lang/Object;)V", ACC_PUBLIC),
        method("getUserObject", "()Ljava/lang/Object;", ACC_PUBLIC),
        method(
            "addAnimationTrack",
            "(Ljavax/microedition/m3g/AnimationTrack;)V",
            ACC_PUBLIC,
        ),
        method(
            "getAnimationTrack",
            "(I)Ljavax/microedition/m3g/AnimationTrack;",
            ACC_PUBLIC,
        ),
        method(
            "removeAnimationTrack",
            "(Ljavax/microedition/m3g/AnimationTrack;)V",
            ACC_PUBLIC,
        ),
        method("getAnimationTrackCount", "()I", ACC_PUBLIC),
    ],
};

pub(super) const SPRITE_3D: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/Sprite3D",
    super_name: "javax/microedition/m3g/Node",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[],
    methods: &[
        method(
            "<init>",
            "(ZLjavax/microedition/m3g/Image2D;Ljavax/microedition/m3g/Appearance;)V",
            ACC_PUBLIC,
        ),
        method("isScaled", "()Z", ACC_PUBLIC),
        method(
            "setAppearance",
            "(Ljavax/microedition/m3g/Appearance;)V",
            ACC_PUBLIC,
        ),
        method(
            "getAppearance",
            "()Ljavax/microedition/m3g/Appearance;",
            ACC_PUBLIC,
        ),
        method(
            "setImage",
            "(Ljavax/microedition/m3g/Image2D;)V",
            ACC_PUBLIC,
        ),
        method("getImage", "()Ljavax/microedition/m3g/Image2D;", ACC_PUBLIC),
        method("setCrop", "(IIII)V", ACC_PUBLIC),
        method("getCropX", "()I", ACC_PUBLIC),
        method("getCropY", "()I", ACC_PUBLIC),
        method("getCropWidth", "()I", ACC_PUBLIC),
        method("getCropHeight", "()I", ACC_PUBLIC),
    ],
};

pub(super) const TRANSFORM: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/Transform",
    super_name: "java/lang/Object",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[],
    methods: &[
        method("<init>", "()V", ACC_PUBLIC),
        method(
            "<init>",
            "(Ljavax/microedition/m3g/Transform;)V",
            ACC_PUBLIC,
        ),
        method("setIdentity", "()V", ACC_PUBLIC),
        method("set", "(Ljavax/microedition/m3g/Transform;)V", ACC_PUBLIC),
        method("set", "([F)V", ACC_PUBLIC),
        method("get", "([F)V", ACC_PUBLIC),
        method("invert", "()V", ACC_PUBLIC),
        method("transpose", "()V", ACC_PUBLIC),
        method(
            "postMultiply",
            "(Ljavax/microedition/m3g/Transform;)V",
            ACC_PUBLIC,
        ),
        method("postScale", "(FFF)V", ACC_PUBLIC),
        method("postRotate", "(FFFF)V", ACC_PUBLIC),
        method("postRotateQuat", "(FFFF)V", ACC_PUBLIC),
        method("postTranslate", "(FFF)V", ACC_PUBLIC),
        method(
            "transform",
            "(Ljavax/microedition/m3g/VertexArray;[FZ)V",
            ACC_PUBLIC,
        ),
        method("transform", "([F)V", ACC_PUBLIC),
    ],
};

pub(super) const TRANSFORMABLE: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/Transformable",
    super_name: "javax/microedition/m3g/Object3D",
    flags: ACC_PUBLIC | ACC_SUPER | ACC_ABSTRACT,
    constants: &[],
    methods: &[
        method("setOrientation", "(FFFF)V", ACC_PUBLIC),
        method("preRotate", "(FFFF)V", ACC_PUBLIC),
        method("postRotate", "(FFFF)V", ACC_PUBLIC),
        method("getOrientation", "([F)V", ACC_PUBLIC),
        method("setScale", "(FFF)V", ACC_PUBLIC),
        method("scale", "(FFF)V", ACC_PUBLIC),
        method("getScale", "([F)V", ACC_PUBLIC),
        method("setTranslation", "(FFF)V", ACC_PUBLIC),
        method("translate", "(FFF)V", ACC_PUBLIC),
        method("getTranslation", "([F)V", ACC_PUBLIC),
        method(
            "setTransform",
            "(Ljavax/microedition/m3g/Transform;)V",
            ACC_PUBLIC,
        ),
        method(
            "getTransform",
            "(Ljavax/microedition/m3g/Transform;)V",
            ACC_PUBLIC,
        ),
        method(
            "getCompositeTransform",
            "(Ljavax/microedition/m3g/Transform;)V",
            ACC_PUBLIC,
        ),
    ],
};

pub(super) const WORLD: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/World",
    super_name: "javax/microedition/m3g/Group",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[],
    methods: &[
        method("<init>", "()V", ACC_PUBLIC),
        method(
            "setBackground",
            "(Ljavax/microedition/m3g/Background;)V",
            ACC_PUBLIC,
        ),
        method(
            "getBackground",
            "()Ljavax/microedition/m3g/Background;",
            ACC_PUBLIC,
        ),
        method(
            "setActiveCamera",
            "(Ljavax/microedition/m3g/Camera;)V",
            ACC_PUBLIC,
        ),
        method(
            "getActiveCamera",
            "()Ljavax/microedition/m3g/Camera;",
            ACC_PUBLIC,
        ),
    ],
};
