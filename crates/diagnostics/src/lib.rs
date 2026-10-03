//! Shared, stable diagnostic vocabulary for `J2Play`.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Copies text within a UTF-8 byte budget, marking truncation when an ellipsis fits.
#[must_use]
pub fn bounded_text(value: &str, maximum: usize) -> String {
    if value.len() <= maximum {
        return value.to_owned();
    }
    let ellipsis = if maximum >= "…".len() { "…" } else { "" };
    let end = value.floor_char_boundary(maximum - ellipsis.len());
    format!("{}{ellipsis}", &value[..end])
}

/// Subsystem that produced a diagnostic event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Category {
    Jar,
    ClassLoading,
    Vm,
    Api,
    M3g,
    Micro3d,
    Platform,
}

impl Category {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Jar => "jar",
            Self::ClassLoading => "class-loading",
            Self::Vm => "vm",
            Self::Api => "api",
            Self::M3g => "m3g",
            Self::Micro3d => "micro3d",
            Self::Platform => "platform",
        }
    }
}

/// Categorized emulator error with an optional underlying host error.
#[derive(Debug)]
pub struct EmuError {
    category: Category,
    code: &'static str,
    message: String,
    source: Option<Box<dyn Error + Send + Sync>>,
    java_stack: bool,
}

impl EmuError {
    #[must_use]
    pub fn new(category: Category, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            category,
            code,
            message: message.into(),
            source: None,
            java_stack: false,
        }
    }

    #[must_use]
    pub fn with_source(
        category: Category,
        code: &'static str,
        message: impl Into<String>,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            source: Some(Box::new(source)),
            ..Self::new(category, code, message)
        }
    }

    #[must_use]
    pub const fn category(&self) -> Category {
        self.category
    }
    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.code
    }
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Replaces display context while retaining the source and stack annotation.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Whether the VM has already included Java stack context in the message.
    #[must_use]
    pub const fn has_java_stack(&self) -> bool {
        self.java_stack
    }

    /// Marks an already formatted message as including Java stack context.
    /// This also applies when the diagnostic byte budget truncated the stack.
    #[must_use]
    pub fn with_java_stack(mut self) -> Self {
        self.java_stack = true;
        self
    }
}

impl Display for EmuError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}[{}]: {}",
            self.category.as_str(),
            self.code,
            self.message
        )
    }
}

impl Error for EmuError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/diagnostics/mod.rs"]
mod tests;
