use super::*;
#[cfg(feature = "sdlgpu3-renderer")]
use crate::callback_ownership::finish_sdlgpu_renderer_create;
use crate::callback_ownership::{
    create_window_callback_for_test, destroy_window_callback_for_test,
    render_window_callback_for_test, swap_buffers_callback_for_test,
};
#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
use crate::callback_ownership::{
    renderer_render_window_callback_for_test, renderer_set_window_size_callback_for_test,
};

const OWNED_BACKEND_DATA: usize = 0x101;
const FOREIGN_BACKEND_DATA: usize = 0x102;
const OWNED_PLATFORM_DATA: usize = 0x201;
const FOREIGN_PLATFORM_DATA: usize = 0x202;
const OWNED_VIEWPORT_DATA: usize = 0x301;
const FOREIGN_VIEWPORT_DATA: usize = 0x302;
const OWNED_VIEWPORT_HANDLE: usize = 0x401;
const FOREIGN_VIEWPORT_HANDLE: usize = 0x402;
const FOREIGN_VIEWPORT_HANDLE_RAW: usize = 0x403;
static OWNED_BACKEND_NAME: &[u8] = b"SDL3-test\0";
static FOREIGN_BACKEND_NAME: &[u8] = b"foreign-test\0";
static FOREIGN_CLIPBOARD_TEXT: &[u8] = b"foreign clipboard\0";

thread_local! {
    static DESTROY_OBSERVED_USER_DATA: Cell<usize> = const { Cell::new(0) };
    static PLATFORM_RENDER_COUNT: Cell<usize> = const { Cell::new(0) };
    static PLATFORM_SWAP_COUNT: Cell<usize> = const { Cell::new(0) };
    static RENDERER_RENDER_COUNT: Cell<usize> = const { Cell::new(0) };
    static RENDERER_SET_SIZE_COUNT: Cell<usize> = const { Cell::new(0) };
    static RENDERER_POINTER_SET_SIZE: Cell<(u32, u32)> = const { Cell::new((0, 0)) };
    static OWNED_RENDERER_DESTROY_COUNT: Cell<usize> = const { Cell::new(0) };
    static FOREIGN_RENDERER_DESTROY_COUNT: Cell<usize> = const { Cell::new(0) };
    static RENDERER_DESTROY_OBSERVED_USER_DATA: Cell<usize> = const { Cell::new(0) };
    #[cfg(feature = "multi-viewport")]
    static VULKAN_SURFACE_CREATE_COUNT: Cell<usize> = const { Cell::new(0) };
    #[cfg(feature = "multi-viewport")]
    static FOREIGN_VULKAN_SURFACE_CREATE_COUNT: Cell<usize> = const { Cell::new(0) };
}

unsafe extern "C" fn synthetic_create_window(viewport: *mut sys::ImGuiViewport) {
    if let Some(viewport) = unsafe { viewport.as_mut() } {
        viewport.PlatformUserData = OWNED_VIEWPORT_DATA as *mut _;
        viewport.PlatformHandle = OWNED_VIEWPORT_HANDLE as *mut _;
    }
}

unsafe extern "C" fn synthetic_destroy_window(viewport: *mut sys::ImGuiViewport) {
    if let Some(viewport) = unsafe { viewport.as_mut() } {
        DESTROY_OBSERVED_USER_DATA
            .with(|observed| observed.set(viewport.PlatformUserData as usize));
        viewport.PlatformUserData = std::ptr::null_mut();
        viewport.PlatformHandle = std::ptr::null_mut();
        viewport.PlatformHandleRaw = std::ptr::null_mut();
    }
}

unsafe extern "C" fn foreign_create_window(_viewport: *mut sys::ImGuiViewport) {}

unsafe extern "C" fn foreign_destroy_window(_viewport: *mut sys::ImGuiViewport) {}

unsafe extern "C" fn foreign_get_clipboard_text(
    _context: *mut sys::ImGuiContext,
) -> *const std::ffi::c_char {
    FOREIGN_CLIPBOARD_TEXT.as_ptr().cast()
}

