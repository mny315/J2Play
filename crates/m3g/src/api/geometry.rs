//! Vertex data, mesh variants and picking results.

use super::{ACC_ABSTRACT, ACC_PUBLIC, ACC_SUPER, M3gClassSpec, method};

pub(super) const INDEX_BUFFER: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/IndexBuffer",
    super_name: "javax/microedition/m3g/Object3D",
    flags: ACC_PUBLIC | ACC_SUPER | ACC_ABSTRACT,
    constants: &[],
    methods: &[
        method("getIndexCount", "()I", ACC_PUBLIC),
        method("getIndices", "([I)V", ACC_PUBLIC),
    ],
};

pub(super) const MESH: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/Mesh",
    super_name: "javax/microedition/m3g/Node",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[],
    methods: &[
        method(
            "<init>",
            "(Ljavax/microedition/m3g/VertexBuffer;Ljavax/microedition/m3g/IndexBuffer;Ljavax/microedition/m3g/Appearance;)V",
            ACC_PUBLIC,
        ),
        method(
            "<init>",
            "(Ljavax/microedition/m3g/VertexBuffer;[Ljavax/microedition/m3g/IndexBuffer;[Ljavax/microedition/m3g/Appearance;)V",
            ACC_PUBLIC,
        ),
        method(
            "setAppearance",
            "(ILjavax/microedition/m3g/Appearance;)V",
            ACC_PUBLIC,
        ),
        method(
            "getAppearance",
            "(I)Ljavax/microedition/m3g/Appearance;",
            ACC_PUBLIC,
        ),
        method(
            "getIndexBuffer",
            "(I)Ljavax/microedition/m3g/IndexBuffer;",
            ACC_PUBLIC,
        ),
        method(
            "getVertexBuffer",
            "()Ljavax/microedition/m3g/VertexBuffer;",
            ACC_PUBLIC,
        ),
        method("getSubmeshCount", "()I", ACC_PUBLIC),
    ],
};

pub(super) const MORPHING_MESH: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/MorphingMesh",
    super_name: "javax/microedition/m3g/Mesh",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[],
    methods: &[
        method(
            "<init>",
            "(Ljavax/microedition/m3g/VertexBuffer;[Ljavax/microedition/m3g/VertexBuffer;Ljavax/microedition/m3g/IndexBuffer;Ljavax/microedition/m3g/Appearance;)V",
            ACC_PUBLIC,
        ),
        method(
            "<init>",
            "(Ljavax/microedition/m3g/VertexBuffer;[Ljavax/microedition/m3g/VertexBuffer;[Ljavax/microedition/m3g/IndexBuffer;[Ljavax/microedition/m3g/Appearance;)V",
            ACC_PUBLIC,
        ),
        method(
            "getMorphTarget",
            "(I)Ljavax/microedition/m3g/VertexBuffer;",
            ACC_PUBLIC,
        ),
        method("getMorphTargetCount", "()I", ACC_PUBLIC),
        method("setWeights", "([F)V", ACC_PUBLIC),
        method("getWeights", "([F)V", ACC_PUBLIC),
    ],
};

pub(super) const RAY_INTERSECTION: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/RayIntersection",
    super_name: "java/lang/Object",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[],
    methods: &[
        method("<init>", "()V", ACC_PUBLIC),
        method(
            "getIntersected",
            "()Ljavax/microedition/m3g/Node;",
            ACC_PUBLIC,
        ),
        method("getRay", "([F)V", ACC_PUBLIC),
        method("getDistance", "()F", ACC_PUBLIC),
        method("getSubmeshIndex", "()I", ACC_PUBLIC),
        method("getTextureS", "(I)F", ACC_PUBLIC),
        method("getTextureT", "(I)F", ACC_PUBLIC),
        method("getNormalX", "()F", ACC_PUBLIC),
        method("getNormalY", "()F", ACC_PUBLIC),
        method("getNormalZ", "()F", ACC_PUBLIC),
    ],
};

