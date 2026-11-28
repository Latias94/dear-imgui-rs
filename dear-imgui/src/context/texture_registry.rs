use crate::FrameError;

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::num::NonZeroU64;
use std::rc::Rc;
use std::sync::Arc;

use crate::render::snapshot::{
    CapturedTextureOperation, PendingTextureRequest, ResolvedSnapshotTexture, SnapshotTextureId,
    TextureFeedback, TextureFeedbackResult, TextureOp, TextureRequestKind,
};
use crate::sys;
use crate::texture::{
    ManagedTextureId, ManagedTextureMut, ManagedTextureRef, OwnedTextureData, TextureData,
    TextureError, TextureRect, TextureStatus,
};

use super::binding::{CTX_MUTEX, RawBoundContextGuard};
use super::{Context, ContextId};

pub(crate) type SharedTextureRegistry = Rc<RefCell<ManagedTextureRegistry>>;

#[derive(Debug)]
pub(crate) struct FontAtlasSnapshotTarget {
    atlas: *mut sys::ImFontAtlas,
    context: ContextId,
    textures: Vec<FontAtlasTextureTarget>,
}

#[derive(Copy, Clone, Debug)]
pub(crate) struct FontAtlasTextureTarget {
    id: SnapshotTextureId,
    texture: *mut sys::ImTextureData,
}

impl FontAtlasSnapshotTarget {
    pub(crate) fn new(
        atlas: *mut sys::ImFontAtlas,
        context: ContextId,
        textures: Vec<FontAtlasTextureTarget>,
    ) -> Self {
        Self {
            atlas,
            context,
            textures,
        }
    }

    pub(crate) fn resolve(
        &self,
        native: *const sys::ImTextureData,
    ) -> Option<ResolvedSnapshotTexture> {
        self.textures
            .iter()
            .find(|target| std::ptr::eq(native, target.texture.cast_const()))
            .map(|target| ResolvedSnapshotTexture { id: target.id })
    }

    fn find(&self, id: SnapshotTextureId) -> Option<FontAtlasTextureTarget> {
        self.textures.iter().find(|target| target.id == id).copied()
    }

    pub(crate) fn record_request_reference(&self, id: SnapshotTextureId, epoch: u64) {
        crate::fonts::record_font_atlas_texture_reference(self.atlas, id, epoch);
    }

    fn identity_is_known(&self, id: SnapshotTextureId) -> bool {
        matches!(id, SnapshotTextureId::FontAtlas { context, .. } if context == self.context)
            && crate::fonts::font_atlas_texture_identity_is_known(self.atlas, id)
    }

    pub(crate) fn prune_tombstones(&self, watermark: u64) {
        crate::fonts::prune_font_atlas_texture_tombstones(self.atlas, self.context, watermark);
    }

    pub(crate) fn reset_renderer_bindings(&self) {
        for target in &self.textures {
            let texture = unsafe { TextureData::from_raw(target.texture) };
            if texture.ref_count() != 1 || texture.status() == TextureStatus::Destroyed {
                continue;
            }
            unsafe {
                // The pointer came from this transaction's current atlas list. The renderer has
                // already released the resource represented by its binding.
                texture.set_status(TextureStatus::Destroyed);
            }
        }
        crate::fonts::mark_font_atlas_renderer_reset(self.atlas);
    }
}

impl FontAtlasTextureTarget {
    pub(crate) fn new(id: SnapshotTextureId, texture: *mut sys::ImTextureData) -> Self {
        Self { id, texture }
    }
}

pub(crate) struct ManagedTextureRegistry {
    context: ContextId,
    slots: Vec<TextureSlot>,
    reusable: Vec<u32>,
    frame_removals: Vec<u32>,
    by_native: HashMap<usize, ManagedTextureId>,
    native_refresh_generation: u64,
    sync: TextureSyncQueue,
}

enum TextureSlot {
    Active(TextureEntry),
    Retiring(TextureEntry),
    NativeExposed {
        entry: TextureEntry,
        release_after_refresh: u64,
    },
    Retired {
        generation: NonZeroU64,
    },
    Exhausted,
}

struct TextureEntry {
    retire_after_frame: bool,
    generation: NonZeroU64,
    last_reference_epoch: u64,
    destroy_ack_epoch: Option<u64>,
    texture: OwnedTextureData,
}

impl fmt::Debug for ManagedTextureRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let active = self
            .slots
            .iter()
            .filter(|slot| matches!(slot, TextureSlot::Active(_)))
            .count();
        let retiring = self
            .slots
            .iter()
            .filter(|slot| matches!(slot, TextureSlot::Retiring(_)))
            .count();
        let native_exposed = self
            .slots
            .iter()
            .filter(|slot| matches!(slot, TextureSlot::NativeExposed { .. }))
            .count();
        formatter
            .debug_struct("ManagedTextureRegistry")
            .field("context", &self.context)
            .field("active", &active)
            .field("retire_after_frame", &self.frame_removals.len())
            .field("retiring", &retiring)
            .field("native_exposed", &native_exposed)
            .field("slot_count", &self.slots.len())
            .finish()
    }
}

impl ManagedTextureRegistry {
    pub(crate) fn new(context: ContextId) -> SharedTextureRegistry {
        Rc::new(RefCell::new(Self {
            context,
            slots: Vec::new(),
            reusable: Vec::new(),
            frame_removals: Vec::new(),
            by_native: HashMap::new(),
            native_refresh_generation: 0,
            sync: TextureSyncQueue::default(),
        }))
    }

    fn validate_context(&self, id: ManagedTextureId) -> Result<(), TextureError> {
        if id.context_id() == self.context {
            Ok(())
        } else {
            Err(TextureError::ForeignContext {
                expected: self.context,
                actual: id.context_id(),
            })
        }
    }

