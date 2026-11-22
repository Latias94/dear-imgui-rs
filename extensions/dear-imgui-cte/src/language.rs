use crate::sys;
use std::{
    ffi::{CStr, CString},
    sync::Mutex,
};

// Both the upstream registry and the individual language creators initialize on first lookup.
static LANGUAGE_REGISTRY: Mutex<()> = Mutex::new(());

/// A built-in ImGuiColorTextEdit language definition.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum Language {
    C,
    Cpp,
    CSharp,
    AngelScript,
    Lua,
    Python,
    Glsl,
    Hlsl,
    Json,
    Markdown,
    Sql,
    Julia,
    Wgsl,
}

impl Language {
    pub const ALL: [Self; 13] = [
        Self::C,
        Self::Cpp,
        Self::CSharp,
        Self::AngelScript,
        Self::Lua,
        Self::Python,
        Self::Glsl,
        Self::Hlsl,
        Self::Json,
        Self::Markdown,
        Self::Sql,
        Self::Julia,
        Self::Wgsl,
    ];

    pub const fn name(self) -> &'static str {
        match self.c_name().to_str() {
            Ok(name) => name,
            Err(_) => panic!("built-in language names must be valid UTF-8"),
        }
    }

    const fn c_name(self) -> &'static CStr {
        match self {
            Self::C => c"C",
            Self::Cpp => c"C++",
            Self::CSharp => c"C#",
            Self::AngelScript => c"AngelScript",
            Self::Lua => c"Lua",
            Self::Python => c"Python",
            Self::Glsl => c"GLSL",
            Self::Hlsl => c"HLSL",
            Self::Json => c"JSON",
            Self::Markdown => c"Markdown",
            Self::Sql => c"SQL",
            Self::Julia => c"Julia",
            Self::Wgsl => c"WGSL",
        }
    }

    /// Finds a built-in language by its case-sensitive registry name, such as `C++`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|language| language.name() == name)
    }

    /// Looks up a case-sensitive file extension including its leading dot, such as `.jl`.
    pub fn from_extension(extension: &str) -> Option<Self> {
        let extension = CString::new(extension).ok()?;
        let raw = {
            let _guard = LANGUAGE_REGISTRY.lock().unwrap();
            unsafe { sys::TextEditor_LookupLanguageByExtension(extension.as_ptr()) }
        };
        Self::from_raw(raw)
    }

    pub(crate) fn as_raw(self) -> *const sys::Language {
        let _guard = LANGUAGE_REGISTRY.lock().unwrap();
        unsafe { sys::TextEditor_LookupLanguage(self.c_name().as_ptr()) }
    }

    pub(crate) fn from_raw(raw: *const sys::Language) -> Option<Self> {
        if raw.is_null() {
            return None;
        }
        Self::ALL
            .into_iter()
            .find(|language| language.as_raw() == raw)
    }
}