pub(super) const SKINNED_MESH: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/SkinnedMesh",
    super_name: "javax/microedition/m3g/Mesh",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[],
    methods: &[
        method(
            "<init>",
            "(Ljavax/microedition/m3g/VertexBuffer;Ljavax/microedition/m3g/IndexBuffer;Ljavax/microedition/m3g/Appearance;Ljavax/microedition/m3g/Group;)V",
            ACC_PUBLIC,
        ),
        method(
            "<init>",
            "(Ljavax/microedition/m3g/VertexBuffer;[Ljavax/microedition/m3g/IndexBuffer;[Ljavax/microedition/m3g/Appearance;Ljavax/microedition/m3g/Group;)V",
            ACC_PUBLIC,
        ),
        method(
            "getSkeleton",
            "()Ljavax/microedition/m3g/Group;",
            ACC_PUBLIC,
        ),
        method(
            "addTransform",
            "(Ljavax/microedition/m3g/Node;III)V",
            ACC_PUBLIC,
        ),
        method(
            "getBoneTransform",
            "(Ljavax/microedition/m3g/Node;Ljavax/microedition/m3g/Transform;)V",
            ACC_PUBLIC,
        ),
        method(
            "getBoneVertices",
            "(Ljavax/microedition/m3g/Node;[I[F)I",
            ACC_PUBLIC,
        ),
    ],
};

pub(super) const TRIANGLE_STRIP_ARRAY: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/TriangleStripArray",
    super_name: "javax/microedition/m3g/IndexBuffer",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[],
    methods: &[
        method("<init>", "(I[I)V", ACC_PUBLIC),
        method("<init>", "([I[I)V", ACC_PUBLIC),
    ],
};

pub(super) const VERTEX_ARRAY: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/VertexArray",
    super_name: "javax/microedition/m3g/Object3D",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[],
    methods: &[
        method("<init>", "(III)V", ACC_PUBLIC),
        method("set", "(II[S)V", ACC_PUBLIC),
        method("set", "(II[B)V", ACC_PUBLIC),
        method("getVertexCount", "()I", ACC_PUBLIC),
        method("getComponentCount", "()I", ACC_PUBLIC),
        method("getComponentType", "()I", ACC_PUBLIC),
        method("get", "(II[S)V", ACC_PUBLIC),
        method("get", "(II[B)V", ACC_PUBLIC),
    ],
};

pub(super) const VERTEX_BUFFER: M3gClassSpec = M3gClassSpec {
    name: "javax/microedition/m3g/VertexBuffer",
    super_name: "javax/microedition/m3g/Object3D",
    flags: ACC_PUBLIC | ACC_SUPER,
    constants: &[],
    methods: &[
        method("<init>", "()V", ACC_PUBLIC),
        method("getVertexCount", "()I", ACC_PUBLIC),
        method(
            "setPositions",
            "(Ljavax/microedition/m3g/VertexArray;F[F)V",
            ACC_PUBLIC,
        ),
        method(
            "setTexCoords",
            "(ILjavax/microedition/m3g/VertexArray;F[F)V",
            ACC_PUBLIC,
        ),
        method(
            "setNormals",
            "(Ljavax/microedition/m3g/VertexArray;)V",
            ACC_PUBLIC,
        ),
        method(
            "setColors",
            "(Ljavax/microedition/m3g/VertexArray;)V",
            ACC_PUBLIC,
        ),
        method(
            "getPositions",
            "([F)Ljavax/microedition/m3g/VertexArray;",
            ACC_PUBLIC,
        ),
        method(
            "getTexCoords",
            "(I[F)Ljavax/microedition/m3g/VertexArray;",
            ACC_PUBLIC,
        ),
        method(
            "getNormals",
            "()Ljavax/microedition/m3g/VertexArray;",
            ACC_PUBLIC,
        ),
        method(
            "getColors",
            "()Ljavax/microedition/m3g/VertexArray;",
            ACC_PUBLIC,
        ),
        method("setDefaultColor", "(I)V", ACC_PUBLIC),
        method("getDefaultColor", "()I", ACC_PUBLIC),
    ],
};
