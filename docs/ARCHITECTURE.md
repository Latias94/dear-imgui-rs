# Dear ImGui Rust 绑定架构设计文档

## 1. 项目概述

本项目旨在创建一个现代化的 Dear ImGui Rust 绑定，遵循 Rust 最佳实践，提供类型安全、内存安全且灵活的 API。

### 1.1 设计目标

- **完整性**：支持 Dear ImGui 的所有原生功能和 API
- **安全性**：提供内存安全和类型安全的 Rust 接口
- **灵活性**：允许细粒度控制，不强制特定的架构模式
- **现代性**：使用现代 Rust 特性和惯用法
- **模块化**：后端和功能模块化，按需引入
- **性能**：零成本抽象，最小化运行时开销

### 1.2 核心原则

1. **渐进式封装**：从低级 FFI 到高级 API 的多层抽象
2. **选择性安全**：提供安全和不安全两种 API 选择
3. **后端无关**：核心库与具体后端解耦
4. **向前兼容**：API 设计考虑未来扩展性

## 2. 整体架构

### 2.1 分层架构

```
┌─────────────────────────────────────────────────────────────┐
│                    应用程序层                                │
├─────────────────────────────────────────────────────────────┤
│  高级 API 层 (dear-imgui)                                   │
│  - 类型安全的 Rust API                                      │
│  - Builder 模式和流畅接口                                   │
│  - 自动生命周期管理                                         │
├─────────────────────────────────────────────────────────────┤
│  核心抽象层 (dear-imgui-core)                               │
│  - 安全的 Rust 封装                                         │
│  - 上下文和状态管理                                         │
│  - 错误处理和资源管理                                       │
├─────────────────────────────────────────────────────────────┤
│  FFI 绑定层 (dear-imgui-sys)                                │
│  - 自动生成的 C 绑定                                        │
│  - 原始指针和 unsafe 操作                                   │
│  - 直接映射 C++ API                                         │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 Crate 组织结构

```
dear-imgui-rs/
├── dear-imgui-sys/              # FFI 绑定层
├── dear-imgui-core/             # 核心抽象层
├── dear-imgui/                  # 高级 API 层
├── dear-imgui-derive/           # 过程宏支持
├── backends/                    # 后端实现 (独立 crates)
│   ├── dear-imgui-winit/        # Winit 平台后端
│   ├── dear-imgui-sdl2/         # SDL2 平台后端
│   ├── dear-imgui-glfw/         # GLFW 平台后端
│   ├── dear-imgui-wgpu/         # WGPU 渲染后端
│   ├── dear-imgui-opengl/       # OpenGL 渲染后端
│   ├── dear-imgui-vulkan/       # Vulkan 渲染后端
│   ├── dear-imgui-dx11/         # DirectX 11 渲染后端
│   └── dear-imgui-dx12/         # DirectX 12 渲染后端
├── extensions/                  # 扩展功能 (独立 crates)
│   ├── dear-imgui-plot/         # ImPlot 绑定
│   ├── dear-imgui-node-editor/  # 节点编辑器
│   └── dear-imgui-file-dialog/  # 文件对话框
├── examples/                    # 示例项目
└── docs/                        # 文档
```

## 3. 核心组件设计

### 3.1 FFI 绑定层 (dear-imgui-sys)

**职责**：
- 提供与 Dear ImGui C API 的直接绑定
- 自动生成绑定代码
- 处理平台特定的编译配置

**特性**：
```toml
[features]
default = []
docking = []              # 启用 docking 分支
freetype = []             # 启用 FreeType 字体渲染
tables = []               # 启用表格 API
multi-viewport = []       # 启用多视口支持
```

**关键类型**：
```rust
// 直接映射 C 结构体
#[repr(C)]
pub struct ImGuiContext { /* ... */ }

#[repr(C)]
pub struct ImGuiIO { /* ... */ }

#[repr(C)]
pub struct ImDrawData { /* ... */ }

// 函数绑定
extern "C" {
    pub fn igCreateContext(shared_font_atlas: *mut ImFontAtlas) -> *mut ImGuiContext;
    pub fn igDestroyContext(ctx: *mut ImGuiContext);
    pub fn igNewFrame();
    pub fn igRender();
    // ... 所有 ImGui 函数
}
```

### 3.2 核心抽象层 (dear-imgui-core)

**职责**：
- 提供内存安全的 Rust 封装
- 管理 ImGui 上下文生命周期
- 实现基础的错误处理

**核心类型**：
```rust
// 上下文管理
pub struct Context {
    raw: NonNull<sys::ImGuiContext>,
    _marker: PhantomData<*mut sys::ImGuiContext>,
}

