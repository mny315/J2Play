use super::{
    Attribute, ClassFile, CodeAttribute, Constant, ExceptionHandler, Member,
    append_bootstrap_classes, build_bootstrap_class,
};

mod alerts;
mod canvas;
mod choices;
mod commands_items;
mod display;
mod forms;

pub(super) fn append_generated_lcdui_core(classes: &mut Vec<ClassFile>) {
    alerts::append_alerts(classes);
    canvas::append_canvas(classes);
    choices::append_choices(classes);
    commands_items::append_commands_items(classes);
    display::append_display_classes(classes);
    forms::append_forms(classes);
}
