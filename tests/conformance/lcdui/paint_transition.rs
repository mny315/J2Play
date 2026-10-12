use crate::support::guest::{MethodCode, Pool, code_method, emit_reference};
use classfile::{ClassFile, Constant, Member};

const CANVAS: &str = "fixtures/PaintTransition";
const MIDLET: &str = "fixtures/PaintTransitionMidlet";
const DISPLAY: &str = "javax/microedition/lcdui/Display";
const DISPLAY_TYPE: &str = "Ljavax/microedition/lcdui/Display;";
const TARGET_TYPE: &str = "Ljavax/microedition/lcdui/Displayable;";
const COLOR: u32 = 0x12_34_56;

fn fixture_class(
    name: &str,
    parent: &str,
    methods: Vec<MethodCode<'_>>,
    fields: &[(&str, &str)],
    mut pool: Pool,
) -> ClassFile {
    let this_class = pool.class(name);
    let super_class = pool.class(parent);
    let code_name = pool.utf8("Code");
    let mut init = vec![0x2a];
    emit_reference(&mut init, 0xb7, pool.method(parent, "<init>", "()V"));
    init.push(0xb1);
    let mut members = vec![code_method(
        &mut pool,
        code_name,
        MethodCode {
            flags: 1,
            name: "<init>",
            descriptor: "()V",
            max_stack: 1,
            max_locals: 1,
            code: init,
        },
    )];
    members.extend(
        methods
            .into_iter()
            .map(|method| code_method(&mut pool, code_name, method)),
    );
    let fields = fields
        .iter()
        .map(|(name, descriptor)| Member {
            access_flags: 9,
            name_index: pool.utf8(name),
            descriptor_index: pool.utf8(descriptor),
            attributes: vec![],
        })
        .collect();
    ClassFile {
        minor_version: 0,
        major_version: 48,
        constant_pool: pool.entries,
        access_flags: 0x0021,
        this_class,
        super_class,
        interfaces: vec![],
        fields,
        methods: members,
        attributes: vec![],
    }
}

