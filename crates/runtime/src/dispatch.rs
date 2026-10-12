use diagnostics::EmuError;
use midp::MidletLifecycleEvent;

pub fn midlet_lifecycle_event(
    ams: &mut midp::Ams,
    event: MidletLifecycleEvent,
) -> Result<(), EmuError> {
    match event {
        MidletLifecycleEvent::Callback { callback, outcome } => {
            ams.callback_finished(callback, outcome)
        }
        MidletLifecycleEvent::Notification(notification) => {
            ams.notify(notification);
            Ok(())
        }
    }
}

#[must_use]
pub fn idle_call() -> vm::InstanceCall {
    vm::InstanceCall {
        target: vm::CallTarget::Static {
            class: "javax/microedition/lcdui/Display".to_owned(),
        },
        name: "__hostIdle".to_owned(),
        descriptor: "()V".to_owned(),
        arguments: Vec::new(),
    }
}

#[must_use]
pub fn key_call(event: &platform::MidpKeyEvent) -> vm::InstanceCall {
    let name = match event.kind {
        platform::KeyKind::Pressed => "__hostKeyPressed",
        platform::KeyKind::Released => "__hostKeyReleased",
        platform::KeyKind::Repeated => "__hostKeyRepeated",
    };
    vm::InstanceCall {
        target: vm::CallTarget::Static {
            class: "javax/microedition/lcdui/Display".to_owned(),
        },
        name: name.to_owned(),
        descriptor: "(III)V".to_owned(),
        arguments: vec![
            vm::Value::Int(event.key_code),
            vm::Value::Int(event.state.cast_signed()),
            vm::Value::Int(event.lcdui_key_code),
        ],
    }
}

#[must_use]
pub fn pointer_call(event: platform::PointerEvent) -> vm::InstanceCall {
    let name = match event.kind {
        platform::PointerKind::PointerPressed => "__hostPointerPressed",
        platform::PointerKind::PointerReleased => "__hostPointerReleased",
        platform::PointerKind::PointerDragged => "__hostPointerDragged",
    };
    vm::InstanceCall {
        target: vm::CallTarget::Static {
            class: "javax/microedition/lcdui/Display".to_owned(),
        },
        name: name.to_owned(),
        descriptor: "(II)V".to_owned(),
        arguments: vec![vm::Value::Int(event.x), vm::Value::Int(event.y)],
    }
}

#[must_use]
pub fn text_input_call(event: platform::TextInputEvent) -> vm::InstanceCall {
    let value = match event.kind {
        platform::TextInputKind::Commit => i32::from(event.code_unit),
        platform::TextInputKind::DeleteBackward => 8,
        platform::TextInputKind::DeleteForward => 127,
    };
    vm::InstanceCall {
        target: vm::CallTarget::Static {
            class: "javax/microedition/lcdui/Display".to_owned(),
        },
        name: "__hostTextInput".to_owned(),
        descriptor: "(I)V".to_owned(),
        arguments: vec![vm::Value::Int(value)],
    }
}