unsafe extern "C" fn foreign_set_clipboard_text(
    _context: *mut sys::ImGuiContext,
    _text: *const std::ffi::c_char,
) {
}

unsafe extern "C" fn foreign_platform_render_window(
    _viewport: *mut sys::ImGuiViewport,
    _argument: *mut std::ffi::c_void,
) {
}

unsafe extern "C" fn foreign_platform_swap_buffers(
    _viewport: *mut sys::ImGuiViewport,
    _argument: *mut std::ffi::c_void,
) {
}

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
unsafe extern "C" fn synthetic_renderer_render_window(
    _viewport: *mut sys::ImGuiViewport,
    _argument: *mut std::ffi::c_void,
) {
    RENDERER_RENDER_COUNT.with(|count| count.set(count.get() + 1));
}

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
unsafe extern "C" fn synthetic_renderer_set_window_size(
    _viewport: *mut sys::ImGuiViewport,
    _size: sys::ImVec2_c,
) {
    RENDERER_SET_SIZE_COUNT.with(|count| count.set(count.get() + 1));
}

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
unsafe extern "C" fn foreign_renderer_render_window(
    _viewport: *mut sys::ImGuiViewport,
    _argument: *mut std::ffi::c_void,
) {
}

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
unsafe extern "C" fn foreign_renderer_set_window_size(
    _viewport: *mut sys::ImGuiViewport,
    _size: sys::ImVec2_c,
) {
}

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
unsafe extern "C" fn foreign_renderer_set_window_size_pointer(
    _viewport: *mut sys::ImGuiViewport,
    _size: *const sys::ImVec2,
) {
}

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
unsafe extern "C" fn recording_renderer_set_window_size_pointer(
    _viewport: *mut sys::ImGuiViewport,
    size: *const sys::ImVec2,
) {
    if let Some(size) = unsafe { size.as_ref() } {
        RENDERER_POINTER_SET_SIZE.with(|recorded| {
            recorded.set((size.x.to_bits(), size.y.to_bits()));
        });
    }
}

unsafe extern "C" fn synthetic_platform_render_window(
    _viewport: *mut sys::ImGuiViewport,
    _argument: *mut std::ffi::c_void,
) {
    PLATFORM_RENDER_COUNT.with(|count| count.set(count.get() + 1));
}

unsafe extern "C" fn synthetic_platform_swap_buffers(
    _viewport: *mut sys::ImGuiViewport,
    _argument: *mut std::ffi::c_void,
) {
    PLATFORM_SWAP_COUNT.with(|count| count.set(count.get() + 1));
}

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
unsafe extern "C" fn synthetic_renderer_destroy_window(viewport: *mut sys::ImGuiViewport) {
    OWNED_RENDERER_DESTROY_COUNT.with(|count| count.set(count.get() + 1));
    if let Some(viewport) = unsafe { viewport.as_mut() } {
        RENDERER_DESTROY_OBSERVED_USER_DATA
            .with(|observed| observed.set(viewport.RendererUserData as usize));
        viewport.RendererUserData = std::ptr::null_mut();
    }
}

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
unsafe extern "C" fn foreign_renderer_destroy_window(_viewport: *mut sys::ImGuiViewport) {
    FOREIGN_RENDERER_DESTROY_COUNT.with(|count| count.set(count.get() + 1));
}

unsafe extern "C" fn failing_create_window(_viewport: *mut sys::ImGuiViewport) {}

unsafe extern "C" fn foreign_set_window_alpha(_viewport: *mut sys::ImGuiViewport, _alpha: f32) {}

fn registration_with_lifecycle(
    context: &mut Context,
    renderer_shutdown: Option<Rc<dyn Fn()>>,
    platform_shutdown: Rc<dyn Fn()>,
) -> RuntimeRegistration {
    registration_with_backend_lifecycle(
        context,
        renderer_shutdown,
        platform_shutdown,
        PlatformGraphicsKind::Other,
        NativeRendererKind::None,
    )
}

