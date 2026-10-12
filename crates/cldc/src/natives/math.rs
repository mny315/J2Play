use super::{
    EmuError, NativeRegistry, NativeSignature, NativeValue, cldc_error, java_double_max,
    java_double_min, java_float_max, java_float_min,
};

pub(super) fn register_math_natives(registry: &mut NativeRegistry) -> Result<(), EmuError> {
    for (name, operation) in [
        ("sin", f64::sin as fn(f64) -> f64),
        ("cos", f64::cos),
        ("tan", f64::tan),
        ("sqrt", f64::sqrt),
        ("ceil", f64::ceil),
        ("floor", f64::floor),
        ("toRadians", f64::to_radians),
        ("toDegrees", f64::to_degrees),
    ] {
        registry.register(
            NativeSignature::new("java/lang/Math", name, "(D)D"),
            move |_, args| {
                let [NativeValue::Double(value)] = args else {
                    return Err(cldc_error(
                        "native-arguments",
                        format!("Math.{name} expects one double"),
                    ));
                };
                Ok(Some(NativeValue::Double(operation(*value))))
            },
        )?;
    }
    for (name, operation) in [
        ("min", java_float_min as fn(f32, f32) -> f32),
        ("max", java_float_max),
    ] {
        registry.register(
            NativeSignature::new("java/lang/Math", name, "(FF)F"),
            move |_, args| {
                let [NativeValue::Float(left), NativeValue::Float(right)] = args else {
                    return Err(cldc_error(
                        "native-arguments",
                        format!("Math.{name} expects two floats"),
                    ));
                };
                Ok(Some(NativeValue::Float(operation(*left, *right))))
            },
        )?;
    }
    for (name, operation) in [
        ("min", java_double_min as fn(f64, f64) -> f64),
        ("max", java_double_max),
    ] {
        registry.register(
            NativeSignature::new("java/lang/Math", name, "(DD)D"),
            move |_, args| {
                let [NativeValue::Double(left), NativeValue::Double(right)] = args else {
                    return Err(cldc_error(
                        "native-arguments",
                        format!("Math.{name} expects two doubles"),
                    ));
                };
                Ok(Some(NativeValue::Double(operation(*left, *right))))
            },
        )?;
    }

    Ok(())
}
