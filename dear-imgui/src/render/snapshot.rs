//! Pointer-free rendering snapshots.
//!
//! A [`FrameSnapshot`] is created by an owning [`crate::Context`] for one registered
//! [`DetachedRendererConsumer`]. It can cross threads, but it cannot be cloned or constructed
//! from arbitrary native draw data. Dropping it reports an abandoned epoch;
//! [`FrameSnapshot::commit`] reports renderer feedback for ordered reconciliation by the Context.

use std::collections::HashSet;
use std::marker::PhantomData;
use std::num::NonZeroU64;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use crate::render::draw_data::{
    DrawData, DrawIdx, DrawList, DrawVert, StandardDrawCallback, classify_standard_draw_callback,
};
use crate::sys;
use crate::texture::{
    ManagedTextureError, ManagedTextureId, TextureFormat, TextureId, TextureRect, TextureStatus,
};
use crate::{ContextId, Id};
use thiserror::Error;

/// Pointer-free identity used by detached renderers.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum SnapshotTextureId {
    /// A Context-owned user texture.
    User(ManagedTextureId),
    /// One live or retiring texture allocation of the Context's font atlas.
    FontAtlas {
        /// Context that produced this snapshot.
        context: ContextId,
        /// Opaque namespace for this atlas's current managed-renderer ownership period.
        stamp: u64,
        /// Atlas-local allocation generation captured by this snapshot.
        generation: u64,
    },
}

/// How a draw command binds its texture.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum TextureBinding {
    /// Application-owned texture binding.
    Legacy(TextureId),
    /// Context-resolved managed texture binding.
    Managed(SnapshotTextureId),
}

/// Context, consumer generation, and ordered sequence for one detached frame.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct SnapshotEpoch {
    context: ContextId,
    consumer_generation: NonZeroU64,
    sequence: NonZeroU64,
}

impl SnapshotEpoch {
    pub(crate) const fn new(
        context: ContextId,
        consumer_generation: NonZeroU64,
        sequence: NonZeroU64,
    ) -> Self {
        Self {
            context,
            consumer_generation,
            sequence,
        }
    }

    /// Context that produced the epoch.
    #[must_use]
    pub const fn context_id(self) -> ContextId {
        self.context
    }

    /// Generation of the registered renderer consumer.
    #[must_use]
    pub const fn consumer_generation(self) -> u64 {
        self.consumer_generation.get()
    }

    /// Monotonic Context-local epoch sequence.
    #[must_use]
    pub const fn sequence(self) -> u64 {
        self.sequence.get()
    }

    pub(crate) const fn consumer_generation_raw(self) -> NonZeroU64 {
        self.consumer_generation
    }
}

struct RendererConsumerState {
    context: ContextId,
    generation: NonZeroU64,
    sender: Sender<SnapshotMessage>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl RendererConsumerState {
    fn new(context: ContextId, generation: NonZeroU64, sender: Sender<SnapshotMessage>) -> Self {
        Self {
            context,
            generation,
            sender,
            _not_send_or_sync: PhantomData,
        }
    }
}

impl Drop for RendererConsumerState {
    fn drop(&mut self) {
        let _ = self.sender.send(SnapshotMessage::Detach {
            context: self.context,
            generation: self.generation,
        });
    }
}

mod consumer_sealed {
    pub trait Sealed {}
}

/// Shared read-only identity implemented by the two renderer consumer capabilities.
///
/// This trait is sealed. It exists so lifecycle operations such as renderer texture reset can
/// accept either consumer kind without erasing the distinction at frame and snapshot entry points.
pub trait RendererConsumerCapability: consumer_sealed::Sealed {
    /// Context that owns this consumer.
    fn context_id(&self) -> ContextId;

    /// Current consumer generation.
    fn generation(&self) -> u64;
}

/// Non-cloneable capability for Context-borrowed synchronous rendering.
///
/// Create it with [`crate::Context::create_synchronous_renderer_consumer`]. It cannot be used to
/// create detached snapshots.
#[must_use = "keep the consumer alive while rendering managed texture requests"]
pub struct SynchronousRendererConsumer(RendererConsumerState);

impl SynchronousRendererConsumer {
    pub(crate) fn new(
        context: ContextId,
        generation: NonZeroU64,
        sender: Sender<SnapshotMessage>,
    ) -> Self {
        Self(RendererConsumerState::new(context, generation, sender))
    }

    /// Context that owns this consumer.
    #[must_use]
    pub const fn context_id(&self) -> ContextId {
        self.0.context
    }

    /// Current consumer generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.0.generation.get()
    }
}

impl std::fmt::Debug for SynchronousRendererConsumer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SynchronousRendererConsumer")
            .field("context", &self.0.context)
            .field("generation", &self.0.generation)
            .finish_non_exhaustive()
    }
}

impl consumer_sealed::Sealed for SynchronousRendererConsumer {}

impl RendererConsumerCapability for SynchronousRendererConsumer {
    fn context_id(&self) -> ContextId {
        self.context_id()
    }

    fn generation(&self) -> u64 {
        self.generation()
    }
}

/// Non-cloneable capability for pointer-free detached rendering.
///
/// Create it with [`crate::Context::create_detached_renderer_consumer`]. Snapshots created with
/// this capability are `Send + Sync`; the capability itself remains UI-thread bound.
#[must_use = "keep the consumer alive while detached snapshots or completions remain active"]
pub struct DetachedRendererConsumer(RendererConsumerState);

impl DetachedRendererConsumer {
    pub(crate) fn new(
        context: ContextId,
        generation: NonZeroU64,
        sender: Sender<SnapshotMessage>,
    ) -> Self {
        Self(RendererConsumerState::new(context, generation, sender))
    }

    /// Context that owns this consumer.
    #[must_use]
    pub const fn context_id(&self) -> ContextId {
        self.0.context
    }

    /// Current consumer generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.0.generation.get()
    }
}

impl std::fmt::Debug for DetachedRendererConsumer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DetachedRendererConsumer")
            .field("context", &self.0.context)
            .field("generation", &self.0.generation)
            .finish_non_exhaustive()
    }
}

impl consumer_sealed::Sealed for DetachedRendererConsumer {}

impl RendererConsumerCapability for DetachedRendererConsumer {
    fn context_id(&self) -> ContextId {
        self.context_id()
    }

    fn generation(&self) -> u64 {
        self.generation()
    }
}

/// A thread-safe snapshot of everything needed to render one frame.
///
/// This type is intentionally not `Clone`. It owns exactly one completion ticket.
///
/// ```compile_fail
/// use dear_imgui_rs::render::FrameSnapshot;
///
/// fn duplicate(snapshot: FrameSnapshot) {
///     let _copy = snapshot.clone();
/// }
/// ```
///
/// Snapshot contents are read-only so their completion ticket and request set stay coherent.
///
/// ```compile_fail
/// use dear_imgui_rs::render::FrameSnapshot;
///
/// fn discard_requests(snapshot: &mut FrameSnapshot) {
///     snapshot.texture_requests.clear();
/// }
/// ```
#[derive(Debug)]
pub struct FrameSnapshot {
    main_draw: MainDrawSnapshot,
    viewports: Vec<ViewportDrawDataSnapshot>,
    texture_requests: Vec<TextureRequest>,
    epoch: SnapshotEpoch,
    completion: CompletionTicket,
}

