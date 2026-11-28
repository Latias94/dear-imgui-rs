use dear_imgui_reflect::ImGuiReflect;

#[derive(ImGuiReflect)]
struct TextSizing {
    #[imgui(multiline, auto_resize, min_width = 120.0)]
    value: String,
}

#[derive(ImGuiReflect)]
struct TextLines {
    #[imgui(multiline, auto_resize, lines = 3)]
    value: String,
}

#[derive(ImGuiReflect)]
struct ClampWithoutRange {
    #[imgui(as_drag, clamp)]
    value: f32,
}

#[derive(ImGuiReflect)]
struct InputWithRange {
    #[imgui(as_input, min = 0, max = 10)]
    value: i32,
}

#[derive(ImGuiReflect)]
enum Duplicate {
    Value(#[imgui(name = "First", name = "Second")] i32),
}

#[derive(ImGuiReflect)]
struct FormatMismatch {
    #[imgui(format = "%s")]
    value: i32,
}

fn main() {}