// 帧管理
pub struct Frame<'ctx> {
    context: &'ctx mut Context,
    _marker: PhantomData<&'ctx mut Context>,
}

// UI 构建器
pub struct Ui<'frame> {
    frame: &'frame mut Frame<'frame>,
    _marker: PhantomData<&'frame mut Frame<'frame>>,
}
```

### 3.3 高级 API 层 (dear-imgui)

**职责**：
- 提供符合 Rust 习惯的高级 API
- 实现 Builder 模式和流畅接口
- 提供类型安全的控件封装

**API 设计示例**：
```rust
// 窗口 API
impl<'ui> Ui<'ui> {
    pub fn window(&mut self, title: impl AsRef<str>) -> Window<'_, 'ui> {
        Window::new(self, title.as_ref())
    }
}

// 控件 API
impl<'ui> Ui<'ui> {
    pub fn button(&mut self, label: impl AsRef<str>) -> bool { /* ... */ }
    
    pub fn slider<T>(&mut self, label: impl AsRef<str>, value: &mut T, range: RangeInclusive<T>) -> bool 
    where T: SliderValue { /* ... */ }
    
    pub fn input_text(&mut self, label: impl AsRef<str>, buf: &mut String) -> bool { /* ... */ }
}
```

## 4. 后端架构设计

### 4.1 后端分离原则

每个后端作为独立的 crate，遵循以下原则：
- **平台后端**：处理窗口管理、事件处理、输入
- **渲染后端**：处理图形渲染、纹理管理
- **可组合性**：平台后端和渲染后端可以自由组合

### 4.2 平台后端 Trait

```rust
pub trait PlatformBackend {
    type Window;
    type Event;
    type Error: std::error::Error + Send + Sync + 'static;
    
    // 窗口管理
    fn create_window(&mut self, desc: WindowDescriptor) -> Result<Self::Window, Self::Error>;
    fn destroy_window(&mut self, window: Self::Window);
    
    // 事件处理
    fn poll_events(&mut self) -> Vec<Self::Event>;
    fn handle_event(&mut self, event: &Self::Event, io: &mut Io) -> bool;
    
    // 平台特定功能
    fn set_clipboard_text(&mut self, text: &str);
    fn get_clipboard_text(&mut self) -> Option<String>;
}
```

### 4.3 渲染后端 Trait

```rust
pub trait RenderBackend {
    type Texture: Texture;
    type Error: std::error::Error + Send + Sync + 'static;
    
    // 渲染管理
    fn render(&mut self, draw_data: &DrawData) -> Result<(), Self::Error>;
    fn set_viewport(&mut self, size: [u32; 2]);
    
    // 纹理管理
    fn create_texture(&mut self, desc: TextureDescriptor) -> Result<Self::Texture, Self::Error>;
    fn update_texture(&mut self, texture: &mut Self::Texture, data: &[u8]) -> Result<(), Self::Error>;
    fn destroy_texture(&mut self, texture: Self::Texture);
}
```

## 5. 类型安全设计

### 5.1 ID 系统

```rust
// 类型安全的 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Id(u32);

impl Id {
    pub fn new(s: &str) -> Self {
        Self(hash_string(s))
    }
    
    pub fn with_index(self, index: usize) -> Self {
        Self(self.0.wrapping_add(index as u32))
    }
}

// 支持多种 ID 输入类型
pub trait IntoId {
    fn into_id(self) -> Id;
}

impl IntoId for &str { /* ... */ }
impl IntoId for String { /* ... */ }
impl IntoId for u32 { /* ... */ }
impl IntoId for Id { /* ... */ }
```

### 5.2 错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum ImGuiError {
    #[error("Context not initialized")]
    ContextNotInitialized,
    
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
    
    #[error("Backend error: {0}")]
    BackendError(#[from] Box<dyn std::error::Error + Send + Sync>),
    
    #[error("Texture error: {0}")]
    TextureError(String),
}

pub type Result<T> = std::result::Result<T, ImGuiError>;
```

## 6. 性能考虑

### 6.1 零成本抽象

- 使用 `#[inline]` 标记热路径函数
- 避免不必要的堆分配
- 使用 `MaybeUninit` 优化初始化
- 编译时计算常量

### 6.2 内存管理

- 使用 RAII 管理资源生命周期
- 提供自定义分配器支持
- 最小化 FFI 调用开销

## 7. 扩展性设计

### 7.1 插件系统