    fn slot(&self, id: ManagedTextureId) -> Result<&TextureSlot, TextureError> {
        self.validate_context(id)?;
        let slot = self
            .slots
            .get(id.slot() as usize)
            .ok_or(TextureError::UnknownSlot(id))?;
        let generation = match slot {
            TextureSlot::Active(entry)
            | TextureSlot::Retiring(entry)
            | TextureSlot::NativeExposed { entry, .. } => entry.generation,
            TextureSlot::Retired { generation } => *generation,
            TextureSlot::Exhausted => return Err(TextureError::AlreadyRemoved(id)),
        };
        if generation != id.generation() {
            return Err(TextureError::StaleGeneration(id));
        }
        Ok(slot)
    }

    fn active_entry(&self, id: ManagedTextureId) -> Result<&TextureEntry, TextureError> {
        match self.slot(id)? {
            TextureSlot::Active(entry) if !entry.retire_after_frame => Ok(entry),
            TextureSlot::Active(_) | TextureSlot::Retiring(_) => Err(TextureError::Retiring(id)),
            TextureSlot::NativeExposed { .. }
            | TextureSlot::Retired { .. }
            | TextureSlot::Exhausted => Err(TextureError::AlreadyRemoved(id)),
        }
    }

    fn active_entry_mut(
        &mut self,
        id: ManagedTextureId,
    ) -> Result<&mut TextureEntry, TextureError> {
        self.active_entry(id)?;
        let TextureSlot::Active(entry) = &mut self.slots[id.slot() as usize] else {
            unreachable!("validated active texture slot changed without a mutable alias")
        };
        Ok(entry)
    }

    fn allocate_slot(&mut self) -> (u32, NonZeroU64) {
        while let Some(slot_index) = self.reusable.pop() {
            let slot = &mut self.slots[slot_index as usize];
            let TextureSlot::Retired { generation } = slot else {
                debug_assert!(false, "reusable texture slot was not retired");
                continue;
            };
            let Some(next) = generation.get().checked_add(1).and_then(NonZeroU64::new) else {
                *slot = TextureSlot::Exhausted;
                continue;
            };
            return (slot_index, next);
        }

        let slot_index = u32::try_from(self.slots.len())
            .expect("managed texture registry exhausted its slot identity space");
        (slot_index, NonZeroU64::MIN)
    }

    fn register(&mut self, mut texture: OwnedTextureData, watermark: u64) -> ManagedTextureId {
        self.reap_destroyed(watermark);
        let (slot_index, generation) = self.allocate_slot();
        let id = ManagedTextureId::new(self.context, slot_index, generation);
        let native = texture.as_mut().as_raw_mut();
        assert_eq!(
            texture.ref_count(),
            0,
            "Context::register_texture() received texture data already owned by native state"
        );
        unsafe {
            sys::igRegisterUserTexture(native);
        }
        let entry = TextureEntry {
            retire_after_frame: false,
            generation,
            last_reference_epoch: 0,
            destroy_ack_epoch: None,
            texture,
        };
        if slot_index as usize == self.slots.len() {
            self.slots.push(TextureSlot::Active(entry));
        } else {
            self.slots[slot_index as usize] = TextureSlot::Active(entry);
        }
        let previous = self.by_native.insert(native as usize, id);
        assert!(
            previous.is_none(),
            "native texture allocation was registered twice"
        );
        id
    }

    pub(crate) fn resolve(&self, id: ManagedTextureId) -> Result<sys::ImTextureRef, TextureError> {
        let entry = self.active_entry(id)?;
        Ok(sys::ImTextureRef {
            _TexData: entry.texture.as_raw() as *mut sys::ImTextureData,
            _TexID: 0 as sys::ImTextureID,
        })
    }

    pub(crate) fn id_for_native(
        &self,
        native: *const sys::ImTextureData,
    ) -> Option<ManagedTextureId> {
        self.by_native.get(&(native as usize)).copied()
    }

    pub(crate) fn resolve_snapshot_texture(
        &self,
        native: *const sys::ImTextureData,
        atlas: &FontAtlasSnapshotTarget,
    ) -> Result<ResolvedSnapshotTexture, FrameError> {
        if let Some(id) = self.id_for_native(native) {
            match self.slot(id)? {
                TextureSlot::Active(_) | TextureSlot::Retiring(_) => {}
                TextureSlot::NativeExposed { .. }
                | TextureSlot::Retired { .. }
                | TextureSlot::Exhausted => {
                    return Err(TextureError::AlreadyRemoved(id).into());
                }
            }
            return Ok(ResolvedSnapshotTexture {
                id: SnapshotTextureId::User(id),
            });
        }
        if let Some(resolved) = atlas.resolve(native) {
            return Ok(resolved);
        }
        Err(FrameError::UnknownManagedTexture)
    }

    pub(crate) fn record_snapshot_references(
        &mut self,
        ids: &std::collections::HashSet<ManagedTextureId>,
        epoch: u64,
    ) -> Result<(), TextureError> {
        for id in ids {
            match self.slot(*id)? {
                TextureSlot::Active(_) | TextureSlot::Retiring(_) => {}
                TextureSlot::NativeExposed { .. }
                | TextureSlot::Retired { .. }
                | TextureSlot::Exhausted => {
                    return Err(TextureError::AlreadyRemoved(*id));
                }
            }
        }
        for id in ids {
            let slot = &mut self.slots[id.slot() as usize];
            let entry = match slot {
                TextureSlot::Active(entry) | TextureSlot::Retiring(entry) => entry,
                TextureSlot::NativeExposed { .. }
                | TextureSlot::Retired { .. }
                | TextureSlot::Exhausted => unreachable!(),
            };
            entry.last_reference_epoch = entry.last_reference_epoch.max(epoch);
        }
        Ok(())
    }

