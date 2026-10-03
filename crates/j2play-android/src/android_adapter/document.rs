use super::{
    Activity, Arc, AtomicBool, Env, Global, JObject, JValue, JavaVM, Mutex, Ordering, Result, call,
    lock, message, new, platform_error, string, text,
};
use frontend_ui::{DocumentKind, DocumentOutcome, PickedDocument};
use std::sync::Condvar;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(30);
const JAR_REQUEST: i32 = 4101;
const JAD_REQUEST: i32 = 4102;

#[derive(Default)]
pub(super) struct Documents {
    selected: Option<DocumentKind>,
    job: Option<Job>,
}

struct Operation {
    cancel: Global<JObject<'static>>,
    descriptor: Mutex<Option<Arc<Global<JObject<'static>>>>>,
    completed: (Mutex<bool>, Condvar),
    aborted: AtomicBool,
    deadline: Instant,
}

struct Job {
    operation: Arc<Operation>,
    worker: Option<JoinHandle<()>>,
    timeout: Option<JoinHandle<()>>,
}

impl Operation {
    fn request_cancel(&self) {
        let _completed = lock(&self.completed.0);
        self.aborted.store(true, Ordering::Release);
        self.completed.1.notify_all();
    }

    fn cancel(&self, env: &mut Env<'_>) {
        self.aborted.store(true, Ordering::Release);
        let _ = call(env, &self.cancel, "cancel", "()V", &[]);
        env.exception_clear();
        if let Some(descriptor) = lock(&self.descriptor).clone() {
            let _ = call(env, &descriptor, "close", "()V", &[]);
            env.exception_clear();
        }
    }

    fn check(&self) -> Result<()> {
        if self.aborted.load(Ordering::Acquire) || Instant::now() >= self.deadline {
            Err(message("Document copy timed out or was cancelled"))
        } else {
            Ok(())
        }
    }
}

impl Job {
    fn finished(&self) -> bool {
        self.worker.as_ref().is_none_or(JoinHandle::is_finished)
            && self.timeout.as_ref().is_none_or(JoinHandle::is_finished)
    }
    fn join(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        if let Some(timeout) = self.timeout.take() {
            let _ = timeout.join();
        }
    }
}

fn request(kind: DocumentKind) -> i32 {
    match kind {
        DocumentKind::Jar => JAR_REQUEST,
        DocumentKind::Jad => JAD_REQUEST,
    }
}
fn kind(request: i32) -> Option<DocumentKind> {
    match request {
        JAR_REQUEST => Some(DocumentKind::Jar),
        JAD_REQUEST => Some(DocumentKind::Jad),
        _ => None,
    }
}

pub(super) fn pick(env: &mut Env<'_>, state: &Arc<Activity>, kind: DocumentKind) {
    let result = pick_inner(env, state, kind);
    if let Err(error) = result {
        env.exception_clear();
        state
            .bridge
            .document_result(Err(platform_error("document-picker", error.to_string())));
    }
}

