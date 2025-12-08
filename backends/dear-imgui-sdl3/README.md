dear-imgui-sdl3
================

SDL3 平台后端封装，用于将 Dear ImGui 接入 SDL3 窗口系统。实现基于 upstream C++ 后端：

- `imgui_impl_sdl3.cpp`（平台层）
- `imgui_impl_opengl3.cpp`（OpenGL3 渲染层）

本 crate 主要用途：

- 在 SDL3 窗口上驱动 Dear ImGui 平台事件（键盘/鼠标/多显示器/多视口）。
- 在 OpenGL3 环境下渲染 Dear ImGui（可选）。
- 为其它渲染后端（如 `dear-imgui-wgpu`）提供 SDL3 平台支持。

构建依赖
--------

1. 系统需要安装 SDL3 头文件和库：

- **macOS（推荐 Homebrew）**
  - `brew install sdl3`
  - 头文件通常位于：`/opt/homebrew/include/SDL3/SDL.h`
  - 默认情况下，本 crate 会：
    - 优先使用 `pkg-config sdl3` 获取 include 路径；
    - 若找不到 pkg-config 条目，再尝试 `/opt/homebrew/include` 等常用路径。

- **Linux（Debian / Ubuntu 等）**
  - 安装开发包（以发行版为准），例如：
    - `sudo apt-get install libsdl3-dev`（或未来对应的包名）
  - 头文件通常位于：`/usr/include/SDL3/SDL.h`
  - 本 crate 会：
    - 优先通过 `pkg-config sdl3` 获取 include 路径；
    - 若 pkg-config 可用，则无需额外环境变量。

- **Windows**
  - 请通过以下方式之一安装 SDL3：
    - 使用预编译二进制并配置好 include/lib 目录；
    - 使用 vcpkg 等包管理器安装 SDL3，再通过 `sdl3-sys` 的说明配置工具链。
  - 本 crate 只负责查找头文件路径；链接参数由 `sdl3-sys`/`sdl3` crates 负责。

2. 依赖 crate

`backends/dear-imgui-sdl3/Cargo.toml` 中声明了：

- `sdl3 = "0.16.2"`
- `sdl3-sys = "0.5"`

链接到 SDL3 动态库/静态库的工作由这些 crate 负责，本 crate 仅在 `build.rs` 中查找头文件用于编译 C++ 后端源文件。

环境变量（SDL3_INCLUDE_DIR）
---------------------------

`build.rs` 中的 SDL3 头文件查找顺序：

1. **显式指定：`SDL3_INCLUDE_DIR`**

   如设置：

   - macOS（Homebrew 自定义路径）：

     ```bash
     export SDL3_INCLUDE_DIR=/opt/homebrew/include
     ```

   - Linux（手工安装到自定义前缀）：

     ```bash
     export SDL3_INCLUDE_DIR=/opt/sdl3/include
     ```

   - Windows（假设你的头文件在 `C:\libs\SDL3\include`）：

     ```powershell
     $env:SDL3_INCLUDE_DIR="C:\libs\SDL3\include"
     ```

   行为：

   - `build.rs` 会将该目录加入 C/C++ include 路径；
   - 期望在该目录下能找到 `SDL3/SDL.h`。

2. **pkg-config：`sdl3`**

   若未设置 `SDL3_INCLUDE_DIR`，`build.rs` 会尝试：

   ```bash
   pkg-config --cflags sdl3
   ```

   - 成功时，从 pkg-config 的 `include_paths` 中添加头文件搜索路径；
   - 适用于大多数 Linux 发行版和配置了 pkg-config 的 macOS 环境。

3. **常见默认路径**

   若以上两步都失败，`build.rs` 会尝试以下目录：

   - `/opt/homebrew/include`
   - `/usr/local/include`
   - `/opt/local/include`

   并在其中查找 `SDL3/SDL.h`。

   这是为了兼容常见的 Homebrew / MacPorts 安装布局。

查找失败时的行为
----------------

如果上述步骤都没有找到有效的 SDL3 头文件，会直接 panic 并给出提示：

> dear-imgui-sdl3: could not find SDL3 headers. \
> Install SDL3 development files (e.g. `brew install sdl3`) \
> or set SDL3_INCLUDE_DIR to the SDL3 include path.

此时通常有两种解决方案：

1. 安装系统开发包（带头文件）并确认 `pkg-config sdl3` 正常工作；
2. 手动设置 `SDL3_INCLUDE_DIR` 指向正确的 include 根目录。

使用示例
--------

### SDL3 + OpenGL3 多视口

```bash
cargo run -p dear-imgui-examples --bin sdl3_opengl_multi_viewport --features multi-viewport
```

### SDL3 + WGPU（单窗口）

```bash
cargo run -p dear-imgui-examples --bin sdl3_wgpu
```

注意：目前 WebGPU 路线（包括 SDL3 + WGPU）沿用 upstream `imgui_impl_wgpu` 的设计，不开启多视口支持；多视口推荐使用 SDL3 + OpenGL3 或 winit + OpenGL 路线。

IME 与手柄配置
--------------

- **IME 显示（中文输入法等）**

  官方后端要求在创建窗口前开启 `SDL_HINT_IME_SHOW_UI`：

  ```rust
  // 在 SDL3 创建任何窗口之前调用
  dear_imgui_sdl3::enable_native_ime_ui();
  ```

- **手柄模式**

  默认模式会自动打开第一个可用手柄，你也可以切换为“全部合并”模式：

  ```rust
  use dear_imgui_sdl3::{set_gamepad_mode, GamepadMode};

  set_gamepad_mode(GamepadMode::AutoAll);
  ```
