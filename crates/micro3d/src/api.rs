//! Canonical Rust-owned `MascotCapsule Micro3D Version 3` bootstrap inventory.

use classfile::{Attribute, ClassFile, Constant, Member};
use natives::{NativeRegistry, NativeSignature};

const ACC_PUBLIC: u16 = 0x0001;
const ACC_STATIC: u16 = 0x0008;
const ACC_FINAL: u16 = 0x0010;
const ACC_SUPER: u16 = 0x0020;
const ACC_NATIVE: u16 = 0x0100;

struct ClassSpec {
    name: &'static str,
    fields: &'static [FieldSpec],
    methods: &'static [MethodSpec],
}

struct FieldSpec {
    name: &'static str,
    value: Option<i32>,
}

struct MethodSpec {
    name: &'static str,
    descriptor: &'static str,
    flags: u16,
}

struct Pool {
    entries: Vec<Option<Constant>>,
}

impl Pool {
    fn new() -> Self {
        Self {
            entries: vec![None],
        }
    }

    fn push(&mut self, constant: Constant) -> u16 {
        let index =
            u16::try_from(self.entries.len()).expect("Micro3D bootstrap constant pool fits in u16");
        self.entries.push(Some(constant));
        index
    }

    fn utf8(&mut self, value: &str) -> u16 {
        self.push(Constant::Utf8(value.to_owned()))
    }

    fn class(&mut self, name: &str) -> u16 {
        let name_index = self.utf8(name);
        self.push(Constant::Class { name_index })
    }
}

/// Returns the documented public `MascotCapsule Micro3D Version 3` surface.
#[must_use]
pub fn bootstrap_classes() -> Vec<ClassFile> {
    CLASSES.iter().map(build_class).collect()
}

fn build_class(spec: &ClassSpec) -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class(spec.name);
    let super_class = pool.class("java/lang/Object");
    let constant_value_name = pool.utf8("ConstantValue");
    let exceptions_name = pool.utf8("Exceptions");

    let fields = spec
        .fields
        .iter()
        .map(|field| {
            let attributes = field.value.map_or_else(Vec::new, |value| {
                let value_index = pool.push(Constant::Integer(value));
                vec![Attribute::Raw {
                    name_index: constant_value_name,
                    name: "ConstantValue".to_owned(),
                    bytes: value_index.to_be_bytes().to_vec(),
                }]
            });
            Member {
                access_flags: if field.value.is_some() {
                    ACC_PUBLIC | ACC_STATIC | ACC_FINAL
                } else {
                    ACC_PUBLIC
                },
                name_index: pool.utf8(field.name),
                descriptor_index: pool.utf8("I"),
                attributes,
            }
        })
        .collect();

    let methods = spec
        .methods
        .iter()
        .map(|method| {
            let exception_names = declared_exceptions(spec.name, method.name, method.descriptor);
            let attributes = if exception_names.is_empty() {
                Vec::new()
            } else {
                let mut bytes = Vec::with_capacity(2 + exception_names.len() * 2);
                bytes.extend_from_slice(
                    &u16::try_from(exception_names.len()).unwrap().to_be_bytes(),
                );
                for exception in exception_names {
                    bytes.extend_from_slice(&pool.class(exception).to_be_bytes());
                }
                vec![Attribute::Raw {
                    name_index: exceptions_name,
                    name: "Exceptions".to_owned(),
                    bytes,
                }]
            };
            Member {
                access_flags: method.flags | ACC_NATIVE,
                name_index: pool.utf8(method.name),
                descriptor_index: pool.utf8(method.descriptor),
                attributes,
            }
        })
        .collect();

    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: ACC_PUBLIC | ACC_SUPER,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields,
        methods,
        attributes: Vec::new(),
    }
}

fn declared_exceptions(class: &str, name: &str, descriptor: &str) -> &'static [&'static str] {
    if name == "<init>" && descriptor.starts_with("(Ljava/lang/String;") {
        return &["java/io/IOException"];
    }
    if class != "com/mascotcapsule/micro3d/v3/Graphics3D" {
        return &[];
    }
    match name {
        "bind" => &[
            "java/lang/IllegalStateException",
            "java/lang/NullPointerException",
        ],
        "release" => &[
            "java/lang/IllegalArgumentException",
            "java/lang/NullPointerException",
        ],
        "renderFigure" | "drawFigure" | "flush" => &["java/lang/IllegalStateException"],
        _ => &[],
    }
}