```rust
pub trait ImGuiPlugin {
    fn name(&self) -> &'static str;
    fn initialize(&mut self, ctx: &mut Context) -> Result<()>;
    fn shutdown(&mut self, ctx: &mut Context);
}
```

### 7.2 自定义控件

```rust
pub trait CustomWidget {
    fn render(&self, ui: &mut Ui) -> bool;
}
```

## 8. API 设计示例

### 8.1 基础使用模式

```rust
use dear_imgui::*;

fn main() -> Result<()> {
    let mut ctx = Context::new()?;

    // 设置字体和样式
    ctx.fonts_mut().add_font_from_file("assets/font.ttf", 16.0)?;
    ctx.style_mut().apply_theme(Theme::DARK);

    // 主循环
    loop {
        let mut frame = ctx.frame();

        // 构建 UI
        frame.window("Main Window")
            .size([800.0, 600.0])
            .position([100.0, 100.0])
            .flags(WindowFlags::NO_RESIZE)
            .show(|ui| {
                ui.text("Hello, Dear ImGui!");

                if ui.button("Exit") {
                    return false; // 退出循环
                }

                ui.separator();

                // 使用各种控件
                ui.slider("Value", &mut value, 0.0..=100.0);
                ui.input_text("Text", &mut text_buffer);
                ui.checkbox("Enabled", &mut enabled);
            });

        // 渲染
        if !render_frame(&frame)? {
            break;
        }
    }

    Ok(())
}
```

### 8.2 高级功能示例

```rust
// Tables API
frame.window("Data Table").show(|ui| {
    ui.table("MyTable", 3)
        .flags(TableFlags::BORDERS | TableFlags::RESIZABLE)
        .build(|table| {
            table.setup_column("Name");
            table.setup_column("Age");
            table.setup_column("City");
            table.headers_row();

            for person in &people {
                table.next_row();
                table.next_column();
                ui.text(&person.name);
                table.next_column();
                ui.text(person.age.to_string());
                table.next_column();
                ui.text(&person.city);
            }
        });
});

// Docking API
if let Some(dockspace) = frame.dockspace("MainDockSpace") {
    dockspace.window("Tool 1").show(|ui| {
        ui.text("Tool 1 content");
    });

    dockspace.window("Tool 2").show(|ui| {
        ui.text("Tool 2 content");
    });
}

// 自定义绘制
frame.window("Custom Drawing").show(|ui| {
    let draw_list = ui.get_window_draw_list();
    let canvas_pos = ui.cursor_screen_pos();
    let canvas_size = ui.content_region_avail();

    draw_list.add_rect(
        canvas_pos,
        [canvas_pos[0] + canvas_size[0], canvas_pos[1] + canvas_size[1]],
        Color::WHITE,
        0.0,
        DrawFlags::NONE,
        2.0,
    );

    draw_list.add_circle(
        [canvas_pos[0] + 50.0, canvas_pos[1] + 50.0],
        30.0,
        Color::RED,
        32,
        2.0,
    );
});
```

### 8.3 过程宏使用示例

```rust
use dear_imgui::prelude::*;
use dear_imgui_derive::ImGui;

#[derive(ImGui)]
struct AppConfig {
    #[imgui(slider(min = 0.0, max = 100.0, format = "%.1f"))]
    volume: f32,

    #[imgui(input_text(hint = "Enter your name..."))]
    username: String,

    #[imgui(checkbox)]
    fullscreen: bool,

    #[imgui(combo(items = ["Low", "Medium", "High"]))]
    quality: usize,

    #[imgui(color_edit)]
    background_color: [f32; 4],
}

fn render_config(ui: &mut Ui, config: &mut AppConfig) {
    ui.window("Settings").show(|ui| {
        config.render_ui(ui); // 自动生成的方法
    });
}
```

## 9. 测试策略

### 9.1 单元测试
- 每个 API 函数的基础功能测试
- 边界条件和错误情况测试
- 内存安全性测试

### 9.2 集成测试
- 完整的渲染流程测试
- 多后端兼容性测试
- 性能基准测试

### 9.3 示例测试
- 所有示例程序的自动化测试
- 视觉回归测试
- 跨平台兼容性验证

## 10. 文档策略

### 10.1 API 文档
- 完整的 rustdoc 注释
- 代码示例和用法说明
- 性能注意事项

### 10.2 教程文档
- 快速入门指南
- 从其他绑定的迁移指南
- 最佳实践和设计模式

### 10.3 参考文档
- 完整的 API 参考
- 后端集成指南
- 故障排除和 FAQ

这个架构设计确保了完整性、安全性和灵活性的平衡，为创建一个现代化的 Dear ImGui Rust 绑定提供了坚实的基础。
