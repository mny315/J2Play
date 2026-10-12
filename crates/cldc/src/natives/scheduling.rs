use super::{
    EmuError, NativeRegistry, NativeSignature, NativeValue, calendar_add_field, calendar_field,
    calendar_replace_date_time, calendar_replace_field, calendar_roll_field, cldc_error,
};

pub(super) fn register_scheduling_natives(registry: &mut NativeRegistry) -> Result<(), EmuError> {
    registry.register(
        NativeSignature::new("java/util/Calendar", "fieldValue", "(JI)I"),
        |_, args| {
            let [NativeValue::Long(millis), NativeValue::Int(field)] = args else {
                return Err(cldc_error(
                    "native-arguments",
                    "Calendar.fieldValue expects long and int",
                ));
            };
            Ok(Some(NativeValue::Int(calendar_field(*millis, *field)?)))
        },
    )?;
    registry.register(
        NativeSignature::new("java/util/Calendar", "replaceField", "(JII)J"),
        |_, args| {
            let [
                NativeValue::Long(millis),
                NativeValue::Int(field),
                NativeValue::Int(value),
            ] = args
            else {
                return Err(cldc_error(
                    "native-arguments",
                    "Calendar.replaceField expects long and two ints",
                ));
            };
            Ok(Some(NativeValue::Long(calendar_replace_field(
                *millis, *field, *value,
            )?)))
        },
    )?;
    registry.register(
        NativeSignature::new("java/util/Calendar", "replaceDateTime", "(JIIIIII)J"),
        |_, args| {
            let [
                NativeValue::Long(millis),
                NativeValue::Int(year),
                NativeValue::Int(month),
                NativeValue::Int(day),
                NativeValue::Int(hour),
                NativeValue::Int(minute),
                NativeValue::Int(second),
            ] = args
            else {
                return Err(cldc_error(
                    "native-arguments",
                    "Calendar.replaceDateTime expects long and six ints",
                ));
            };
            Ok(Some(NativeValue::Long(calendar_replace_date_time(
                *millis, *year, *month, *day, *hour, *minute, *second,
            )?)))
        },
    )?;
    registry.register(
        NativeSignature::new("java/util/Calendar", "addField", "(JII)J"),
        |_, args| {
            let [
                NativeValue::Long(millis),
                NativeValue::Int(field),
                NativeValue::Int(amount),
            ] = args
            else {
                return Err(cldc_error(
                    "native-arguments",
                    "Calendar.addField expects long and two ints",
                ));
            };
            Ok(Some(NativeValue::Long(calendar_add_field(
                *millis, *field, *amount,
            )?)))
        },
    )?;
    registry.register(
        NativeSignature::new("java/util/Calendar", "rollField", "(JII)J"),
        |_, args| {
            let [
                NativeValue::Long(millis),
                NativeValue::Int(field),
                NativeValue::Int(amount),
            ] = args
            else {
                return Err(cldc_error(
                    "native-arguments",
                    "Calendar.rollField expects long and two ints",
                ));
            };
            Ok(Some(NativeValue::Long(calendar_roll_field(
                *millis, *field, *amount,
            )?)))
        },
    )?;
    registry.register(
        NativeSignature::new(
            "java/util/Timer",
            "schedule0",
            "(Ljava/util/TimerTask;JJZ)V",
        ),
        |context, args| {
            let [
                NativeValue::Reference(Some(timer)),
                NativeValue::Reference(Some(task)),
                NativeValue::Long(deadline),
                NativeValue::Long(period),
                NativeValue::Int(fixed_rate),
            ] = args
            else {
                return Err(cldc_error(
                    "native-arguments",
                    "Timer.schedule expects timer, task, deadline, period and mode",
                ));
            };
            context.schedule_timer_task(*timer, *task, *deadline, *period, *fixed_rate != 0)?;
            Ok(None)
        },
    )?;
    registry.register(
        NativeSignature::new("java/util/Timer", "cancel0", "()V"),
        |context, args| {
            let [NativeValue::Reference(Some(timer))] = args else {
                return Err(cldc_error(
                    "native-arguments",
                    "Timer.cancel expects receiver",
                ));
            };
            context.cancel_timer(*timer);
            Ok(None)
        },
    )?;

    Ok(())
}
