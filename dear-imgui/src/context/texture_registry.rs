use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::num::NonZeroU64;
use std::rc::Rc;
use std::sync::Arc;

use crate::render::snapshot::{
    PendingTextureRequest, RendererConsumerError, ResolvedSnapshotTexture, SnapshotError,
    SnapshotTextureId, TextureFeedback, TextureFeedbackResult, TextureOp, TextureRequestKind,
};
use crate::sys;
use crate::texture::{
    ManagedTextureError, ManagedTextureId, ManagedTextureMut, ManagedTextureMutationError,
    ManagedTextureRef, OwnedTextureData, TextureData, TextureDataError, TextureStatus,
};

use super::binding::CTX_MUTEX;
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
    revision: u64,
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
            .map(|target| ResolvedSnapshotTexture {
                id: target.id,
                revision: target.revision,
            })
    }

    fn find(&self, id: SnapshotTextureId) -> Option<FontAtlasTextureTarget> {
        self.textures.iter().find(|target| target.id == id).copied()
    }

    fn track_operation(&self, id: SnapshotTextureId, operation: &mut Arc<TextureOp>) -> u64 {
        let target = self
            .find(id)
            .expect("font atlas operation must target the current texture list");
        unsafe {
            TextureData::from_raw(target.texture).claim_managed_queue();
        }
        crate::fonts::track_font_atlas_texture_operation(self.atlas, id, operation)
    }

    pub(crate) fn record_request_reference(&self, id: SnapshotTextureId, epoch: u64) {
        crate::fonts::record_font_atlas_texture_reference(self.atlas, id, epoch);
    }

    fn identity_is_known(&self, id: SnapshotTextureId) -> bool {
        matches!(id, SnapshotTextureId::FontAtlas { context, .. } if context == self.context)
            && crate::fonts::font_atlas_texture_identity_is_known(self.atlas, id)
    }

    fn revision_is_current(&self, id: SnapshotTextureId, revision: u64) -> bool {
        self.find(id).is_some()
            && crate::fonts::font_atlas_texture_revision_is_current(self.atlas, id, revision)
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
    pub(crate) fn new(
        id: SnapshotTextureId,
        revision: u64,
        texture: *mut sys::ImTextureData,
    ) -> Self {
        Self {
            id,
            revision,
            texture,
        }
    }
}

pub(crate) struct ManagedTextureRegistry {
    context: ContextId,
    slots: Vec<TextureSlot>,
    reusable: Vec<u32>,
    by_native: HashMap<usize, ManagedTextureId>,
    native_refresh_generation: u64,
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
    generation: NonZeroU64,
    revision: u64,
    operation: Option<Arc<TextureOp>>,
    last_reference_epoch: u64,
    destroy_ack_epoch: Option<u64>,
    texture: OwnedTextureData,
}

impl TextureEntry {
    fn advance_revision(&mut self) {
        advance_revision(&mut self.revision);
    }
}

struct ManagedTextureMutationRevision<'revision> {
    revision: &'revision mut u64,
    mutated: bool,
}

impl ManagedTextureMutationRevision<'_> {
    fn new(revision: &mut u64) -> ManagedTextureMutationRevision<'_> {
        ManagedTextureMutationRevision {
            revision,
            mutated: false,
        }
    }
}

impl Drop for ManagedTextureMutationRevision<'_> {
    fn drop(&mut self) {
        if self.mutated {
            advance_revision(self.revision);
        }
    }
}