    fn with_texture<R>(
        &self,
        id: ManagedTextureId,
        f: impl for<'texture> FnOnce(ManagedTextureRef<'texture>) -> R,
    ) -> Result<R, TextureError> {
        let entry = self.active_entry(id)?;
        Ok(f(ManagedTextureRef::new(&entry.texture)))
    }

    fn with_texture_mut<R>(
        &mut self,
        id: ManagedTextureId,
        f: impl for<'texture> FnOnce(ManagedTextureMut<'texture>) -> R,
    ) -> Result<R, TextureError> {
        let entry = self.active_entry_mut(id)?;
        Ok(f(ManagedTextureMut::new(&mut entry.texture)))
    }

    pub(crate) fn stage_snapshot_operations(
        &mut self,
        requests: &[CapturedTextureOperation],
        atlas: &FontAtlasSnapshotTarget,
    ) -> Result<Vec<PendingTextureRequest>, FrameError> {
        // Capture validated bytes first. Native OK means the persistent queue owns them,
        // not that the renderer has uploaded them.
        let mut staged = Vec::with_capacity(requests.len());
        for request in requests.iter() {
            let native = match request.texture {
                SnapshotTextureId::User(id) => match self.slot(id)? {
                    TextureSlot::Active(entry) | TextureSlot::Retiring(entry) => {
                        entry.texture.as_raw()
                    }
                    _ => return Err(TextureError::AlreadyRemoved(id).into()),
                },
                SnapshotTextureId::FontAtlas { .. } => atlas
                    .find(request.texture)
                    .ok_or(FrameError::UnknownManagedTexture)?
                    .texture
                    .cast_const(),
            };
            let texture = unsafe { TextureData::from_raw(native.cast_mut()) };
            let mut operation = Arc::clone(&request.op);
            if let Some(create) = self
                .sync
                .operation(request.texture)
                .and_then(|previous| update_pending_create(previous, operation.as_ref()))
            {
                operation = Arc::new(create);
            } else if let TextureOp::Update {
                format,
                width,
                height,
                rects,
            } = operation.as_ref()
            {
                let previous_update = self
                    .sync
                    .operation(request.texture)
                    .filter(|previous| matches!(previous, TextureOp::Update { .. }));
                if previous_update.is_none() && rects.len() == 1 {
                    staged.push((request.texture, native, operation));
                    continue;
                }
                let previous = previous_update.unwrap_or(operation.as_ref());
                let rect = cumulative_update_rect(previous, operation.as_ref()).ok_or(
                    FrameError::TextureFullUpdateOutOfRange {
                        id: request.texture,
                        width: *width,
                        height: *height,
                    },
                )?;
                let pixels = texture.pixels().ok_or(FrameError::TexturePixelsMissing {
                    id: request.texture,
                    status: texture.status(),
                })?;
                let upload = crate::render::snapshot::copy_upload_rect(
                    pixels,
                    *width,
                    *height,
                    texture.raw_bytes_per_pixel_i32() as usize,
                    rect,
                )
                .ok_or(FrameError::TextureInvalidLayout {
                    id: request.texture,
                    width: texture.raw_width_i32(),
                    height: texture.raw_height_i32(),
                    bpp: texture.raw_bytes_per_pixel_i32(),
                })?;
                operation = Arc::new(TextureOp::Update {
                    format: *format,
                    width: *width,
                    height: *height,
                    rects: vec![upload],
                });
            }
            staged.push((request.texture, native, operation));
        }
        for (id, native, operation) in staged {
            let destroy = matches!(operation.as_ref(), TextureOp::Destroy);
            self.sync.stage(id, operation);
            let texture = unsafe { TextureData::from_raw(native.cast_mut()) };
            texture.claim_managed_queue();
            if !destroy {
                unsafe {
                    texture.set_status(TextureStatus::OK);
                }
            }
            if matches!(id, SnapshotTextureId::FontAtlas { .. }) {
                crate::fonts::mark_font_atlas_texture_staged(atlas.atlas);
            }
        }
        Ok(self.sync.requests())
    }

    fn remove(
        &mut self,
        id: ManagedTextureId,
        watermark: u64,
        retain_frame: bool,
    ) -> Result<(), TextureError> {
        self.reap_destroyed(watermark);
        let entry = self.active_entry_mut(id).map_err(|error| match error {
            TextureError::Retiring(_) => TextureError::AlreadyRetiring(id),
            other => other,
        })?;
        if retain_frame {
            // Revoke the handle now. Renderers can still resolve the native texture and upload
            // its current pixels until all draw lists for this frame have expired.
            entry.retire_after_frame = true;
            self.frame_removals.push(id.slot());
        } else {
            self.begin_retirement(id.slot() as usize, watermark);
        }
        Ok(())
    }

    pub(super) fn finish_frame_removals(&mut self, watermark: u64) {
        while let Some(index) = self.frame_removals.pop() {
            self.begin_retirement(index as usize, watermark);
        }
    }

    fn begin_retirement(&mut self, slot_index: usize, watermark: u64) {
        let TextureSlot::Active(mut entry) =
            std::mem::replace(&mut self.slots[slot_index], TextureSlot::Exhausted)
        else {
            unreachable!("texture awaiting retirement must still be registered")
        };
        let has_renderer_binding =
            !entry.texture.tex_id().is_null() || !entry.texture.backend_user_data().is_null();
        let has_outstanding_reference = entry.last_reference_epoch > watermark;
        let queue_owned = unsafe { !(*entry.texture.as_raw()).QueueUserData.is_null() };
        if has_renderer_binding || has_outstanding_reference || queue_owned {
            mark_want_destroy(&mut entry.texture);
            self.slots[slot_index] = TextureSlot::Retiring(entry);
        } else {
            self.unregister_and_expose(slot_index, entry);
        }
    }

    pub(crate) fn apply_snapshot_feedback(
        &mut self,
        feedback: &[TextureFeedback],
        atlas: &FontAtlasSnapshotTarget,
        epoch: u64,
    ) -> Result<usize, FrameError> {
        let mut applied = 0;
        for item in feedback {
            let key = item.key();
            if matches!(
                item.result(),
                TextureFeedbackResult::Superseded | TextureFeedbackResult::Retry
            ) {
                continue;
            }
            match key.texture {
                SnapshotTextureId::User(id) => match self.slot(id)? {
                    TextureSlot::Active(_) => {
                        if item.result() == TextureFeedbackResult::Destroyed {
                            return Err(FrameError::InvalidFeedbackTransition {
                                texture: key.texture,
                            });
                        }
                    }
                    TextureSlot::Retiring(_) => {}
                    TextureSlot::NativeExposed { .. }
                    | TextureSlot::Retired { .. }
                    | TextureSlot::Exhausted => {
                        return Err(TextureError::AlreadyRemoved(id).into());
                    }
                },
                SnapshotTextureId::FontAtlas { .. } => {
                    if !atlas.identity_is_known(key.texture) {
                        return Err(FrameError::StaleFontAtlas);
                    }
                }
            }
            match (key.kind, item.result()) {
                (
                    TextureRequestKind::Create | TextureRequestKind::Update,
                    TextureFeedbackResult::Uploaded { texture_id },
                ) if !texture_id.is_null() => {}
                (TextureRequestKind::Destroy, TextureFeedbackResult::Destroyed) => {}
                _ => {
                    return Err(FrameError::InvalidFeedbackTransition {
                        texture: key.texture,
                    });
                }
            }
        }

        for item in feedback {
            let key = item.key();
            if matches!(
                item.result(),
                TextureFeedbackResult::Superseded | TextureFeedbackResult::Retry
            ) {
                continue;
            }
            let native = match key.texture {
                SnapshotTextureId::User(id) => match &self.slots[id.slot() as usize] {
                    TextureSlot::Active(entry) | TextureSlot::Retiring(entry) => {
                        entry.texture.as_raw()
                    }
                    _ => unreachable!("feedback identities were validated"),
                },
                SnapshotTextureId::FontAtlas { .. } => {
                    let Some(target) = atlas.find(key.texture) else {
                        continue;
                    };
                    target.texture.cast_const()
                }
            };
            let texture = unsafe { TextureData::from_raw(native.cast_mut()) };
            // Native changes may have happened since the last capture. In particular a
            // pending Create must stay self-contained until that newer content is staged.
            let expected_status = if key.kind == TextureRequestKind::Destroy {
                TextureStatus::WantDestroy
            } else {
                TextureStatus::OK
            };
            if texture.status() != expected_status
                || !self.sync.acknowledge(key.texture, key.revision)
            {
                continue;
            }
            match item.result() {
                TextureFeedbackResult::Uploaded { texture_id } => unsafe {
                    texture.set_tex_id(texture_id);
                },
                TextureFeedbackResult::Destroyed => {
                    unsafe {
                        texture.set_status(TextureStatus::Destroyed);
                    }
                    if let SnapshotTextureId::User(id) = key.texture
                        && let TextureSlot::Retiring(entry) = &mut self.slots[id.slot() as usize]
                    {
                        entry.destroy_ack_epoch = Some(epoch);
                    }
                    self.sync.forget(key.texture);
                }
                TextureFeedbackResult::Superseded | TextureFeedbackResult::Retry => unreachable!(),
            }
            applied += 1;
        }
        Ok(applied)
    }

    pub(crate) fn reap_destroyed(&mut self, watermark: u64) {
        let ready = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| match slot {
                TextureSlot::Retiring(entry)
                    if entry.texture.status() == TextureStatus::Destroyed
                        && entry.texture.tex_id().is_null()
                        && entry.texture.backend_user_data().is_null()
                        && entry
                            .destroy_ack_epoch
                            .is_some_and(|epoch| epoch <= watermark)
                        && entry.last_reference_epoch <= watermark =>
                {
                    Some(index)
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        for slot_index in ready {
            let generation = match &self.slots[slot_index] {
                TextureSlot::Retiring(entry) => entry.generation,
                _ => continue,
            };
            let placeholder = TextureSlot::Retired { generation };
            let TextureSlot::Retiring(entry) =
                std::mem::replace(&mut self.slots[slot_index], placeholder)
            else {
                continue;
            };
            self.unregister_and_expose(slot_index, entry);
        }
    }

    pub(crate) fn reset_renderer_bindings(&mut self, watermark: u64) {
        self.sync.clear();
        for slot in &mut self.slots {
            let (entry, retiring) = match slot {
                TextureSlot::Active(entry) => (entry, false),
                TextureSlot::Retiring(entry) => (entry, true),
                TextureSlot::NativeExposed { .. }
                | TextureSlot::Retired { .. }
                | TextureSlot::Exhausted => continue,
            };
            if entry.texture.status() == TextureStatus::Destroyed {
                if retiring {
                    entry.destroy_ack_epoch = Some(watermark);
                }
                continue;
            }
            unsafe {
                // The renderer has already released the resource. Resetting through the
                // Context-owned allocation avoids stale pointers cached by PlatformIO.Textures.
                entry.texture.set_status(TextureStatus::Destroyed);
            }
            if retiring {
                entry.destroy_ack_epoch = Some(watermark);
            }
        }
        self.reap_destroyed(watermark);
    }

    fn unregister_and_expose(&mut self, slot_index: usize, mut entry: TextureEntry) {
        let native = entry.texture.as_mut().as_raw_mut();
        unsafe {
            sys::igUnregisterUserTexture(native);
        }
        self.by_native.remove(&(native as usize));
        let release_after_refresh = self
            .native_refresh_generation
            .checked_add(1)
            .expect("native texture-list refresh generation exhausted");
        self.slots[slot_index] = TextureSlot::NativeExposed {
            entry,
            release_after_refresh,
        };
    }

    pub(crate) fn observe_native_texture_list_refresh(&mut self) {
        self.native_refresh_generation = self
            .native_refresh_generation
            .checked_add(1)
            .expect("native texture-list refresh generation exhausted");
        let ready = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| match slot {
                TextureSlot::NativeExposed {
                    release_after_refresh,
                    ..
                } if *release_after_refresh <= self.native_refresh_generation => Some(index),
                _ => None,
            })
            .collect::<Vec<_>>();
        for slot_index in ready {
            let generation = match &self.slots[slot_index] {
                TextureSlot::NativeExposed { entry, .. } => entry.generation,
                _ => continue,
            };
            let previous = std::mem::replace(
                &mut self.slots[slot_index],
                TextureSlot::Retired { generation },
            );
            let TextureSlot::NativeExposed { entry, .. } = previous else {
                unreachable!("native-exposed texture changed without a mutable alias")
            };
            drop(entry);
            self.reusable
                .push(u32::try_from(slot_index).expect("texture slot index must fit u32"));
        }
    }

    pub(super) fn prepare_teardown(&mut self) {
        let registered = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                matches!(slot, TextureSlot::Active(_) | TextureSlot::Retiring(_)).then_some(index)
            })
            .collect::<Vec<_>>();
        for slot_index in registered {
            let generation = match &self.slots[slot_index] {
                TextureSlot::Active(entry) | TextureSlot::Retiring(entry) => entry.generation,
                _ => continue,
            };
            let previous = std::mem::replace(
                &mut self.slots[slot_index],
                TextureSlot::Retired { generation },
            );
            let entry = match previous {
                TextureSlot::Active(entry) | TextureSlot::Retiring(entry) => entry,
                _ => unreachable!("registered texture changed without a mutable alias"),
            };
            self.unregister_and_expose(slot_index, entry);
        }
        self.by_native.clear();
        self.reusable.clear();
        self.frame_removals.clear();
    }

    pub(super) fn release_after_native_destroy(&mut self) {
        self.by_native.clear();
        self.reusable.clear();
        self.slots.clear();
    }
}

