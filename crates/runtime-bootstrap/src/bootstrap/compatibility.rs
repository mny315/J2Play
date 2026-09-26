use super::{
    ACC_ABSTRACT, ACC_FINAL, ACC_NATIVE, ACC_PRIVATE, ACC_PROTECTED, ACC_PUBLIC, ACC_STATIC,
    ACC_SYNCHRONIZED, Attribute, ClassFile, CodeAttribute, Constant, LCDUI_HEIGHT_MARKER_BYTES,
    LCDUI_WIDTH_MARKER_BYTES, Member, append_constant,
};

mod canvas;
mod class_builder;
mod display;
mod file_streams;
mod graphics;
mod hierarchy;
mod key_input;
mod serial_dispatch;
mod text_field;
mod text_input;
mod text_preedit;

pub(crate) use canvas::*;
pub(crate) use class_builder::*;
pub(crate) use display::*;
pub(crate) use graphics::*;
pub(crate) use hierarchy::*;
pub(crate) use key_input::*;
pub(crate) use serial_dispatch::*;
pub(crate) use text_field::*;
pub(crate) use text_input::*;

#[cfg(test)]
pub(crate) const RUST_DIRECT_COMPATIBILITY_FIXTURES: &[&str] = &[
    "java/lang/Class::forName(Ljava/lang/String;)Ljava/lang/Class;",
    "java/lang/Class::newInstance()Ljava/lang/Object;",
    "java/lang/Class::isAssignableFrom(Ljava/lang/Class;)Z",
    "java/lang/Class::isArray()Z",
    "java/lang/Class::isInterface()Z",
    "java/lang/Class::toString()Ljava/lang/String;",
    "java/lang/Class::isInstance(Ljava/lang/Object;)Z",
    "java/lang/ref/Reference::clear()V",
    "java/lang/ref/Reference::get()Ljava/lang/Object;",
    "java/lang/ref/WeakReference::<init>(Ljava/lang/Object;)V",
    "java/lang/ClassNotFoundException::<init>()V",
    "java/lang/ClassNotFoundException::<init>(Ljava/lang/String;)V",
    "java/lang/IllegalAccessException::<init>()V",
    "java/lang/IllegalAccessException::<init>(Ljava/lang/String;)V",
    "java/lang/InstantiationException::<init>()V",
    "java/lang/InstantiationException::<init>(Ljava/lang/String;)V",
    "java/lang/String::getBytes(Ljava/lang/String;)[B",
    "java/lang/String::<init>(Ljava/lang/StringBuffer;)V",
    "java/lang/String::regionMatches(ZILjava/lang/String;II)Z",
    "java/lang/String::intern()Ljava/lang/String;",
    "java/lang/String::valueOf(D)Ljava/lang/String;",
    "java/lang/System::identityHashCode(Ljava/lang/Object;)I",
    "java/lang/Math::ceil(D)D",
    "java/lang/Math::floor(D)D",
    "java/lang/Math::tan(D)D",
    "java/lang/Math::toRadians(D)D",
    "java/lang/Math::toDegrees(D)D",
    "java/lang/Math::min(FF)F",
    "java/lang/Math::max(FF)F",
    "java/lang/Math::min(DD)D",
    "java/lang/Math::max(DD)D",
    "java/util/Random::<init>()V",
    "java/util/Random::nextFloat()F",
    "java/util/Random::nextDouble()D",
    "java/io/InterruptedIOException::<init>()V",
    "java/io/InterruptedIOException::<init>(Ljava/lang/String;)V",
    "java/io/ByteArrayOutputStream::write([BII)V",
    "java/io/ByteArrayOutputStream::toString()Ljava/lang/String;",
    "java/io/DataInputStream::readUTF(Ljava/io/DataInput;)Ljava/lang/String;",
    "java/lang/StringBuffer::delete(II)Ljava/lang/StringBuffer;",
    "java/lang/StringBuffer::deleteCharAt(I)Ljava/lang/StringBuffer;",
    "java/io/InputStreamReader::<init>(Ljava/io/InputStream;)V",
    "java/io/InputStreamReader::<init>(Ljava/io/InputStream;Ljava/lang/String;)V",
    "java/io/InputStreamReader::close()V",
    "java/io/InputStreamReader::read()I",
    "java/io/InputStreamReader::read([CII)I",
    "java/io/Reader::close()V",
    "java/io/Reader::markSupported()Z",
    "java/io/Reader::read()I",
    "java/io/Reader::read([C)I",
    "java/io/Reader::read([CII)I",
    "java/io/Reader::skip(J)J",
];