impl FrameSnapshot {
    /// Main viewport draw data.
    #[must_use]
    pub const fn draw_data(&self) -> &DrawDataSnapshot {
        self.main_draw.draw_data(self.viewports.as_slice())
    }

    /// Per-viewport draw data captured for this frame.
    #[must_use]
    pub fn viewports(&self) -> &[ViewportDrawDataSnapshot] {
        &self.viewports
    }

    /// Managed texture work associated with this epoch.
    #[must_use]
    pub fn texture_requests(&self) -> &[TextureRequest] {
        &self.texture_requests
    }

    /// Ordered identity of this detached frame.
    #[must_use]
    pub const fn epoch(&self) -> SnapshotEpoch {
        self.epoch
    }

    /// Draw data for a specific viewport, if captured.
    #[must_use]
    pub fn viewport_draw(&self, viewport_id: Id) -> Option<&DrawDataSnapshot> {
        self.viewports
            .iter()
            .find(|viewport| viewport.viewport_id == viewport_id)
            .map(|viewport| &viewport.draw)
    }

    /// Commit exactly one renderer outcome for every request and complete this epoch.
    ///
    /// Snapshot-local feedback is validated before it is sent to the owning Context. Use
    /// [`TextureRequest::retry`] for work that should be emitted again and
    /// [`TextureRequest::superseded`] for a request the renderer deliberately did not apply.
    /// Stateful validation and mutation still occur only when this epoch reaches the Context's
    /// contiguous completion watermark.
    pub fn commit(
        self,
        feedback: impl IntoIterator<Item = TextureFeedback>,
    ) -> Result<(), SnapshotCommitError> {
        let feedback = feedback.into_iter().collect::<Vec<_>>();
        let expected = self
            .texture_requests
            .iter()
            .map(|request| request.key)
            .collect::<HashSet<_>>();
        validate_texture_feedback(self.epoch, &expected, &feedback)?;
        self.completion.commit(feedback)
    }
}

#[derive(Debug)]
enum MainDrawSnapshot {
    Standalone(DrawDataSnapshot),
    Viewport(usize),
}

impl MainDrawSnapshot {
    const fn draw_data<'a>(
        &'a self,
        viewports: &'a [ViewportDrawDataSnapshot],
    ) -> &'a DrawDataSnapshot {
        match self {
            Self::Standalone(draw) => draw,
            Self::Viewport(index) => &viewports[*index].draw,
        }
    }
}

/// Thread-safe draw data for one Dear ImGui viewport.
///
/// The main-viewport role is captured with the source Context and remains meaningful after the
/// native viewport and Context are no longer current.
#[derive(Debug)]
pub struct ViewportDrawDataSnapshot {
    pub viewport_id: Id,
    pub draw: DrawDataSnapshot,
    is_main: bool,
}

impl ViewportDrawDataSnapshot {
    /// Construct detached draw data with its Context-relative viewport role captured explicitly.
    ///
    /// Pass the result of [`crate::platform_io::Viewport::is_main`] from the live source viewport;
    /// do not infer `is_main` from the numeric viewport ID.
    ///
    /// ```
    /// use dear_imgui_rs::{
    ///     Id,
    ///     render::{DrawDataSnapshot, ViewportDrawDataSnapshot},
    /// };
    ///
    /// let draw = DrawDataSnapshot {
    ///     frame_count: 1,
    ///     display_pos: [0.0, 0.0],
    ///     display_size: [640.0, 480.0],
    ///     framebuffer_scale: [1.0, 1.0],
    ///     draw_lists: Vec::new(),
    /// };
    /// let viewport = ViewportDrawDataSnapshot::new(Id::from(7_u32), true, draw);
    /// assert!(viewport.is_main());
    /// ```
    #[must_use]
    pub const fn new(viewport_id: Id, is_main: bool, draw: DrawDataSnapshot) -> Self {
        Self {
            viewport_id,
            draw,
            is_main,
        }
    }

    /// Whether this was the source Context's main viewport when the snapshot was captured.
    #[must_use]
    pub const fn is_main(&self) -> bool {
        self.is_main
    }
}

/// Thread-safe draw data snapshot.
#[derive(Debug)]
pub struct DrawDataSnapshot {
    /// Frame counter of the Context that emitted this draw data.
    pub frame_count: usize,
    pub display_pos: [f32; 2],
    pub display_size: [f32; 2],
    pub framebuffer_scale: [f32; 2],
    pub draw_lists: Vec<DrawListSnapshot>,
}

/// Thread-safe draw list snapshot.
#[derive(Debug)]
pub struct DrawListSnapshot {
    pub vtx: Vec<DrawVert>,
    pub idx: Vec<DrawIdx>,
    pub commands: Vec<DrawCmdSnapshot>,
}

/// Thread-safe draw command snapshot.
#[derive(Debug)]
pub enum DrawCmdSnapshot {
    Elements {
        count: usize,
        clip_rect: [f32; 4],
        texture: TextureBinding,
        vtx_offset: usize,
        idx_offset: usize,
    },
    ResetRenderState,
    SetSamplerLinear,
    SetSamplerNearest,
}

/// Operation kind encoded into a texture request and its feedback.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum TextureRequestKind {
    Create,
    Update,
    Destroy,
}

/// Opaque identity for one managed texture upload request.
///
/// Pair this value with [`TextureRequest::texture`]. It is stable across retries of the same
/// create or update request. Equality is meaningful only for the same texture; identities from
/// different textures have no uniqueness or ordering semantics.
///
/// The identity intentionally exposes no representation:
///
/// ```compile_fail
/// use dear_imgui_rs::render::TextureUploadIdentity;
///
/// fn reveal(identity: TextureUploadIdentity) {
///     let TextureUploadIdentity {} = identity;
/// }
/// ```
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct TextureUploadIdentity {
    revision: u64,
    kind: TextureRequestKind,
}

impl std::fmt::Debug for TextureUploadIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("TextureUploadIdentity(..)")
    }
}

/// A managed texture operation requested by Dear ImGui.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextureOp {
    Create {
        format: TextureFormat,
        width: u32,
        height: u32,
        row_pitch: usize,
        pixels: Vec<u8>,
    },
    Update {
        format: TextureFormat,
        width: u32,
        height: u32,
        rects: Vec<TextureUploadRect>,
    },
    Destroy,
}

impl TextureOp {
    const fn kind(&self) -> TextureRequestKind {
        match self {
            Self::Create { .. } => TextureRequestKind::Create,
            Self::Update { .. } => TextureRequestKind::Update,
            Self::Destroy => TextureRequestKind::Destroy,
        }
    }
}

