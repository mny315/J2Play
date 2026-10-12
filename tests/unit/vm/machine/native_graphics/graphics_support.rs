use super::*;

pub(super) fn direct_graphics_target(
    machine: &mut Machine<'_, '_>,
    width: i32,
    height: i32,
) -> (Handle, Handle) {
    let pixels = machine
        .heap
        .managed
        .allocate_array(ArrayKind::Int, width * height)
        .unwrap();
    let image = machine
        .heap
        .managed
        .allocate_object(
            "javax/microedition/lcdui/Image",
            HashMap::from([
                (
                    "javax/microedition/lcdui/Image.width:I".into(),
                    HeapValue::Int(width),
                ),
                (
                    "javax/microedition/lcdui/Image.height:I".into(),
                    HeapValue::Int(height),
                ),
                (
                    "javax/microedition/lcdui/Image.pixels:[I".into(),
                    HeapValue::Reference(Some(pixels)),
                ),
            ]),
        )
        .unwrap();
    let mut fields = [
        ("tx", 0),
        ("ty", 0),
        ("clipX", 0),
        ("clipY", 0),
        ("clipW", width),
        ("clipH", height),
        ("stroke", 0),
        ("color", 0xff12_3456_u32.cast_signed()),
    ]
    .map(|(name, value)| {
        (
            format!("javax/microedition/lcdui/Graphics.{name}:I"),
            HeapValue::Int(value),
        )
    })
    .into_iter()
    .collect::<HashMap<_, _>>();
    fields.insert(
        "javax/microedition/lcdui/Graphics.target:Ljavax/microedition/lcdui/Image;".into(),
        HeapValue::Reference(Some(image)),
    );
    let graphics = machine
        .heap
        .managed
        .allocate_object("javax/microedition/lcdui/Graphics", fields)
        .unwrap();
    let (pixels, _, _) = machine.graphics_target(graphics).unwrap();
    (graphics, pixels)
}

pub(super) fn set_graphics_fields(
    machine: &mut Machine<'_, '_>,
    graphics: Handle,
    fields: &[(&str, i32)],
) {
    for &(name, value) in fields {
        machine
            .heap
            .managed
            .set_field(
                graphics,
                &format!("javax/microedition/lcdui/Graphics.{name}:I"),
                HeapValue::Int(value),
            )
            .unwrap();
    }
}
