//! Canonical Rust-owned JSR-184 1.1 bootstrap inventory.

use classfile::{Attribute, ClassFile, Constant, Member};
use natives::{NativeRegistry, NativeSignature};

mod animation;
mod appearance;
mod entry_points;
mod geometry;
mod scene;

const ACC_PUBLIC: u16 = 0x0001;
const ACC_STATIC: u16 = 0x0008;
const ACC_FINAL: u16 = 0x0010;
const ACC_SUPER: u16 = 0x0020;
const ACC_NATIVE: u16 = 0x0100;
const ACC_ABSTRACT: u16 = 0x0400;

struct M3gClassSpec {
    name: &'static str,
    super_name: &'static str,
    flags: u16,
    constants: &'static [(&'static str, i32)],
    methods: &'static [M3gMethodSpec],
}

struct M3gMethodSpec {
    name: &'static str,
    descriptor: &'static str,
    flags: u16,
}

const fn method(name: &'static str, descriptor: &'static str, flags: u16) -> M3gMethodSpec {
    M3gMethodSpec {
        name,
        descriptor,
        flags,
    }
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
            u16::try_from(self.entries.len()).expect("JSR-184 bootstrap constant pool fits in u16");
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

/// Returns the exact public JSR-184 1.1 class-file surface.
#[must_use]
pub fn bootstrap_classes() -> Vec<ClassFile> {
    M3G_CLASSES.iter().map(build_class).collect()
}

fn build_class(spec: &M3gClassSpec) -> ClassFile {
    let mut pool = Pool::new();
    let this_class = pool.class(spec.name);
    let super_class = pool.class(spec.super_name);
    let constant_value_name = pool.utf8("ConstantValue");
    let exceptions_name = pool.utf8("Exceptions");
    let io_exception = pool.class("java/io/IOException");
    let mut fields = Vec::with_capacity(spec.constants.len());
    for &(name, value) in spec.constants {
        let name_index = pool.utf8(name);
        let descriptor_index = pool.utf8("I");
        let index = pool.push(Constant::Integer(value));
        let attributes = vec![Attribute::Raw {
            name_index: constant_value_name,
            name: "ConstantValue".to_owned(),
            bytes: index.to_be_bytes().to_vec(),
        }];
        fields.push(Member {
            access_flags: ACC_PUBLIC | ACC_STATIC | ACC_FINAL,
            name_index,
            descriptor_index,
            attributes,
        });
    }
    let mut methods = Vec::with_capacity(spec.methods.len());
    for method in spec.methods {
        let attributes = if spec.name == "javax/microedition/m3g/Loader" && method.name == "load" {
            let mut bytes = Vec::with_capacity(4);
            bytes.extend_from_slice(&1_u16.to_be_bytes());
            bytes.extend_from_slice(&io_exception.to_be_bytes());
            vec![Attribute::Raw {
                name_index: exceptions_name,
                name: "Exceptions".to_owned(),
                bytes,
            }]
        } else {
            Vec::new()
        };
        methods.push(Member {
            access_flags: method.flags | ACC_NATIVE,
            name_index: pool.utf8(method.name),
            descriptor_index: pool.utf8(method.descriptor),
            attributes,
        });
    }
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: spec.flags,
        this_class,
        super_class,
        interfaces: Vec::new(),
        fields,
        methods,
        attributes: Vec::new(),
    }
}

/// Registers every public native signature from the canonical inventory.
/// The generic dispatch remains fail-closed until the VM-owned bridge handles it.
pub fn register_natives(registry: &mut NativeRegistry) -> Result<(), diagnostics::EmuError> {
    for class in M3G_CLASSES {
        for method in class.methods {
            let signature = NativeSignature::new(class.name, method.name, method.descriptor);
            let display = signature.display();
            registry.register(signature, move |_, _| {
                Err(diagnostics::EmuError::new(
                    diagnostics::Category::M3g,
                    "m3g-bridge-unavailable",
                    format!("M3G VM bridge did not intercept {display}"),
                ))
            })?;
        }
    }
    Ok(())
}

#[must_use]
pub fn api_counts() -> (usize, usize, usize) {
    (
        M3G_CLASSES.len(),
        M3G_CLASSES.iter().map(|class| class.methods.len()).sum(),
        M3G_CLASSES.iter().map(|class| class.constants.len()).sum(),
    )
}

// JSR-184 1.1 maintenance Javadoc inventory, in canonical class order.
const M3G_CLASSES: &[M3gClassSpec] = &[
    animation::ANIMATION_CONTROLLER,
    animation::ANIMATION_TRACK,
    appearance::APPEARANCE,
    appearance::BACKGROUND,
    scene::CAMERA,
    appearance::COMPOSITING_MODE,
    appearance::FOG,
    entry_points::GRAPHICS_3D,
    scene::GROUP,
    appearance::IMAGE_2D,
    geometry::INDEX_BUFFER,
    animation::KEYFRAME_SEQUENCE,
    scene::LIGHT,
    entry_points::LOADER,
    appearance::MATERIAL,
    geometry::MESH,
    geometry::MORPHING_MESH,
    scene::NODE,
    scene::OBJECT_3D,
    appearance::POLYGON_MODE,
    geometry::RAY_INTERSECTION,
    geometry::SKINNED_MESH,
    scene::SPRITE_3D,
    appearance::TEXTURE_2D,
    scene::TRANSFORM,
    scene::TRANSFORMABLE,
    geometry::TRIANGLE_STRIP_ARRAY,
    geometry::VERTEX_ARRAY,
    geometry::VERTEX_BUFFER,
    scene::WORLD,
];

#[cfg(test)]
#[path = "../../../tests/unit/m3g/api/mod.rs"]
mod tests;