/// Registers fail-closed native fallbacks for the whole public surface.
pub fn register_natives(registry: &mut NativeRegistry) -> Result<(), diagnostics::EmuError> {
    for class in CLASSES {
        for method in class.methods {
            let signature = NativeSignature::new(class.name, method.name, method.descriptor);
            let display = signature.display();
            registry.register(signature, move |_, _| {
                Err(diagnostics::EmuError::new(
                    diagnostics::Category::Micro3d,
                    "micro3d-bridge-unavailable",
                    format!("Micro3D VM bridge did not intercept {display}"),
                ))
            })?;
        }
    }
    Ok(())
}

#[must_use]
pub fn api_counts() -> (usize, usize, usize) {
    (
        CLASSES.len(),
        CLASSES.iter().map(|class| class.methods.len()).sum(),
        CLASSES.iter().map(|class| class.fields.len()).sum(),
    )
}

macro_rules! method {
    ($name:literal, $descriptor:literal) => {
        MethodSpec {
            name: $name,
            descriptor: $descriptor,
            flags: ACC_PUBLIC | ACC_FINAL,
        }
    };
    (static $name:literal, $descriptor:literal) => {
        MethodSpec {
            name: $name,
            descriptor: $descriptor,
            flags: ACC_PUBLIC | ACC_STATIC | ACC_FINAL,
        }
    };
    (ctor $descriptor:literal) => {
        MethodSpec {
            name: "<init>",
            descriptor: $descriptor,
            flags: ACC_PUBLIC,
        }
    };
}

macro_rules! int_field {
    ($name:literal) => {
        FieldSpec {
            name: $name,
            value: None,
        }
    };
    ($name:literal = $value:expr) => {
        FieldSpec {
            name: $name,
            value: Some($value),
        }
    };
}

