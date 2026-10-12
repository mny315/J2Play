//! EGL wrapper objects and the current LCDUI rendering surface.

use super::super::{
    Allocation, ArrayKind, CallOutcome, EmuError, HeapValue, Machine, Value, heap_error,
    int_argument, optional_reference_argument, reference_argument, type_error, vm_error,
};

impl Machine<'_, '_> {
    #[allow(clippy::too_many_lines)]
    pub(in crate::machine) fn invoke_egl_native(
        &mut self,
        name: &str,
        descriptor: &str,
        args: &[Value],
    ) -> Result<CallOutcome, EmuError> {
        match (name, descriptor) {
            ("getEGL", "()Ljavax/microedition/khronos/egl/EGL;") => {
                let handle = if let Some(handle) = self.jsr239.egl {
                    handle
                } else {
                    let handle = self
                        .allocate_native_instance("javax/microedition/khronos/egl/EGLImpl", args)?;
                    self.jsr239.egl = Some(handle);
                    handle
                };
                Ok(CallOutcome::Return(Some(Value::Reference(Some(handle)))))
            }
            ("getGL", "()Ljavax/microedition/khronos/opengles/GL;") => {
                let handle = if let Some(handle) = self.jsr239.gl {
                    handle
                } else {
                    let handle = self.allocate_native_instance(
                        "javax/microedition/khronos/opengles/GLImpl",
                        args,
                    )?;
                    self.jsr239.gl = Some(handle);
                    handle
                };
                Ok(CallOutcome::Return(Some(Value::Reference(Some(handle)))))
            }
            ("eglGetDisplay", _) => {
                self.jsr239_new_handle("javax/microedition/khronos/egl/EGLDisplay", args)
            }
            ("eglInitialize", _) => {
                if let Some(versions) = optional_reference_argument(args, 2)? {
                    let versions = self.graphics_int_array_mut(versions)?;
                    let destination = versions.get_mut(..2).ok_or_else(|| {
                        vm_error(
                            "illegal-argument-exception",
                            "EGL version array is too short",
                        )
                    })?;
                    destination.fill(HeapValue::Int(1));
                }
                Ok(CallOutcome::Return(Some(Value::Int(1))))
            }
            ("eglGetConfigs", _) | ("eglChooseConfig", _) => {
                let (configs_index, size_index, count_index) = if name == "eglGetConfigs" {
                    (2, 3, 4)
                } else {
                    (3, 4, 5)
                };
                let count = reference_argument(args, count_index)?;
                if self.graphics_int_array_mut(count)?.is_empty() {
                    return Err(vm_error(
                        "illegal-argument-exception",
                        "EGL config count array is empty",
                    ));
                }
                let configs = optional_reference_argument(args, configs_index)?;
                let size = int_argument(args, size_index)?;
                if let Some(configs) = configs {
                    let Allocation::Array {
                        kind: ArrayKind::Reference(_),
                        elements,
                    } = self.heap.managed.get(configs).map_err(heap_error)?
                    else {
                        return Err(type_error());
                    };
                    if !usize::try_from(size).is_ok_and(|size| size <= elements.len()) {
                        return Err(vm_error(
                            "illegal-argument-exception",
                            "EGL config destination range is invalid",
                        ));
                    }
                }
                let returned = i32::from(configs.is_none() || size > 0);
                if let Some(configs) = configs.filter(|_| size > 0) {
                    let config = self.allocate_native_instance(
                        "javax/microedition/khronos/egl/EGLConfig",
                        args,
                    )?;
                    self.heap
                        .managed
                        .array_set(configs, 0, HeapValue::Reference(Some(config)))
                        .map_err(heap_error)?;
                }
                self.heap
                    .managed
                    .array_set(count, 0, HeapValue::Int(returned))
                    .map_err(heap_error)?;
                Ok(CallOutcome::Return(Some(Value::Int(1))))
            }
            ("eglCreateContext", _) => {
                self.jsr239_new_handle("javax/microedition/khronos/egl/EGLContext", args)
            }
            ("eglCreateWindowSurface", _) => {
                let invalid_window = || {
                    vm_error(
                        "illegal-argument-exception",
                        "EGL window requires Graphics from Canvas or GameCanvas",
                    )
                };
                let graphics = optional_reference_argument(args, 3)?.ok_or_else(invalid_window)?;
                if !self.program.is_assignable_to(
                    &self.object_class(graphics)?,
                    "javax/microedition/lcdui/Graphics",
                ) {
                    return Err(invalid_window());
                }
                let HeapValue::Reference(Some(target)) = self.heap.managed.field(
                    graphics,
                    "javax/microedition/lcdui/Graphics.canvas:Ljavax/microedition/lcdui/Canvas;",
                ).map_err(heap_error)? else {
                    return Err(invalid_window());
                };
                let surface = self
                    .allocate_native_instance("javax/microedition/khronos/egl/EGLSurface", args)?;
                self.heap
                    .managed
                    .set_field(
                        surface,
                        "javax/microedition/khronos/egl/EGLSurface.target:Ljava/lang/Object;",
                        HeapValue::Reference(Some(target)),
                    )
                    .map_err(heap_error)?;
                Ok(CallOutcome::Return(Some(Value::Reference(Some(surface)))))
            }
            ("eglMakeCurrent", _) => {
                self.jsr239.target = optional_reference_argument(args, 2)?
                    .map(|surface| {
                        self.graphics_reference_field(
                            surface,
                            "javax/microedition/khronos/egl/EGLSurface.target:Ljava/lang/Object;",
                        )
                    })
                    .transpose()?;
                Ok(CallOutcome::Return(Some(Value::Int(1))))
            }
            ("eglTerminate", _) | ("eglWaitGL", _) | ("eglWaitNative", _) => {
                Ok(CallOutcome::Return(Some(Value::Int(1))))
            }
            _ => Err(vm_error("method-not-found", format!("{name}{descriptor}"))),
        }
    }

    pub(in crate::machine) fn jsr239_new_handle(
        &mut self,
        class: &str,
        args: &[Value],
    ) -> Result<CallOutcome, EmuError> {
        let handle = self.allocate_native_instance(class, args)?;
        Ok(CallOutcome::Return(Some(Value::Reference(Some(handle)))))
    }
}
