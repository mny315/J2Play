use super::{
    Allocation, ArrayKind, CallOutcome, EmuError, Handle, HeapValue, Machine, Method, NativeResume,
    Value, display_key, heap_error, int_argument, reference_argument,
    suspended_native_pending_call, type_error, vm_error,
};
use natives::CharacterEncoding;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct ReaderState {
    reader: Handle,
    input: Handle,
    decoder: ReaderDecoder,
    target: Option<ReaderTarget>,
}

impl ReaderState {
    pub(super) fn append_roots(&self, roots: &mut Vec<Handle>) {
        roots.extend([self.reader, self.input]);
        roots.extend(self.target.as_ref().map(|target| target.array));
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ReaderTarget {
    array: Handle,
    range: std::ops::Range<usize>,
    decoded: Vec<u16>,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
enum ReaderDecoder {
    Start,
    Utf16 {
        first: i32,
        encoding: i32,
    },
    Utf8 {
        remaining: u8,
        minimum: i32,
        codepoint: i32,
    },
}

impl Machine<'_, '_> {
    pub(super) fn input_stream_reader_init(
        &mut self,
        args: &[Value],
        encoding_argument: Option<usize>,
    ) -> Result<(), EmuError> {
        const UTF8: i32 = 0;
        const LATIN1: i32 = 1;
        const ASCII: i32 = 2;
        const UTF16_BE: i32 = 3;
        const UTF16_LE: i32 = 4;
        const UTF16: i32 = 5;

        let reader = reference_argument(args, 0)?;
        let input = reference_argument(args, 1)?;
        let encoding = {
            let name = if let Some(index) = encoding_argument {
                let name = reference_argument(args, index)?;
                self.heap
                    .string_values
                    .get(&name)
                    .map(|units| String::from_utf16_lossy(units))
                    .ok_or_else(|| vm_error("type-mismatch", "encoding name must be a String"))?
            } else {
                self.native_context
                    .system_property("microedition.encoding")
                    .unwrap_or("UTF-8")
                    .to_owned()
            };
            match CharacterEncoding::for_name(&name) {
                Some(CharacterEncoding::Utf8) => UTF8,
                Some(CharacterEncoding::Latin1) => LATIN1,
                Some(CharacterEncoding::Ascii) => ASCII,
                Some(CharacterEncoding::Utf16Be) => UTF16_BE,
                Some(CharacterEncoding::Utf16Le) => UTF16_LE,
                Some(CharacterEncoding::Utf16) => UTF16,
                None => return Err(vm_error("unsupported-encoding", name)),
            }
        };
        for (field, value) in [
            (
                "java/io/Reader.lock:Ljava/lang/Object;",
                HeapValue::Reference(Some(reader)),
            ),
            (
                "java/io/InputStreamReader.in:Ljava/io/InputStream;",
                HeapValue::Reference(Some(input)),
            ),
            (
                "java/io/InputStreamReader.encoding:I",
                HeapValue::Int(encoding),
            ),
            ("java/io/InputStreamReader.pending:I", HeapValue::Int(0)),
            ("java/io/InputStreamReader.closed:Z", HeapValue::Int(0)),
        ] {
            self.heap
                .managed
                .set_field(reader, field, value)
                .map_err(heap_error)?;
        }
        Ok(())
    }

    pub(super) fn input_stream_reader_read(
        &mut self,
        method: &Method,
        args: &[Value],
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        let reader = reference_argument(args, 0)?;
        let target = if method.key.descriptor == "([CII)I" {
            let array = reference_argument(args, 1)?;
            let offset = int_argument(args, 2)?;
            let length = int_argument(args, 3)?;
            let Allocation::Array {
                kind: ArrayKind::Char,
                elements,
            } = self.heap.managed.get(array).map_err(heap_error)?
            else {
                return Err(type_error());
            };
            let end = offset.checked_add(length);
            if offset < 0
                || length < 0
                || end
                    .is_none_or(|end| usize::try_from(end).map_or(true, |end| end > elements.len()))
            {
                return Err(vm_error(
                    "array-index-out-of-bounds-exception",
                    "InputStreamReader destination range is invalid",
                ));
            }
            if length == 0 {
                return Ok(CallOutcome::Return(Some(Value::Int(0))));
            }
            let length = usize::try_from(length).map_err(|_| type_error())?;
            let offset = usize::try_from(offset).map_err(|_| type_error())?;
            let mut decoded = Vec::new();
            decoded.try_reserve_exact(length).map_err(|_| {
                vm_error(
                    "resource-limit",
                    "cannot allocate the character read buffer",
                )
            })?;
            Some(ReaderTarget {
                array,
                range: offset..offset + length,
                decoded,
            })
        } else {
            None
        };
        let input = self.graphics_reference_field(
            reader,
            "java/io/InputStreamReader.in:Ljava/io/InputStream;",
        )?;
        self.continue_input_stream_reader_read(
            method,
            ReaderState {
                reader,
                input,
                decoder: ReaderDecoder::Start,
                target,
            },
            None,
            depth,
        )
    }

    pub(super) fn resume_input_stream_reader_read(
        &mut self,
        method: &Method,
        state: ReaderState,
        child: Box<super::SuspendedCall>,
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        match self.resume_suspended_call(child, depth + 1)? {
            CallOutcome::Return(Some(Value::Int(byte))) => {
                self.continue_input_stream_reader_read(method, state, Some(byte), depth)
            }
            CallOutcome::Return(_) => Err(type_error()),
            CallOutcome::Throw(exception) => {
                self.finish_input_stream_reader_read(&state, Some(exception))
            }
            CallOutcome::Suspend(child) => Ok(suspended_native_pending_call(
                method,
                NativeResume::InputStreamReaderRead(state),
                child,
            )),
        }
    }

    fn continue_input_stream_reader_read(
        &mut self,
        method: &Method,
        mut state: ReaderState,
        mut resumed_byte: Option<i32>,
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        let mut work = 0_usize;
        loop {
            if work.is_multiple_of(1_024) && self.native_context.execution_cancelled() {
                return Err(vm_error("execution-cancelled", "character read cancelled"));
            }
            work += 1;
            if self.graphics_int_field(state.reader, "java/io/InputStreamReader.closed:Z")? != 0 {
                return Err(vm_error("reader-io", "InputStreamReader is closed"));
            }
            if resumed_byte.is_none() && matches!(state.decoder, ReaderDecoder::Start) {
                let pending =
                    self.graphics_int_field(state.reader, "java/io/InputStreamReader.pending:I")?;
                if !(-256..=0xffff).contains(&pending) {
                    return Err(vm_error(
                        "reader-io",
                        "InputStreamReader contains an invalid pending character",
                    ));
                }
                if pending != 0 {
                    self.heap
                        .managed
                        .set_field(
                            state.reader,
                            "java/io/InputStreamReader.pending:I",
                            HeapValue::Int(0),
                        )
                        .map_err(heap_error)?;
                    if pending > 0 {
                        if let Some(outcome) = self.accept_reader_unit(&mut state, pending)? {
                            return Ok(outcome);
                        }
                        continue;
                    }
                    resumed_byte = Some(-pending - 1);
                }
            }
            let byte = if let Some(byte) = resumed_byte.take() {
                byte
            } else {
                match self.call_int_virtual(state.input, "read", "()I", depth)? {
                    Ok(byte) => byte,
                    Err(CallOutcome::Throw(exception)) => {
                        return self.finish_input_stream_reader_read(&state, Some(exception));
                    }
                    Err(CallOutcome::Suspend(child)) => {
                        return Ok(suspended_native_pending_call(
                            method,
                            NativeResume::InputStreamReaderRead(state),
                            child,
                        ));
                    }
                    Err(CallOutcome::Return(_)) => return Err(type_error()),
                }
            };
            if !(-1..=255).contains(&byte) {
                return Err(vm_error(
                    "reader-io",
                    "InputStream.read returned a value outside -1..=255",
                ));
            }
            if let Some(unit) = self.decode_reader_byte(state.reader, &mut state.decoder, byte)?
                && let Some(outcome) = self.accept_reader_unit(&mut state, unit)?
            {
                return Ok(outcome);
            }
        }
    }

    fn accept_reader_unit(
        &mut self,
        state: &mut ReaderState,
        unit: i32,
    ) -> Result<Option<CallOutcome>, EmuError> {
        state.decoder = ReaderDecoder::Start;
        let Some(target) = &mut state.target else {
            return Ok(Some(CallOutcome::Return(Some(Value::Int(unit)))));
        };
        if unit >= 0 {
            target
                .decoded
                .push(u16::try_from(unit).map_err(|_| type_error())?);
        }
        if unit < 0 || target.decoded.len() == target.range.len() {
            return self.finish_input_stream_reader_read(state, None).map(Some);
        }
        Ok(None)
    }

    fn finish_input_stream_reader_read(
        &mut self,
        state: &ReaderState,
        exception: Option<Handle>,
    ) -> Result<CallOutcome, EmuError> {
        if let Some(exception) = exception
            && (state
                .target
                .as_ref()
                .is_none_or(|target| target.decoded.is_empty())
                || !self.is_instance(&self.object_class(exception)?, "java/io/IOException"))
        {
            // Only an I/O failure can end a successful partial read. Guest
            // runtime exceptions and errors must survive the native wrapper.
            return Ok(CallOutcome::Throw(exception));
        }
        let target = state.target.as_ref().ok_or_else(type_error)?;
        if target.decoded.is_empty() {
            return Ok(CallOutcome::Return(Some(Value::Int(-1))));
        }
        let Allocation::Array {
            kind: ArrayKind::Char,
            elements,
        } = self
            .heap
            .managed
            .get_mut(target.array)
            .map_err(heap_error)?
        else {
            return Err(type_error());
        };
        let destination = elements
            .get_mut(target.range.clone())
            .ok_or_else(type_error)?;
        for (slot, &unit) in destination.iter_mut().zip(&target.decoded) {
            *slot = HeapValue::Int(i32::from(unit));
        }
        Ok(CallOutcome::Return(Some(Value::Int(
            i32::try_from(target.decoded.len()).map_err(|_| type_error())?,
        ))))
    }

    fn decode_reader_byte(
        &mut self,
        reader: Handle,
        decoder: &mut ReaderDecoder,
        byte: i32,
    ) -> Result<Option<i32>, EmuError> {
        const REPLACEMENT: i32 = 0xfffd;
        match *decoder {
            ReaderDecoder::Start => {
                if byte < 0 {
                    return Ok(Some(-1));
                }
                let encoding =
                    self.graphics_int_field(reader, "java/io/InputStreamReader.encoding:I")?;
                match encoding {
                    1 => return Ok(Some(byte)),
                    2 => return Ok(Some(if byte < 0x80 { byte } else { REPLACEMENT })),
                    3..=5 => {
                        *decoder = ReaderDecoder::Utf16 {
                            first: byte,
                            encoding,
                        }
                    }
                    0 => {
                        if byte < 0x80 {
                            return Ok(Some(byte));
                        }
                        let (remaining, minimum, codepoint) = match byte {
                            0xc2..=0xdf => (1, 0x80, byte & 0x1f),
                            0xe0..=0xef => (2, 0x800, byte & 0x0f),
                            0xf0..=0xf4 => (3, 0x1_0000, byte & 0x07),
                            _ => return Ok(Some(REPLACEMENT)),
                        };
                        *decoder = ReaderDecoder::Utf8 {
                            remaining,
                            minimum,
                            codepoint,
                        };
                    }
                    _ => {
                        return Err(vm_error(
                            "unsupported-encoding",
                            "invalid InputStreamReader encoding state",
                        ));
                    }
                }
                Ok(None)
            }
            ReaderDecoder::Utf16 { first, encoding } => {
                if byte < 0 {
                    return Ok(Some(REPLACEMENT));
                }
                let big_endian = (first << 8) | byte;
                if encoding == 5 {
                    let selected = if big_endian == 0xfffe { 4 } else { 3 };
                    self.heap
                        .managed
                        .set_field(
                            reader,
                            "java/io/InputStreamReader.encoding:I",
                            HeapValue::Int(selected),
                        )
                        .map_err(heap_error)?;
                    if matches!(big_endian, 0xfeff | 0xfffe) {
                        *decoder = ReaderDecoder::Start;
                        return Ok(None);
                    }
                }
                Ok(Some(if encoding == 4 {
                    (byte << 8) | first
                } else {
                    big_endian
                }))
            }
            ReaderDecoder::Utf8 {
                remaining,
                minimum,
                codepoint,
            } => {
                if !(0x80..=0xbf).contains(&byte) {
                    if byte >= 0 {
                        self.heap
                            .managed
                            .set_field(
                                reader,
                                "java/io/InputStreamReader.pending:I",
                                HeapValue::Int(-byte - 1),
                            )
                            .map_err(heap_error)?;
                    }
                    return Ok(Some(REPLACEMENT));
                }
                let codepoint = (codepoint << 6) | (byte & 0x3f);
                if remaining > 1 {
                    *decoder = ReaderDecoder::Utf8 {
                        remaining: remaining - 1,
                        minimum,
                        codepoint,
                    };
                    return Ok(None);
                }
                if codepoint < minimum
                    || codepoint > 0x10_ffff
                    || (0xd800..=0xdfff).contains(&codepoint)
                {
                    return Ok(Some(REPLACEMENT));
                }
                if codepoint <= 0xffff {
                    return Ok(Some(codepoint));
                }
                let supplementary = codepoint - 0x1_0000;
                let high = 0xd800 | (supplementary >> 10);
                let low = 0xdc00 | (supplementary & 0x3ff);
                self.heap
                    .managed
                    .set_field(
                        reader,
                        "java/io/InputStreamReader.pending:I",
                        HeapValue::Int(low),
                    )
                    .map_err(heap_error)?;
                Ok(Some(high))
            }
        }
    }

    pub(super) fn input_stream_reader_close(
        &mut self,
        method: &Method,
        args: &[Value],
        depth: usize,
    ) -> Result<CallOutcome, EmuError> {
        let reader = reference_argument(args, 0)?;
        if self.graphics_int_field(reader, "java/io/InputStreamReader.closed:Z")? != 0 {
            return Ok(CallOutcome::Return(None));
        }
        let input = self.graphics_reference_field(
            reader,
            "java/io/InputStreamReader.in:Ljava/io/InputStream;",
        )?;
        let class = self.object_class(input)?;
        let key = self.resolve_virtual(&class, "close", "()V")?;
        let close = self
            .program
            .methods
            .get(&key)
            .ok_or_else(|| vm_error("method-not-found", display_key(&key)))?;
        let outcome = self.call(close, [Value::Reference(Some(input))], depth + 1)?;
        self.finish_input_stream_reader_close(method, reader, outcome)
    }

    pub(super) fn finish_input_stream_reader_close(
        &mut self,
        method: &Method,
        reader: Handle,
        outcome: CallOutcome,
    ) -> Result<CallOutcome, EmuError> {
        match outcome {
            CallOutcome::Return(None) => {
                self.heap
                    .managed
                    .set_field(
                        reader,
                        "java/io/InputStreamReader.closed:Z",
                        HeapValue::Int(1),
                    )
                    .map_err(heap_error)?;
                Ok(CallOutcome::Return(None))
            }
            CallOutcome::Return(Some(_)) => Err(type_error()),
            CallOutcome::Throw(exception) => Ok(CallOutcome::Throw(exception)),
            CallOutcome::Suspend(child) => Ok(suspended_native_pending_call(
                method,
                NativeResume::InputStreamReaderClose { reader },
                child,
            )),
        }
    }
}