/// Adds CLDC 1.1 methods that were absent from the mechanically migrated
/// legacy inventory. Keeping this layer in Rust makes it the canonical owner
/// of all post-migration bootstrap additions.
pub(crate) fn augment_core_compatibility_surface(classes: &mut Vec<ClassFile>) {
    for class in classes.iter_mut() {
        let Some(name) = class.class_name(class.this_class).map(str::to_owned) else {
            continue;
        };
        match name.as_str() {
            "javax/microedition/io/file/FileConnectionImpl" => {
                file_streams::repair_connection(class);
            }
            "javax/microedition/io/file/FileConnectionImpl$FileOutput" => {
                file_streams::repair_output(class);
            }
            "java/io/PrintStream" => repair_print_stream_hierarchy(class),
            "java/io/ByteArrayOutputStream" => append_byte_array_output_compatibility(class),
            "java/io/DataInputStream" => {
                append_native_method(
                    class,
                    ACC_PUBLIC | ACC_STATIC,
                    "readUTF",
                    "(Ljava/io/DataInput;)Ljava/lang/String;",
                );
                delegate_data_input_stream_read_utf(class);
            }
            "java/lang/String" => {
                append_native_method(class, ACC_PUBLIC, "getBytes", "(Ljava/lang/String;)[B");
                append_native_method(class, ACC_PUBLIC, "<init>", "(Ljava/lang/StringBuffer;)V");
                append_native_method(
                    class,
                    ACC_PUBLIC,
                    "regionMatches",
                    "(ZILjava/lang/String;II)Z",
                );
                append_native_method(class, ACC_PUBLIC, "intern", "()Ljava/lang/String;");
                append_native_method(
                    class,
                    ACC_PUBLIC | ACC_STATIC,
                    "valueOf",
                    "(D)Ljava/lang/String;",
                );
            }
            "java/lang/StringBuffer" => {
                append_native_method(
                    class,
                    ACC_PUBLIC | ACC_SYNCHRONIZED,
                    "delete",
                    "(II)Ljava/lang/StringBuffer;",
                );
                append_native_method(
                    class,
                    ACC_PUBLIC | ACC_SYNCHRONIZED,
                    "deleteCharAt",
                    "(I)Ljava/lang/StringBuffer;",
                );
            }
            "javax/microedition/lcdui/Graphics" => {
                replace_method_with_native(class, "fillTriangle", "(IIIIII)V");
                replace_method_with_native(class, "setClip", "(IIII)V");
                append_graphics_draw_substring(class);
                append_graphics_canvas_support(class);
                append_nokia_direct_graphics_surface(class);
                append_native_method(class, ACC_PUBLIC, "setGrayScale", "(I)V");
                for name in ["getRedComponent", "getGreenComponent", "getBlueComponent"] {
                    append_native_method(class, ACC_PUBLIC, name, "()I");
                }
                append_native_method(class, ACC_PUBLIC, "getStrokeStyle", "()I");
            }
            "javax/microedition/lcdui/Image" => append_image_compatibility_surface(class),
            "javax/microedition/lcdui/Canvas" => {
                separate_canvas_serial_dispatch(class);
                append_canvas_callback_key_adapter(class);
                append_canvas_normalized_key_dispatch(class);
                stabilize_canvas_paint_graphics(class);
                isolate_canvas_paint_callback_exceptions(class);
                append_canvas_size_change_dispatch(class);
                for (name, descriptor) in [
                    ("getWidth", "()I"),
                    ("getHeight", "()I"),
                    ("setFullScreenMode", "(Z)V"),
                ] {
                    replace_method_with_native(class, name, descriptor);
                }
                for (name, descriptor) in [
                    ("getGameAction", "(I)I"),
                    ("getKeyCode", "(I)I"),
                    ("getKeyName", "(I)Ljava/lang/String;"),
                    ("hasPointerEvents", "()Z"),
                    ("hasPointerMotionEvents", "()Z"),
                ] {
                    replace_method_with_native(class, name, descriptor);
                }
                for name in [
                    "sizeChanged",
                    "pointerPressed",
                    "pointerReleased",
                    "pointerDragged",
                ] {
                    append_code_method(class, ACC_PROTECTED, name, "(II)V", 0, 3, vec![0xb1]);
                }
            }
            "java/util/Random" => {
                append_random_compatibility(class);
            }
            "javax/microedition/lcdui/Display" => {
                append_native_method(class, ACC_STATIC, "__setTextInputActive", "(Z)V");
                replace_method_with_native(class, "vibrate", "(I)Z");
                limit_display_serial_dispatch_to_one_event(class);
                isolate_display_serial_callback_exceptions(class);
                guard_display_same_current(class);
                append_display_text_input_target_check(class);
                refresh_text_input_after_host_activity(class);
                append_display_normalized_key_dispatch(class);
                defer_display_transition(class);
                append_native_method(class, ACC_PUBLIC, "numColors", "()I");
                append_host_pointer_dispatch(class);
                append_host_text_input_dispatch(class);
            }
            "javax/microedition/lcdui/TextBox" => append_text_box_host_input(class),
            "javax/microedition/lcdui/Form" => append_form_host_input(class),
            "javax/microedition/lcdui/Screen" => share_screen_framebuffer(class),
            "javax/microedition/lcdui/TextField" => {
                implement_text_field_input(class);
                text_preedit::implement_text_preedit(class);
            }
            "javax/microedition/rms/RecordStore" => allow_owned_authmode_any_open(class),
            _ => {}
        }
        apply_dark_lcdui_theme(class, &name);
    }
    classes.push(file_streams::input_class());
    classes.push(super::java_lang_throwable_subclass((
        "javax/microedition/io/file/ConnectionClosedException",
        "java/lang/RuntimeException",
        false,
    )));
}