fn mark_want_destroy(texture: &mut TextureData) {
    unsafe {
        (*texture.as_raw_mut()).WantDestroyNextFrame = true;
        sys::ImTextureData_SetStatus(texture.as_raw_mut(), sys::ImTextureStatus_WantDestroy);
    }
}

impl Context {
    /// Transfer an owned user texture into this Context's managed registry.
    pub fn register_texture(&mut self, texture: OwnedTextureData) -> ManagedTextureId {
        let _guard = CTX_MUTEX.lock();
        let _bound = RawBoundContextGuard::bind(self.raw);
        let watermark = self.snapshot_hub.completion_watermark();
        self.texture_registry
            .borrow_mut()
            .register(texture, watermark)
    }

    /// Read an active managed texture inside a non-escaping closure.
    ///
    /// The facade deliberately has no raw-pointer accessor.
    ///
    /// ```compile_fail
    /// use dear_imgui_rs::{Context, ManagedTextureId, sys};
    ///
    /// fn leak_native(context: &Context, id: ManagedTextureId) -> *const sys::ImTextureData {
    ///     context.with_texture(id, |texture| texture.as_raw()).unwrap()
    /// }
    /// ```
    pub fn with_texture<R>(
        &self,
        id: ManagedTextureId,
        f: impl for<'texture> FnOnce(ManagedTextureRef<'texture>) -> R,
    ) -> Result<R, TextureError> {
        let _guard = CTX_MUTEX.lock();
        let _bound = RawBoundContextGuard::bind(self.raw);
        self.texture_registry.borrow().with_texture(id, f)
    }

    /// Mutate an active managed texture inside a non-escaping closure.
    ///
    /// Renderer-owned state can only be changed by request-bound feedback.
    ///
    /// ```compile_fail
    /// use dear_imgui_rs::{Context, ManagedTextureId, TextureStatus};
    ///
    /// fn bypass_renderer(context: &mut Context, id: ManagedTextureId) {
    ///     context
    ///         .with_texture_mut(id, |mut texture| texture.set_status(TextureStatus::OK))
    ///         .unwrap();
    /// }
    /// ```
    pub fn with_texture_mut<R>(
        &mut self,
        id: ManagedTextureId,
        f: impl for<'texture> FnOnce(ManagedTextureMut<'texture>) -> R,
    ) -> Result<R, TextureError> {
        let _guard = CTX_MUTEX.lock();
        let _bound = RawBoundContextGuard::bind(self.raw);
        self.texture_registry.borrow_mut().with_texture_mut(id, f)
    }

    /// Mutate an active managed texture with flattened access and pixel-validation errors.
    ///
    /// # Errors
    ///
    /// Returns a [`TextureError`] if the handle is unavailable or the closure rejects its pixel
    /// data. Successful [`ManagedTextureMut`] operations before a later error remain applied
    /// and immediately invalidate older renderer feedback.
    ///
    /// ```
    /// use dear_imgui_rs::{
    ///     Context, TextureError, OwnedTextureData, TextureFormat,
    /// };
    ///
    /// let mut context = Context::create();
    /// let texture = OwnedTextureData::from_pixels(TextureFormat::RGBA32, 1, 1, &[0; 4])?;
    /// let id = context.register_texture(texture);
    /// let error = context
    ///     .try_with_texture_mut(id, |mut texture| texture.replace_pixels(&[0; 3]))
    ///     .unwrap_err();
    /// assert!(matches!(
    ///     error,
    ///     TextureError::ByteLengthMismatch { expected: 4, actual: 3 }
    /// ));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn try_with_texture_mut<R>(
        &mut self,
        id: ManagedTextureId,
        f: impl for<'texture> FnOnce(ManagedTextureMut<'texture>) -> Result<R, TextureError>,
    ) -> Result<R, TextureError> {
        self.with_texture_mut(id, f)?
    }

    /// Stop accepting new draw references and retire a managed texture.
    ///
    /// An open or rendered frame retains its texture data and binding so already submitted
    /// commands can still be rendered or captured. Retirement starts when that frame expires.
    pub fn remove_texture(&mut self, id: ManagedTextureId) -> Result<(), TextureError> {
        let _guard = CTX_MUTEX.lock();
        let _bound = RawBoundContextGuard::bind(self.raw);
        let watermark = self.snapshot_hub.completion_watermark();
        let retain_frame =
            self.frame_lifecycle_state_unlocked() != super::FrameLifecycleState::Idle;
        self.texture_registry
            .borrow_mut()
            .remove(id, watermark, retain_frame)
    }

    pub(crate) fn collect_retired_textures(&mut self) {
        let watermark = self.snapshot_hub.completion_watermark();
        self.texture_registry.borrow_mut().reap_destroyed(watermark);
    }
}

