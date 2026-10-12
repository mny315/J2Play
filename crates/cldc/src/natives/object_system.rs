use super::{EmuError, NativeRegistry, NativeSignature, NativeValue, cldc_error, require_arity};

pub(super) fn register_object_system_natives(
    registry: &mut NativeRegistry,
) -> Result<(), EmuError> {
    registry.register(
        NativeSignature::new("java/lang/Object", "hashCode", "()I"),
        |_, args| {
            let [NativeValue::Reference(Some(reference))] = args else {
                return Err(cldc_error(
                    "native-arguments",
                    "Object.hashCode expects receiver",
                ));
            };
            Ok(Some(NativeValue::Int(
                (*reference ^ (*reference >> 32)) as i32,
            )))
        },
    )?;
    registry.register(
        NativeSignature::new("java/lang/Object", "__toString", "(I)Ljava/lang/String;"),
        |context, args| {
            let [
                NativeValue::Reference(Some(reference)),
                NativeValue::Int(hash),
            ] = args
            else {
                return Err(cldc_error(
                    "native-arguments",
                    "Object.__toString expects receiver and hash code",
                ));
            };
            let class = context.object_class(*reference)?.replace('/', ".");
            let text = format!("{class}@{:x}", *hash as u32);
            Ok(Some(NativeValue::Reference(Some(
                context.intern_java_string(&text)?,
            ))))
        },
    )?;
    registry.register(
        NativeSignature::new("java/lang/Object", "getClass", "()Ljava/lang/Class;"),
        |context, args| {
            let [NativeValue::Reference(Some(reference))] = args else {
                return Err(cldc_error(
                    "native-arguments",
                    "Object.getClass expects receiver",
                ));
            };
            let class = context.object_class(*reference)?;
            Ok(Some(NativeValue::Reference(Some(
                context.intern_java_class(&class)?,
            ))))
        },
    )?;
    registry.register(
        NativeSignature::new("java/lang/Class", "getName", "()Ljava/lang/String;"),
        |context, args| {
            let [NativeValue::Reference(Some(reference))] = args else {
                return Err(cldc_error(
                    "native-arguments",
                    "Class.getName expects receiver",
                ));
            };
            let name = context.read_java_class(*reference)?.replace('/', ".");
            Ok(Some(NativeValue::Reference(Some(
                context.intern_java_string(&name)?,
            ))))
        },
    )?;
    registry.register(
        NativeSignature::new(
            "java/lang/Class",
            "getResourceBytes",
            "(Ljava/lang/String;)[B",
        ),
        |context, args| {
            let [
                NativeValue::Reference(Some(class)),
                NativeValue::Reference(Some(name)),
            ] = args
            else {
                return Err(cldc_error(
                    "native-arguments",
                    "Class.getResourceAsStream expects receiver and non-null name",
                ));
            };
            let class = context.read_java_class(*class)?;
            let name = context.read_java_string(*name)?;
            let path = if let Some(absolute) = name.strip_prefix('/') {
                absolute.to_owned()
            } else if let Some((package, _)) = class.rsplit_once('/') {
                format!("{package}/{name}")
            } else {
                name
            };
            match context.read_resource(&path)? {
                Some(bytes) => Ok(Some(NativeValue::Reference(Some(
                    context.allocate_java_byte_array(&bytes)?,
                )))),
                None => Ok(Some(NativeValue::Reference(None))),
            }
        },
    )?;
    registry.register(
        NativeSignature::new("java/lang/System", "currentTimeMillis", "()J"),
        |context, args| {
            require_arity(args, 0)?;
            Ok(Some(NativeValue::Long(context.wall_clock_millis())))
        },
    )?;
    registry.register(
        NativeSignature::new(
            "java/lang/System",
            "getProperty0",
            "(Ljava/lang/String;)Ljava/lang/String;",
        ),
        |context, args| {
            let [NativeValue::Reference(Some(reference))] = args else {
                return Err(cldc_error(
                    "native-arguments",
                    "System.getProperty expects a non-null String",
                ));
            };
            let name = context.read_java_string(*reference)?;
            let value = context.system_property(&name).map(str::to_owned);
            match value {
                Some(value) => Ok(Some(NativeValue::Reference(Some(
                    context.intern_java_string(&value)?,
                )))),
                None => Ok(Some(NativeValue::Reference(None))),
            }
        },
    )?;

    Ok(())
}