fn pick_inner(env: &mut Env<'_>, state: &Arc<Activity>, kind: DocumentKind) -> Result<()> {
    {
        let mut documents = lock(&state.document);
        if documents.selected.is_some() {
            return Err(message("Another document selection is already active"));
        }
        if let Some(job) = &mut documents.job {
            if !job.finished() {
                return Err(message("The previous document operation is still stopping"));
            }
            job.join();
        }
        documents.job = None;
    }
    let action = string(env, "android.intent.action.OPEN_DOCUMENT")?;
    let intent = new(
        env,
        "android/content/Intent",
        "(Ljava/lang/String;)V",
        &[JValue::Object(&action)],
    )?;
    let category = string(env, "android.intent.category.OPENABLE")?;
    call(
        env,
        &intent,
        "addCategory",
        "(Ljava/lang/String;)Landroid/content/Intent;",
        &[JValue::Object(&category)],
    )?;
    call(
        env,
        &intent,
        "addFlags",
        "(I)Landroid/content/Intent;",
        &[JValue::Int(1 | 64)],
    )?;
    let types: &[&str] = match kind {
        DocumentKind::Jar => &[
            "application/java-archive",
            "application/x-java-archive",
            "application/x-jar",
            "application/jar",
            "application/java",
            "application/x-java",
            "application/j2me-archive",
            "application/zip",
            "application/x-zip-compressed",
            "application/octet-stream",
        ],
        DocumentKind::Jad => &[
            "text/vnd.sun.j2me.app-descriptor",
            "text/plain",
            "application/octet-stream",
        ],
    };
    let primary = string(env, types[0])?;
    call(
        env,
        &intent,
        "setType",
        "(Ljava/lang/String;)Landroid/content/Intent;",
        &[JValue::Object(&primary)],
    )?;
    let mime_types = env.new_object_array(
        i32::try_from(types.len()).map_err(|_| message("Too many MIME types"))?,
        jni::jni_str!("java/lang/String"),
        JObject::null(),
    )?;
    for (index, mime) in types.iter().enumerate() {
        let mime = string(env, mime)?;
        mime_types.set_element(env, index, &mime)?;
    }
    let extra = string(env, "android.intent.extra.MIME_TYPES")?;
    call(
        env,
        &intent,
        "putExtra",
        "(Ljava/lang/String;[Ljava/lang/String;)Landroid/content/Intent;",
        &[JValue::Object(&extra), JValue::Object(&mime_types)],
    )?;
    call(
        env,
        &state.object,
        "startActivityForResult",
        "(Landroid/content/Intent;I)V",
        &[JValue::Object(&intent), JValue::Int(request(kind))],
    )?;
    lock(&state.document).selected = Some(kind);
    Ok(())
}

pub(super) fn save(env: &mut Env<'_>, state: &Activity, saved: &JObject<'_>) -> Result<()> {
    let key = string(env, "j2play.document_request")?;
    let value = lock(&state.document).selected.map_or(-1, request);
    call(
        env,
        saved,
        "putInt",
        "(Ljava/lang/String;I)V",
        &[JValue::Object(&key), JValue::Int(value)],
    )?;
    Ok(())
}

pub(super) fn restore(env: &mut Env<'_>, state: &Activity, saved: &JObject<'_>) -> Result<()> {
    if saved.is_null() {
        return Ok(());
    }
    let key = string(env, "j2play.document_request")?;
    let request = call(
        env,
        saved,
        "getInt",
        "(Ljava/lang/String;I)I",
        &[JValue::Object(&key), JValue::Int(-1)],
    )?
    .i()?;
    let selected = kind(request);
    lock(&state.document).selected = selected;
    state
        .bridge
        .document_busy
        .store(selected.is_some(), Ordering::Release);
    Ok(())
}

pub(super) fn result(
    env: &mut Env<'_>,
    state: &Arc<Activity>,
    request: i32,
    result: i32,
    data: &JObject<'_>,
) {
    let Some(kind) = kind(request) else {
        return;
    };
    {
        let mut documents = lock(&state.document);
        if documents.selected != Some(kind) {
            return;
        }
        documents.selected = None;
    }
    if result != -1 || data.is_null() {
        state
            .bridge
            .document_result(Ok(DocumentOutcome::Cancelled { kind }));
        return;
    }
    let result = (|| {
        let uri = call(env, data, "getData", "()Landroid/net/Uri;", &[])?.l()?;
        if uri.is_null() {
            return Err(message("Document provider returned no selected document"));
        }
        start(env, state, kind, &uri)
    })();
    if let Err(error) = result {
        env.exception_clear();
        state
            .bridge
            .document_result(Err(platform_error("document-copy", error.to_string())));
    }
}