/// A tightly-packed pixel upload for a sub-rectangle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextureUploadRect {
    pub rect: TextureRect,
    pub row_pitch: usize,
    pub data: Vec<u8>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct TextureRequestKey {
    pub(crate) epoch: SnapshotEpoch,
    pub(crate) texture: SnapshotTextureId,
    pub(crate) revision: u64,
    pub(crate) kind: TextureRequestKind,
}

/// One texture request tied to this snapshot's exact epoch and revision.
#[derive(Debug)]
pub struct TextureRequest {
    key: TextureRequestKey,
    op: Arc<TextureOp>,
}

impl TextureRequest {
    /// Texture addressed by this request.
    #[must_use]
    pub const fn texture(&self) -> SnapshotTextureId {
        self.key.texture
    }

    /// Request operation and owned upload bytes.
    #[must_use]
    pub fn operation(&self) -> &TextureOp {
        self.op.as_ref()
    }

    /// Operation kind encoded into feedback validation.
    #[must_use]
    pub const fn kind(&self) -> TextureRequestKind {
        self.key.kind
    }

    /// Opaque retry identity for a create or update request.
    ///
    /// Pair this value with [`Self::texture`]. The identity is stable across retries of the same
    /// request. Equality is meaningful only for the same texture; identities from different
    /// textures have no uniqueness or ordering semantics. Destroy requests return `None`.
    #[must_use]
    pub const fn upload_identity(&self) -> Option<TextureUploadIdentity> {
        match self.key.kind {
            TextureRequestKind::Create | TextureRequestKind::Update => {
                Some(TextureUploadIdentity {
                    revision: self.key.revision,
                    kind: self.key.kind,
                })
            }
            TextureRequestKind::Destroy => None,
        }
    }

    /// Complete a create or update request with its renderer texture identifier.
    pub fn uploaded(&self, texture_id: TextureId) -> Result<TextureFeedback, TextureFeedbackError> {
        if self.key.kind == TextureRequestKind::Destroy {
            return Err(TextureFeedbackError::UploadForDestroy);
        }
        if texture_id.is_null() {
            return Err(TextureFeedbackError::NullTextureId);
        }
        Ok(TextureFeedback {
            key: self.key,
            result: TextureFeedbackResult::Uploaded { texture_id },
        })
    }

    /// Complete a destroy request.
    pub fn destroyed(&self) -> Result<TextureFeedback, TextureFeedbackError> {
        if self.key.kind != TextureRequestKind::Destroy {
            return Err(TextureFeedbackError::DestroyForUpload);
        }
        Ok(TextureFeedback {
            key: self.key,
            result: TextureFeedbackResult::Destroyed,
        })
    }

    /// Complete this request without mutating its Context-owned binding.
    ///
    /// This is appropriate when renderer-local identity or tombstone state proves the captured
    /// request no longer applies. If the Context still considers the operation current, it may be
    /// emitted again in a later frame.
    #[must_use]
    pub const fn superseded(&self) -> TextureFeedback {
        TextureFeedback {
            key: self.key,
            result: TextureFeedbackResult::Superseded,
        }
    }

    /// Leave this request pending so a later frame can retry it.
    #[must_use]
    pub const fn retry(&self) -> TextureFeedback {
        TextureFeedback {
            key: self.key,
            result: TextureFeedbackResult::Retry,
        }
    }
}

/// Feedback produced by the detached renderer.
#[derive(Debug)]
pub struct TextureFeedback {
    key: TextureRequestKey,
    result: TextureFeedbackResult,
}

impl TextureFeedback {
    pub(crate) const fn key(&self) -> TextureRequestKey {
        self.key
    }

