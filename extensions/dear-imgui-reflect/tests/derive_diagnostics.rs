#![cfg(feature = "derive")]

#[test]
fn rejects_invalid_imgui_attributes() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