fn registration_with_backend_lifecycle(
    context: &mut Context,
    renderer_shutdown: Option<Rc<dyn Fn()>>,
    platform_shutdown: Rc<dyn Fn()>,
    platform_graphics: PlatformGraphicsKind,
    native_renderer: NativeRendererKind,
) -> RuntimeRegistration {
    registration_with_backend_lifecycle_and_texture_update(
        context,
        renderer_shutdown,
        None,
        None,
        platform_shutdown,
        platform_graphics,
        native_renderer,
    )
}

fn registration_with_backend_lifecycle_and_texture_update(
    context: &mut Context,
    renderer_shutdown: Option<Rc<dyn Fn()>>,
    renderer_device_objects_destroy: Option<Rc<dyn Fn()>>,
    renderer_texture_update: Option<Rc<dyn Fn(&mut TextureData)>>,
    platform_shutdown: Rc<dyn Fn()>,
    platform_graphics: PlatformGraphicsKind,
    native_renderer: NativeRendererKind,
) -> RuntimeRegistration {
    let platform_session = Sdl3PlatformSession::acquire().unwrap();
    let baseline = preflight_platform_claim(context, native_renderer).unwrap();
    let control = Rc::new(RuntimeControl::new_with_backend(
        context,
        NativeLifecycle::new(
            renderer_shutdown,
            renderer_device_objects_destroy,
            renderer_texture_update,
            platform_shutdown,
        ),
        Some(platform_session),
        platform_graphics,
        native_renderer,
    ));
    let platform_attachment = context
        .register_attachment::<Sdl3PlatformAttachmentMarker>(
            ContextAttachmentRole::Platform,
            Rc::new(PlatformAttachment {
                control: Rc::clone(&control),
            }),
        )
        .unwrap();
    let renderer_attachment = control.lifecycle.renderer_shutdown.as_ref().map(|_| {
        context
            .register_attachment::<Sdl3RendererAttachmentMarker>(
                ContextAttachmentRole::Renderer,
                Rc::new(RendererAttachment {
                    control: Rc::clone(&control),
                }),
            )
            .unwrap()
    });
    RuntimeRegistration {
        control,
        renderer_consumer: None,
        baseline: Some(baseline),
        platform_attachment: Some(platform_attachment),
        renderer_attachment,
    }
}

fn test_registration(
    context: &mut Context,
    renderer_count: Rc<Cell<usize>>,
    platform_count: Rc<Cell<usize>>,
) -> RuntimeRegistration {
    let registration = registration_with_lifecycle(
        context,
        Some({
            let renderer_count = Rc::clone(&renderer_count);
            Rc::new(move || renderer_count.set(renderer_count.get() + 1))
        }),
        {
            let platform_count = Rc::clone(&platform_count);
            Rc::new(move || platform_count.set(platform_count.get() + 1))
        },
    );
    registration.control.platform_initialized.set(true);
    registration.control.renderer_initialized.set(true);
    registration
}

fn synthetic_claimed_registration(
    context: &mut Context,
    platform_count: Rc<Cell<usize>>,
    observed_backend_data: Rc<Cell<usize>>,
    observed_main_viewport_data: Rc<Cell<usize>>,
    create_window: unsafe extern "C" fn(*mut sys::ImGuiViewport),
) -> RuntimeRegistration {
    synthetic_claimed_registration_with_renderer(
        context,
        None,
        None,
        platform_count,
        observed_backend_data,
        observed_main_viewport_data,
        create_window,
    )
}

fn synthetic_claimed_registration_with_renderer(
    context: &mut Context,
    renderer_shutdown: Option<Rc<dyn Fn()>>,
    platform_shutdown_hook: Option<Rc<dyn Fn()>>,
    platform_count: Rc<Cell<usize>>,
    observed_backend_data: Rc<Cell<usize>>,
    observed_main_viewport_data: Rc<Cell<usize>>,
    create_window: unsafe extern "C" fn(*mut sys::ImGuiViewport),
) -> RuntimeRegistration {
    synthetic_claimed_registration_with_graphics(
        context,
        renderer_shutdown,
        platform_shutdown_hook,
        platform_count,
        observed_backend_data,
        observed_main_viewport_data,
        create_window,
        PlatformGraphicsKind::Other,
        #[cfg(feature = "multi-viewport")]
        None,
    )
}

