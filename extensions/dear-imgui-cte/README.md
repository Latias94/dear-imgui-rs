# dear-imgui-cte

Preview safe bindings for ImGuiColorTextEdit, integrated with the
`dear-imgui-rs` context lifecycle.

```rust,no_run
use dear_imgui_cte::{CteUiExt, Language, TextEditor};
use dear_imgui_rs::Context;

# fn main() -> Result<(), dear_imgui_cte::CteError> {
let mut imgui = Context::create();
let mut editor = TextEditor::create(&imgui);
editor.set_text("int main() {}\n")?;
editor.set_language(Some(Language::Cpp));

let ui = imgui.frame();
ui.text_editor(&mut editor, "Source")
    .size([640.0, 480.0])
    .build()?;
# Ok(())
# }
```

`TextEditor` owns its native handle, is bound to the `Context` used at creation,
and is neither `Send` nor `Sync`. Safe string getters return owned `String`
values, and palettes are copied Rust values. Rendering with a `Ui` from another
context is rejected before calling native code.
