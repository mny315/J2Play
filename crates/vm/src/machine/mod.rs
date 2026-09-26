use bytecode::{Instruction, decode};
use classfile::{Attribute, ClassFile, Constant, ExceptionHandler};
use diagnostics::{Category, EmuError};
use heap::{
    Allocation, ArrayAccessKind, ArrayKind, FieldToken, Handle, Heap, HeapError, HeapValue,
};
#[cfg(test)]
use natives::NativeSignature;
use natives::{
    HostServices, ManagedHeapLimitDecision, ManagedHeapLimitNotice, MethodSignature as MethodKey,
    NativeRegistry, NativeValue, VibrationRequest, VmAccess, VmTelemetry,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

const MAIN_THREAD_ID: u64 = u64::MAX;

/// Overflow ceiling for interactive execution. Cancellation is checked every
/// 1,024 instructions; this limit does not set the session's intended duration.
pub const INTERACTIVE_INSTRUCTION_LIMIT: u64 = u64::MAX / 4;

mod acceleration;
mod call;
mod checkpoint;
mod class_state;
mod compatibility;
mod compatibility_byte_streams;
mod compatibility_collections;
mod compatibility_data_input;
mod compatibility_dispatch;
mod compatibility_graphics;
mod compatibility_reader;
mod compatibility_reflection;
mod compatibility_string_buffer;
mod compatibility_strings;
mod compatibility_system;
mod constant_pool;
mod continuation;
mod diagnostic_helpers;
mod exceptions;
mod execution;
mod execution_trace;
mod font;
mod frame_presentation;
mod frame_storage;
#[path = "graphics/mod.rs"]
mod graphics_dispatch;
mod heap_state;
mod heap_values;
mod host_context;
mod interpreter;
mod interpreter_batch;
mod interpreter_helpers;
mod interpreter_loops;
mod jsr239;
mod leaf_cache;
mod limits;
mod m3g_dispatch;
mod m3g_runtime;
#[path = "micro3d/mod.rs"]
mod micro3d_dispatch;
mod nio;
mod pixel_ops;
mod presentation_helpers;
mod program;
mod program_execution;
mod resource_helpers;
#[path = "scheduler/mod.rs"]
mod scheduler;
mod string_array_scan;
mod string_values;
mod telemetry;
mod text_search;
mod three_d_support;
mod values;
mod vm_native;

use call::{CallOutcome, validate_call_arguments};
#[cfg(test)]
use class_state::ConstantPoolRuntimeCache;
use class_state::{ClassState, FieldRuntimeSlots, ResolvedMethodRef, VirtualMethodTarget};
use compatibility::{
    LCDUI_GRAPHICS_DRAW_IMAGE_FNV1A64, LCDUI_GRAPHICS_DRAW_RGB_FNV1A64,
    LCDUI_GRAPHICS_FILL_RECT_FNV1A64, LCDUI_GRAPHICS_SET_COLOR_FNV1A64,
    LCDUI_GRAPHICS_SET_RGB_COLOR_FNV1A64, LCDUI_GRAPHICS_TRANSLATE_FNV1A64,
    check_intrinsic_cancellation, compatibility_intrinsic_candidate, simple_case_unit,
};
use constant_pool::{
    FieldRef, field_key, parse_descriptor_type, parse_method_descriptor, resolve_any_method,
    resolve_class, resolve_field,
};
use continuation::{
    ClassInitializationOutcome, ClassInitializationPhase, NativeResume, PendingCall,
    PendingMonitorEntry, SuspendedCall, SuspendedClassInitialization, SuspendedDriverCall,
    SuspendedDriverWake, suspended_class_initialization, suspended_monitor_entry,
    suspended_native_call, suspended_native_pending_call,
};
use diagnostic_helpers::{
    annotate_resolution, class_for_name_target, display_key, java_error_class, type_error, vm_error,
};
pub use execution::{
    CallTarget, DriverStep, DriverTurn, Execution, InstanceCall, M3gExecutionMetrics, ThreadFailure,
};
use font::{
    lcd_ui_font_dimensions, lcd_ui_small_unicode_column_is_set, lcd_ui_unicode_column_is_set,
    lcd_ui_unicode_raster_height, lcd_ui_unicode_row, system_font_has_glyph, unifont_bmp_glyph,
    unifont_bmp_glyph_width,
};
use frame_storage::FrameStoragePool;
use graphics_dispatch::ImmutableImagePixels;
#[cfg(test)]
use heap_state::FrameRoots;
use heap_state::HeapState;
use heap_values::{
    array_descriptor, array_kind_from_descriptor, array_opcode_error, collect_native_heap,
    default_value, estimate_constant_pool, heap_error, primitive_array_kind, reference_component,
    reject_canonical_string_mutation, require_array_opcode, slot_count, typed_array_access,
    value_reference,
};
use host_context::live_mmapi_handles;
use interpreter_helpers::{
    binary, bitwise, branch, branch_wide, compare, constant_field_value, constant_string_units,
    constant_utf8, constant_value, convert, get_local, get_local_typed, kind_for_return_opcode,
    kind_for_typed_opcode, negate, require_kind, set_local, switch_lookup, switch_table, test_pair,
    test_zero, valid_return_address,
};
use interpreter_loops::{CountedArrayFill, link_counted_array_fills};
use jsr239::Jsr239State;
pub use limits::Limits;
use limits::MAX_LCD_PIXELS;
use m3g_runtime::{M3gGraphicsState, M3gSpritePickContext, M3gState};
use pixel_ops::{
    ShapePainter, argb_to_nokia_pixel, composite_argb_slot, draw_argb_line, nokia_pixel_to_argb,
    paint_argb_pixel, transform_image_pixels,
};
use presentation_helpers::{
    graphics_clip_rectangle, image_axis_origin, image_origin, nokia_direct_transform,
    unicode_text_origin,
};
use resource_helpers::{
    data_input_utf_byte, data_input_utf_length, decode_data_input_utf, fnv1a64,
};
use scheduler::{Monitor, ScheduledJavaTask, SchedulerState, ThreadState};
use string_values::StringValues;
use text_search::two_way_utf16_index;
use three_d_support::{
    command_slice, command_value, jsr239_argb, jsr239_frustum, jsr239_texture_retained_bytes,
    m3g_image_format, m3g_object_class, m3g_png_format, m3g_scaled_sprite_half_extent,
    m3g_sprite_crop_coordinate, m3g_sprite_crop_sample, micro3d_command_scissor,
    micro3d_preserve_translation, micro3d_primitive_payload_counts, normalize_m3g_resource,
};
pub use values::Value;
use values::{
    MethodDescriptor, ValueKind, finite_float_argument, finite_non_negative, float_argument,
    int_argument, java_utf16_case_equal, long_argument, m3g_bounded_count, m3g_bounded_image,
    m3g_non_negative_u32, m3g_non_negative_usize, m3g_positive_usize, native_to_value,
    optional_reference_argument, reference_argument, value_to_native,
};

/// Stable detail attached to VM and Java `OutOfMemoryError` diagnostics when
/// the bounded managed heap, rather than another resource budget, is full.
pub const MANAGED_HEAP_LIMIT_MESSAGE: &str = "managed heap limit exceeded";

/// Diagnostic code reserved for a confirmed failure of the managed heap budget.
pub const MANAGED_HEAP_LIMIT_CODE: &str = "managed-heap-limit";

/// Identifies managed heap failures without interpreting guest exception text.
#[must_use]
pub fn is_managed_heap_limit_error(error: &EmuError) -> bool {
    error.category() == Category::Vm
        && matches!(
            error.code(),
            MANAGED_HEAP_LIMIT_CODE | "managed-heap-profile-change-requested"
        )
}
// A few caught OOMs can be a deliberate fallback. Eight equivalent failures
// inside a compact guest-work window are instead a strong signal that the
// current profile cannot satisfy a retried allocation.
const MANAGED_HEAP_NOTICE_CATCHES: u64 = 8;
const MANAGED_HEAP_NOTICE_WINDOW_INSTRUCTIONS: u64 = 100_000_000;
// At 4-5 MIPS this gives about 7 ms between host polls while amortizing
// continuation save/restore over each worker slice.
const WORKER_QUANTUM: u64 = 32_768;
// Limit clock queries to roughly one per millisecond at the configured rate.
// Newly scheduled tasks request an immediate poll.
const MAX_TIMER_POLL_INSTRUCTIONS: u64 = 256;

fn timer_poll_instruction_interval(instructions_per_second: Option<u64>) -> u64 {
    instructions_per_second.map_or(MAX_TIMER_POLL_INSTRUCTIONS, |rate| {
        (rate / 1_000).clamp(1, MAX_TIMER_POLL_INSTRUCTIONS)
    })
}
const HOST_STACK_RED_ZONE: usize = 512 * 1024;
const HOST_STACK_SEGMENT: usize = 8 * 1024 * 1024;
// Shallow calls have ample room on ordinary host test threads and on the
// explicitly sized emulator worker. Deeper recursive interpreter frames must
// retain stacker's per-frame red-zone check because debug frames can be large.
const DIRECT_HOST_FRAME_DEPTH: usize = 8;
const UNIFONT_BMP: &[u8] = include_bytes!("../../assets/fonts/unifont-bmp.bin");
const UNIFONT_HEADER_SIZE: usize = 16;
const UNIFONT_ENTRY_SIZE: usize = 33;
const JAVA_RANDOM_MULTIPLIER: i64 = 0x0005_deec_e66d;

const MMAPI_HANDLE_FIELDS: [(&str, &str); 4] = [
    (
        "javax/microedition/media/PlayerImpl",
        "javax/microedition/media/PlayerImpl.handle:J",
    ),
    (
        "com/siemens/mp/media/PlayerImpl",
        "com/siemens/mp/media/PlayerImpl.handle:J",
    ),
    (
        "com/nokia/mid/sound/Sound",
        "com/nokia/mid/sound/Sound.handle:J",
    ),
    (
        "com/samsung/util/AudioClip",
        "com/samsung/util/AudioClip.handle:J",
    ),
];

fn host_stack_has_red_zone(remaining: Option<usize>) -> bool {
    remaining.is_some_and(|bytes| bytes >= HOST_STACK_RED_ZONE)
}

/// Runs a VM worker on a stack that `stacker` can track.
/// When pthread stack bounds are unavailable (including on Android), one segment
/// is kept for the worker's lifetime to avoid allocating a segment per Java call.
pub fn with_vm_host_stack<R>(callback: impl FnOnce() -> R) -> R {
    if host_stack_has_red_zone(stacker::remaining_stack()) {
        callback()
    } else {
        stacker::grow(HOST_STACK_SEGMENT, callback)
    }
}

fn build_instruction_index(code_len: usize, instructions: &[Instruction]) -> Vec<u16> {
    let mut index = vec![u16::MAX; code_len];
    for (position, instruction) in instructions.iter().enumerate() {
        if let Some(slot) = index.get_mut(instruction.offset) {
            *slot = u16::try_from(position).unwrap_or(u16::MAX);
        }
    }
    index
}

#[derive(Clone, Copy, Debug)]
struct RuntimeInstructionMeta {
    next_pc: u32,
    operands: [u8; 5],
    opcode: u8,
    batch_opcode: interpreter_batch::Opcode,
    branch_index: u16,
    counted_array_fill: u16,
}

fn build_runtime_instructions(
    instructions: &[Instruction],
    instruction_index: &[u16],
) -> Result<Vec<RuntimeInstructionMeta>, EmuError> {
    let mut runtime = instructions
        .iter()
        .enumerate()
        .map(|(index, instruction)| {
            let next_pc = instruction
                .offset
                .checked_add(1)
                .and_then(|offset| offset.checked_add(instruction.operands.len()))
                .and_then(|offset| u32::try_from(offset).ok())
                .ok_or_else(|| vm_error("memory-limit", "bytecode offset overflow"))?;
            let mut operands = [0; 5];
            for (target, source) in operands.iter_mut().zip(&instruction.operands) {
                *target = *source;
            }
            let mut batch_opcode = interpreter_batch::Opcode::decode(instruction.opcode, operands);
            if let Some(next) = instructions.get(index + 1)
                && next.offset == next_pc as usize
            {
                batch_opcode = batch_opcode.followed_by(next.opcode);
            }
            Ok(RuntimeInstructionMeta {
                next_pc,
                operands,
                opcode: instruction.opcode,
                batch_opcode,
                branch_index: if matches!(instruction.opcode, 0x99..=0xa7 | 0xc6..=0xc7) {
                    branch(instruction.offset, &instruction.operands)
                        .ok()
                        .and_then(|pc| instruction_index.get(pc).copied())
                        .unwrap_or(u16::MAX)
                } else {
                    u16::MAX
                },
                counted_array_fill: 0,
            })
        })
        .collect::<Result<Vec<_>, EmuError>>()?;
    string_array_scan::link(&mut runtime);
    Ok(runtime)
}

#[derive(Clone, Debug)]
// Classfile flags and the independent optimization eligibility are orthogonal.
#[allow(clippy::struct_excessive_bools)]
struct Method {
    key: MethodKey,
    stack_key: Arc<MethodKey>,
    // Program-owned keys avoid an Arc clone/drop on every guest call.
    // Synthetic methods use the owned key instead.
    stack_key_id: Option<usize>,
    // Program-linked methods use a compact per-class constant-pool index.
    // Hand-built test methods leave this unset and simply bypass the cache.
    constant_pool_id: Option<usize>,
    // Guard Rust-owned LCDUI bootstrap implementations against unexpected changes.
    code_fingerprint: u64,
    max_stack: usize,
    max_locals: usize,
    code: Arc<Vec<u8>>,
    instructions: Arc<Vec<Instruction>>,
    // Compact hot-path metadata keeps the interpreter away from the much
    // larger disassembly records and their separately allocated operands.
    runtime_instructions: Arc<Vec<RuntimeInstructionMeta>>,
    instruction_index: Arc<Vec<u16>>,
    constants: Arc<Vec<Option<Constant>>>,
    descriptor: MethodDescriptor,
    is_static: bool,
    is_synchronized: bool,
    exception_table: Arc<Vec<ExceptionHandler>>,
    is_native: bool,
    compiled_integer: Option<usize>,
    readonly_leaf: bool,
    readonly_call_tree: bool,
    compatibility_candidate: Option<bool>,
}

#[derive(Clone, Debug)]
#[allow(clippy::struct_field_names)]
struct Field {
    key: Arc<str>,
    declaring_class: String,
    kind: ValueKind,
    is_static: bool,
    // Shared identity for the declared field and every object-layout slot
    // derived from it. Unlike the symbolic key, checking it does not scan or
    // compare strings on every interpreted getfield/putfield.
    field_token: FieldToken,
    // Superclass-first index used by resolved instance-field bytecodes. Class
    // definitions retain `None`; the lazily linked constant-pool copy receives
    // the slot after full hierarchy resolution.
    instance_slot: Option<usize>,
    initial: Value,
    constant_string: Option<Vec<u16>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NoArgConstructor {
    Missing,
    NonPublic,
    Public,
}

#[derive(Clone, Debug)]
struct Class {
    super_name: Option<String>,
    interfaces: Vec<String>,
    fields: Vec<Field>,
    is_public: bool,
    is_abstract: bool,
    is_interface: bool,
    no_arg_constructor: NoArgConstructor,
}

/// Runtime method table produced from validated class files.
pub struct Program {
    methods: HashMap<MethodKey, Method>,
    integer_methods: Vec<acceleration::PreparedIntegerMethod>,
    stack_keys: Vec<Arc<MethodKey>>,
    classes: HashMap<String, Class>,
    constant_pool_count: usize,
    counted_array_fills: Vec<CountedArrayFill>,
    runtime_bytes: usize,
    natives: NativeRegistry,
}

impl Default for Program {
    fn default() -> Self {
        Self {
            methods: HashMap::new(),
            integer_methods: Vec::new(),
            stack_keys: Vec::new(),
            classes: HashMap::new(),
            constant_pool_count: 0,
            counted_array_fills: Vec::new(),
            runtime_bytes: 0,
            natives: NativeRegistry::new(),
        }
    }
}

struct Machine<'program, 'context> {
    program: &'program Program,
    limits: Limits,
    execution: ExecutionState,
    classes: ClassState,
    heap: HeapState,
    scheduler: SchedulerState,
    graphics: GraphicsState,
    device: DeviceState,
    m3g: M3gState,
    micro3d: Micro3dState,
    jsr239: Jsr239State,
    native_context: &'context mut dyn HostServices,
}

#[derive(Debug, Default)]
struct ExecutionState {
    instructions: u64,
    stack_slots: usize,
    frame_slots: usize,
    trace: execution_trace::ExecutionTrace,
    tracing: bool,
    profiling: bool,
    caught_exceptions: HashMap<(Arc<str>, JavaStackFrame), CaughtExceptionRecord>,
    method_instructions: HashMap<MethodKey, u64>,
    thread_failure_count: u64,
    thread_failures: Vec<ThreadFailure>,
    call_stack: Vec<ActiveJavaStackFrame>,
    frame_storage_pool: std::rc::Rc<std::cell::RefCell<FrameStoragePool>>,
    counters: ExecutionCounters,
    leaf_cache: leaf_cache::LeafCache,
}

#[derive(Debug)]
#[allow(clippy::struct_field_names)]
struct GraphicsState {
    // Reused staging buffer for the active profile's LCD frame.
    present_scratch: Vec<u32>,
    presented_dimensions: Option<(u32, u32)>,
    // Reuse the drawRGB source snapshot across calls.
    draw_rgb_scratch: Vec<i32>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct DeviceState {
    // Nokia DeviceControl levels: display (0), keypad (1). Host visibility is unchanged.
    light_levels: [i32; 2],
    random_seed_sequence: u64,
    // Retain guest state independently of the host actuator's availability.
    last_vibration_request: VibrationRequest,
}

/// Observability-only counters kept out of the interpreter's semantic state.
#[derive(Debug, Default)]
struct ExecutionCounters {
    native_calls: u64,
    arraycopy_calls: u64,
    arraycopy_elements: u64,
    draw_region_calls: u64,
    draw_image_calls: u64,
    draw_rgb_calls: u64,
    fill_rect_calls: u64,
    telemetry_last_instructions: u64,
}

#[derive(Clone, Debug)]
struct CaughtExceptionRecord {
    count: u64,
    managed_heap_burst_count: u64,
    first_instruction: u64,
    last_instruction: u64,
    first_description: String,
    last_description: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Micro3dState {
    runtime: micro3d::Runtime,
    target: Option<Handle>,
    target_scissor: [u32; 4],
    command_scissor: [u32; 4],
    // Micro3D commands are composited into LCDUI only by flush(). Keep the
    // renderer detached between command batches so 2D drawing performed after
    // bind() becomes the background of the next 3D batch.
    render_pending: bool,
    render_diagnostic_calls: u64,
    render_diagnostics: VecDeque<String>,
}

impl Micro3dState {
    fn append_roots(&self, roots: &mut Vec<Handle>) {
        roots.extend(self.target);
    }

    fn sweep(&mut self, heap: &Heap) {
        self.runtime
            .sweep_guest_objects(|reference| heap.get(Handle::from_raw(reference)).is_ok());
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
struct JavaStackFrame {
    key: Arc<MethodKey>,
    bytecode_pc: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ActiveJavaStackKey {
    Linked(usize),
    Owned(Arc<MethodKey>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActiveJavaStackFrame {
    key: ActiveJavaStackKey,
    bytecode_pc: usize,
}

impl Method {
    fn active_stack_frame(&self, bytecode_pc: usize) -> ActiveJavaStackFrame {
        let key = self.stack_key_id.map_or_else(
            || ActiveJavaStackKey::Owned(Arc::clone(&self.stack_key)),
            ActiveJavaStackKey::Linked,
        );
        ActiveJavaStackFrame { key, bytecode_pc }
    }
}

impl Program {
    fn active_stack_key<'a>(&'a self, frame: &'a ActiveJavaStackFrame) -> &'a Arc<MethodKey> {
        match &frame.key {
            ActiveJavaStackKey::Linked(index) => &self.stack_keys[*index],
            ActiveJavaStackKey::Owned(key) => key,
        }
    }
}

struct DefaultNativeContext;
struct MachineNativeContext<'a> {
    host: &'a mut dyn HostServices,
    heap: &'a mut HeapState,
    m3g: &'a mut M3gState,
    micro3d: &'a mut Micro3dState,
    jsr239: &'a Jsr239State,
    classes: &'a mut ClassState,
    program: &'a Program,
    execution: &'a ExecutionState,
    scheduler: &'a mut SchedulerState,
    arguments: &'a [Value],
}

#[cfg(test)]
#[path = "../../../../tests/unit/vm/machine/mod.rs"]
mod tests;