#[cfg(feature = "multi-viewport")]
unsafe extern "C" fn synthetic_create_vk_surface(
    _viewport: *mut sys::ImGuiViewport,
    _instance: sys::ImU64,
    _allocators: *const std::ffi::c_void,
    surface: *mut sys::ImU64,
) -> std::os::raw::c_int {
    VULKAN_SURFACE_CREATE_COUNT.with(|count| count.set(count.get() + 1));
    if let Some(surface) = unsafe { surface.as_mut() } {
        *surface = 0x5151;
    }
    0
}

#[cfg(feature = "multi-viewport")]
unsafe extern "C" fn foreign_create_vk_surface(
    _viewport: *mut sys::ImGuiViewport,
    _instance: sys::ImU64,
    _allocators: *const std::ffi::c_void,
    surface: *mut sys::ImU64,
) -> std::os::raw::c_int {
    FOREIGN_VULKAN_SURFACE_CREATE_COUNT.with(|count| count.set(count.get() + 1));
    if let Some(surface) = unsafe { surface.as_mut() } {
        *surface = 0x6161;
    }
    0
}

#[cfg(feature = "multi-viewport")]
type CreateVkSurfaceFn = unsafe extern "C" fn(
    *mut sys::ImGuiViewport,
    sys::ImU64,
    *const std::ffi::c_void,
    *mut sys::ImU64,
) -> std::os::raw::c_int;

fn synthetic_claimed_registration_with_graphics(
    context: &mut Context,
    renderer_shutdown: Option<Rc<dyn Fn()>>,
    platform_shutdown_hook: Option<Rc<dyn Fn()>>,
    platform_count: Rc<Cell<usize>>,
    observed_backend_data: Rc<Cell<usize>>,
    observed_main_viewport_data: Rc<Cell<usize>>,
    create_window: unsafe extern "C" fn(*mut sys::ImGuiViewport),
    platform_graphics: PlatformGraphicsKind,
    #[cfg(feature = "multi-viewport")] create_vk_surface: Option<CreateVkSurfaceFn>,
) -> RuntimeRegistration {
    let mut registration = registration_with_backend_lifecycle(
        context,
        renderer_shutdown,
        Rc::new({
            let platform_count = Rc::clone(&platform_count);
            let observed_backend_data = Rc::clone(&observed_backend_data);
            let observed_main_viewport_data = Rc::clone(&observed_main_viewport_data);
            let platform_shutdown_hook = platform_shutdown_hook.clone();
            move || unsafe {
                platform_count.set(platform_count.get() + 1);
                if let Some(hook) = &platform_shutdown_hook {
                    hook();
                }
                let io = sys::igGetIO_Nil();
                let platform_io = sys::igGetPlatformIO_Nil();
                let main_viewport = sys::igGetMainViewport();
                observed_backend_data.set((*io).BackendPlatformUserData as usize);
                observed_main_viewport_data.set((*main_viewport).PlatformUserData as usize);

                sys::ImGuiPlatformIO_ClearPlatformHandlers(platform_io);
                (*io).BackendPlatformUserData = std::ptr::null_mut();
                (*io).BackendPlatformName = std::ptr::null();
                (*io).BackendFlags &= !SDL_PLATFORM_RESERVED_FLAGS;
                (*main_viewport).PlatformUserData = std::ptr::null_mut();
                (*main_viewport).PlatformHandle = std::ptr::null_mut();
                (*main_viewport).PlatformHandleRaw = std::ptr::null_mut();
            }
        }),
        platform_graphics,
        NativeRendererKind::None,
    );

    context.binding().with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        let platform_io = sys::igGetPlatformIO_Nil();
        let main_viewport = sys::igGetMainViewport();
        (*io).BackendPlatformUserData = OWNED_BACKEND_DATA as *mut _;
        (*io).BackendPlatformName = OWNED_BACKEND_NAME.as_ptr().cast();
        (*io).BackendFlags |= sys::ImGuiBackendFlags_HasMouseCursors as i32
            | sys::ImGuiBackendFlags_HasSetMousePos as i32
            | sys::ImGuiBackendFlags_PlatformHasViewports as i32
            | sys::ImGuiBackendFlags_HasParentViewport as i32;
        (*platform_io).Platform_CreateWindow = Some(create_window);
        (*platform_io).Platform_DestroyWindow = Some(synthetic_destroy_window);
        (*platform_io).Platform_RenderWindow = Some(synthetic_platform_render_window);
        (*platform_io).Platform_SwapBuffers = Some(synthetic_platform_swap_buffers);
        #[cfg(feature = "multi-viewport")]
        {
            (*platform_io).Platform_CreateVkSurface = create_vk_surface;
        }
        (*platform_io).Platform_ClipboardUserData = OWNED_PLATFORM_DATA as *mut _;
        (*main_viewport).PlatformUserData = OWNED_VIEWPORT_DATA as *mut _;
        (*main_viewport).PlatformHandle = OWNED_VIEWPORT_HANDLE as *mut _;
    });

    let baseline = registration.baseline.take().unwrap();
    let ownership = context.binding().with_bound_context(|| unsafe {
        PlatformCallbackOwnership::claim(&registration.control, baseline).unwrap()
    });
    registration
        .control
        .callbacks
        .borrow_mut()
        .replace(ownership);
    registration.control.platform_initialized.set(true);
    registration
        .control
        .renderer_initialized
        .set(registration.control.lifecycle.renderer_shutdown.is_some());
    registration
}

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
fn synthetic_renderer_registration(context: &mut Context) -> RuntimeRegistration {
    synthetic_renderer_registration_with_pointer_callback(context, None)
}