fn append_byte_array_output_compatibility(class: &mut ClassFile) {
    const OWNER: &str = "java/io/ByteArrayOutputStream";
    append_native_method(class, ACC_PUBLIC, "write", "([BII)V");
    let [string_hi, string_lo] = append_class(class, "java/lang/String").to_be_bytes();
    let [init_hi, init_lo] =
        append_method_ref(class, "java/lang/String", "<init>", "([BII)V").to_be_bytes();
    let [buf_hi, buf_lo] = append_field_ref(class, OWNER, "buf", "[B").to_be_bytes();
    let [count_hi, count_lo] = append_field_ref(class, OWNER, "count", "I").to_be_bytes();
    append_code_method(
        class,
        ACC_PUBLIC,
        "toString",
        "()Ljava/lang/String;",
        5,
        1,
        vec![
            0xbb, string_hi, string_lo, 0x59, // new String, dup
            0x2a, 0xb4, buf_hi, buf_lo, 0x03, // buf, offset 0
            0x2a, 0xb4, count_hi, count_lo, // count
            0xb7, init_hi, init_lo, 0xb0, // String(buf, 0, count), areturn
        ],
    );
}

fn append_random_compatibility(class: &mut ClassFile) {
    // Host policy supplies only the initial seed. Ordinary calls preserve
    // overridden setSeed/next methods, including their exceptions and yields.
    append_native_method(class, ACC_PRIVATE | ACC_STATIC, "__initialSeed", "()J");
    let [seed_hi, seed_lo] =
        append_method_ref(class, "java/util/Random", "__initialSeed", "()J").to_be_bytes();
    let [init_hi, init_lo] =
        append_method_ref(class, "java/util/Random", "<init>", "(J)V").to_be_bytes();
    let [next_hi, next_lo] =
        append_method_ref(class, "java/util/Random", "next", "(I)I").to_be_bytes();
    append_code_method(
        class,
        ACC_PUBLIC,
        "<init>",
        "()V",
        3,
        1,
        vec![0x2a, 0xb8, seed_hi, seed_lo, 0xb7, init_hi, init_lo, 0xb1],
    );
    append_code_method(
        class,
        ACC_PUBLIC,
        "nextFloat",
        "()F",
        3,
        1,
        vec![
            0x2a, 0x10, 24, 0xb6, next_hi, next_lo, 0x86, // next(24), i2f
            0x04, 0x10, 24, 0x78, 0x86, 0x6e, 0xae, // divide by 2^24, freturn
        ],
    );
    append_code_method(
        class,
        ACC_PUBLIC,
        "nextDouble",
        "()D",
        5,
        1,
        vec![
            0x2a, 0x10, 26, 0xb6, next_hi, next_lo, 0x85, 0x10, 27, 0x79, 0x2a, 0x10, 27, 0xb6,
            next_hi, next_lo, 0x85, 0x61, 0x8a, 0x0a, 0x10, 53, 0x79, 0x8a, 0x6f,
            0xaf, // divide by 2^53, dreturn
        ],
    );
}
