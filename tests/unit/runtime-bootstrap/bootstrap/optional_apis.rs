use super::*;
use crate::*;

fn assert_string_constants(class: &ClassFile, expected: &[(&str, &str)]) {
    for &(name, value) in expected {
        let field = class
            .fields
            .iter()
            .find(|field| class.utf8(field.name_index) == Some(name))
            .unwrap();
        assert_eq!(field.access_flags, ACC_PUBLIC | ACC_STATIC | ACC_FINAL);
        let Attribute::Raw { bytes, .. } = &field.attributes[0] else {
            panic!("{name} must have a ConstantValue attribute");
        };
        let value_index = u16::from_be_bytes([bytes[0], bytes[1]]);
        let Some(Constant::String { string_index }) = class
            .constant_pool
            .get(usize::from(value_index))
            .and_then(Option::as_ref)
        else {
            panic!("{name} must reference a string constant");
        };
        assert_eq!(class.utf8(*string_index), Some(value));
    }
}

#[test]
fn sensor_discovery_declares_context_constants_and_an_empty_inventory() {
    let classes = production_bootstrap_classes();
    let sensor_info = classes
        .iter()
        .find(|class| {
            class.class_name(class.this_class) == Some("javax/microedition/sensor/SensorInfo")
        })
        .unwrap();
    assert_string_constants(
        sensor_info,
        &[
            ("CONTEXT_TYPE_AMBIENT", "ambient"),
            ("CONTEXT_TYPE_DEVICE", "device"),
            ("CONTEXT_TYPE_USER", "user"),
        ],
    );
    let sensor_manager = classes
        .iter()
        .find(|class| {
            class.class_name(class.this_class) == Some("javax/microedition/sensor/SensorManager")
        })
        .unwrap();
    let find_sensors = sensor_manager
        .methods
        .iter()
        .find(|method| sensor_manager.utf8(method.name_index) == Some("findSensors"))
        .unwrap();
    let Attribute::Code(code) = &find_sensors.attributes[0] else {
        panic!("findSensors must return a real empty SensorInfo array");
    };
    assert_eq!(code.code.first(), Some(&0x03));
    assert_eq!(code.code.get(1), Some(&0xbd));
    assert_eq!(code.code.last(), Some(&0xb0));
}

#[test]
fn unavailable_bluetooth_declares_an_exception_and_native_property_lookup() {
    let classes = production_bootstrap_classes();
    let local_device = classes
        .iter()
        .find(|class| class.class_name(class.this_class) == Some("javax/bluetooth/LocalDevice"))
        .unwrap();
    let get_local_device = local_device
        .methods
        .iter()
        .find(|method| local_device.utf8(method.name_index) == Some("getLocalDevice"))
        .unwrap();
    let Attribute::Code(code) = &get_local_device.attributes[0] else {
        panic!("unavailable Bluetooth must throw from executable bytecode");
    };
    assert_eq!(code.code.last(), Some(&0xbf));
    let get_property = local_device
        .methods
        .iter()
        .find(|method| local_device.utf8(method.name_index) == Some("getProperty"))
        .unwrap();
    assert_eq!(
        get_property.access_flags,
        ACC_PUBLIC | ACC_STATIC | ACC_NATIVE
    );
    assert!(get_property.attributes.is_empty());
}

#[test]
fn wireless_messaging_declares_message_interfaces_and_type_constants() {
    let classes = production_bootstrap_classes();
    let binary_message = classes
        .iter()
        .find(|class| {
            class.class_name(class.this_class) == Some("javax/wireless/messaging/BinaryMessage")
        })
        .unwrap();
    assert_eq!(
        binary_message.access_flags,
        ACC_PUBLIC | ACC_INTERFACE | ACC_ABSTRACT
    );
    assert_eq!(
        binary_message.class_name(binary_message.interfaces[0]),
        Some("javax/wireless/messaging/Message")
    );
    for (name, descriptor) in [("getPayloadData", "()[B"), ("setPayloadData", "([B)V")] {
        assert!(binary_message.methods.iter().any(|method| {
            binary_message.utf8(method.name_index) == Some(name)
                && binary_message.utf8(method.descriptor_index) == Some(descriptor)
                && method.access_flags == ACC_PUBLIC | ACC_ABSTRACT
        }));
    }

    let text_message = classes
        .iter()
        .find(|class| {
            class.class_name(class.this_class) == Some("javax/wireless/messaging/TextMessage")
        })
        .unwrap();
    assert_eq!(
        text_message.class_name(text_message.interfaces[0]),
        Some("javax/wireless/messaging/Message")
    );

    let message_listener = classes
        .iter()
        .find(|class| {
            class.class_name(class.this_class) == Some("javax/wireless/messaging/MessageListener")
        })
        .unwrap();
    assert!(message_listener.methods.iter().any(|method| {
        message_listener.utf8(method.name_index) == Some("notifyIncomingMessage")
            && message_listener.utf8(method.descriptor_index)
                == Some("(Ljavax/wireless/messaging/MessageConnection;)V")
            && method.access_flags == ACC_PUBLIC | ACC_ABSTRACT
    }));

    let message_connection = classes
        .iter()
        .find(|class| {
            class.class_name(class.this_class) == Some("javax/wireless/messaging/MessageConnection")
        })
        .unwrap();
    assert_eq!(
        message_connection.access_flags,
        ACC_PUBLIC | ACC_INTERFACE | ACC_ABSTRACT
    );
    assert_eq!(
        message_connection.class_name(message_connection.interfaces[0]),
        Some("javax/microedition/io/Connection")
    );
    for (name, descriptor) in [
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
    ] {
        assert!(message_connection.methods.iter().any(|method| {
            message_connection.utf8(method.name_index) == Some(name)
                && message_connection.utf8(method.descriptor_index) == Some(descriptor)
                && method.access_flags == ACC_PUBLIC | ACC_ABSTRACT
        }));
    }
    assert_string_constants(
        message_connection,
        &[
            ("TEXT_MESSAGE", "text"),
            ("BINARY_MESSAGE", "binary"),
            ("MULTIPART_MESSAGE", "multipart"),
        ],
    );
}

#[test]
fn bundled_nokia_full_canvas_detection_distinguishes_the_guest_adapter() {
    let classes = production_bootstrap_classes();
    let full_canvas = classes
        .iter()
        .find(|class| class.class_name(class.this_class) == Some("com/nokia/mid/ui/FullCanvas"))
        .unwrap();

    let constructor_index = full_canvas
        .methods
        .iter()
        .position(|method| full_canvas.utf8(method.name_index) == Some("<init>"))
        .unwrap();
    let mut bundled_adapter = full_canvas.clone();
    bundled_adapter.methods = vec![bundled_adapter.methods[constructor_index].clone()];
    let canvas_init = find_method_ref(
        &bundled_adapter,
        "javax/microedition/lcdui/Canvas",
        "<init>",
        "()V",
    )
    .unwrap()
    .to_be_bytes();
    let Attribute::Code(code) = &mut bundled_adapter.methods[0].attributes[0] else {
        panic!("FullCanvas constructor must have bytecode");
    };
    code.max_stack = 1;
    code.max_locals = 1;
    code.code = vec![0x2a, 0xb7, canvas_init[0], canvas_init[1], 0xb1];
    assert!(is_bundled_nokia_full_canvas_adapter(&bundled_adapter));
    assert!(!is_bundled_nokia_full_canvas_adapter(full_canvas));
}
