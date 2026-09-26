use super::{
    Attribute, ClassFile, CodeAttribute, Constant, ExceptionHandler, Member,
    append_bootstrap_classes, build_bootstrap_class,
};

mod graphics;
mod image;
mod items;
mod lists_screens;
mod text_components;

pub(super) fn append_generated_lcdui_graphics(classes: &mut Vec<ClassFile>) {
    graphics::append_graphics(classes);
    image::append_image(classes);
    items::append_items(classes);
    lists_screens::append_lists_screens(classes);
    text_components::append_text_components(classes);
}