#[derive(Default)]
struct TextureSyncQueue {
    entries: HashMap<SnapshotTextureId, PendingTexture>,
    next_revision: u64,
}

struct PendingTexture {
    revision: u64,
    operation: Option<Arc<TextureOp>>,
}

impl TextureSyncQueue {
    fn operation(&self, id: SnapshotTextureId) -> Option<&TextureOp> {
        self.entries.get(&id)?.operation.as_deref()
    }

    fn stage(&mut self, id: SnapshotTextureId, operation: Arc<TextureOp>) {
        if matches!(operation.as_ref(), TextureOp::Destroy)
            && matches!(self.operation(id), Some(TextureOp::Destroy))
        {
            return;
        }
        self.next_revision = self
            .next_revision
            .checked_add(1)
            .expect("texture revision exhausted");
        self.entries.insert(
            id,
            PendingTexture {
                revision: self.next_revision,
                operation: Some(operation),
            },
        );
    }

    fn requests(&self) -> Vec<PendingTextureRequest> {
        self.entries
            .iter()
            .filter_map(|(&texture, pending)| {
                Some(PendingTextureRequest {
                    texture,
                    revision: pending.revision,
                    op: Arc::clone(pending.operation.as_ref()?),
                })
            })
            .collect()
    }

    fn acknowledge(&mut self, id: SnapshotTextureId, revision: u64) -> bool {
        let Some(entry) = self.entries.get_mut(&id) else {
            return false;
        };
        if entry.revision != revision {
            return false;
        }
        // A later epoch can confirm the same upload with a newer backend binding.
        // Keep only its receipt identity; the potentially large payload is released.
        entry.operation = None;
        true
    }