    pub(crate) const fn result(&self) -> TextureFeedbackResult {
        self.result
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum TextureFeedbackResult {
    Uploaded { texture_id: TextureId },
    Destroyed,
    Superseded,
    Retry,
}

pub(crate) fn validate_texture_feedback(
    epoch: SnapshotEpoch,
    expected: &HashSet<TextureRequestKey>,
    feedback: &[TextureFeedback],
) -> Result<(), RendererConsumerError> {
    let mut seen = HashSet::with_capacity(feedback.len());
    for item in feedback {
        let key = item.key();
        if key.epoch.context_id() != epoch.context_id() {
            return Err(RendererConsumerError::ForeignContext {
                expected: epoch.context_id(),
                actual: key.epoch.context_id(),
            });
        }
        if key.epoch.consumer_generation_raw() != epoch.consumer_generation_raw() {
            return Err(RendererConsumerError::StaleConsumerGeneration {
                expected: epoch.consumer_generation(),
                actual: key.epoch.consumer_generation(),
            });
        }
        if key.epoch.sequence() != epoch.sequence() || !expected.contains(&key) {
            return Err(RendererConsumerError::FeedbackNotRequested {
                epoch: epoch.sequence(),
                texture: key.texture,
            });
        }
        if !seen.insert(key) {
            return Err(RendererConsumerError::DuplicateFeedback {
                epoch: epoch.sequence(),
                texture: key.texture,
            });
        }
        let transition_is_valid = match (key.kind, item.result()) {
            (
                TextureRequestKind::Create | TextureRequestKind::Update,
                TextureFeedbackResult::Uploaded { texture_id },
            ) => !texture_id.is_null(),
            (TextureRequestKind::Destroy, TextureFeedbackResult::Destroyed)
            | (_, TextureFeedbackResult::Superseded | TextureFeedbackResult::Retry) => true,
            _ => false,
        };
        if !transition_is_valid {
            return Err(RendererConsumerError::InvalidFeedbackTransition {
                texture: key.texture,
            });
        }
    }
    let missing = expected.len().saturating_sub(seen.len());
    if missing != 0 {
        return Err(RendererConsumerError::MissingFeedback {
            epoch: epoch.sequence(),
            count: missing,
        });
    }
    Ok(())
}

/// Error returned when feedback does not match the request operation.
#[derive(Copy, Clone, Debug, Eq, Error, PartialEq)]
pub enum TextureFeedbackError {
    #[error("an upload result cannot complete a destroy request")]
    UploadForDestroy,
    #[error("a destroy result cannot complete a create or update request")]
    DestroyForUpload,
    #[error("an upload result requires a non-null renderer texture identifier")]
    NullTextureId,
}

/// Error returned when a snapshot cannot be captured.
#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("user callback commands are not supported by detached snapshots")]
    UserCallbackUnsupported,
    #[error("draw data contains a managed texture not owned by this Context or its font atlas")]
    UnknownManagedTexture,
    #[error("managed texture {id:?} has status {status:?} but no pixel buffer is available")]
    TexturePixelsMissing {
        id: SnapshotTextureId,
        status: TextureStatus,
    },
    #[error(
        "managed texture {id:?} has invalid dimensions/format (width={width}, height={height}, bpp={bpp})"
    )]
    TextureInvalidLayout {
        id: SnapshotTextureId,
        width: i32,
        height: i32,
        bpp: i32,
    },
    #[error(
        "managed texture {id:?} full update exceeds ImTextureRect limits (width={width}, height={height})"
    )]
    TextureFullUpdateOutOfRange {
        id: SnapshotTextureId,
        width: u32,
        height: u32,
    },
    #[error(transparent)]
    Consumer(#[from] RendererConsumerError),
    #[error(transparent)]
    ManagedTexture(#[from] ManagedTextureError),
}

/// Renderer registration or completion contract violation.
#[derive(Copy, Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum RendererConsumerError {
    #[error("this Context already has an active renderer consumer")]
    ConsumerAlreadyActive,
    #[error(
        "managed font-atlas rendering requires exactly one registered Context; found {registered_contexts}"
    )]
    SharedFontAtlasRequiresExclusiveContext { registered_contexts: usize },
    #[error(
        "the font atlas contains legacy-preloaded data; clear and repopulate it before attaching a managed renderer"
    )]
    FontAtlasRequiresManagedRebuild,
    #[error(
        "the shared font atlas still belongs to a renderer whose texture release was not committed"
    )]
    SharedFontAtlasRendererReleasePending,
    #[error("the previous renderer consumer is still draining outstanding epochs")]
    ConsumerDraining,
    #[error("this Context has no active renderer consumer")]
    NoActiveConsumer,
    #[error("{caller} requires a renderer that advertises RENDERER_HAS_TEXTURES")]
    RendererTexturesUnavailable { caller: &'static str },
    #[error("renderer consumer belongs to Context {actual:?}, not Context {expected:?}")]
    ForeignContext {
        expected: ContextId,
        actual: ContextId,
    },
    #[error("renderer consumer generation {actual} is stale; current generation is {expected}")]
    StaleConsumerGeneration { expected: u64, actual: u64 },
    #[error("renderer consumer generation space is exhausted")]
    ConsumerGenerationExhausted,
    #[error("snapshot epoch space is exhausted")]
    EpochExhausted,
    #[error("snapshot completion references unknown epoch {epoch}")]
    UnknownEpoch { epoch: u64 },
    #[error("snapshot epoch {epoch} was completed more than once")]
    EpochAlreadyCompleted { epoch: u64 },
    #[error("renderer consumer still owns {count} outstanding epoch(s)")]
    OutstandingEpochs { count: usize },
    #[error("snapshot epoch {epoch} contains duplicate feedback for {texture:?}")]
    DuplicateFeedback {
        epoch: u64,
        texture: SnapshotTextureId,
    },
    #[error("snapshot epoch {epoch} is missing {count} required feedback outcome(s)")]
    MissingFeedback { epoch: u64, count: usize },
    #[error("snapshot epoch {epoch} did not request feedback for {texture:?}")]
    FeedbackNotRequested {
        epoch: u64,
        texture: SnapshotTextureId,
    },
    #[error("feedback result does not match the request kind for {texture:?}")]
    InvalidFeedbackTransition { texture: SnapshotTextureId },
    #[error("font-atlas feedback targets a stale atlas allocation or generation")]
    StaleFontAtlas,
    #[error(transparent)]
    ManagedTexture(#[from] ManagedTextureError),
}

/// Failure to deliver completion after the owning Context was destroyed.
#[derive(Copy, Clone, Debug, Eq, Error, PartialEq)]
pub enum SnapshotCommitError {
    #[error(transparent)]
    InvalidFeedback(#[from] RendererConsumerError),
    #[error("the snapshot's owning Context no longer accepts completion")]
    ContextDropped,
}

/// Work applied while polling detached completions.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct SnapshotCompletionProgress {
    pub(crate) watermark: u64,
    pub(crate) committed: usize,
    pub(crate) abandoned: usize,
    pub(crate) feedback_applied: usize,
}

impl SnapshotCompletionProgress {
    /// Highest contiguous completed epoch sequence.
    #[must_use]
    pub const fn watermark(self) -> u64 {
        self.watermark
    }

    /// Number of committed epochs consumed by this poll.
    #[must_use]
    pub const fn committed(self) -> usize {
        self.committed
    }

    /// Number of abandoned epochs consumed by this poll.
    #[must_use]
    pub const fn abandoned(self) -> usize {
        self.abandoned
    }

    /// Number of feedback items applied by this poll.
    #[must_use]
    pub const fn feedback_applied(self) -> usize {
        self.feedback_applied
    }
}

#[derive(Debug)]
struct CompletionTicket {
    epoch: SnapshotEpoch,
    sender: Sender<SnapshotMessage>,
    completed: bool,
}

impl CompletionTicket {
    fn commit(mut self, feedback: Vec<TextureFeedback>) -> Result<(), SnapshotCommitError> {
        self.completed = true;
        self.sender
            .send(SnapshotMessage::Completion(SnapshotCompletion {
                epoch: self.epoch,
                outcome: SnapshotCompletionOutcome::Committed(feedback),
            }))
            .map_err(|_| SnapshotCommitError::ContextDropped)
    }
}

impl Drop for CompletionTicket {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        let _ = self
            .sender
            .send(SnapshotMessage::Completion(SnapshotCompletion {
                epoch: self.epoch,
                outcome: SnapshotCompletionOutcome::Abandoned,
            }));
        self.completed = true;
    }
}

#[derive(Debug)]
pub(crate) enum SnapshotMessage {
    Completion(SnapshotCompletion),
    Detach {
        context: ContextId,
        generation: NonZeroU64,
    },
}

#[derive(Debug)]
pub(crate) struct SnapshotCompletion {
    pub(crate) epoch: SnapshotEpoch,
    pub(crate) outcome: SnapshotCompletionOutcome,
}

#[derive(Debug)]
pub(crate) enum SnapshotCompletionOutcome {
    Committed(Vec<TextureFeedback>),
    Abandoned,
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct ResolvedSnapshotTexture {
    pub(crate) id: SnapshotTextureId,
    pub(crate) revision: u64,
}

#[derive(Debug)]
pub(crate) struct PendingTextureRequest {
    pub(crate) texture: SnapshotTextureId,
    pub(crate) revision: u64,
    pub(crate) op: Arc<TextureOp>,
}

#[derive(Debug)]
pub(crate) struct PendingSnapshot {
    main_draw: MainDrawSnapshot,
    pub(crate) viewports: Vec<ViewportDrawDataSnapshot>,
    pub(crate) texture_requests: Vec<PendingTextureRequest>,
}

impl PendingSnapshot {
    fn draw_data(&self) -> &DrawDataSnapshot {
        self.main_draw.draw_data(&self.viewports)
    }

    pub(crate) fn referenced_user_textures(&self) -> HashSet<ManagedTextureId> {
        let mut referenced = HashSet::new();
        if matches!(&self.main_draw, MainDrawSnapshot::Standalone(_)) {
            collect_referenced_user_textures(self.draw_data(), &mut referenced);
        }
        for viewport in &self.viewports {
            collect_referenced_user_textures(&viewport.draw, &mut referenced);
        }
        for request in &self.texture_requests {
            if let SnapshotTextureId::User(id) = request.texture {
                referenced.insert(id);
            }
        }
        referenced
    }

