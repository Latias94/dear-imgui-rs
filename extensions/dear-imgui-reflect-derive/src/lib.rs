use proc_macro::TokenStream;
use syn::{Data, DeriveInput, parse_macro_input};

mod attrs;
mod diagnostics;
mod enum_codegen;
mod field_codegen;
mod internal;
mod settings_codegen;
mod struct_codegen;
#[cfg(test)]
mod tests;

/// Derive macro for [`dear_imgui_reflect::ImGuiReflect`].
///
/// Currently supports:
///
/// - Structs with named fields, tuple fields, and unit structs. Each field must implement
///   [`dear_imgui_reflect::ImGuiValue`], either directly or via a blanket
///   implementation (for example, another type that implements
///   [`dear_imgui_reflect::ImGuiReflect`]).
/// - Enums with unit, tuple, or named payload variants. Variants are edited via a combo box
///   (default) or radio buttons via `#[imgui(enum_style = "radio")]`.
///   Switching to a payload variant constructs its payload using `Default`, so payload field types
///   must implement `Default` to allow variant switching.
///
/// Supported field attributes:
///
/// - `#[imgui(skip)]` — do not generate any UI for this field.
/// - `#[imgui(name = "Custom Label")]` — override the label used for this field.
/// - `#[imgui(slider, min = ..., max = ..., format = "...")]` — use a slider
///   with the given range/format for numeric fields.
/// - `#[imgui(multiline, hint = "...", read_only)]` — use multiline text
///   widgets for String/ImString fields.
#[proc_macro_derive(ImGuiReflect, attributes(imgui))]
pub fn derive_imgui_reflect(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ident = input.ident;
    let generics = input.generics;
    let attrs = input.attrs;

    match input.data {
        Data::Struct(data) => struct_codegen::derive_for_struct(ident, generics, data),
        Data::Enum(data) => enum_codegen::derive_for_enum(ident, generics, attrs, data),
        Data::Union(data) => diagnostics::union_not_supported(data),
    }
}
