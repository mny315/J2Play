use super::{EmuError, NativeRegistry, NativeSignature, NativeValue, cldc_error};

pub(super) fn register_throwable_console_natives(
    registry: &mut NativeRegistry,
) -> Result<(), EmuError> {
    registry.register(
        NativeSignature::new("java/lang/Throwable", "toString", "()Ljava/lang/String;"),
        |context, args| {
            let [NativeValue::Reference(Some(reference))] = args else {
                return Err(cldc_error(
                    "native-arguments",
                    "Throwable.toString expects receiver",
                ));
            };
            let text = throwable_text(context, *reference)?;
            Ok(Some(NativeValue::Reference(Some(
                context.intern_java_string(&text)?,
            ))))
        },
    )?;
    registry.register(
        NativeSignature::new("java/lang/Throwable", "printStackTrace", "()V"),
        |context, args| {
            let [NativeValue::Reference(Some(reference))] = args else {
                return Err(cldc_error(
                    "native-arguments",
                    "Throwable.printStackTrace expects receiver",
                ));
            };
            let heading = throwable_text(context, *reference)?;
            context.write_console_error(&heading)?;
            for index in 0.. {
                let Some(frame) = context.read_throwable_trace_frame(*reference, index)? else {
                    break;
                };
                context.write_console_error(&frame)?;
            }
            Ok(None)
        },
    )?;
    registry.register(
        NativeSignature::new("java/io/PrintStream", "write0", "(Ljava/lang/String;Z)V"),
        |context, args| {
            let [
                NativeValue::Reference(Some(_receiver)),
                NativeValue::Reference(Some(text)),
                NativeValue::Int(newline),
            ] = args
            else {
                return Err(cldc_error(
                    "native-arguments",
                    "PrintStream.write0 expects receiver, text and newline flag",
                ));
            };
            let text = context.read_java_string(*text)?;
            context.write_console_output(&text, *newline != 0)?;
            Ok(None)
        },
    )?;
    registry.register(
        NativeSignature::new("java/lang/Throwable", "fillInStackTrace0", "()V"),
        |context, args| {
            let [NativeValue::Reference(Some(reference))] = args else {
                return Err(cldc_error(
                    "native-arguments",
                    "Throwable.fillInStackTrace expects receiver",
                ));
            };
            context.capture_throwable_trace(*reference)?;
            Ok(None)
        },
    )?;

    Ok(())
}

fn throwable_text(context: &dyn natives::VmAccess, reference: u64) -> Result<String, EmuError> {
    let mut text = context.object_class(reference)?.replace('/', ".");
    if let Some(message) = context.read_reference_field(
        reference,
        "java/lang/Throwable.detailMessage:Ljava/lang/String;",
    )? {
        text.push_str(": ");
        text.push_str(&context.read_java_string(message)?);
    }
    Ok(text)
}