    pub(crate) fn into_frame(
        self,
        epoch: SnapshotEpoch,
        sender: Sender<SnapshotMessage>,
    ) -> (FrameSnapshot, HashSet<TextureRequestKey>) {
        let (texture_requests, expected) = finalize_texture_requests(self.texture_requests, epoch);
        (
            FrameSnapshot {
                main_draw: self.main_draw,
                viewports: self.viewports,
                texture_requests,
                epoch,
                completion: CompletionTicket {
                    epoch,
                    sender,
                    completed: false,
                },
            },
            expected,
        )
    }
}

fn collect_referenced_user_textures(
    draw: &DrawDataSnapshot,
    referenced: &mut HashSet<ManagedTextureId>,
) {
    for list in &draw.draw_lists {
        for command in &list.commands {
            if let DrawCmdSnapshot::Elements {
                texture: TextureBinding::Managed(SnapshotTextureId::User(id)),
                ..
            } = command
            {
                referenced.insert(*id);
            }
        }
    }
}

pub(crate) fn finalize_texture_requests(
    pending: Vec<PendingTextureRequest>,
    epoch: SnapshotEpoch,
) -> (Vec<TextureRequest>, HashSet<TextureRequestKey>) {
    let mut expected = HashSet::with_capacity(pending.len());
    let texture_requests = pending
        .into_iter()
        .map(|request| {
            let key = TextureRequestKey {
                epoch,
                texture: request.texture,
                revision: request.revision,
                kind: request.op.kind(),
            };
            expected.insert(key);
            TextureRequest {
                key,
                op: request.op,
            }
        })
        .collect();
    (texture_requests, expected)
}

pub(crate) fn capture_texture_requests_only(
    draw_data: &DrawData,
    resolve: &mut impl FnMut(
        *const sys::ImTextureData,
    ) -> Result<ResolvedSnapshotTexture, SnapshotError>,
) -> Result<Vec<PendingTextureRequest>, SnapshotError> {
    snapshot_texture_requests(draw_data, resolve)
}

pub(crate) fn capture_draw_data(
    draw_data: &DrawData,
    resolve: &mut impl FnMut(
        *const sys::ImTextureData,
    ) -> Result<ResolvedSnapshotTexture, SnapshotError>,
) -> Result<PendingSnapshot, SnapshotError> {
    preflight_detached_callbacks(draw_data)?;
    let draw = snapshot_draw_data(draw_data, resolve)?;
    let texture_requests = snapshot_texture_requests(draw_data, resolve)?;
    let (main_draw, viewports) = match owner_viewport_identity(draw_data) {
        Some((viewport_id, is_main)) => (
            MainDrawSnapshot::Viewport(0),
            vec![ViewportDrawDataSnapshot::new(viewport_id, is_main, draw)],
        ),
        None => (MainDrawSnapshot::Standalone(draw), Vec::new()),
    };
    Ok(PendingSnapshot {
        main_draw,
        viewports,
        texture_requests,
    })
}

#[cfg(feature = "multi-viewport")]
/// Copies every live viewport draw list into pointer-free Rust-owned storage.
///
/// # Safety
///
/// Every non-null `Viewport::draw_data()` pointer in `platform_io` must remain valid for the
/// entire call. The function never retains those pointers or references after returning.
pub(crate) unsafe fn capture_platform_io(
    platform_io: &crate::platform_io::PlatformIo,
    resolve: &mut impl FnMut(
        *const sys::ImTextureData,
    ) -> Result<ResolvedSnapshotTexture, SnapshotError>,
) -> Result<PendingSnapshot, SnapshotError> {
    for viewport in platform_io.viewports_iter() {
        let raw_draw_data = viewport.draw_data();
        if raw_draw_data.is_null() {
            continue;
        }
        // SAFETY: required by this function's contract and copied before the call returns.
        let draw_data = draw_data_from_sys(unsafe { &*raw_draw_data });
        if draw_data.valid() {
            preflight_detached_callbacks(draw_data)?;
        }
    }

    let mut viewports = Vec::new();
    let mut main_draw_index = None;
    let mut main_draw_data = None;
    for viewport in platform_io.viewports_iter() {
        let raw_draw_data = viewport.draw_data();
        if raw_draw_data.is_null() {
            continue;
        }
        // SAFETY: required by this function's contract and copied before the call returns.
        let draw_data = draw_data_from_sys(unsafe { &*raw_draw_data });
        if !draw_data.valid() {
            continue;
        }
        let is_main = viewport.is_main()
            || owner_viewport_identity(draw_data).is_some_and(|(_, is_main)| is_main);
        if main_draw_index.is_none() && is_main {
            main_draw_index = Some(viewports.len());
            main_draw_data = Some(draw_data);
        }
        viewports.push(ViewportDrawDataSnapshot::new(
            viewport.id(),
            is_main,
            snapshot_draw_data(draw_data, resolve)?,
        ));
    }

    let Some(main_draw_index) = main_draw_index else {
        return Ok(PendingSnapshot {
            main_draw: MainDrawSnapshot::Standalone(empty_draw_data_snapshot()),
            viewports: Vec::new(),
            texture_requests: Vec::new(),
        });
    };
    let texture_requests = snapshot_texture_requests(
        main_draw_data.expect("main viewport draw data was recorded"),
        resolve,
    )?;
    Ok(PendingSnapshot {
        main_draw: MainDrawSnapshot::Viewport(main_draw_index),
        viewports,
        texture_requests,
    })
}

#[cfg(feature = "multi-viewport")]
fn empty_draw_data_snapshot() -> DrawDataSnapshot {
    DrawDataSnapshot {
        frame_count: 0,
        display_pos: [0.0, 0.0],
        display_size: [0.0, 0.0],
        framebuffer_scale: [1.0, 1.0],
        draw_lists: Vec::new(),
    }
}

fn owner_viewport_identity(draw_data: &DrawData) -> Option<(Id, bool)> {
    let owner_viewport = draw_data.owner_viewport();
    if owner_viewport.is_null() {
        return None;
    }
    let raw = unsafe { (*owner_viewport).ID };
    (raw != 0).then(|| {
        let viewport = unsafe { crate::platform_io::Viewport::from_raw(owner_viewport) };
        (Id::from(raw), viewport.is_main())
    })
}

#[cfg(feature = "multi-viewport")]
fn draw_data_from_sys(draw_data: &sys::ImDrawData) -> &DrawData {
    unsafe { <DrawData as crate::internal::RawCast<sys::ImDrawData>>::from_raw(draw_data) }
}

fn snapshot_draw_data(
    draw_data: &DrawData,
    resolve: &mut impl FnMut(
        *const sys::ImTextureData,
    ) -> Result<ResolvedSnapshotTexture, SnapshotError>,
) -> Result<DrawDataSnapshot, SnapshotError> {
    let mut draw_lists = Vec::with_capacity(draw_data.draw_lists_count());
    for draw_list in draw_data.draw_lists() {
        draw_lists.push(snapshot_draw_list(draw_list, resolve)?);
    }
    Ok(DrawDataSnapshot {
        frame_count: draw_data.frame_count(),
        display_pos: draw_data.display_pos(),
        display_size: draw_data.display_size(),
        framebuffer_scale: draw_data.framebuffer_scale(),
        draw_lists,
    })
}

fn detached_callback_kind(
    callback: sys::ImDrawCallback,
) -> Result<Option<StandardDrawCallback>, SnapshotError> {
    match callback {
        None => Ok(None),
        Some(_) => classify_standard_draw_callback(callback)
            .map(Some)
            .ok_or(SnapshotError::UserCallbackUnsupported),
    }
}

fn preflight_detached_callbacks(draw_data: &DrawData) -> Result<(), SnapshotError> {
    for draw_list in draw_data.draw_lists() {
        for command in unsafe { draw_list.cmd_buffer() } {
            let _ = detached_callback_kind(command.UserCallback)?;
        }
    }
    Ok(())
}

fn snapshot_draw_list(
    draw_list: &DrawList,
    resolve: &mut impl FnMut(
        *const sys::ImTextureData,
    ) -> Result<ResolvedSnapshotTexture, SnapshotError>,
) -> Result<DrawListSnapshot, SnapshotError> {
    let vtx = draw_list.vtx_buffer().to_vec();
    let idx = draw_list.idx_buffer().to_vec();
    let mut commands = Vec::new();
    for cmd in unsafe { draw_list.cmd_buffer() } {
        if let Some(callback) = detached_callback_kind(cmd.UserCallback)? {
            match callback {
                StandardDrawCallback::ResetRenderState => {
                    commands.push(DrawCmdSnapshot::ResetRenderState)
                }
                StandardDrawCallback::SetSamplerLinear => {
                    commands.push(DrawCmdSnapshot::SetSamplerLinear)
                }
                StandardDrawCallback::SetSamplerNearest => {
                    commands.push(DrawCmdSnapshot::SetSamplerNearest)
                }
            }
            continue;
        }

        commands.push(DrawCmdSnapshot::Elements {
            count: count_from_u32("DrawCmdSnapshot::Elements::count", cmd.ElemCount),
            clip_rect: [
                cmd.ClipRect.x,
                cmd.ClipRect.y,
                cmd.ClipRect.z,
                cmd.ClipRect.w,
            ],
            texture: snapshot_texture_binding(cmd.TexRef, resolve)?,
            vtx_offset: count_from_u32("DrawCmdSnapshot::Elements::vtx_offset", cmd.VtxOffset),
            idx_offset: count_from_u32("DrawCmdSnapshot::Elements::idx_offset", cmd.IdxOffset),
        });
    }
    Ok(DrawListSnapshot { vtx, idx, commands })
}

fn count_from_u32(caller: &str, raw: u32) -> usize {
    usize::try_from(raw).unwrap_or_else(|_| panic!("{caller} exceeded usize range"))
}

fn snapshot_texture_binding(
    tex_ref: sys::ImTextureRef,
    resolve: &mut impl FnMut(
        *const sys::ImTextureData,
    ) -> Result<ResolvedSnapshotTexture, SnapshotError>,
) -> Result<TextureBinding, SnapshotError> {
    if !tex_ref._TexData.is_null() {
        return resolve(tex_ref._TexData.cast_const())
            .map(|resolved| TextureBinding::Managed(resolved.id));
    }
    Ok(TextureBinding::Legacy(TextureId::from(
        tex_ref._TexID as u64,
    )))
}

fn snapshot_texture_requests(
    draw_data: &DrawData,
    resolve: &mut impl FnMut(
        *const sys::ImTextureData,
    ) -> Result<ResolvedSnapshotTexture, SnapshotError>,
) -> Result<Vec<PendingTextureRequest>, SnapshotError> {
    let mut out = Vec::new();
    for texture in draw_data.textures() {
        let status = texture.status();
        if matches!(status, TextureStatus::OK | TextureStatus::Destroyed) {
            continue;
        }
        let resolved = resolve(texture.as_raw())?;
        let id = resolved.id;
        if status == TextureStatus::WantDestroy {
            out.push(PendingTextureRequest {
                texture: id,
                revision: resolved.revision,
                op: Arc::new(TextureOp::Destroy),
            });
            continue;
        }

        let raw_width = texture.raw_width_i32();
        let raw_height = texture.raw_height_i32();
        let raw_bpp = texture.raw_bytes_per_pixel_i32();
        let (width, height, bpp) = validated_texture_layout(id, raw_width, raw_height, raw_bpp)?;
        let format = texture.format();
        let pixels = texture
            .pixels()
            .ok_or(SnapshotError::TexturePixelsMissing { id, status })?;
        let expected = usize::try_from(width)
            .ok()
            .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
            .and_then(|count| count.checked_mul(bpp))
            .ok_or(SnapshotError::TextureInvalidLayout {
                id,
                width: raw_width,
                height: raw_height,
                bpp: raw_bpp,
            })?;
        if pixels.len() < expected {
            return Err(SnapshotError::TextureInvalidLayout {
                id,
                width: raw_width,
                height: raw_height,
                bpp: raw_bpp,
            });
        }

        let op = match status {
            TextureStatus::WantCreate => TextureOp::Create {
                format,
                width,
                height,
                row_pitch: usize::try_from(width)
                    .ok()
                    .and_then(|width| width.checked_mul(bpp))
                    .ok_or(SnapshotError::TextureInvalidLayout {
                        id,
                        width: raw_width,
                        height: raw_height,
                        bpp: raw_bpp,
                    })?,
                pixels: pixels[..expected].to_vec(),
            },
            TextureStatus::WantUpdates => {
                let mut rects: Vec<TextureRect> = texture.updates().collect();
                if rects.is_empty() {
                    let rect = texture.update_rect();
                    if rect.w != 0 && rect.h != 0 {
                        rects.push(rect);
                    } else {
                        rects.push(full_texture_update_rect(id, width, height)?);
                    }
                }
                TextureOp::Update {
                    format,
                    width,
                    height,
                    rects: rects
                        .into_iter()
                        .filter_map(|rect| copy_upload_rect(pixels, width, height, bpp, rect))
                        .collect(),
                }
            }
            TextureStatus::OK | TextureStatus::WantDestroy | TextureStatus::Destroyed => {
                unreachable!("non-upload statuses were handled before layout validation")
            }
        };
        out.push(PendingTextureRequest {
            texture: id,
            revision: resolved.revision,
            op: Arc::new(op),
        });
    }
    Ok(out)
}

fn full_texture_update_rect(
    id: SnapshotTextureId,
    width: u32,
    height: u32,
) -> Result<TextureRect, SnapshotError> {
    let out_of_range = || SnapshotError::TextureFullUpdateOutOfRange { id, width, height };
    Ok(TextureRect {
        x: 0,
        y: 0,
        w: u16::try_from(width).map_err(|_| out_of_range())?,
        h: u16::try_from(height).map_err(|_| out_of_range())?,
    })
}

fn validated_texture_layout(
    id: SnapshotTextureId,
    width: i32,
    height: i32,
    bpp: i32,
) -> Result<(u32, u32, usize), SnapshotError> {
    let invalid = || SnapshotError::TextureInvalidLayout {
        id,
        width,
        height,
        bpp,
    };
    let width = u32::try_from(width)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(invalid)?;
    let height = u32::try_from(height)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(invalid)?;
    let bpp = usize::try_from(bpp)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(invalid)?;
    Ok((width, height, bpp))
}

#[cfg(test)]
mod upload_identity_tests {
    use super::*;

