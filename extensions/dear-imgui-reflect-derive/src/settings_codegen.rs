use proc_macro2::Span;

pub(crate) fn reflect_settings_ident() -> syn::Ident {
    syn::Ident::new("__imgui_reflect_settings", Span::call_site())
}
