use proc_macro::TokenStream;

pub(crate) fn union_not_supported(data: syn::DataUnion) -> TokenStream {
    syn::Error::new_spanned(
        data.union_token,
        "ImGuiReflect cannot be derived for unions",
    )
    .to_compile_error()
    .into()
}
