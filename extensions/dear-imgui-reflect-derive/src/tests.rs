#[test]
fn parses_basic_imgui_field_attrs() {
    let field: syn::Field = syn::parse_quote! {
        #[imgui(name = "Speed", slider, min = 0.0, max = 1.0, format = "%.2f")]
        speed: f32
    };
    let attrs = crate::attrs::parse_field_attrs(&field).expect("attrs parse");
    assert!(attrs.numeric.slider);
    assert_eq!(attrs.name.unwrap().value(), "Speed");
    assert!(attrs.numeric.min.is_some());
    assert!(attrs.numeric.max.is_some());
    assert_eq!(attrs.numeric.format.unwrap().value(), "%.2f");
}