pub(super) fn start(
    env: &mut Env<'_>,
    state: &Arc<Activity>,
    kind: DocumentKind,
    uri: &JObject<'_>,
) -> Result<()> {
    {
        let mut documents = lock(&state.document);
        if let Some(job) = &mut documents.job {
            if !job.finished() {
                return Err(message("The previous document operation is still stopping"));
            }
            job.join();
        }
        documents.job = None;
    }
    let uri = env.new_global_ref(uri)?;
    let cancellation = new(env, "android/os/CancellationSignal", "()V", &[])?;
    let operation = Arc::new(Operation {
        cancel: env.new_global_ref(cancellation)?,
        descriptor: Mutex::new(None),
        completed: (Mutex::new(false), Condvar::new()),
        aborted: AtomicBool::new(false),
        deadline: Instant::now() + TIMEOUT,
    });
    let generation = state.bridge.begin_document_copy();
    // Arm cancellation before starting any provider operation, including open().
    let timeout_state = Arc::downgrade(state);
    let timeout_operation = Arc::clone(&operation);
    let timeout = thread::Builder::new()
        .name("j2play-document-deadline".into())
        .spawn(move || {
            let (done, condition) = &timeout_operation.completed;
            let (done, _) = condition
                .wait_timeout_while(lock(done), TIMEOUT, |done| {
                    !*done && !timeout_operation.aborted.load(Ordering::Acquire)
                })
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if *done {
                return;
            }
            drop(done);
            timeout_operation.aborted.store(true, Ordering::Release);
            if let Some(state) = timeout_state.upgrade() {
                publish(&state, generation, Err(message("Document copy timed out")));
            }
            let _ = JavaVM::singleton().and_then(|vm| {
                vm.attach_current_thread(|env| -> jni::errors::Result<()> {
                    timeout_operation.cancel(env);
                    Ok(())
                })
            });
        })?;
    let worker_state = Arc::clone(state);
    let worker_operation = Arc::clone(&operation);
    let worker = thread::Builder::new()
        .name("j2play-document-copy".into())
        .spawn(move || {
            let result = (|| {
                JavaVM::singleton()?.attach_current_thread(|env| -> Result<DocumentOutcome> {
                    let result = read(env, &worker_state, kind, &uri, &worker_operation);
                    env.exception_clear();
                    if let Some(descriptor) = lock(&worker_operation.descriptor).take() {
                        let _ = call(env, &descriptor, "close", "()V", &[]);
                        env.exception_clear();
                    }
                    result
                })
            })();
            if !worker_operation.aborted.load(Ordering::Acquire) {
                publish(&worker_state, generation, result);
            }
            *lock(&worker_operation.completed.0) = true;
            worker_operation.completed.1.notify_all();
        });
    match worker {
        Ok(worker) => {
            lock(&state.document).job = Some(Job {
                operation,
                worker: Some(worker),
                timeout: Some(timeout),
            });
        }
        Err(error) => {
            *lock(&operation.completed.0) = true;
            operation.completed.1.notify_all();
            let _ = timeout.join();
            return Err(error.into());
        }
    }
    Ok(())
}

fn publish(state: &Activity, generation: u64, result: Result<DocumentOutcome>) {
    state.bridge.complete_document_copy(
        generation,
        result.map_err(|error| platform_error("document-copy", error.to_string())),
    );
}

fn read(
    env: &mut Env<'_>,
    state: &Activity,
    kind: DocumentKind,
    uri: &JObject<'_>,
    operation: &Operation,
) -> Result<DocumentOutcome> {
    let maximum: usize = match kind {
        DocumentKind::Jar => 64 * 1024 * 1024,
        DocumentKind::Jad => 1024 * 1024,
    };
    operation.check()?;
    let resolver = call(
        env,
        &state.object,
        "getContentResolver",
        "()Landroid/content/ContentResolver;",
        &[],
    )?
    .l()?;
    let mode = string(env, "r")?;
    let descriptor = call(env, &resolver, "openAssetFileDescriptor", "(Landroid/net/Uri;Ljava/lang/String;Landroid/os/CancellationSignal;)Landroid/content/res/AssetFileDescriptor;", &[JValue::Object(uri), JValue::Object(&mode), JValue::Object(&operation.cancel)])?.l()?;
    if descriptor.is_null() {
        return Err(message("Document provider returned no readable stream"));
    }
    *lock(&operation.descriptor) = Some(Arc::new(env.new_global_ref(&descriptor)?));
    operation.check()?;
    let length = call(env, &descriptor, "getLength", "()J", &[])?.j()?;
    if length > i64::try_from(maximum).unwrap_or(i64::MAX) {
        return Err(message("Selected document exceeds the import size limit"));
    }
    let input = call(
        env,
        &descriptor,
        "createInputStream",
        "()Ljava/io/FileInputStream;",
        &[],
    )?
    .l()?;
    let buffer = env.new_byte_array(32 * 1024)?;
    let mut scratch = vec![0_i8; 32 * 1024];
    let mut bytes = Vec::new();
    loop {
        operation.check()?;
        let count = env.with_local_frame(16, |env| -> Result<i32> {
            Ok(call(env, &input, "read", "([B)I", &[JValue::Object(&buffer)])?.i()?)
        })?;
        if count == -1 {
            break;
        }
        let count = usize::try_from(count).map_err(|_| message("Invalid document read length"))?;
        if count > scratch.len() || count > maximum - bytes.len() {
            return Err(message("Selected document exceeds the import size limit"));
        }
        if count == 0 {
            thread::sleep(Duration::from_millis(1));
            continue;
        }
        buffer.get_region(env, 0, &mut scratch[..count])?;
        bytes
            .try_reserve(count)
            .map_err(|_| message("Cannot allocate document import buffer"))?;
        bytes.extend(scratch[..count].iter().map(|byte| byte.to_ne_bytes()[0]));
    }
    call(env, &input, "close", "()V", &[])?;
    operation.check()?;
    let mut display_name =
        display_name(env, &resolver, uri, &operation.cancel).unwrap_or_else(|_| {
            env.exception_clear();
            None
        });
    if display_name.is_none() {
        display_name = file_name(env, uri).unwrap_or_else(|_| {
            env.exception_clear();
            None
        });
    }
    operation.check()?;
    Ok(DocumentOutcome::Selected(PickedDocument {
        kind,
        display_name,
        bytes,
    }))
}

