use super::{
    EmuError, NativeRegistry, NativeSignature, NativeValue, calendar_add_field, calendar_field,
    calendar_replace_date_time, calendar_replace_field, calendar_roll_field, character_digit,
    cldc_error, format_java_float, java_double_max, java_double_min, java_float_max,
    java_float_min, parse_java_f32, parse_java_f64, require_arity, simple_case_mapping,
    unicode_digit,
};

mod math;
mod numbers;
mod object_system;
mod scheduling;
mod text;
mod throwable_console;

pub(super) fn register_all(registry: &mut NativeRegistry) -> Result<(), EmuError> {
    object_system::register_object_system_natives(registry)?;
    text::register_text_natives(registry)?;
    throwable_console::register_throwable_console_natives(registry)?;
    numbers::register_number_natives(registry)?;
    scheduling::register_scheduling_natives(registry)?;
    math::register_math_natives(registry)
}
