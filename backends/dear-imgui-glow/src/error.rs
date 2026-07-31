//! Error types for the Dear ImGui Glow renderer

use dear_imgui_rs::TextureFormat;
use thiserror::Error;

/// Errors that can occur during renderer initialization
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum InitError {
    /// Failed to create OpenGL buffer object
    #[error("Failed to create buffer object: {0}")]
    CreateBufferObject(String),

    /// Failed to create OpenGL texture
    #[error("Failed to create texture: {0}")]
    CreateTexture(String),

    /// Failed to create an OpenGL sampler object
    #[error("Failed to create sampler object: {0}")]
    CreateSampler(String),

    /// Failed to create OpenGL shader
    #[error("Failed to create shader: {0}")]
    CreateShader(String),

    /// Failed to compile shader
    #[error("Failed to compile shader: {0}")]
    CompileShader(String),

    /// Failed to link shader program
    #[error("Failed to link program: {0}")]
    LinkProgram(String),

    /// Failed to create vertex array object
    #[error("Failed to create vertex array: {0}")]
    CreateVertexArray(String),

    /// OpenGL version not supported
    #[error("Unsupported OpenGL version: {0}")]
    UnsupportedVersion(String),

    /// An owned OpenGL context is required for this operation.
    #[error("No OpenGL context available")]
    MissingGlContext,

    /// A compiled shader program is missing a required vertex attribute.
    #[error("Could not find shader attribute: {0}")]
    MissingShaderAttribute(&'static str),

    /// Texture dimensions overflowed while computing the required byte length.
    #[error("{format:?} texture size overflow")]
    TextureSizeOverflow { format: TextureFormat },

    /// Texture dimensions do not fit OpenGL's signed size parameters.
    #[error("Texture {dimension} is out of range for OpenGL: {value}")]
    TextureDimensionOutOfRange { dimension: &'static str, value: u32 },

    /// Texture byte length does not match the expected size for the format.
    #[error("{format:?} texture data size mismatch: expected {expected} bytes, got {actual}")]
    TextureDataSizeMismatch {
        format: TextureFormat,
        expected: usize,
        actual: usize,
    },

    /// TextureId zero/null is not valid for this operation.
    #[error("TextureId must be non-zero for OpenGL")]
    NullTextureId,

    /// TextureId allocation space was exhausted.
    #[error("TextureId allocation space is exhausted")]
    TextureIdExhausted,

    /// The texture map does not contain the requested texture ID.
    #[error("TextureId is not registered: {0:?}")]
    UnknownTextureId(dear_imgui_rs::TextureId),

    /// A texture upload row is shorter than the pixels required by its format.
    #[error("{format:?} texture row pitch is too small: expected at least {minimum}, got {actual}")]
    TextureRowPitchTooSmall {
        format: TextureFormat,
        minimum: usize,
        actual: usize,
    },

    /// A managed texture update rectangle lies outside its texture allocation.
    #[error(
        "texture update rectangle ({x}, {y}, {width}, {height}) exceeds texture size {texture_width}x{texture_height}"
    )]
    TextureUpdateOutOfBounds {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        texture_width: u32,
        texture_height: u32,
    },

    /// The Context could not register this renderer's sole consumer capability.
    #[error(transparent)]
    RendererConsumer(#[from] dear_imgui_rs::render::RendererConsumerError),

    /// A renderer-owned Context state slot was already occupied.
    #[error("Glow renderer state slot `{field}` is already occupied")]
    RendererStateOccupied { field: &'static str },

    /// A renderer callback slot was already occupied.
    #[error("Glow renderer callback `{callback}` is already occupied")]
    RendererCallbackOccupied { callback: &'static str },

    /// One or more flags reserved by Glow were already set.
    #[error("Glow renderer capability flags {flags:#x} are already occupied")]
    RendererCapabilityOccupied { flags: i32 },

    /// Generic initialization error
    #[error("Initialization error: {0}")]
    Generic(String),
}

// Display and Error traits are automatically implemented by thiserror

/// Errors that can occur during rendering
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum RenderError {
    /// OpenGL error
    #[error("OpenGL error: {0}")]
    OpenGLError(String),

    /// A draw command references a texture that is not registered with this renderer.
    #[error("texture ID is not registered: {0:?}")]
    UnknownTextureId(dear_imgui_rs::TextureId),

    /// A managed texture update arrived without its matching GPU allocation.
    #[error("managed texture {0:?} received an update before creation")]
    ManagedTextureMissing(dear_imgui_rs::render::SnapshotTextureId),

    /// Draw data produced a non-finite framebuffer or projection value.
    #[error("draw data field `{field}` is not finite: {value}")]
    NonFiniteDrawValue { field: &'static str, value: f32 },

    /// A positive framebuffer dimension does not fit OpenGL's signed viewport parameters.
    #[error("framebuffer {dimension} is out of range for OpenGL: {value}")]
    FramebufferDimensionOutOfRange { dimension: &'static str, value: f64 },

    /// A draw command contains a non-finite clip rectangle.
    #[error("draw command contains a non-finite clip rectangle: {0:?}")]
    NonFiniteClipRect([f32; 4]),

    /// A draw count or offset does not fit OpenGL's signed draw parameters.
    #[error("{field} exceeds OpenGL draw parameter limits: {value}")]
    DrawParameterOutOfRange { field: &'static str, value: usize },

    /// A draw command's index range overflows or exceeds its parent draw list.
    #[error("draw command index range {start}..{end} exceeds index buffer length {len}")]
    DrawCommandIndexRangeOutOfBounds {
        start: usize,
        end: usize,
        len: usize,
    },

    /// A draw command references a vertex outside its parent draw list.
    #[error("draw command references vertex {index}, but vertex buffer length is {len}")]
    DrawCommandVertexOutOfBounds { index: usize, len: usize },

    /// A command requested a base vertex on a context that cannot draw with one.
    #[error(
        "draw command uses vertex offset {offset}, but this OpenGL context does not support it"
    )]
    VertexOffsetUnsupported { offset: usize },

    /// Desktop `GL_FRAMEBUFFER_SRGB` control was requested on OpenGL ES or WebGL.
    #[error("GL_FRAMEBUFFER_SRGB control is unavailable on OpenGL ES and WebGL contexts")]
    FramebufferSrgbUnsupported,

    /// Renderer was destroyed
    #[error("Renderer was destroyed")]
    RendererDestroyed,

    /// An OpenGL context is required for this operation.
    #[error(
        "No OpenGL context available. Use the matching *_with_context method for externally managed contexts."
    )]
    MissingGlContext,

    /// Renderer device object initialization failed.
    #[error("Device object initialization failed: {0}")]
    DeviceObjectInit(#[source] InitError),

    /// Failed to create an OpenGL resource while rendering.
    #[error("Failed to create {resource}: {error}")]
    CreateResource {
        resource: &'static str,
        error: String,
    },

    /// This renderer is no longer attached to a Context consumer.
    #[error("Glow renderer is not attached to a Dear ImGui renderer consumer")]
    RendererNotAttached,

    /// A rendered frame belongs to a different Context.
    #[error("rendered frame belongs to Context {actual:?}, not Context {expected:?}")]
    ContextMismatch {
        expected: dear_imgui_rs::ContextId,
        actual: dear_imgui_rs::ContextId,
    },

    /// A frame was rendered without the managed-texture consumer epoch Glow requires.
    #[error("rendered frame does not carry a managed-texture renderer epoch")]
    MissingRendererEpoch,

    /// A rendered frame belongs to an obsolete or foreign consumer generation.
    #[error(
        "rendered frame consumer generation {actual} does not match renderer generation {expected}"
    )]
    ConsumerGenerationMismatch { expected: u64, actual: u64 },

    /// Context-owned renderer epoch validation failed.
    #[error(transparent)]
    RendererConsumer(#[from] dear_imgui_rs::render::RendererConsumerError),

    /// A custom texture map panicked while preparing renderer-owned resources for release.
    #[error("custom TextureMap::clear panicked during renderer teardown")]
    TextureMapCleanupPanicked,

    /// A Context-owned renderer state slot changed after Glow published it.
    #[error("Glow renderer state slot `{field}` drifted while attached")]
    RendererStateDrift { field: &'static str },

    /// The current Dear ImGui Context has no PlatformIO for transient callback state.
    #[error("the bound Dear ImGui Context has no PlatformIO")]
    MissingPlatformIo,

    /// A renderer callback owned by Glow was replaced while attached.
    #[error("Glow renderer callback `{callback}` was replaced while attached")]
    RendererCallbackReplaced { callback: &'static str },

    /// A capability reserved by Glow disappeared while attached.
    #[error("Glow renderer capability `{flag}` drifted while attached")]
    RendererCapabilityDrift { flag: &'static str },

    /// The originating Context can no longer be entered for state validation.
    #[error(transparent)]
    ContextBinding(#[from] dear_imgui_rs::ContextBindingError),

    /// Texture feedback was constructed for the wrong request kind.
    #[error(transparent)]
    TextureFeedback(#[from] dear_imgui_rs::render::TextureFeedbackError),

    /// Generic rendering error
    #[error("Rendering error: {0}")]
    Generic(String),
}

// Display and Error traits are automatically implemented by thiserror

/// Result type for initialization operations
pub type InitResult<T> = Result<T, InitError>;

/// Result type for rendering operations
pub type RenderResult<T> = Result<T, RenderError>;