#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
fn synthetic_renderer_registration_with_pointer_callback(
    context: &mut Context,
    original_pointer_callback: Option<
        unsafe extern "C" fn(*mut sys::ImGuiViewport, *const sys::ImVec2),
    >,
) -> RuntimeRegistration {
    #[cfg(feature = "opengl3-renderer")]
    let native_renderer = NativeRendererKind::OpenGl3;
    #[cfg(all(not(feature = "opengl3-renderer"), feature = "sdlrenderer3-renderer"))]
    let native_renderer = NativeRendererKind::SdlRenderer3;
    #[cfg(all(
        not(feature = "opengl3-renderer"),
        not(feature = "sdlrenderer3-renderer"),
        feature = "sdlgpu3-renderer"
    ))]
    let native_renderer = NativeRendererKind::SdlGpu3;

    let mut registration = registration_with_backend_lifecycle(
        context,
        Some(Rc::new(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            sys::ImGuiPlatformIO_ClearRendererHandlers(platform_io);
            (*io).BackendRendererUserData = std::ptr::null_mut();
            (*io).BackendRendererName = std::ptr::null();
            (*io).BackendFlags &= !SDL_RENDERER_RESERVED_FLAGS;
        })),
        Rc::new(|| unsafe {
            let io = sys::igGetIO_Nil();
            let platform_io = sys::igGetPlatformIO_Nil();
            let main_viewport = sys::igGetMainViewport();
            sys::ImGuiPlatformIO_ClearPlatformHandlers(platform_io);
            (*io).BackendPlatformUserData = std::ptr::null_mut();
            (*io).BackendPlatformName = std::ptr::null();
            (*io).BackendFlags &= !SDL_PLATFORM_RESERVED_FLAGS;
            (*main_viewport).PlatformUserData = std::ptr::null_mut();
            (*main_viewport).PlatformHandle = std::ptr::null_mut();
            (*main_viewport).PlatformHandleRaw = std::ptr::null_mut();
        }),
        PlatformGraphicsKind::Other,
        native_renderer,
    );

    context.binding().with_bound_context(|| unsafe {
        let io = sys::igGetIO_Nil();
        let platform_io = sys::igGetPlatformIO_Nil();
        let main_viewport = sys::igGetMainViewport();
        (*io).BackendPlatformUserData = OWNED_BACKEND_DATA as *mut _;
        (*io).BackendPlatformName = OWNED_BACKEND_NAME.as_ptr().cast();
        (*io).BackendRendererUserData = OWNED_BACKEND_DATA as *mut _;
        (*io).BackendRendererName = OWNED_BACKEND_NAME.as_ptr().cast();
        (*io).BackendFlags |= sys::ImGuiBackendFlags_HasMouseCursors as i32
            | sys::ImGuiBackendFlags_HasSetMousePos as i32
            | sys::ImGuiBackendFlags_PlatformHasViewports as i32
            | sys::ImGuiBackendFlags_HasParentViewport as i32
            | sys::ImGuiBackendFlags_RendererHasViewports as i32;
        (*platform_io).Platform_CreateWindow = Some(synthetic_create_window);
        (*platform_io).Platform_DestroyWindow = Some(synthetic_destroy_window);
        (*platform_io).Renderer_RenderWindow = Some(synthetic_renderer_render_window);
        (*platform_io).Renderer_DestroyWindow = Some(synthetic_renderer_destroy_window);
        (*platform_io).Renderer_SetWindowSize = Some(synthetic_renderer_set_window_size);
        if let Some(original_pointer_callback) = original_pointer_callback {
            sys::ImGuiPlatformIO_Set_Renderer_SetWindowSize_PointerParam(
                platform_io,
                Some(original_pointer_callback),
            );
        }
        (*main_viewport).PlatformUserData = OWNED_VIEWPORT_DATA as *mut _;
        (*main_viewport).PlatformHandle = OWNED_VIEWPORT_HANDLE as *mut _;
    });

    let baseline = registration.baseline.take().unwrap();
    let renderer_baseline = baseline.snapshot();
    let (platform, renderer) = context.binding().with_bound_context(|| unsafe {
        (
            PlatformCallbackOwnership::claim(&registration.control, baseline).unwrap(),
            RendererCallbackOwnership::claim(&registration.control, &renderer_baseline)
                .unwrap()
                .unwrap(),
        )
    });
    registration
        .control
        .callbacks
        .borrow_mut()
        .replace(platform);
    registration
        .control
        .renderer_callbacks
        .borrow_mut()
        .replace(renderer);
    registration.control.platform_initialized.set(true);
    registration.control.renderer_initialized.set(true);
    registration
}