    fn request(context: ContextId, sequence: u64, revision: u64, op: TextureOp) -> TextureRequest {
        let texture = SnapshotTextureId::FontAtlas {
            context,
            stamp: 7,
            generation: 3,
        };
        let kind = op.kind();
        TextureRequest {
            key: TextureRequestKey {
                epoch: SnapshotEpoch::new(
                    context,
                    NonZeroU64::new(1).unwrap(),
                    NonZeroU64::new(sequence).unwrap(),
                ),
                texture,
                revision,
                kind,
            },
            op: Arc::new(op),
        }
    }

    fn create_op() -> TextureOp {
        TextureOp::Create {
            format: TextureFormat::RGBA32,
            width: 1,
            height: 1,
            row_pitch: 4,
            pixels: vec![1, 2, 3, 4],
        }
    }

    #[test]
    fn upload_identity_is_stable_across_epoch_retries() {
        let context = crate::Context::create();
        let first = request(context.id(), 1, 11, create_op());
        let retry = request(context.id(), 2, 11, create_op());

        assert_eq!(first.upload_identity(), retry.upload_identity());
    }

    #[test]
    fn upload_identity_changes_with_revision_or_operation_kind() {
        let context = crate::Context::create();
        let create = request(context.id(), 1, 11, create_op());
        let revised = request(context.id(), 2, 12, create_op());
        let update = request(
            context.id(),
            3,
            11,
            TextureOp::Update {
                format: TextureFormat::RGBA32,
                width: 1,
                height: 1,
                rects: Vec::new(),
            },
        );

        assert_ne!(create.upload_identity(), revised.upload_identity());
        assert_ne!(create.upload_identity(), update.upload_identity());
    }

