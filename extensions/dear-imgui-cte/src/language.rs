use crate::sys;

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
}

impl Language {
    pub const ALL: [Self; 11] = [
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
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::C => "C",
            Self::Cpp => "C++",
            Self::CSharp => "C#",
            Self::AngelScript => "AngelScript",
            Self::Lua => "Lua",
            Self::Python => "Python",
            Self::Glsl => "GLSL",
            Self::Hlsl => "HLSL",
            Self::Json => "JSON",
            Self::Markdown => "Markdown",
            Self::Sql => "SQL",
        }
    }

    pub(crate) fn as_raw(self) -> *const sys::Language {
        unsafe {
            match self {
                Self::C => sys::Language_C(),
                Self::Cpp => sys::Language_Cpp(),
                Self::CSharp => sys::Language_Cs(),
                Self::AngelScript => sys::Language_AngelScript(),
                Self::Lua => sys::Language_Lua(),
                Self::Python => sys::Language_Python(),
                Self::Glsl => sys::Language_Glsl(),
                Self::Hlsl => sys::Language_Hlsl(),
                Self::Json => sys::Language_Json(),
                Self::Markdown => sys::Language_Markdown(),
                Self::Sql => sys::Language_Sql(),
            }
        }
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