    fn forget(&mut self, id: SnapshotTextureId) {
        self.entries.remove(&id);
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

fn cumulative_update_rect(previous: &TextureOp, incoming: &TextureOp) -> Option<TextureRect> {
    let TextureOp::Update { rects: old, .. } = previous else {
        return None;
    };
    let TextureOp::Update { rects: new, .. } = incoming else {
        return None;
    };
    let mut rects = old.iter().chain(new).map(|upload| upload.rect);
    let first = rects.next()?;
    let (mut x, mut y, mut right, mut bottom) = (
        first.x,
        first.y,
        u32::from(first.x) + u32::from(first.w),
        u32::from(first.y) + u32::from(first.h),
    );
    for rect in rects {
        x = x.min(rect.x);
        y = y.min(rect.y);
        right = right.max(u32::from(rect.x) + u32::from(rect.w));
        bottom = bottom.max(u32::from(rect.y) + u32::from(rect.h));
    }
    Some(TextureRect {
        x,
        y,
        w: u16::try_from(right - u32::from(x)).ok()?,
        h: u16::try_from(bottom - u32::from(y)).ok()?,
    })
}

fn update_pending_create(previous: &TextureOp, incoming: &TextureOp) -> Option<TextureOp> {
    let TextureOp::Create {
        format,
        width,
        height,
        row_pitch,
        pixels,
    } = previous
    else {
        return None;
    };
    let TextureOp::Update { rects, .. } = incoming else {
        return None;
    };
    let mut pixels = pixels.clone();
    let bpp = crate::texture::get_format_bytes_per_pixel(*format);
    for upload in rects {
        for row in 0..usize::from(upload.rect.h) {
            let target =
                (usize::from(upload.rect.y) + row) * row_pitch + usize::from(upload.rect.x) * bpp;
            let source = row * upload.row_pitch;
            pixels[target..target + upload.row_pitch]
                .copy_from_slice(&upload.data[source..source + upload.row_pitch]);
        }
    }
    Some(TextureOp::Create {
        format: *format,
        width: *width,
        height: *height,
        row_pitch: *row_pitch,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::snapshot_hub::SnapshotHub;
    use crate::texture::{TextureFormat, TextureId};

    fn texture() -> OwnedTextureData {
        OwnedTextureData::from_pixels(TextureFormat::RGBA32, 1, 1, &[1, 2, 3, 4]).unwrap()
    }

    #[test]
    fn native_debug_ids_do_not_participate_in_managed_identity() {
        let mut context = Context::create();
        let first = texture();
        let second = texture();
        assert_eq!(first.native_unique_id(), 0);
        assert_eq!(second.native_unique_id(), 0);
        let first_id = context.register_texture(first);
        let second_id = context.register_texture(second);
        assert_ne!(first_id, second_id);
        assert_eq!(first_id.context_id(), context.id());
        assert_eq!(second_id.context_id(), context.id());
    }

    #[test]
    fn foreign_and_reused_handles_fail_before_native_access() {
        let mut context_a = Context::create();
        let first_id = context_a.register_texture(texture());
        let suspended_a = context_a;
        let mut context_b = Context::create();
        assert!(matches!(
            context_b.with_texture(first_id, |_| ()),
            Err(TextureError::ForeignContext { .. })
        ));
        assert!(matches!(
            context_b.try_with_texture_mut(first_id, |mut texture| {
                texture.replace_pixels(&[4, 3, 2, 1])
            }),
            Err(TextureError::ForeignContext { .. })
        ));
        drop(context_b);
        let mut context_a = suspended_a;
        context_a.remove_texture(first_id).expect("unused texture");
        assert_eq!(
            context_a.try_with_texture_mut(first_id, |mut texture| {
                texture.replace_pixels(&[4, 3, 2, 1])
            }),
            Err(TextureError::AlreadyRemoved(first_id))
        );
        let before_refresh_id = context_a.register_texture(texture());
        assert_ne!(
            before_refresh_id.slot(),
            first_id.slot(),
            "a native-exposed allocation must not be reused before the texture list refreshes"
        );
        context_a
            .texture_registry
            .borrow_mut()
            .observe_native_texture_list_refresh();
        let replacement_id = context_a.register_texture(texture());
        assert_eq!(replacement_id.slot(), first_id.slot());
        assert_ne!(replacement_id.generation(), first_id.generation());
        assert_eq!(
            context_a.with_texture(first_id, |_| ()),
            Err(TextureError::StaleGeneration(first_id))
        );
    }

    #[test]
    fn managed_queue_marker_is_idempotent_and_cleared_by_destroy_feedback() {
        let mut context = Context::create();
        let id = context.register_texture(texture());
        let native = context
            .texture_registry
            .borrow()
            .active_entry(id)
            .expect("registered texture")
            .texture
            .as_raw()
            .cast_mut();
        let atlas = FontAtlasSnapshotTarget::new(std::ptr::null_mut(), context.id(), Vec::new());
        let create = Arc::new(TextureOp::Create {
            format: TextureFormat::RGBA32,
            width: 1,
            height: 1,
            row_pitch: 4,
            pixels: vec![1, 2, 3, 4],
        });
        let captured = vec![CapturedTextureOperation {
            texture: SnapshotTextureId::User(id),
            op: create,
        }];

        {
            let mut registry = context.texture_registry.borrow_mut();
            let pending = registry
                .stage_snapshot_operations(&captured, &atlas)
                .expect("create request should claim the managed queue");
            let first_revision = pending[0].revision;
            let pending = registry
                .stage_snapshot_operations(&[], &atlas)
                .expect("claiming the same managed queue must be idempotent");
            assert_eq!(pending[0].revision, first_revision);
        }
        unsafe {
            assert_eq!((*native).QueueUserData, native.cast());
        }

        {
            let mut registry = context.texture_registry.borrow_mut();
            let entry = registry.active_entry_mut(id).expect("active texture");
            unsafe {
                entry.texture.set_tex_id(TextureId::new(73));
                entry.texture.set_status(TextureStatus::OK);
            }
            registry
                .remove(id, 0, false)
                .expect("bound texture should retire");
        }

        let captured = vec![CapturedTextureOperation {
            texture: SnapshotTextureId::User(id),
            op: Arc::new(TextureOp::Destroy),
        }];
        let destroy = context
            .texture_registry
            .borrow_mut()
            .stage_snapshot_operations(&captured, &atlas)
            .expect("destroy request should remain queue-owned");

        let mut hub = SnapshotHub::new(context.id());
        let generation = hub
            .validate_consumer_admission()
            .expect("fresh snapshot hub should admit a renderer");
        let consumer = hub.commit_synchronous_consumer_admission(generation);
        let (epoch, requests) = hub
            .begin_synchronous(&consumer, destroy, &atlas)
            .expect("destroy request should enter a synchronous epoch");
        let feedback = requests[0]
            .destroyed()
            .expect("destroy request should produce matching feedback");
        context
            .texture_registry
            .borrow_mut()
            .apply_snapshot_feedback(&[feedback], &atlas, epoch.sequence())
            .expect("matching destroy feedback should reconcile");

        unsafe {
            assert!((*native).QueueUserData.is_null());
        }
    }

    #[test]
    fn destroy_status_without_request_bound_ack_cannot_reap_allocation() {
        let mut context = Context::create();
        let id = context.register_texture(texture());
        let native = {
            let mut registry = context.texture_registry.borrow_mut();
            let entry = registry.active_entry_mut(id).expect("active texture");
            let texture = &mut entry.texture;
            unsafe {
                // This test seeds renderer-owned state before exercising retirement validation.
                texture.set_tex_id(TextureId::new(91));
                texture.set_status(TextureStatus::OK);
            }
            texture.as_raw_mut()
        };
        context.remove_texture(id).expect("begin retirement");
        assert_eq!(
            context.remove_texture(id),
            Err(TextureError::AlreadyRetiring(id))
        );
        assert_eq!(
            context.texture_registry.borrow().id_for_native(native),
            Some(id)
        );
        unsafe {
            // Deliberately bypass feedback to prove that native status alone cannot retire data.
            TextureData::from_raw(native).set_status(TextureStatus::Destroyed);
        }
        context.collect_retired_textures();
        assert_eq!(
            context.texture_registry.borrow().id_for_native(native),
            Some(id),
            "native status alone must not impersonate request-bound feedback"
        );
        {
            let mut registry = context.texture_registry.borrow_mut();
            let TextureSlot::Retiring(entry) = &mut registry.slots[id.slot() as usize] else {
                panic!("texture should still be retiring");
            };
            entry.destroy_ack_epoch = Some(0);
        }
        context.collect_retired_textures();
        assert_eq!(
            context.texture_registry.borrow().id_for_native(native),
            None
        );
    }

    #[test]
    fn removed_storage_outlives_the_cached_native_texture_list() {
        let mut context = Context::create();
        context.io_mut().set_display_size([128.0, 128.0]);
        context.io_mut().set_delta_time(1.0 / 60.0);
        context
            .font_atlas()
            .try_claim_legacy_renderer()
            .expect("legacy renderer font atlas should be available")
            .build();
        let id = context.register_texture(texture());
        let native = context
            .texture_registry
            .borrow()
            .active_entry(id)
            .expect("registered texture")
            .texture
            .as_raw();

        drop(context.begin_frame());
        let platform_io = unsafe { &*context.platform_io_ptr("test") };
        assert!((0..platform_io.Textures.Size).any(|index| unsafe {
            std::ptr::eq(
                *platform_io.Textures.Data.add(index as usize),
                native.cast_mut(),
            )
        }));

        context.remove_texture(id).expect("unused texture");
        assert!(matches!(
            &context.texture_registry.borrow().slots[id.slot() as usize],
            TextureSlot::NativeExposed { entry, .. }
                if std::ptr::eq(entry.texture.as_raw(), native)
        ));

        drop(context.begin_frame());
        assert!(matches!(
            &context.texture_registry.borrow().slots[id.slot() as usize],
            TextureSlot::Retired { generation } if *generation == id.generation()
        ));
    }
}

#[cfg(test)]
mod sync_tests {
    use super::{TextureSyncQueue, cumulative_update_rect, update_pending_create};
    use crate::ContextId;
    use crate::render::snapshot::{SnapshotTextureId, TextureOp, TextureUploadRect};
    use crate::texture::{TextureFormat, TextureRect};
    use std::sync::Arc;

    fn id() -> SnapshotTextureId {
        SnapshotTextureId::FontAtlas {
            context: ContextId::allocate().unwrap(),
            stamp: 1,
            generation: 1,
        }
    }

    fn create() -> TextureOp {
        TextureOp::Create {
            format: TextureFormat::Alpha8,
            width: 4,
            height: 1,
            row_pitch: 4,
            pixels: vec![0; 4],
        }
    }

    fn update(x: u16, value: u8) -> TextureOp {
        TextureOp::Update {
            format: TextureFormat::Alpha8,
            width: 4,
            height: 1,
            rects: vec![TextureUploadRect {
                rect: TextureRect {
                    x,
                    y: 0,
                    w: 1,
                    h: 1,
                },
                row_pitch: 1,
                data: vec![value],
            }],
        }
    }

    #[test]
    fn retries_share_payload_and_stale_feedback_cannot_consume_newer_work() {
        let mut queue = TextureSyncQueue::default();
        let id = id();
        queue.stage(id, Arc::new(create()));
        let first = queue.requests().pop().unwrap();
        let retry = queue.requests().pop().unwrap();
        assert!(Arc::ptr_eq(&first.op, &retry.op));
        let merged = update_pending_create(queue.operation(id).unwrap(), &update(2, 7)).unwrap();
        queue.stage(id, Arc::new(merged));
        assert!(!queue.acknowledge(id, first.revision));
        let latest = queue.requests().pop().unwrap();
        assert!(
            matches!(latest.op.as_ref(), TextureOp::Create { pixels, .. } if pixels == &[0, 0, 7, 0])
        );
        assert!(queue.acknowledge(id, latest.revision));
        assert!(queue.requests().is_empty());
        assert!(queue.entries[&id].operation.is_none());
    }

    #[test]
    fn reset_never_reuses_an_upload_identity_and_destroy_is_stable() {
        let mut queue = TextureSyncQueue::default();
        let id = id();
        queue.stage(id, Arc::new(create()));
        let first = queue.requests().pop().unwrap();
        queue.clear();
        queue.stage(id, Arc::new(create()));
        assert_ne!(first.revision, queue.requests()[0].revision);
        queue.stage(id, Arc::new(TextureOp::Destroy));
        let destroy = queue.requests().pop().unwrap();
        queue.stage(id, Arc::new(TextureOp::Destroy));
        assert_eq!(destroy.revision, queue.requests()[0].revision);
        assert!(!queue.acknowledge(id, first.revision));
    }

    #[test]
    fn dirty_update_union_is_bounded_by_one_rectangle() {
        assert_eq!(
            cumulative_update_rect(&update(0, 1), &update(3, 2)),
            Some(TextureRect {
                x: 0,
                y: 0,
                w: 4,
                h: 1
            })
        );
    }
}
