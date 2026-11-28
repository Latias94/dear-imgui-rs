pub(crate) fn union_not_supported(data: syn::DataUnion) -> syn::Error {
    syn::Error::new_spanned(
        data.union_token,
        "ImGuiReflect cannot be derived for unions",
    )
}
