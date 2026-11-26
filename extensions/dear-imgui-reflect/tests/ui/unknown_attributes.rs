use dear_imgui_reflect::ImGuiReflect;

#[derive(ImGuiReflect)]
#[imgui(slider)]
struct StructScope;

#[derive(ImGuiReflect)]
#[imgui(dropdown)]
enum EnumScope {
    Value,
}

#[derive(ImGuiReflect)]
enum VariantScope {
    #[imgui(skip)]
    Value,
}

#[derive(ImGuiReflect)]
struct FieldScope {
    #[imgui(sldier)]
    value: i32,
}

#[derive(ImGuiReflect)]
enum PayloadScope {
    Value(#[imgui(min_wdith = 42.0)] String),
}

fn main() {}
