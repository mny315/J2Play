//! Animation controllers, tracks and keyframe sequences.

use super::{ACC_PUBLIC, ACC_SUPER, M3gClassSpec, method};

pub(super) const ANIMATION_CONTROLLER: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/AnimationController",
    super_name: "javax/microedition/m3g/Object3D",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[],
    methods: &[
        method("<init>", "()V", ACC_PUBLIC),
        method("setActiveInterval", "(II)V", ACC_PUBLIC),
        method("getActiveIntervalStart", "()I", ACC_PUBLIC),
        method("getActiveIntervalEnd", "()I", ACC_PUBLIC),
        method("setSpeed", "(FI)V", ACC_PUBLIC),
        method("getSpeed", "()F", ACC_PUBLIC),
        method("setPosition", "(FI)V", ACC_PUBLIC),
        method("getPosition", "(I)F", ACC_PUBLIC),
        method("getRefWorldTime", "()I", ACC_PUBLIC),
        method("setWeight", "(F)V", ACC_PUBLIC),
        method("getWeight", "()F", ACC_PUBLIC),
    ],
};

pub(super) const ANIMATION_TRACK: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/AnimationTrack",
    super_name: "javax/microedition/m3g/Object3D",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[
        ("ALPHA", 256),
        ("AMBIENT_COLOR", 257),
        ("COLOR", 258),
        ("CROP", 259),
        ("DENSITY", 260),
        ("DIFFUSE_COLOR", 261),
        ("EMISSIVE_COLOR", 262),
        ("FAR_DISTANCE", 263),
        ("FIELD_OF_VIEW", 264),
        ("INTENSITY", 265),
        ("MORPH_WEIGHTS", 266),
        ("NEAR_DISTANCE", 267),
        ("ORIENTATION", 268),
        ("PICKABILITY", 269),
        ("SCALE", 270),
        ("SHININESS", 271),
        ("SPECULAR_COLOR", 272),
        ("SPOT_ANGLE", 273),
        ("SPOT_EXPONENT", 274),
        ("TRANSLATION", 275),
        ("VISIBILITY", 276),
    ],
    methods: &[
        method(
            "<init>",
            "(Ljavax/microedition/m3g/KeyframeSequence;I)V",
            ACC_PUBLIC,
        ),
        method(
            "setController",
            "(Ljavax/microedition/m3g/AnimationController;)V",
            ACC_PUBLIC,
        ),
        method(
            "getController",
            "()Ljavax/microedition/m3g/AnimationController;",
            ACC_PUBLIC,
        ),
        method(
            "getKeyframeSequence",
            "()Ljavax/microedition/m3g/KeyframeSequence;",
            ACC_PUBLIC,
        ),
        method("getTargetProperty", "()I", ACC_PUBLIC),
    ],
};

pub(super) const KEYFRAME_SEQUENCE: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/KeyframeSequence",
    super_name: "javax/microedition/m3g/Object3D",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[
        ("LINEAR", 176),
        ("SLERP", 177),
        ("SPLINE", 178),
        ("SQUAD", 179),
        ("STEP", 180),
        ("CONSTANT", 192),
        ("LOOP", 193),
    ],
    methods: &[
        method("<init>", "(III)V", ACC_PUBLIC),
        method("getComponentCount", "()I", ACC_PUBLIC),
        method("getKeyframeCount", "()I", ACC_PUBLIC),
        method("getInterpolationType", "()I", ACC_PUBLIC),
        method("setKeyframe", "(II[F)V", ACC_PUBLIC),
        method("getKeyframe", "(I[F)I", ACC_PUBLIC),
        method("setValidRange", "(II)V", ACC_PUBLIC),
        method("getValidRangeFirst", "()I", ACC_PUBLIC),
        method("getValidRangeLast", "()I", ACC_PUBLIC),
        method("setDuration", "(I)V", ACC_PUBLIC),
        method("getDuration", "()I", ACC_PUBLIC),
        method("setRepeatMode", "(I)V", ACC_PUBLIC),
        method("getRepeatMode", "()I", ACC_PUBLIC),
    ],
};