    #[test]
    fn destroy_request_has_no_upload_identity() {
        let context = crate::Context::create();
        let destroy = request(context.id(), 1, 11, TextureOp::Destroy);

        assert_eq!(destroy.upload_identity(), None);
    }

    #[test]
    fn upload_identity_debug_output_is_opaque() {
        let context = crate::Context::create();
        let request = request(context.id(), 1, 11, create_op());
        let identity = request.upload_identity().unwrap();

        assert_eq!(format!("{identity:?}"), "TextureUploadIdentity(..)");
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    #[test]
    fn invalid_texture_layout_preserves_all_raw_dimensions() {
        let context = crate::Context::create();
        let id = SnapshotTextureId::FontAtlas {
            context: context.id(),
            stamp: 1,
            generation: 1,
        };
        let error = validated_texture_layout(id, 17, -3, 4).unwrap_err();
        assert!(matches!(
            error,
            SnapshotError::TextureInvalidLayout {
                id: actual,
                width: 17,
                height: -3,
                bpp: 4,
            } if actual == id
        ));
    }

    #[test]
    fn full_texture_updates_reject_dimensions_the_native_rect_cannot_represent() {
        let context = crate::Context::create();
        let id = SnapshotTextureId::FontAtlas {
            context: context.id(),
            stamp: 2,
            generation: 1,
        };

        assert_eq!(
            full_texture_update_rect(id, u16::MAX as u32, 1).unwrap(),
            TextureRect {
                x: 0,
                y: 0,
                w: u16::MAX,
                h: 1,
            }
        );
        assert!(matches!(
            full_texture_update_rect(id, u16::MAX as u32 + 1, 1),
            Err(SnapshotError::TextureFullUpdateOutOfRange {
                id: actual,
                width,
                height: 1,
            }) if actual == id && width == u16::MAX as u32 + 1
        ));
    }
}

fn copy_upload_rect(
    pixels: &[u8],
    width: u32,
    height: u32,
    bpp: usize,
    rect: TextureRect,
) -> Option<TextureUploadRect> {
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    if width == 0 || height == 0 || bpp == 0 {
        return None;
    }
    let x = usize::from(rect.x);
    let y = usize::from(rect.y);
    let x_end = x.saturating_add(usize::from(rect.w)).min(width);
    let y_end = y.saturating_add(usize::from(rect.h)).min(height);
    if x >= x_end || y >= y_end {
        return None;
    }
    let rect_width = x_end - x;
    let rect_height = y_end - y;
    let full_row_pitch = width.checked_mul(bpp)?;
    let row_pitch = rect_width.checked_mul(bpp)?;
    let mut data = vec![0; row_pitch.checked_mul(rect_height)?];
    for row in 0..rect_height {
        let source = y
            .checked_add(row)?
            .checked_mul(full_row_pitch)?
            .checked_add(x.checked_mul(bpp)?)?;
        let destination = row.checked_mul(row_pitch)?;
        data.get_mut(destination..destination.checked_add(row_pitch)?)?
            .copy_from_slice(pixels.get(source..source.checked_add(row_pitch)?)?);
    }
    Some(TextureUploadRect {
        rect: TextureRect {
            x: rect.x,
            y: rect.y,
            w: rect_width.min(u16::MAX as usize) as u16,
            h: rect_height.min(u16::MAX as usize) as u16,
        },
        row_pitch,
        data,
    })
}

#[cfg(test)]
mod callback_preflight_tests {
    use super::*;

    unsafe extern "C" fn raw_callback(
        _draw_list: *const sys::ImDrawList,
        _command: *const sys::ImDrawCmd,
    ) {
    }