const CLASSES: &[ClassSpec] = &[
    ClassSpec {
        name: "com/mascotcapsule/micro3d/v3/ActionTable",
        fields: &[],
        methods: &[
            method!(ctor "([B)V"),
            method!(ctor "(Ljava/lang/String;)V"),
            method!("dispose", "()V"),
            method!("getNumAction", "()I"),
            method!("getNumActions", "()I"),
            method!("getNumFrame", "(I)I"),
            method!("getNumFrames", "(I)I"),
        ],
    },
    ClassSpec {
        name: "com/mascotcapsule/micro3d/v3/AffineTrans",
        fields: &[
            int_field!("m00"),
            int_field!("m01"),
            int_field!("m02"),
            int_field!("m03"),
            int_field!("m10"),
            int_field!("m11"),
            int_field!("m12"),
            int_field!("m13"),
            int_field!("m20"),
            int_field!("m21"),
            int_field!("m22"),
            int_field!("m23"),
        ],
        methods: &[
            method!(ctor "()V"),
            method!(ctor "(IIIIIIIIIIII)V"),
            method!(ctor "(Lcom/mascotcapsule/micro3d/v3/AffineTrans;)V"),
            method!(ctor "([[I)V"),
            method!(ctor "([I)V"),
            method!(ctor "([II)V"),
            method!("setRotationX", "(I)V"),
            method!("setRotationY", "(I)V"),
            method!("setRotationZ", "(I)V"),
            method!("setIdentity", "()V"),
            method!("get", "([I)V"),
            method!("get", "([II)V"),
            method!("set", "([II)V"),
            method!("set", "(IIIIIIIIIIII)V"),
            method!("set", "(Lcom/mascotcapsule/micro3d/v3/AffineTrans;)V"),
            method!("set", "([[I)V"),
            method!("set", "([I)V"),
            method!(
                "transPoint",
                "(Lcom/mascotcapsule/micro3d/v3/Vector3D;)Lcom/mascotcapsule/micro3d/v3/Vector3D;"
            ),
            method!(
                "transform",
                "(Lcom/mascotcapsule/micro3d/v3/Vector3D;)Lcom/mascotcapsule/micro3d/v3/Vector3D;"
            ),
            method!("rotationX", "(I)V"),
            method!("rotationY", "(I)V"),
            method!("rotationZ", "(I)V"),
            method!("multiply", "(Lcom/mascotcapsule/micro3d/v3/AffineTrans;)V"),
            method!("mul", "(Lcom/mascotcapsule/micro3d/v3/AffineTrans;)V"),
            method!(
                "multiply",
                "(Lcom/mascotcapsule/micro3d/v3/AffineTrans;Lcom/mascotcapsule/micro3d/v3/AffineTrans;)V"
            ),
            method!(
                "mul",
                "(Lcom/mascotcapsule/micro3d/v3/AffineTrans;Lcom/mascotcapsule/micro3d/v3/AffineTrans;)V"
            ),
            method!("rotationV", "(Lcom/mascotcapsule/micro3d/v3/Vector3D;I)V"),
            method!("setRotation", "(Lcom/mascotcapsule/micro3d/v3/Vector3D;I)V"),
            method!(
                "setViewTrans",
                "(Lcom/mascotcapsule/micro3d/v3/Vector3D;Lcom/mascotcapsule/micro3d/v3/Vector3D;Lcom/mascotcapsule/micro3d/v3/Vector3D;)V"
            ),
            method!(
                "lookAt",
                "(Lcom/mascotcapsule/micro3d/v3/Vector3D;Lcom/mascotcapsule/micro3d/v3/Vector3D;Lcom/mascotcapsule/micro3d/v3/Vector3D;)V"
            ),
        ],
    },
    ClassSpec {
        name: "com/mascotcapsule/micro3d/v3/Effect3D",
        fields: &[
            int_field!("NORMAL_SHADING" = 0),
            int_field!("TOON_SHADING" = 1),
        ],
        methods: &[
            method!(ctor "()V"),
            method!(ctor "(Lcom/mascotcapsule/micro3d/v3/Light;IZLcom/mascotcapsule/micro3d/v3/Texture;)V"),
            method!("getLight", "()Lcom/mascotcapsule/micro3d/v3/Light;"),
            method!("setLight", "(Lcom/mascotcapsule/micro3d/v3/Light;)V"),
            method!("getShading", "()I"),
            method!("getShadingType", "()I"),
            method!("setShading", "(I)V"),
            method!("setShadingType", "(I)V"),
            method!("getThreshold", "()I"),
            method!("getToonThreshold", "()I"),
            method!("getThresholdHigh", "()I"),
            method!("getToonHigh", "()I"),
            method!("getThresholdLow", "()I"),
            method!("getToonLow", "()I"),
            method!("setThreshold", "(III)V"),
            method!("setToonParams", "(III)V"),
            method!("isSemiTransparentEnabled", "()Z"),
            method!("isTransparency", "()Z"),
            method!("setSemiTransparentEnabled", "(Z)V"),
            method!("setTransparency", "(Z)V"),
            method!("getSphereMap", "()Lcom/mascotcapsule/micro3d/v3/Texture;"),
            method!(
                "getSphereTexture",
                "()Lcom/mascotcapsule/micro3d/v3/Texture;"
            ),
            method!("setSphereMap", "(Lcom/mascotcapsule/micro3d/v3/Texture;)V"),
            method!(
                "setSphereTexture",
                "(Lcom/mascotcapsule/micro3d/v3/Texture;)V"
            ),
        ],
    },
    ClassSpec {
        name: "com/mascotcapsule/micro3d/v3/Figure",
        fields: &[],
        methods: &[
            method!(ctor "([B)V"),
            method!(ctor "(Ljava/lang/String;)V"),
            method!("dispose", "()V"),
            method!(
                "setPosture",
                "(Lcom/mascotcapsule/micro3d/v3/ActionTable;II)V"
            ),
            method!("getTexture", "()Lcom/mascotcapsule/micro3d/v3/Texture;"),
            method!("setTexture", "(Lcom/mascotcapsule/micro3d/v3/Texture;)V"),
            method!("setTexture", "([Lcom/mascotcapsule/micro3d/v3/Texture;)V"),
            method!("getNumTextures", "()I"),
            method!("selectTexture", "(I)V"),
            method!("getNumPattern", "()I"),
            method!("setPattern", "(I)V"),
        ],
    },
    ClassSpec {
        name: "com/mascotcapsule/micro3d/v3/FigureLayout",
        fields: &[],
        methods: &[
            method!(ctor "()V"),
            method!(ctor "(Lcom/mascotcapsule/micro3d/v3/AffineTrans;IIII)V"),
            method!(
                "getAffineTrans",
                "()Lcom/mascotcapsule/micro3d/v3/AffineTrans;"
            ),
            method!(
                "setAffineTrans",
                "(Lcom/mascotcapsule/micro3d/v3/AffineTrans;)V"
            ),
            method!(
                "setAffineTransArray",
                "([Lcom/mascotcapsule/micro3d/v3/AffineTrans;)V"
            ),
            method!(
                "setAffineTrans",
                "([Lcom/mascotcapsule/micro3d/v3/AffineTrans;)V"
            ),
            method!("selectAffineTrans", "(I)V"),
            method!("getScaleX", "()I"),
            method!("getScaleY", "()I"),
            method!("setScale", "(II)V"),
            method!("getParallelWidth", "()I"),
            method!("getParallelHeight", "()I"),
            method!("setParallelSize", "(II)V"),
            method!("getCenterX", "()I"),
            method!("getCenterY", "()I"),
            method!("setCenter", "(II)V"),
            method!("setPerspective", "(III)V"),
            method!("setPerspective", "(IIII)V"),
        ],
    },
    ClassSpec {
        name: "com/mascotcapsule/micro3d/v3/Graphics3D",
        fields: &[
            int_field!("COMMAND_LIST_VERSION_1_0" = -33_554_431),
            int_field!("COMMAND_END" = i32::MIN),
            int_field!("COMMAND_NOP" = -2_130_706_432),
            int_field!("COMMAND_FLUSH" = -2_113_929_216),
            int_field!("COMMAND_ATTRIBUTE" = -2_097_152_000),
            int_field!("COMMAND_CLIP" = -2_080_374_784),
            int_field!("COMMAND_CENTER" = -2_063_597_568),
            int_field!("COMMAND_TEXTURE_INDEX" = -2_046_820_352),
            int_field!("COMMAND_AFFINE_INDEX" = -2_030_043_136),
            int_field!("COMMAND_PARALLEL_SCALE" = -1_879_048_192),
            int_field!("COMMAND_PARALLEL_SIZE" = -1_862_270_976),
            int_field!("COMMAND_PERSPECTIVE_FOV" = -1_845_493_760),
            int_field!("COMMAND_PERSPECTIVE_WH" = -1_828_716_544),
            int_field!("COMMAND_AMBIENT_LIGHT" = -1_610_612_736),
            int_field!("COMMAND_DIRECTION_LIGHT" = -1_593_835_520),
            int_field!("COMMAND_THRESHOLD" = -1_358_954_496),
            int_field!("PRIMITVE_POINTS" = 16_777_216),
            int_field!("PRIMITVE_LINES" = 33_554_432),
            int_field!("PRIMITVE_TRIANGLES" = 50_331_648),
            int_field!("PRIMITVE_QUADS" = 67_108_864),
            int_field!("PRIMITVE_POINT_SPRITES" = 83_886_080),
            int_field!("POINT_SPRITE_LOCAL_SIZE" = 0),
            int_field!("POINT_SPRITE_PIXEL_SIZE" = 1),
            int_field!("POINT_SPRITE_PERSPECTIVE" = 0),
            int_field!("POINT_SPRITE_NO_PERS" = 2),
            int_field!("ENV_ATTR_LIGHTING" = 1),
            int_field!("ENV_ATTR_SPHERE_MAP" = 2),
            int_field!("ENV_ATTR_TOON_SHADING" = 4),
            int_field!("ENV_ATTR_SEMI_TRANSPARENT" = 8),
            int_field!("PATTR_LIGHTING" = 1),
            int_field!("PATTR_SPHERE_MAP" = 2),
            int_field!("PATTR_COLORKEY" = 16),
            int_field!("PATTR_BLEND_NORMAL" = 0),
            int_field!("PATTR_BLEND_HALF" = 32),
            int_field!("PATTR_BLEND_ADD" = 64),
            int_field!("PATTR_BLEND_SUB" = 96),
            int_field!("PDATA_NORMAL_NONE" = 0),
            int_field!("PDATA_NORMAL_PER_FACE" = 512),
            int_field!("PDATA_NORMAL_PER_VERTEX" = 768),
            int_field!("PDATA_COLOR_NONE" = 0),
            int_field!("PDATA_COLOR_PER_COMMAND" = 1024),
            int_field!("PDATA_COLOR_PER_FACE" = 2048),
            int_field!("PDATA_TEXURE_COORD_NONE" = 0),
            int_field!("PDATA_POINT_SPRITE_PARAMS_PER_CMD" = 4096),
            int_field!("PDATA_POINT_SPRITE_PARAMS_PER_FACE" = 8192),
            int_field!("PDATA_POINT_SPRITE_PARAMS_PER_VERTEX" = 12288),
            int_field!("PDATA_TEXURE_COORD" = 12288),
        ],
        methods: &[
            method!(ctor "()V"),
            method!("bind", "(Ljavax/microedition/lcdui/Graphics;)V"),
            method!("release", "(Ljavax/microedition/lcdui/Graphics;)V"),
            method!(
                "renderPrimitives",
                "(Lcom/mascotcapsule/micro3d/v3/Texture;IILcom/mascotcapsule/micro3d/v3/FigureLayout;Lcom/mascotcapsule/micro3d/v3/Effect3D;II[I[I[I[I)V"
            ),
            method!(
                "drawCommandList",
                "([Lcom/mascotcapsule/micro3d/v3/Texture;IILcom/mascotcapsule/micro3d/v3/FigureLayout;Lcom/mascotcapsule/micro3d/v3/Effect3D;[I)V"
            ),
            method!(
                "drawCommandList",
                "(Lcom/mascotcapsule/micro3d/v3/Texture;IILcom/mascotcapsule/micro3d/v3/FigureLayout;Lcom/mascotcapsule/micro3d/v3/Effect3D;[I)V"
            ),
            method!(
                "renderFigure",
                "(Lcom/mascotcapsule/micro3d/v3/Figure;IILcom/mascotcapsule/micro3d/v3/FigureLayout;Lcom/mascotcapsule/micro3d/v3/Effect3D;)V"
            ),
            method!(
                "drawFigure",
                "(Lcom/mascotcapsule/micro3d/v3/Figure;IILcom/mascotcapsule/micro3d/v3/FigureLayout;Lcom/mascotcapsule/micro3d/v3/Effect3D;)V"
            ),
            method!("flush", "()V"),
            method!("dispose", "()V"),
        ],
    },
    ClassSpec {
        name: "com/mascotcapsule/micro3d/v3/Light",
        fields: &[],
        methods: &[
            method!(ctor "()V"),
            method!(ctor "(Lcom/mascotcapsule/micro3d/v3/Vector3D;II)V"),
            method!("getDirIntensity", "()I"),
            method!("getParallelLightIntensity", "()I"),
            method!("setDirIntensity", "(I)V"),
            method!("setParallelLightIntensity", "(I)V"),
            method!("getAmbIntensity", "()I"),
            method!("getAmbientIntensity", "()I"),
            method!("setAmbIntensity", "(I)V"),
            method!("setAmbientIntensity", "(I)V"),
            method!("getDirection", "()Lcom/mascotcapsule/micro3d/v3/Vector3D;"),
            method!(
                "getParallelLightDirection",
                "()Lcom/mascotcapsule/micro3d/v3/Vector3D;"
            ),
            method!("setDirection", "(Lcom/mascotcapsule/micro3d/v3/Vector3D;)V"),
            method!(
                "setParallelLightDirection",
                "(Lcom/mascotcapsule/micro3d/v3/Vector3D;)V"
            ),
        ],
    },
    ClassSpec {
        name: "com/mascotcapsule/micro3d/v3/Texture",
        fields: &[],
        methods: &[
            method!(ctor "([BZ)V"),
            method!(ctor "(Ljava/lang/String;Z)V"),
            method!("dispose", "()V"),
        ],
    },
    ClassSpec {
        name: "com/mascotcapsule/micro3d/v3/Util3D",
        fields: &[],
        methods: &[
            method!(static "sqrt", "(I)I"),
            method!(static "sin", "(I)I"),
            method!(static "cos", "(I)I"),
        ],
    },
    ClassSpec {
        name: "com/mascotcapsule/micro3d/v3/Vector3D",
        fields: &[int_field!("x"), int_field!("y"), int_field!("z")],
        methods: &[
            method!(ctor "()V"),
            method!(ctor "(Lcom/mascotcapsule/micro3d/v3/Vector3D;)V"),
            method!(ctor "(III)V"),
            method!("unit", "()V"),
            method!("getX", "()I"),
            method!("getY", "()I"),
            method!("getZ", "()I"),
            method!("setX", "(I)V"),
            method!("setY", "(I)V"),
            method!("setZ", "(I)V"),
            method!("set", "(Lcom/mascotcapsule/micro3d/v3/Vector3D;)V"),
            method!("set", "(III)V"),
            method!("innerProduct", "(Lcom/mascotcapsule/micro3d/v3/Vector3D;)I"),
            method!("outerProduct", "(Lcom/mascotcapsule/micro3d/v3/Vector3D;)V"),
            method!(static "innerProduct", "(Lcom/mascotcapsule/micro3d/v3/Vector3D;Lcom/mascotcapsule/micro3d/v3/Vector3D;)I"),
            method!(static "outerProduct", "(Lcom/mascotcapsule/micro3d/v3/Vector3D;Lcom/mascotcapsule/micro3d/v3/Vector3D;)Lcom/mascotcapsule/micro3d/v3/Vector3D;"),
        ],
    },
];

#[cfg(test)]
#[path = "../../../tests/unit/micro3d/api/mod.rs"]
mod tests;
