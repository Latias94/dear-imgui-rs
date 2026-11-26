use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Tokens};
use quote::{ToTokens, quote, quote_spanned};
use syn::{Data, DeriveInput, Generics, Ident, parse_macro_input};

mod attrs;
mod diagnostics;
mod enum_codegen;
mod field_codegen;
mod internal;
mod numeric_codegen;
mod numeric_format;
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
///   with a format validated against the exact numeric field type.
/// - `#[imgui(multiline, lines = 3, read_only)]` — use multiline text
///   widgets for `String` fields. `hint = "..."` is single-line only.
#[proc_macro_derive(ImGuiReflect, attributes(imgui))]
pub fn derive_imgui_reflect(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ident = input.ident;
    let generics = input.generics;
    let attrs = input.attrs;

    let expansion = match input.data {
        Data::Struct(data) => struct_codegen::derive_for_struct(&ident, generics, &attrs, data),
        Data::Enum(data) => enum_codegen::derive_for_enum(&ident, generics, &attrs, data),
        Data::Union(data) => Err(diagnostics::union_not_supported(data)),
    };
    expansion
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn option_tokens<T: ToTokens>(value: Option<T>) -> Tokens {
    match value {
        Some(value) => quote!(::core::option::Option::Some(#value)),
        None => quote!(::core::option::Option::None),
    }
}

fn impl_reflect(ident: &Ident, generics: &Generics, body: Tokens) -> Tokens {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let receiver = quote!(self);
    quote_spanned! {Span::mixed_site()=>
        impl #impl_generics ::dear_imgui_reflect::ImGuiReflect for #ident #ty_generics #where_clause {
            fn imgui_reflect(
                &mut #receiver,
                __dear_imgui_reflect_inspector: &mut ::dear_imgui_reflect::Inspector<'_, '_>,
                __dear_imgui_reflect_label: &str,
            ) -> bool {
                let __dear_imgui_reflect_ui = __dear_imgui_reflect_inspector.ui();
                #body
            }
        }
    }
}