fn paint_transition_fixture(service_repaints: bool) -> ClassFile {
    let mut pool = Pool::new();
    let display = pool.field(CANVAS, "display", DISPLAY_TYPE);
    let target = pool.field(CANVAS, "target", TARGET_TYPE);
    let set_current = pool.method(
        DISPLAY,
        "setCurrent",
        "(Ljavax/microedition/lcdui/Displayable;)V",
    );
    let idle = pool.method(DISPLAY, "__hostIdle", "()V");
    let color = pool.push(Constant::Integer(i32::try_from(COLOR).unwrap()));
    let mut paint = vec![0x2b];
    emit_reference(&mut paint, 0x13, color); // ldc_w
    emit_reference(
        &mut paint,
        0xb6,
        pool.method("javax/microedition/lcdui/Graphics", "setColor", "(I)V"),
    );
    paint.extend_from_slice(&[0x2b, 0x03, 0x03, 0x11, 0x01, 0x40, 0x11, 0x01, 0x40]);
    emit_reference(
        &mut paint,
        0xb6,
        pool.method("javax/microedition/lcdui/Graphics", "fillRect", "(IIII)V"),
    );
    emit_reference(&mut paint, 0xb2, display);
    emit_reference(&mut paint, 0xb2, target);
    emit_reference(&mut paint, 0xb6, set_current);
    paint.push(0xb1);

    let mut run = vec![];
    emit_reference(&mut run, 0xbb, pool.class(MIDLET));
    run.push(0x59);
    emit_reference(&mut run, 0xb7, pool.method(MIDLET, "<init>", "()V"));
    emit_reference(
        &mut run,
        0xb8,
        pool.method(
            DISPLAY,
            "getDisplay",
            "(Ljavax/microedition/midlet/MIDlet;)Ljavax/microedition/lcdui/Display;",
        ),
    );
    emit_reference(&mut run, 0xb3, display);
    emit_reference(&mut run, 0xbb, pool.class("javax/microedition/lcdui/Form"));
    run.extend_from_slice(&[0x59, 0x01]);
    emit_reference(
        &mut run,
        0xb7,
        pool.method(
            "javax/microedition/lcdui/Form",
            "<init>",
            "(Ljava/lang/String;)V",
        ),
    );
    emit_reference(&mut run, 0xb3, target);
    emit_reference(&mut run, 0xbb, pool.class(CANVAS));
    run.push(0x59);
    emit_reference(&mut run, 0xb7, pool.method(CANVAS, "<init>", "()V"));
    run.push(0x4b); // astore_0
    emit_reference(&mut run, 0xb2, display);
    run.push(0x2a);
    emit_reference(&mut run, 0xb6, set_current);
    if service_repaints {
        run.push(0x2a);
        emit_reference(&mut run, 0xb6, pool.method(CANVAS, "repaint", "()V"));
        run.push(0x2a);
        emit_reference(
            &mut run,
            0xb6,
            pool.method(CANVAS, "serviceRepaints", "()V"),
        );
    } else {
        emit_reference(&mut run, 0xb8, idle);
    }
    // A display turn must publish the replacement without requiring key input.
    emit_reference(&mut run, 0xb8, idle);
    run.push(0xb1);
    fixture_class(
        CANVAS,
        "javax/microedition/lcdui/Canvas",
        vec![
            MethodCode {
                flags: 4,
                name: "paint",
                descriptor: "(Ljavax/microedition/lcdui/Graphics;)V",
                max_stack: 5,
                max_locals: 2,
                code: paint,
            },
            MethodCode {
                flags: 9,
                name: "run",
                descriptor: "()V",
                max_stack: 3,
                max_locals: 1,
                code: run,
            },
        ],
        &[("display", DISPLAY_TYPE), ("target", TARGET_TYPE)],
        pool,
    )
}

#[test]
fn display_change_from_paint_does_not_leave_the_old_canvas_over_the_new_screen() {
    if crate::support::isolate(concat!(
        module_path!(),
        "::",
        "display_change_from_paint_does_not_leave_the_old_canvas_over_the_new_screen"
    )) {
        return;
    }
    for service_repaints in [false, true] {
        let limits = vm::Limits::default();
        let mut program = vm::Program::new();
        for class in runtime_bootstrap::production_bootstrap_classes() {
            program.add_class(&class, &limits).unwrap();
        }
        let methods = [
            ("startApp", "()V", 1),
            ("pauseApp", "()V", 1),
            ("destroyApp", "(Z)V", 2),
        ]
        .into_iter()
        .map(|(name, descriptor, max_locals)| MethodCode {
            flags: 4,
            name,
            descriptor,
            max_stack: 0,
            max_locals,
            code: vec![0xb1],
        })
        .collect();
        let midlet = fixture_class(
            MIDLET,
            "javax/microedition/midlet/MIDlet",
            methods,
            &[],
            Pool::new(),
        );
        program.add_class(&midlet, &limits).unwrap();
        program
            .add_class(&paint_transition_fixture(service_repaints), &limits)
            .unwrap();
        cldc::register_core_natives(program.native_registry_mut()).unwrap();
        midp::register_natives(program.native_registry_mut()).unwrap();
        let mut host = super::canvas_modes::FrameCapture::default();
        program
            .execute_with_context(CANVAS, "run", "()V", limits, false, &mut host)
            .unwrap();
        let is_canvas =
            |frame: &(u32, u32, Vec<u32>)| frame.2.iter().all(|pixel| pixel & 0xff_ffff == COLOR);
        assert!(
            host.frames.iter().any(is_canvas),
            "the original Canvas must paint"
        );
        assert!(
            !is_canvas(host.frames.last().expect("replacement frame")),
            "the completed Canvas paint overwrote the replacement Screen; serviceRepaints={service_repaints}"
        );
    }
}
