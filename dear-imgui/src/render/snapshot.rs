//! Pointer-free rendering snapshots.
//!
//! A [`FrameSnapshot`] is created by an owning [`crate::Context`] for one registered
//! [`RendererConsumer`]. It can cross threads, but it cannot be cloned or constructed from
//! arbitrary native draw data. Dropping it reports an abandoned epoch; [`FrameSnapshot::commit`]
//! reports renderer feedback for ordered reconciliation by the Context.

use std::collections::HashSet;
use std::marker::PhantomData;
use std::num::NonZeroU64;
use std::rc::Rc;
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

#[cfg(feature = "multi-viewport")]
const IMGUI_VIEWPORT_DEFAULT_ID: u32 = 0x1111_1111;

/// Pointer-free identity used by detached renderers.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum SnapshotTextureId {
    /// A Context-owned user texture.
    User(ManagedTextureId),
    /// The current texture allocation of the Context's font atlas.
    FontAtlas {
        /// Context that produced this snapshot.
        context: ContextId,
        /// Process-unique atlas allocation stamp.
        stamp: u64,
        /// Atlas content generation captured by this snapshot.
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

/// The sole renderer capability for one Context.
///
/// The first rendered frame claims either the synchronous or detached mode for this generation.
/// The capability is deliberately non-cloneable and UI-thread bound. Detached snapshots created
/// with it are `Send + Sync`; the capability itself is neither.
#[must_use = "keep the consumer alive while rendering managed texture requests"]
pub struct RendererConsumer {
    context: ContextId,
    generation: NonZeroU64,
    sender: Sender<SnapshotMessage>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl std::fmt::Debug for RendererConsumer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RendererConsumer")
            .field("context", &self.context)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl RendererConsumer {
    pub(crate) fn new(
        context: ContextId,
        generation: NonZeroU64,
        sender: Sender<SnapshotMessage>,
    ) -> Self {
        Self {
            context,
            generation,
            sender,
            _not_send_or_sync: PhantomData,
        }
    }

    /// Context that owns this consumer.
    #[must_use]
    pub const fn context_id(&self) -> ContextId {
        self.context
    }

    /// Current consumer generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation.get()
    }

    pub(crate) const fn generation_raw(&self) -> NonZeroU64 {
        self.generation
    }
}

impl Drop for RendererConsumer {
    fn drop(&mut self) {
        let _ = self.sender.send(SnapshotMessage::Detach {
            context: self.context,
            generation: self.generation,
        });
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

    /// Commit renderer feedback and complete this epoch.
    ///
    /// Missing feedback is allowed: unacknowledged requests remain pending and are emitted again
    /// by a later snapshot. Feedback is validated and applied only when this epoch reaches the
    /// Context's contiguous completion watermark.
    pub fn commit(
        self,
        feedback: impl IntoIterator<Item = TextureFeedback>,
    ) -> Result<(), SnapshotCommitError> {
        self.completion.commit(feedback.into_iter().collect())
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
#[derive(Debug)]
pub struct ViewportDrawDataSnapshot {
    pub viewport_id: Id,
    pub draw: DrawDataSnapshot,
}

/// Thread-safe draw data snapshot.
#[derive(Debug)]
pub struct DrawDataSnapshot {
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
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TextureUploadIdentity {
    revision: u64,
    kind: TextureRequestKind,
}

/// A managed texture operation requested by Dear ImGui.
#[derive(Debug)]
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
#[derive(Debug)]
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
    op: TextureOp,
}

impl TextureRequest {
    /// Texture addressed by this request.
    #[must_use]
    pub const fn texture(&self) -> SnapshotTextureId {
        self.key.texture
    }

    /// Request operation and owned upload bytes.
    #[must_use]
    pub const fn operation(&self) -> &TextureOp {
        &self.op
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
}

/// Error returned when feedback does not match the request operation.
#[derive(Copy, Clone, Debug, Eq, Error, PartialEq)]
pub enum TextureFeedbackError {
    #[error("an upload result cannot complete a destroy request")]
    UploadForDestroy,
    #[error("a destroy result cannot complete a create or update request")]
    DestroyForUpload,
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
    #[error("the previous renderer consumer is still draining outstanding epochs")]
    ConsumerDraining,
    #[error("this Context has no active renderer consumer")]
    NoActiveConsumer,
    #[error("the active renderer consumer is already committed to a different render path")]
    ConsumerModeMismatch,
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
    pub(crate) op: TextureOp,
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
    let draw = snapshot_draw_data(draw_data, resolve)?;
    let texture_requests = snapshot_texture_requests(draw_data, resolve)?;
    let (main_draw, viewports) = match owner_viewport_id(draw_data) {
        Some(viewport_id) => (
            MainDrawSnapshot::Viewport(0),
            vec![ViewportDrawDataSnapshot { viewport_id, draw }],
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
pub(crate) fn capture_platform_io(
    platform_io: &crate::platform_io::PlatformIo,
    resolve: &mut impl FnMut(
        *const sys::ImTextureData,
    ) -> Result<ResolvedSnapshotTexture, SnapshotError>,
) -> Result<PendingSnapshot, SnapshotError> {
    let mut viewports = Vec::new();
    let mut main_draw_index = None;
    let mut main_draw_data = None;
    for viewport in platform_io.viewports_iter() {
        let Some(raw_draw_data) = viewport.draw_data_ref() else {
            continue;
        };
        let draw_data = draw_data_from_sys(raw_draw_data);
        if !draw_data.valid() {
            continue;
        }
        if main_draw_index.is_none() && is_main_platform_viewport(viewport.id(), draw_data) {
            main_draw_index = Some(viewports.len());
            main_draw_data = Some(draw_data);
        }
        viewports.push(ViewportDrawDataSnapshot {
            viewport_id: viewport.id(),
            draw: snapshot_draw_data(draw_data, resolve)?,
        });
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
        display_pos: [0.0, 0.0],
        display_size: [0.0, 0.0],
        framebuffer_scale: [1.0, 1.0],
        draw_lists: Vec::new(),
    }
}

fn owner_viewport_id(draw_data: &DrawData) -> Option<Id> {
    let owner_viewport = draw_data.owner_viewport();
    if owner_viewport.is_null() {
        return None;
    }
    let raw = unsafe { (*owner_viewport).ID };
    (raw != 0).then_some(Id::from(raw))
}

#[cfg(feature = "multi-viewport")]
fn draw_data_from_sys(draw_data: &sys::ImDrawData) -> &DrawData {
    unsafe { <DrawData as crate::internal::RawCast<sys::ImDrawData>>::from_raw(draw_data) }
}

#[cfg(feature = "multi-viewport")]
fn is_main_platform_viewport(viewport_id: Id, draw_data: &DrawData) -> bool {
    viewport_id.raw() == IMGUI_VIEWPORT_DEFAULT_ID
        || owner_viewport_id(draw_data)
            .is_some_and(|owner_id| owner_id.raw() == IMGUI_VIEWPORT_DEFAULT_ID)
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
        display_pos: draw_data.display_pos(),
        display_size: draw_data.display_size(),
        framebuffer_scale: draw_data.framebuffer_scale(),
        draw_lists,
    })
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
        if cmd.UserCallback.is_some() {
            match classify_standard_draw_callback(cmd.UserCallback) {
                Some(StandardDrawCallback::ResetRenderState) => {
                    commands.push(DrawCmdSnapshot::ResetRenderState);
                    continue;
                }
                Some(StandardDrawCallback::SetSamplerLinear) => {
                    commands.push(DrawCmdSnapshot::SetSamplerLinear);
                    continue;
                }
                Some(StandardDrawCallback::SetSamplerNearest) => {
                    commands.push(DrawCmdSnapshot::SetSamplerNearest);
                    continue;
                }
                None => return Err(SnapshotError::UserCallbackUnsupported),
            }
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
                op: TextureOp::Destroy,
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
                        rects.push(TextureRect {
                            x: 0,
                            y: 0,
                            w: width.min(u16::MAX as u32) as u16,
                            h: height.min(u16::MAX as u32) as u16,
                        });
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
            op,
        });
    }
    Ok(out)
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
            op,
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

#[cfg(all(test, feature = "multi-viewport"))]
mod tests {
    use super::*;

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
        let secondary = viewport(0x222, std::ptr::null_mut());
        let main = viewport(IMGUI_VIEWPORT_DEFAULT_ID, std::ptr::null_mut());
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
        let pending = capture_platform_io(&platform_io, &mut |_| {
            Err(SnapshotError::UnknownManagedTexture)
        })
        .expect("empty draw data should capture");
        assert_eq!(pending.draw_data().display_size, [640.0, 360.0]);
        assert_eq!(pending.viewports[0].draw.display_size, [320.0, 200.0]);
        assert!(std::ptr::eq(
            pending.draw_data(),
            &pending.viewports[1].draw
        ));

        unsafe {
            sys::ImDrawData_destroy(secondary_draw);
            sys::ImDrawData_destroy(main_draw);
            sys::ImGuiViewport_destroy(secondary);
            sys::ImGuiViewport_destroy(main);
        }
    }
}