fn registry_contains(key: usize) -> bool {
    super::registration::registry_contains(key)
}

#[cfg(feature = "multi-viewport")]
fn synthetic_vulkan_registration(
    context: &mut Context,
    platform_count: Rc<Cell<usize>>,
) -> RuntimeRegistration {
    synthetic_vulkan_registration_with_callback(
        context,
        platform_count,
        Some(synthetic_create_vk_surface),
    )
}

#[cfg(feature = "multi-viewport")]
fn synthetic_vulkan_registration_with_callback(
    context: &mut Context,
    platform_count: Rc<Cell<usize>>,
    callback: Option<CreateVkSurfaceFn>,
) -> RuntimeRegistration {
    synthetic_claimed_registration_with_graphics(
        context,
        None,
        None,
        platform_count,
        Rc::new(Cell::new(0)),
        Rc::new(Cell::new(0)),
        synthetic_create_window,
        PlatformGraphicsKind::Vulkan,
        callback,
    )
}

struct ActiveExternalRendererMarker;
struct ActiveExternalRendererAttachment;

impl ContextAttachment for ActiveExternalRendererAttachment {}

mod lifecycle;
mod platform_ownership;
#[cfg(any(
    feature = "opengl3-renderer",
    feature = "sdlrenderer3-renderer",
    feature = "sdlgpu3-renderer"
))]
mod renderer_ownership;
mod viewport_callbacks;
