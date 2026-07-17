use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::num::NonZeroU64;
use std::rc::Rc;

use crate::sys;
use crate::texture::{
    ManagedTextureError, ManagedTextureId, OwnedTextureData, TextureData, TextureStatus,
};

use super::binding::CTX_MUTEX;
use super::{Context, ContextId};

pub(crate) type SharedTextureRegistry = Rc<RefCell<ManagedTextureRegistry>>;

pub(crate) struct ManagedTextureRegistry {
    context: ContextId,
    slots: Vec<TextureSlot>,
    reusable: Vec<u32>,
    by_native: HashMap<usize, ManagedTextureId>,
}

enum TextureSlot {
    Active(TextureEntry),
    Retiring(TextureEntry),
    Retired { generation: NonZeroU64 },
    Exhausted,
}

struct TextureEntry {
    generation: NonZeroU64,
    revision: u64,
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
        formatter
            .debug_struct("ManagedTextureRegistry")
            .field("context", &self.context)
            .field("active", &active)
            .field("retiring", &retiring)
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
            TextureSlot::Active(entry) | TextureSlot::Retiring(entry) => entry.generation,
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
            TextureSlot::Retired { .. } | TextureSlot::Exhausted => {
                Err(ManagedTextureError::AlreadyRemoved(id))
            }
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
            TextureSlot::Active(entry) | TextureSlot::Retiring(entry) => entry.generation,
            TextureSlot::Retired { generation } => *generation,
            TextureSlot::Exhausted => return Err(ManagedTextureError::AlreadyRemoved(id)),
        };
        if generation != id.generation() {
            return Err(ManagedTextureError::StaleGeneration(id));
        }
        match slot {
            TextureSlot::Active(entry) => Ok(entry),
            TextureSlot::Retiring(_) => Err(ManagedTextureError::Retiring(id)),
            TextureSlot::Retired { .. } | TextureSlot::Exhausted => {
                Err(ManagedTextureError::AlreadyRemoved(id))
            }
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

    fn register(&mut self, mut texture: OwnedTextureData) -> ManagedTextureId {
        self.reap_destroyed();
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
            revision: 1,
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

    #[allow(dead_code, reason = "used by Context-owned snapshot capture in U3")]
    pub(crate) fn id_for_native(
        &self,
        native: *const sys::ImTextureData,
    ) -> Option<ManagedTextureId> {
        self.by_native.get(&(native as usize)).copied()
    }

    fn with_texture<R>(
        &self,
        id: ManagedTextureId,
        f: impl for<'texture> FnOnce(&'texture TextureData) -> R,
    ) -> Result<R, ManagedTextureError> {
        let entry = self.active_entry(id)?;
        Ok(f(&entry.texture))
    }

    fn with_texture_mut<R>(
        &mut self,
        id: ManagedTextureId,
        f: impl for<'texture> FnOnce(&'texture mut TextureData) -> R,
    ) -> Result<R, ManagedTextureError> {
        let entry = self.active_entry_mut(id)?;
        entry.revision = entry
            .revision
            .checked_add(1)
            .expect("managed texture revision space exhausted");
        Ok(f(&mut entry.texture))
    }

    fn remove(&mut self, id: ManagedTextureId) -> Result<(), ManagedTextureError> {
        self.reap_destroyed();
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
            TextureSlot::Retired { generation } if *generation == id.generation() => {
                return Err(ManagedTextureError::AlreadyRemoved(id));
            }
            TextureSlot::Retired { .. } => {
                return Err(ManagedTextureError::StaleGeneration(id));
            }
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
        let native = entry.texture.as_mut().as_raw_mut();
        let has_renderer_binding =
            !entry.texture.tex_id().is_null() || !entry.texture.backend_user_data().is_null();
        if has_renderer_binding {
            unsafe {
                (*native).WantDestroyNextFrame = true;
                sys::ImTextureData_SetStatus(native, sys::ImTextureStatus_WantDestroy);
            }
            self.slots[slot_index] = TextureSlot::Retiring(entry);
        } else {
            self.unregister_and_retire(slot_index, entry);
        }
        Ok(())
    }

    fn apply_feedback(
        &mut self,
        feedback: &[crate::render::snapshot::TextureFeedback],
    ) -> Result<usize, ManagedTextureError> {
        let mut seen = HashSet::with_capacity(feedback.len());
        for item in feedback {
            if !seen.insert(item.id) {
                return Err(ManagedTextureError::DuplicateFeedback(item.id));
            }
            if !matches!(item.status, TextureStatus::OK | TextureStatus::Destroyed) {
                return Err(ManagedTextureError::InvalidFeedbackStatus {
                    id: item.id,
                    status: item.status,
                });
            }
            match self.slot(item.id)? {
                TextureSlot::Active(_) | TextureSlot::Retiring(_) => {}
                TextureSlot::Retired { .. } | TextureSlot::Exhausted => {
                    return Err(ManagedTextureError::AlreadyRemoved(item.id));
                }
            }
        }

        for item in feedback {
            let slot = &mut self.slots[item.id.slot() as usize];
            let (entry, retiring) = match slot {
                TextureSlot::Active(entry) => (entry, false),
                TextureSlot::Retiring(entry) => (entry, true),
                TextureSlot::Retired { .. } | TextureSlot::Exhausted => {
                    unreachable!("feedback batch was validated before mutation")
                }
            };

            if item.status == TextureStatus::Destroyed {
                if retiring {
                    unsafe {
                        (*entry.texture.as_mut().as_raw_mut()).WantDestroyNextFrame = true;
                    }
                }
                entry.texture.set_status(TextureStatus::Destroyed);
                continue;
            }

            if let Some(tex_id) = item.tex_id {
                entry.texture.set_tex_id(tex_id);
            }
            if let Some(backend_user_data) = item.backend_user_data {
                entry
                    .texture
                    .set_backend_user_data(backend_user_data as *mut std::ffi::c_void);
            }
            entry.texture.set_status(TextureStatus::OK);
            if retiring {
                let native = entry.texture.as_mut().as_raw_mut();
                unsafe {
                    (*native).WantDestroyNextFrame = true;
                    sys::ImTextureData_SetStatus(native, sys::ImTextureStatus_WantDestroy);
                }
            }
        }
        let applied = feedback.len();
        self.reap_destroyed();
        Ok(applied)
    }

    fn reap_destroyed(&mut self) {
        let ready = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| match slot {
                TextureSlot::Retiring(entry)
                    if entry.texture.status() == TextureStatus::Destroyed
                        && entry.texture.tex_id().is_null()
                        && entry.texture.backend_user_data().is_null() =>
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
            self.unregister_and_retire(slot_index, entry);
        }
    }

    fn unregister_and_retire(&mut self, slot_index: usize, mut entry: TextureEntry) {
        let native = entry.texture.as_mut().as_raw_mut();
        unsafe {
            sys::igUnregisterUserTexture(native);
        }
        self.by_native.remove(&(native as usize));
        let generation = entry.generation;
        drop(entry);
        self.slots[slot_index] = TextureSlot::Retired { generation };
        self.reusable
            .push(u32::try_from(slot_index).expect("texture slot index must fit u32"));
    }

    pub(super) fn teardown(&mut self) {
        self.by_native.clear();
        self.reusable.clear();
        for slot in &mut self.slots {
            let generation = match slot {
                TextureSlot::Active(entry) | TextureSlot::Retiring(entry) => entry.generation,
                TextureSlot::Retired { .. } | TextureSlot::Exhausted => continue,
            };
            let previous = std::mem::replace(slot, TextureSlot::Retired { generation });
            let entry = match previous {
                TextureSlot::Active(entry) | TextureSlot::Retiring(entry) => entry,
                TextureSlot::Retired { .. } | TextureSlot::Exhausted => unreachable!(),
            };
            unsafe {
                sys::igUnregisterUserTexture(entry.texture.as_raw() as *mut sys::ImTextureData);
            }
            drop(entry);
        }
    }
}