fn display_name(
    env: &mut Env<'_>,
    resolver: &JObject<'_>,
    uri: &JObject<'_>,
    cancellation: &JObject<'_>,
) -> Result<Option<String>> {
    let name = string(env, "_display_name")?;
    let projection = env.new_object_array(1, jni::jni_str!("java/lang/String"), &name)?;
    let cursor = call(env, resolver, "query", "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;Landroid/os/CancellationSignal;)Landroid/database/Cursor;", &[JValue::Object(uri), JValue::Object(&projection), JValue::Object(&JObject::null()), JValue::Object(&JObject::null()), JValue::Object(&JObject::null()), JValue::Object(cancellation)])?.l()?;
    if cursor.is_null() {
        return Ok(None);
    }
    let result = (|| {
        if !call(env, &cursor, "moveToFirst", "()Z", &[])?.z()? {
            return Ok(None);
        }
        let column = call(
            env,
            &cursor,
            "getColumnIndex",
            "(Ljava/lang/String;)I",
            &[JValue::Object(&name)],
        )?
        .i()?;
        if column < 0 {
            return Ok(None);
        }
        let value = call(
            env,
            &cursor,
            "getString",
            "(I)Ljava/lang/String;",
            &[JValue::Int(column)],
        )?
        .l()?;
        if value.is_null() {
            return Ok(None);
        }
        let value = text(env, &value, 512)?;
        Ok((!value.contains('\0')).then_some(value))
    })();
    env.exception_clear();
    let _ = call(env, &cursor, "close", "()V", &[]);
    result
}

fn file_name(env: &mut Env<'_>, uri: &JObject<'_>) -> Result<Option<String>> {
    let scheme = call(env, uri, "getScheme", "()Ljava/lang/String;", &[])?.l()?;
    if scheme.is_null() || text(env, &scheme, 32)? != "file" {
        return Ok(None);
    }
    let leaf = call(env, uri, "getLastPathSegment", "()Ljava/lang/String;", &[])?.l()?;
    if leaf.is_null() {
        return Ok(None);
    }
    let leaf = text(env, &leaf, 512)?;
    Ok((!leaf.is_empty()
        && !leaf
            .chars()
            .any(|c| c.is_control() || c == '/' || c == '\\'))
    .then_some(leaf))
}

pub(super) fn cancel(state: &Activity) -> Result<()> {
    let operation = lock(&state.document)
        .job
        .as_ref()
        .map(|job| Arc::clone(&job.operation));
    if let Some(operation) = operation {
        // CancellationSignal can itself call into a stalled provider. The
        // owned deadline thread performs that SDK call; the UI only signals it.
        operation.request_cancel();
        let deadline = Instant::now() + Duration::from_millis(1500);
        loop {
            let finished = lock(&state.document).job.as_ref().is_none_or(Job::finished);
            if finished {
                break;
            }
            if Instant::now() >= deadline {
                return Err(message(
                    "Android document worker did not stop within its deadline",
                ));
            }
            thread::sleep(Duration::from_millis(5));
        }
        if let Some(mut job) = lock(&state.document).job.take() {
            job.join();
        }
    }
    Ok(())
}
