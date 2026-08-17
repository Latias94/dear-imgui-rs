//! Low-level FFI bindings for cimCTE and ImGuiColorTextEdit.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unnecessary_transmutes)]
#![allow(unsafe_op_in_unsafe_fn)]
#![allow(clippy::all)]
#![allow(unpredictable_function_pointer_comparisons)]

pub use dear_imgui_sys::{
    ImDrawList, ImGuiChildFlags, ImGuiContext, ImGuiKeyChord, ImGuiWindowFlags, ImTextureID, ImU32,
    ImVec2, ImVec2_c, ImVec4, ImVec4_c, ImWchar,
};

pub type Color = ::std::os::raw::c_char;
pub const text: Color = 0;
pub const keyword: Color = 1;
pub const declaration: Color = 2;
pub const number: Color = 3;
pub const string: Color = 4;
pub const punctuation: Color = 5;
pub const preprocessor: Color = 6;
pub const identifier: Color = 7;
pub const knownIdentifier: Color = 8;
pub const comment: Color = 9;
pub const background: Color = 10;
pub const cursor: Color = 11;
pub const selection: Color = 12;
pub const whitespace: Color = 13;
pub const matchingBracketBackground: Color = 14;
pub const matchingBracketActive: Color = 15;
pub const matchingBracketLevel1: Color = 16;
pub const matchingBracketLevel2: Color = 17;
pub const matchingBracketLevel3: Color = 18;
pub const matchingBracketError: Color = 19;
pub const lineNumber: Color = 20;
pub const currentLineNumber: Color = 21;
pub const count: Color = 22;

pub type BreakOption = ::std::os::raw::c_char;
pub const mustBreak: BreakOption = 0;
pub const allowBreak: BreakOption = 1;
pub const noBreak: BreakOption = 2;
pub const undefined: BreakOption = 3;

#[repr(C)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct Glyph {
    pub codepoint: ImWchar,
    pub color: Color,
    pub breakOption: BreakOption,
    pub squiggle: usize,
}

impl Default for Glyph {
    fn default() -> Self {
        Self {
            codepoint: 0,
            color: text,
            breakOption: undefined,
            squiggle: 0,
        }
    }
}

const GLYPH_SQUIGGLE_OFFSET: usize = {
    let before_squiggle = ::std::mem::size_of::<ImWchar>() + 2;
    let align = ::std::mem::align_of::<usize>();
    (before_squiggle + align - 1) & !(align - 1)
};

const _: () = assert!(::std::mem::size_of::<Color>() == 1);
const _: () = assert!(::std::mem::size_of::<BreakOption>() == 1);
const _: () = assert!(::std::mem::offset_of!(Glyph, codepoint) == 0);
const _: () = assert!(::std::mem::offset_of!(Glyph, color) == ::std::mem::size_of::<ImWchar>());
const _: () =
    assert!(::std::mem::offset_of!(Glyph, breakOption) == ::std::mem::size_of::<ImWchar>() + 1);
const _: () = assert!(::std::mem::offset_of!(Glyph, squiggle) == GLYPH_SQUIGGLE_OFFSET);
const _: () = assert!(
    ::std::mem::size_of::<Glyph>() == GLYPH_SQUIGGLE_OFFSET + ::std::mem::size_of::<usize>()
);

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
