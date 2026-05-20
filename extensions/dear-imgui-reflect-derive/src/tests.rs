#[test]
fn parses_basic_imgui_field_attrs() {
    let field: syn::Field = syn::parse_quote! {
        #[imgui(name = "Speed", slider, min = 0.0, max = 1.0, format = "%.2f")]
        speed: f32
    };
    let ident = syn::Ident::new("speed", proc_macro2::Span::call_site());

    let attrs = crate::attrs::parse_field_attrs(&ident, &field).expect("attrs parse");

    assert!(attrs.slider);
    assert_eq!(attrs.label_override.unwrap().value(), "Speed");
    assert!(attrs.min_expr.is_some());
    assert!(attrs.max_expr.is_some());
    assert_eq!(attrs.format_str.unwrap().value(), "%.2f");
}

#[test]
fn reflect_settings_identifier_is_stable() {
    assert_eq!(
        crate::settings_codegen::reflect_settings_ident().to_string(),
        "__imgui_reflect_settings"
    );
}