fn advance_revision(revision: &mut u64) {
    *revision = revision
        .checked_add(1)
        .expect("managed texture revision space exhausted");
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
            by_native: HashMap::new(),
            native_refresh_generation: 0,
        }))
    }

    fn validate_context(&self, id: ManagedTextureId) -> Result<(), ManagedTextureError> {
        if id.context_id() == self.context {
            Ok(())
        } else {
            Err(ManagedTextureError::ForeignContext {
                expected: self.context,
                actual: id.context_id(),
            })
        }
    }

    fn slot(&self, id: ManagedTextureId) -> Result<&TextureSlot, ManagedTextureError> {
        self.validate_context(id)?;
        let slot = self
            .slots
            .get(id.slot() as usize)
            .ok_or(ManagedTextureError::UnknownSlot(id))?;
        let generation = match slot {
            TextureSlot::Active(entry)
            | TextureSlot::Retiring(entry)
            | TextureSlot::NativeExposed { entry, .. } => entry.generation,
            TextureSlot::Retired { generation } => *generation,
            TextureSlot::Exhausted => return Err(ManagedTextureError::AlreadyRemoved(id)),
        };
        if generation != id.generation() {
            return Err(ManagedTextureError::StaleGeneration(id));
        }
        Ok(slot)
    }

    fn active_entry(&self, id: ManagedTextureId) -> Result<&TextureEntry, ManagedTextureError> {
        match self.slot(id)? {
            TextureSlot::Active(entry) => Ok(entry),
            TextureSlot::Retiring(_) => Err(ManagedTextureError::Retiring(id)),
            TextureSlot::NativeExposed { .. }
            | TextureSlot::Retired { .. }
            | TextureSlot::Exhausted => Err(ManagedTextureError::AlreadyRemoved(id)),
        }
    }

    fn active_entry_mut(
        &mut self,
        id: ManagedTextureId,
    ) -> Result<&mut TextureEntry, ManagedTextureError> {
        self.validate_context(id)?;
        let slot = self
            .slots
            .get_mut(id.slot() as usize)
            .ok_or(ManagedTextureError::UnknownSlot(id))?;
        let generation = match slot {
            TextureSlot::Active(entry)
            | TextureSlot::Retiring(entry)
            | TextureSlot::NativeExposed { entry, .. } => entry.generation,
            TextureSlot::Retired { generation } => *generation,
            TextureSlot::Exhausted => return Err(ManagedTextureError::AlreadyRemoved(id)),
        };
        if generation != id.generation() {
            return Err(ManagedTextureError::StaleGeneration(id));
        }
        match slot {
            TextureSlot::Active(entry) => Ok(entry),
            TextureSlot::Retiring(_) => Err(ManagedTextureError::Retiring(id)),
            TextureSlot::NativeExposed { .. }
            | TextureSlot::Retired { .. }
            | TextureSlot::Exhausted => Err(ManagedTextureError::AlreadyRemoved(id)),
        }
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
            generation,
            revision: 0,
            operation: None,
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

    pub(crate) fn resolve(
        &self,
        id: ManagedTextureId,
    ) -> Result<sys::ImTextureRef, ManagedTextureError> {
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
    ) -> Result<ResolvedSnapshotTexture, SnapshotError> {
        if let Some(id) = self.id_for_native(native) {
            let entry = match self.slot(id)? {
                TextureSlot::Active(entry) | TextureSlot::Retiring(entry) => entry,
                TextureSlot::NativeExposed { .. }
                | TextureSlot::Retired { .. }
                | TextureSlot::Exhausted => {
                    return Err(ManagedTextureError::AlreadyRemoved(id).into());
                }
            };
            return Ok(ResolvedSnapshotTexture {
                id: SnapshotTextureId::User(id),
                revision: entry.revision,
            });
        }
        if let Some(resolved) = atlas.resolve(native) {
            return Ok(resolved);
        }
        Err(SnapshotError::UnknownManagedTexture)
    }

    pub(crate) fn record_snapshot_references(
        &mut self,
        ids: &std::collections::HashSet<ManagedTextureId>,
        epoch: u64,
    ) -> Result<(), ManagedTextureError> {
        for id in ids {
            match self.slot(*id)? {
                TextureSlot::Active(_) | TextureSlot::Retiring(_) => {}
                TextureSlot::NativeExposed { .. }
                | TextureSlot::Retired { .. }
                | TextureSlot::Exhausted => {
                    return Err(ManagedTextureError::AlreadyRemoved(*id));
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
    ) -> Result<R, ManagedTextureError> {
        let entry = self.active_entry(id)?;
        Ok(f(ManagedTextureRef::new(&entry.texture)))
    }

    fn with_texture_mut<R>(
        &mut self,
        id: ManagedTextureId,
        f: impl for<'texture> FnOnce(ManagedTextureMut<'texture>) -> R,
    ) -> Result<R, ManagedTextureError> {
        let entry = self.active_entry_mut(id)?;
        let TextureEntry {
            revision, texture, ..
        } = entry;
        let mut mutation_revision = ManagedTextureMutationRevision::new(revision);
        let result = f(ManagedTextureMut::new(
            texture,
            &mut mutation_revision.mutated,
        ));
        Ok(result)
    }

    pub(crate) fn track_snapshot_operations(
        &mut self,
        requests: &mut [PendingTextureRequest],
        atlas: &FontAtlasSnapshotTarget,
    ) -> Result<(), ManagedTextureError> {
        for request in requests {
            request.revision = match request.texture {
                SnapshotTextureId::User(id) => {
                    match self.slot(id)? {
                        TextureSlot::Active(_) | TextureSlot::Retiring(_) => {}
                        TextureSlot::NativeExposed { .. }
                        | TextureSlot::Retired { .. }
                        | TextureSlot::Exhausted => {
                            return Err(ManagedTextureError::AlreadyRemoved(id));
                        }
                    }
                    let slot_index = id.slot() as usize;
                    let entry = match &mut self.slots[slot_index] {
                        TextureSlot::Active(entry) | TextureSlot::Retiring(entry) => entry,
                        _ => unreachable!("validated texture slot changed without mutation"),
                    };
                    entry.texture.claim_managed_queue();
                    if entry
                        .operation
                        .as_deref()
                        .is_none_or(|current| current != request.op.as_ref())
                    {
                        entry.advance_revision();
                        entry.operation = Some(Arc::clone(&request.op));
                    } else if let Some(current) = &entry.operation {
                        request.op = Arc::clone(current);
                    }
                    entry.revision
                }
                SnapshotTextureId::FontAtlas { .. } => {
                    atlas.track_operation(request.texture, &mut request.op)
                }
            };
        }
        Ok(())
    }

    fn remove(&mut self, id: ManagedTextureId, watermark: u64) -> Result<(), ManagedTextureError> {
        self.reap_destroyed(watermark);
        self.validate_context(id)?;
        let slot_index = id.slot() as usize;
        let slot = self
            .slots
            .get(slot_index)
            .ok_or(ManagedTextureError::UnknownSlot(id))?;
        match slot {
            TextureSlot::Active(entry) if entry.generation == id.generation() => {}
            TextureSlot::Active(_) => return Err(ManagedTextureError::StaleGeneration(id)),
            TextureSlot::Retiring(entry) if entry.generation == id.generation() => {
                return Err(ManagedTextureError::AlreadyRetiring(id));
            }
            TextureSlot::Retiring(_) => return Err(ManagedTextureError::StaleGeneration(id)),
            TextureSlot::NativeExposed { entry, .. } if entry.generation == id.generation() => {
                return Err(ManagedTextureError::AlreadyRemoved(id));
            }
            TextureSlot::NativeExposed { .. } => {
                return Err(ManagedTextureError::StaleGeneration(id));
            }
            TextureSlot::Retired { generation } if *generation == id.generation() => {
                return Err(ManagedTextureError::AlreadyRemoved(id));
            }
            TextureSlot::Retired { .. } => return Err(ManagedTextureError::StaleGeneration(id)),
            TextureSlot::Exhausted => return Err(ManagedTextureError::AlreadyRemoved(id)),
        }

        let placeholder = TextureSlot::Retired {
            generation: id.generation(),
        };
        let TextureSlot::Active(mut entry) =
            std::mem::replace(&mut self.slots[slot_index], placeholder)
        else {
            unreachable!("validated active texture slot changed without a mutable alias")
        };
        let has_renderer_binding =
            !entry.texture.tex_id().is_null() || !entry.texture.backend_user_data().is_null();
        let has_outstanding_reference = entry.last_reference_epoch > watermark;
        if has_renderer_binding || has_outstanding_reference {
            mark_want_destroy(&mut entry.texture);
            self.slots[slot_index] = TextureSlot::Retiring(entry);
        } else {
            self.unregister_and_expose(slot_index, entry);
        }
        Ok(())
    }

    pub(crate) fn apply_snapshot_feedback(
        &mut self,
        feedback: &[TextureFeedback],
        atlas: &FontAtlasSnapshotTarget,
        epoch: u64,
    ) -> Result<usize, RendererConsumerError> {
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
                            return Err(RendererConsumerError::InvalidFeedbackTransition {
                                texture: key.texture,
                            });
                        }
                    }
                    TextureSlot::Retiring(_) => {}
                    TextureSlot::NativeExposed { .. }
                    | TextureSlot::Retired { .. }
                    | TextureSlot::Exhausted => {
                        return Err(ManagedTextureError::AlreadyRemoved(id).into());
                    }
                },
                SnapshotTextureId::FontAtlas { .. } => {
                    if !atlas.identity_is_known(key.texture) {
                        return Err(RendererConsumerError::StaleFontAtlas);
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
                    return Err(RendererConsumerError::InvalidFeedbackTransition {
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
            match key.texture {
                SnapshotTextureId::User(id) => {
                    let slot = &mut self.slots[id.slot() as usize];
                    let (entry, retiring) = match slot {
                        TextureSlot::Active(entry) => (entry, false),
                        TextureSlot::Retiring(entry) => (entry, true),
                        TextureSlot::NativeExposed { .. }
                        | TextureSlot::Retired { .. }
                        | TextureSlot::Exhausted => unreachable!(),
                    };
                    if entry.revision != key.revision {
                        continue;
                    }
                    match item.result() {
                        TextureFeedbackResult::Uploaded { texture_id } => {
                            unsafe {
                                // The complete feedback batch was validated against this Context,
                                // consumer generation, epoch, request, and texture revision above.
                                entry.texture.set_tex_id(texture_id);
                            }
                            if retiring {
                                mark_want_destroy(&mut entry.texture);
                            } else {
                                unsafe {
                                    // This is the sole validated reconciliation path for managed
                                    // renderer state.
                                    entry.texture.set_status(TextureStatus::OK);
                                }
                            }
                        }
                        TextureFeedbackResult::Destroyed => {
                            unsafe {
                                // Request validation proves that the renderer acknowledged this
                                // texture's matching destroy request for the active generation.
                                entry.texture.set_status(TextureStatus::Destroyed);
                            }
                            entry.destroy_ack_epoch = Some(epoch);
                        }
                        TextureFeedbackResult::Superseded | TextureFeedbackResult::Retry => {
                            unreachable!("non-mutating outcomes were filtered before slot access")
                        }
                    }
                    applied += 1;
                }
                SnapshotTextureId::FontAtlas { .. } => {
                    if !atlas.revision_is_current(key.texture, key.revision) {
                        continue;
                    }
                    let target = atlas.find(key.texture).expect(
                        "current font atlas ledger entry must be present in the fresh observation",
                    );
                    let texture = unsafe { TextureData::from_raw(target.texture) };
                    match item.result() {
                        TextureFeedbackResult::Uploaded { texture_id } => {
                            unsafe {
                                // The atlas target and request identity were validated above.
                                texture.set_tex_id(texture_id);
                            }
                            unsafe {
                                // Matching revision proves this upload completes the current
                                // atlas contents.
                                texture.set_status(TextureStatus::OK);
                            }
                        }
                        TextureFeedbackResult::Destroyed => {
                            unsafe {
                                // The matching request-bound destroy was validated above.
                                texture.set_status(TextureStatus::Destroyed);
                            }
                        }
                        TextureFeedbackResult::Superseded | TextureFeedbackResult::Retry => {
                            unreachable!("non-mutating outcomes were filtered before atlas access")
                        }
                    }
                    applied += 1;
                }
            }
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
        self.assert_current_context("Context::register_texture()");
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
    ) -> Result<R, ManagedTextureError> {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::with_texture()");
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
    ) -> Result<R, ManagedTextureError> {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::with_texture_mut()");
        self.texture_registry.borrow_mut().with_texture_mut(id, f)
    }

    /// Mutate an active managed texture with flattened access and pixel-validation errors.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedTextureMutationError::Access`] when `id` is foreign, stale, unknown, or
    /// retiring. Returns [`ManagedTextureMutationError::Data`] when the closure returns a pixel
    /// validation error. Each [`ManagedTextureMut`] operation is transactional, but the closure is
    /// not: successful operations performed before a later error remain applied and immediately
    /// invalidate older renderer feedback.
    ///
    /// ```
    /// use dear_imgui_rs::{
    ///     Context, ManagedTextureMutationError, OwnedTextureData, TextureDataError, TextureFormat,
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
    ///     ManagedTextureMutationError::Data(TextureDataError::ByteLengthMismatch {
    ///         expected: 4,
    ///         actual: 3,
    ///     })
    /// ));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn try_with_texture_mut<R>(
        &mut self,
        id: ManagedTextureId,
        f: impl for<'texture> FnOnce(ManagedTextureMut<'texture>) -> Result<R, TextureDataError>,
    ) -> Result<R, ManagedTextureMutationError> {
        self.with_texture_mut(id, f)
            .map_err(ManagedTextureMutationError::Access)?
            .map_err(ManagedTextureMutationError::Data)
    }

    /// Stop accepting new draw references and retire a managed texture.
    pub fn remove_texture(&mut self, id: ManagedTextureId) -> Result<(), ManagedTextureError> {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::remove_texture()");
        let watermark = self.snapshot_hub.completion_watermark();
        self.texture_registry.borrow_mut().remove(id, watermark)
    }

    pub(crate) fn collect_retired_textures(&mut self) {
        let watermark = self.snapshot_hub.completion_watermark();
        self.texture_registry.borrow_mut().reap_destroyed(watermark);
    }
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
        let suspended_a = context_a.suspend_or_panic();
        let mut context_b = Context::create();
        assert!(matches!(
            context_b.with_texture(first_id, |_| ()),
            Err(ManagedTextureError::ForeignContext { .. })
        ));
        assert!(matches!(
            context_b.try_with_texture_mut(first_id, |mut texture| {
                texture.replace_pixels(&[4, 3, 2, 1])
            }),
            Err(ManagedTextureMutationError::Access(
                ManagedTextureError::ForeignContext { .. }
            ))
        ));
        drop(context_b);
        let mut context_a = suspended_a.activate().expect("Context A should reactivate");
        context_a.remove_texture(first_id).expect("unused texture");
        assert_eq!(
            context_a.try_with_texture_mut(first_id, |mut texture| {
                texture.replace_pixels(&[4, 3, 2, 1])
            }),
            Err(ManagedTextureMutationError::Access(
                ManagedTextureError::AlreadyRemoved(first_id)
            ))
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
            Err(ManagedTextureError::StaleGeneration(first_id))
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
        let mut pending = vec![PendingTextureRequest {
            texture: SnapshotTextureId::User(id),
            revision: 0,
            op: create,
        }];

        {
            let mut registry = context.texture_registry.borrow_mut();
            registry
                .track_snapshot_operations(&mut pending, &atlas)
                .expect("create request should claim the managed queue");
            let first_revision = pending[0].revision;
            registry
                .track_snapshot_operations(&mut pending, &atlas)
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
            registry.remove(id, 0).expect("bound texture should retire");
        }

        let mut destroy = vec![PendingTextureRequest {
            texture: SnapshotTextureId::User(id),
            revision: 0,
            op: Arc::new(TextureOp::Destroy),
        }];
        context
            .texture_registry
            .borrow_mut()
            .track_snapshot_operations(&mut destroy, &atlas)
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
            Err(ManagedTextureError::AlreadyRetiring(id))
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