    #[test]
    fn raw_callback_preflight_runs_before_any_texture_resolution() {
        let _guard = crate::test_support::imgui_context_guard();
        let mut context = crate::Context::create();
        context.io_mut().set_display_size([128.0, 128.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        let _ = context.font_atlas().build();

        let mut texture = crate::texture::OwnedTextureData::new();
        texture.create(crate::texture::TextureFormat::RGBA32, 1, 1);
        texture.set_data(&[255, 255, 255, 255]);
        let texture = context.register_texture(texture);

        let frame = context.begin_frame();
        frame.ui().image(texture, [16.0, 16.0]);
        unsafe {
            frame.ui().get_foreground_draw_list().add_callback(
                raw_callback,
                std::ptr::null_mut(),
                0,
            );
        }
        let rendered = frame.render_legacy();
        let mut resolve_calls = 0usize;
        let result = capture_draw_data(rendered.draw_data(), &mut |_| {
            resolve_calls += 1;
            Err(SnapshotError::UnknownManagedTexture)
        });

        assert!(matches!(
            result,
            Err(SnapshotError::UserCallbackUnsupported)
        ));
        assert_eq!(resolve_calls, 0);
    }
}

#[cfg(all(test, feature = "multi-viewport"))]
mod tests {
    use super::*;

    unsafe extern "C" fn raw_callback(
        _draw_list: *const sys::ImDrawList,
        _command: *const sys::ImDrawCmd,
    ) {
    }

    fn empty_native_draw_data(
        viewport: *mut sys::ImGuiViewport,
        display_pos: [f32; 2],
        display_size: [f32; 2],
    ) -> *mut sys::ImDrawData {
        let draw_data = unsafe { sys::ImDrawData_ImDrawData() };
        assert!(!draw_data.is_null());
        unsafe {
            (*draw_data).Valid = true;
            (*draw_data).DisplayPos = display_pos.into();
            (*draw_data).DisplaySize = display_size.into();
            (*draw_data).FramebufferScale = sys::ImVec2 { x: 1.0, y: 1.0 };
            (*draw_data).OwnerViewport = viewport;
            (*draw_data).Textures = std::ptr::null_mut();
        }
        draw_data
    }

    fn viewport(id: u32, draw_data: *mut sys::ImDrawData) -> *mut sys::ImGuiViewport {
        let viewport = unsafe { sys::ImGuiViewport_ImGuiViewport() };
        assert!(!viewport.is_null());
        unsafe {
            (*viewport).ID = id;
            (*viewport).DrawData = draw_data;
        }
        viewport
    }

    #[test]
    fn platform_capture_preserves_viewport_order_and_main_identity() {
        let _guard = crate::test_support::imgui_context_guard();
        let mut context = crate::Context::create();
        let main = context.main_viewport().as_raw_mut();
        let previous_main_draw = unsafe { (*main).DrawData };
        let secondary = viewport(unsafe { (*main).ID }, std::ptr::null_mut());
        let secondary_draw = empty_native_draw_data(secondary, [100.0, 50.0], [320.0, 200.0]);
        let main_draw = empty_native_draw_data(main, [0.0, 0.0], [640.0, 360.0]);
        unsafe {
            (*secondary).DrawData = secondary_draw;
            (*main).DrawData = main_draw;
        }
        let mut viewport_ptrs = [secondary, main];
        let mut raw = sys::ImGuiPlatformIO {
            Viewports: sys::ImVector_ImGuiViewportPtr {
                Size: 2,
                Capacity: 2,
                Data: viewport_ptrs.as_mut_ptr(),
            },
            ..Default::default()
        };
        let platform_io = unsafe {
            crate::platform_io::PlatformIo::from_raw(
                (&mut raw as *mut sys::ImGuiPlatformIO).cast_const(),
            )
        };
        let pending = unsafe {
            capture_platform_io(&platform_io, &mut |_| {
                Err(SnapshotError::UnknownManagedTexture)
            })
        }
        .expect("empty draw data should capture");
        assert_eq!(pending.draw_data().display_size, [640.0, 360.0]);
        assert_eq!(pending.viewports[0].draw.display_size, [320.0, 200.0]);
        assert!(!pending.viewports[0].is_main());
        assert!(pending.viewports[1].is_main());
        assert!(std::ptr::eq(
            pending.draw_data(),
            &pending.viewports[1].draw
        ));

        let suspended_context = context.suspend_or_panic();
        let other_context = crate::Context::create();
        assert!(!pending.viewports[0].is_main());
        assert!(pending.viewports[1].is_main());
        drop(other_context);
        let _context = suspended_context
            .activate()
            .expect("the snapshot owner Context should reactivate");

        unsafe {
            (*main).DrawData = previous_main_draw;
            sys::ImDrawData_destroy(secondary_draw);
            sys::ImDrawData_destroy(main_draw);
            sys::ImGuiViewport_destroy(secondary);
        }
    }

    #[test]
    fn platform_callback_preflight_precedes_every_viewport_texture_resolution() {
        let _guard = crate::test_support::imgui_context_guard();
        let mut context = crate::Context::create();
        let _ = context.font_atlas().build();
        let main = context.main_viewport().as_raw_mut();
        let previous_main_draw = unsafe { (*main).DrawData };
        let secondary = viewport(
            unsafe { (*main).ID.wrapping_add(1).max(1) },
            std::ptr::null_mut(),
        );
        let mut texture = crate::texture::OwnedTextureData::new();
        texture.create(crate::texture::TextureFormat::RGBA32, 1, 1);
        texture.set_data(&[255, 255, 255, 255]);

        let mut main_command = sys::ImDrawCmd {
            TexRef: unsafe { sys::ImTextureData_GetTexRef(texture.as_raw_mut()) },
            ..Default::default()
        };
        let mut secondary_command = sys::ImDrawCmd {
            UserCallback: Some(raw_callback),
            ..Default::default()
        };
        let mut main_list = sys::ImDrawList {
            CmdBuffer: sys::ImVector_ImDrawCmd {
                Size: 1,
                Capacity: 1,
                Data: &mut main_command,
            },
            ..Default::default()
        };
        let mut secondary_list = sys::ImDrawList {
            CmdBuffer: sys::ImVector_ImDrawCmd {
                Size: 1,
                Capacity: 1,
                Data: &mut secondary_command,
            },
            ..Default::default()
        };
        let mut main_lists = [&mut main_list as *mut sys::ImDrawList];
        let mut secondary_lists = [&mut secondary_list as *mut sys::ImDrawList];
        let mut main_draw = sys::ImDrawData {
            Valid: true,
            CmdLists: sys::ImVector_ImDrawListPtr {
                Size: 1,
                Capacity: 1,
                Data: main_lists.as_mut_ptr(),
            },
            DisplaySize: sys::ImVec2 { x: 128.0, y: 128.0 },
            FramebufferScale: sys::ImVec2 { x: 1.0, y: 1.0 },
            OwnerViewport: main,
            ..Default::default()
        };
        let mut secondary_draw = sys::ImDrawData {
            Valid: true,
            CmdLists: sys::ImVector_ImDrawListPtr {
                Size: 1,
                Capacity: 1,
                Data: secondary_lists.as_mut_ptr(),
            },
            DisplayPos: sys::ImVec2 { x: 128.0, y: 0.0 },
            DisplaySize: sys::ImVec2 { x: 128.0, y: 128.0 },
            FramebufferScale: sys::ImVec2 { x: 1.0, y: 1.0 },
            OwnerViewport: secondary,
            ..Default::default()
        };
        unsafe {
            (*main).DrawData = &mut main_draw;
            (*secondary).DrawData = &mut secondary_draw;
        }
        let mut viewport_ptrs = [main, secondary];
        let raw = sys::ImGuiPlatformIO {
            Viewports: sys::ImVector_ImGuiViewportPtr {
                Size: 2,
                Capacity: 2,
                Data: viewport_ptrs.as_mut_ptr(),
            },
            ..Default::default()
        };
        let platform_io = unsafe { crate::platform_io::PlatformIo::from_raw(&raw) };
        let mut resolve_calls = 0usize;
        let result = unsafe {
            capture_platform_io(platform_io, &mut |_| {
                resolve_calls += 1;
                Err(SnapshotError::UnknownManagedTexture)
            })
        };

        assert!(matches!(
            result,
            Err(SnapshotError::UserCallbackUnsupported)
        ));
        assert_eq!(resolve_calls, 0);

        unsafe {
            (*main).DrawData = previous_main_draw;
            sys::ImGuiViewport_destroy(secondary);
        }
    }
}