impl Context {
    /// Transfer an owned user texture into this Context's managed registry.
    pub fn register_texture(&mut self, texture: OwnedTextureData) -> ManagedTextureId {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::register_texture()");
        self.texture_registry.borrow_mut().register(texture)
    }

    /// Read an active managed texture inside a non-escaping closure.
    pub fn with_texture<R>(
        &self,
        id: ManagedTextureId,
        f: impl for<'texture> FnOnce(&'texture TextureData) -> R,
    ) -> Result<R, ManagedTextureError> {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::with_texture()");
        self.texture_registry.borrow().with_texture(id, f)
    }

    /// Mutate an active managed texture inside a non-escaping closure.
    pub fn with_texture_mut<R>(
        &mut self,
        id: ManagedTextureId,
        f: impl for<'texture> FnOnce(&'texture mut TextureData) -> R,
    ) -> Result<R, ManagedTextureError> {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::with_texture_mut()");
        self.texture_registry.borrow_mut().with_texture_mut(id, f)
    }

    /// Stop accepting new draw references and retire a managed texture.
    ///
    /// Textures that reached a renderer remain allocated until the renderer clears their native
    /// binding and acknowledges `Destroyed`. Textures that never reached a renderer retire
    /// immediately.
    pub fn remove_texture(&mut self, id: ManagedTextureId) -> Result<(), ManagedTextureError> {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::remove_texture()");
        self.texture_registry.borrow_mut().remove(id)
    }

    /// Apply a validated renderer feedback batch to Context-owned managed textures.
    ///
    /// The complete batch is validated before any native texture state is changed. Detached
    /// consumers should use the generation and epoch envelope added by the snapshot consumer API;
    /// this method is the synchronous reconciliation boundary.
    pub fn apply_texture_feedback(
        &mut self,
        feedback: &[crate::render::snapshot::TextureFeedback],
    ) -> Result<usize, ManagedTextureError> {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::apply_texture_feedback()");
        self.texture_registry.borrow_mut().apply_feedback(feedback)
    }

    pub(crate) fn collect_retired_textures(&mut self) {
        self.texture_registry.borrow_mut().reap_destroyed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture::{TextureFormat, TextureId};

    fn texture() -> OwnedTextureData {
        let mut texture = OwnedTextureData::new();
        texture.create(TextureFormat::RGBA32, 1, 1);
        texture.set_data(&[1, 2, 3, 4]);
        texture
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
    fn foreign_and_reused_handles_fail_before_accessing_native_state() {
        let mut context_a = Context::create();
        let first_id = context_a.register_texture(texture());
        let suspended_a = context_a.suspend();

        let context_b = Context::create();
        assert!(matches!(
            context_b.with_texture(first_id, |_| ()),
            Err(ManagedTextureError::ForeignContext { .. })
        ));
        drop(context_b);

        let mut context_a = suspended_a
            .activate()
            .unwrap_or_else(|_| panic!("Context A should reactivate"));
        context_a
            .remove_texture(first_id)
            .expect("unbound texture should retire immediately");
        assert_eq!(
            context_a.with_texture(first_id, |_| ()),
            Err(ManagedTextureError::AlreadyRemoved(first_id))
        );

        let replacement_id = context_a.register_texture(texture());
        assert_eq!(replacement_id.slot(), first_id.slot());
        assert_ne!(replacement_id.generation(), first_id.generation());
        assert_eq!(
            context_a.with_texture(first_id, |_| ()),
            Err(ManagedTextureError::StaleGeneration(first_id))
        );
    }

    #[test]
    fn retiring_texture_keeps_allocation_until_destroy_acknowledgement() {
        let mut context = Context::create();
        let id = context.register_texture(texture());
        let native = context
            .with_texture_mut(id, |texture| {
                texture.set_tex_id(TextureId::new(91));
                texture.set_status(TextureStatus::OK);
                texture.as_raw_mut()
            })
            .expect("active texture");

        context.remove_texture(id).expect("begin retirement");
        assert_eq!(
            context.remove_texture(id),
            Err(ManagedTextureError::AlreadyRetiring(id))
        );
        assert_eq!(
            context.with_texture(id, |_| ()),
            Err(ManagedTextureError::Retiring(id))
        );
        assert_eq!(
            context.texture_registry.borrow().id_for_native(native),
            Some(id)
        );

        unsafe {
            let texture = TextureData::from_raw(native);
            texture.set_status(TextureStatus::Destroyed);
        }
        context.collect_retired_textures();
        assert_eq!(
            context.texture_registry.borrow().id_for_native(native),
            None
        );
        assert_eq!(
            context.with_texture(id, |_| ()),
            Err(ManagedTextureError::AlreadyRemoved(id))
        );
    }

    #[test]
    fn feedback_batches_validate_atomically_and_requeue_retiring_uploads() {
        let mut context = Context::create();
        let first = context.register_texture(texture());
        let second = context.register_texture(texture());

        let duplicate = [
            crate::render::snapshot::TextureFeedback::with_tex_id(
                first,
                TextureStatus::OK,
                TextureId::new(10),
            ),
            crate::render::snapshot::TextureFeedback::with_tex_id(
                first,
                TextureStatus::OK,
                TextureId::new(11),
            ),
        ];
        assert_eq!(
            context.apply_texture_feedback(&duplicate),
            Err(ManagedTextureError::DuplicateFeedback(first))
        );
        for id in [first, second] {
            context
                .with_texture(id, |texture| {
                    assert_eq!(texture.status(), TextureStatus::WantCreate);
                    assert!(texture.tex_id().is_null());
                })
                .expect("batch validation must not mutate either texture");
        }

        context
            .apply_texture_feedback(&[
                crate::render::snapshot::TextureFeedback::with_tex_id(
                    first,
                    TextureStatus::OK,
                    TextureId::new(20),
                ),
                crate::render::snapshot::TextureFeedback::with_tex_id(
                    second,
                    TextureStatus::OK,
                    TextureId::new(21),
                ),
            ])
            .expect("valid feedback batch");
        context.remove_texture(first).expect("begin retirement");

        context
            .apply_texture_feedback(&[crate::render::snapshot::TextureFeedback::with_tex_id(
                first,
                TextureStatus::OK,
                TextureId::new(22),
            )])
            .expect("late upload acknowledgement should be recorded and requeued for destroy");
        assert_eq!(
            context.with_texture(first, |_| ()),
            Err(ManagedTextureError::Retiring(first))
        );

        context
            .apply_texture_feedback(&[crate::render::snapshot::TextureFeedback::status(
                first,
                TextureStatus::Destroyed,
            )])
            .expect("destroy acknowledgement should retire the texture");
        assert_eq!(
            context.with_texture(first, |_| ()),
            Err(ManagedTextureError::AlreadyRemoved(first))
        );
    }
}
