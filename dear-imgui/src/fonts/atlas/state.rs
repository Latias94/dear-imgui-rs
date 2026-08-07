use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::context::ContextId;
use crate::error::ImGuiError;
use crate::render::snapshot::{RendererConsumerError, SnapshotTextureId, TextureOp};
use crate::sys;

use super::error::FontAtlasModeError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FontAtlasState {
    pub(super) stamp: u64,
    pub(super) generation: u64,
    pub(super) custom_rect_generation: u64,
    texture_borrows: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FontAtlasSnapshotIdentity {
    pub(crate) stamp: u64,
    pub(crate) texture_generation: u64,
    pub(crate) revision: u64,
    pub(crate) texture: *mut sys::ImTextureData,
}

#[derive(Clone, Debug)]
struct FontAtlasTextureLedgerEntry {
    revision: u64,
    operation: Option<Arc<TextureOp>>,
    live: bool,
    last_reference_epoch: HashMap<ContextId, u64>,
}

#[derive(Default)]
struct FontAtlasTextureLedger {
    entries: HashMap<u64, FontAtlasTextureLedgerEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FontAtlasRendererMode {
    Legacy {
        active_capabilities: usize,
    },
    Managed {
        context: usize,
        namespace: u64,
        renderer_reset_committed: bool,
    },
    RendererReleasePending {
        _retired_namespace: u64,
    },
}

pub(super) fn claim_font_atlas_legacy_renderer(
    raw: *mut sys::ImFontAtlas,
) -> Result<(), FontAtlasModeError> {
    assert!(!raw.is_null(), "legacy renderer requires a font atlas");
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let key = raw as usize;
        match states.renderer_modes.get_mut(&key) {
            None => {
                states.renderer_modes.insert(
                    key,
                    FontAtlasRendererMode::Legacy {
                        active_capabilities: 1,
                    },
                );
                Ok(())
            }
            Some(FontAtlasRendererMode::Legacy {
                active_capabilities,
            }) => {
                *active_capabilities = active_capabilities
                    .checked_add(1)
                    .expect("font atlas legacy capability count overflowed");
                Ok(())
            }
            Some(FontAtlasRendererMode::Managed { .. }) => {
                Err(FontAtlasModeError::ManagedRendererActive)
            }
            Some(FontAtlasRendererMode::RendererReleasePending { .. }) => {
                Err(FontAtlasModeError::RendererReleasePending)
            }
        }
    })
}

pub(super) fn release_font_atlas_legacy_renderer(raw: *mut sys::ImFontAtlas) {
    if raw.is_null() {
        return;
    }
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let key = raw as usize;
        match states.renderer_modes.get_mut(&key) {
            Some(FontAtlasRendererMode::Legacy {
                active_capabilities,
            }) => {
                debug_assert!(
                    *active_capabilities > 0,
                    "legacy font atlas capability count underflowed"
                );
                *active_capabilities = active_capabilities.saturating_sub(1);
            }
            Some(
                FontAtlasRendererMode::Managed { .. }
                | FontAtlasRendererMode::RendererReleasePending { .. },
            )
            | None => {
                debug_assert!(
                    false,
                    "legacy font atlas capability was released without a matching legacy claim"
                );
            }
        }
    });
}

pub(super) fn reset_font_atlas_mode_after_full_clear(raw: *mut sys::ImFontAtlas) {
    if raw.is_null() {
        return;
    }
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let key = raw as usize;
        if matches!(
            states.renderer_modes.get(&key),
            Some(FontAtlasRendererMode::Legacy {
                active_capabilities: 0
            })
        ) {
            states.renderer_modes.remove(&key);
        }
    });
}

pub(crate) fn validate_font_atlas_context_registration(
    raw: *mut sys::ImFontAtlas,
) -> Result<(), ImGuiError> {
    if raw.is_null() {
        return Ok(());
    }
    FONT_ATLAS_STATES.with(|states| {
        let states = states.borrow();
        match states.renderer_modes.get(&(raw as usize)) {
            Some(FontAtlasRendererMode::Managed { .. }) => Err(ImGuiError::SharedFontAtlasManaged),
            Some(FontAtlasRendererMode::RendererReleasePending { .. }) => {
                Err(ImGuiError::SharedFontAtlasRendererReleasePending)
            }
            Some(FontAtlasRendererMode::Legacy { .. }) | None => Ok(()),
        }
    })
}

pub(crate) fn validate_font_atlas_managed_renderer(
    raw: *mut sys::ImFontAtlas,
    context: *mut sys::ImGuiContext,
) -> Result<(), RendererConsumerError> {
    assert!(!raw.is_null(), "managed renderer requires a font atlas");
    assert!(
        !context.is_null(),
        "managed renderer requires an ImGui context"
    );
    FONT_ATLAS_STATES.with(|states| {
        let states = states.borrow();
        let atlas_key = raw as usize;
        let registered_contexts = states
            .contexts_by_atlas
            .get(&atlas_key)
            .map_or(0, HashSet::len);
        if registered_contexts != 1
            || !states
                .contexts_by_atlas
                .get(&atlas_key)
                .is_some_and(|contexts| contexts.contains(&(context as usize)))
        {
            return Err(
                RendererConsumerError::SharedFontAtlasRequiresExclusiveContext {
                    registered_contexts,
                },
            );
        }

        match states.renderer_modes.get(&atlas_key).copied() {
            Some(FontAtlasRendererMode::Managed { context: owner, .. })
                if owner == context as usize =>
            {
                return Ok(());
            }
            Some(FontAtlasRendererMode::Managed { .. }) => {
                return Err(
                    RendererConsumerError::SharedFontAtlasRequiresExclusiveContext {
                        registered_contexts,
                    },
                );
            }
            Some(FontAtlasRendererMode::Legacy { .. }) => {
                return Err(RendererConsumerError::FontAtlasRequiresManagedRebuild);
            }
            Some(FontAtlasRendererMode::RendererReleasePending { .. }) => {
                return Err(RendererConsumerError::SharedFontAtlasRendererReleasePending);
            }
            None => {}
        }

        if !font_atlas_supports_managed_renderer(raw) {
            return Err(RendererConsumerError::FontAtlasRequiresManagedRebuild);
        }
        Ok(())
    })
}

pub(crate) fn claim_validated_font_atlas_managed_renderer(
    raw: *mut sys::ImFontAtlas,
    context: *mut sys::ImGuiContext,
) -> u64 {
    debug_assert_eq!(
        validate_font_atlas_managed_renderer(raw, context),
        Ok(()),
        "font atlas renderer admission must be validated before it is committed"
    );
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let atlas_key = raw as usize;
        match states.renderer_modes.get(&atlas_key).copied() {
            Some(FontAtlasRendererMode::Managed {
                context: owner,
                namespace,
                ..
            }) if owner == context as usize => namespace,
            None => states.enter_managed_renderer(atlas_key, context as usize),
            Some(
                FontAtlasRendererMode::Legacy { .. }
                | FontAtlasRendererMode::Managed { .. }
                | FontAtlasRendererMode::RendererReleasePending { .. },
            ) => {
                unreachable!("validated font atlas renderer admission became invalid")
            }
        }
    })
}

fn font_atlas_supports_managed_renderer(raw: *mut sys::ImFontAtlas) -> bool {
    unsafe {
        let builder = (*raw).Builder;
        builder.is_null() || !(*builder).PreloadedAllGlyphsRanges
    }
}

pub(crate) fn font_atlas_snapshot_identities(
    raw: *mut sys::ImFontAtlas,
    context: *mut sys::ImGuiContext,
) -> Vec<FontAtlasSnapshotIdentity> {
    assert!(!raw.is_null(), "font atlas pointer must not be null");
    let observed = unsafe {
        let list = &(*raw).TexList;
        assert!(
            list.Size >= 0,
            "font atlas texture list size must not be negative"
        );
        let mut observed = Vec::with_capacity(list.Size.max(0) as usize + 1);
        if list.Size > 0 {
            assert!(
                !list.Data.is_null(),
                "non-empty font atlas texture list must have storage"
            );
            observed.extend_from_slice(std::slice::from_raw_parts(list.Data, list.Size as usize));
        }
        let current = (*raw).TexData;
        if !current.is_null() && !observed.contains(&current) {
            observed.push(current);
        }
        observed
    };

    let mut unique_ids = HashSet::with_capacity(observed.len());
    let observed = observed
        .into_iter()
        .filter(|texture| !texture.is_null())
        .map(|texture| {
            let unique_id = unsafe { (*texture).UniqueID };
            assert!(
                unique_id > 0,
                "font atlas texture allocation must have a positive unique ID"
            );
            assert!(
                unique_ids.insert(unique_id),
                "font atlas reused a live texture allocation unique ID"
            );
            (unique_id as u64, texture)
        })
        .collect::<Vec<_>>();

    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        states.get_or_insert(raw);
        let active_mode = states.renderer_modes.get(&(raw as usize)).copied();
        let ledger = states.texture_ledgers.entry(raw as usize).or_default();
        let stamp = match active_mode {
            Some(FontAtlasRendererMode::Managed {
                context: owner,
                namespace,
                ..
            }) => {
                debug_assert_eq!(owner, context as usize);
                namespace
            }
            Some(FontAtlasRendererMode::Legacy { .. }) | None => 0,
            Some(FontAtlasRendererMode::RendererReleasePending { .. }) => {
                panic!(
                    "a font atlas with a pending renderer release cannot be observed by a Context"
                )
            }
        };
        for entry in ledger.entries.values_mut() {
            entry.live = false;
        }
        observed
            .into_iter()
            .map(|(texture_generation, texture)| {
                let entry = ledger.entries.entry(texture_generation).or_insert_with(|| {
                    FontAtlasTextureLedgerEntry {
                        revision: 0,
                        operation: None,
                        live: true,
                        last_reference_epoch: HashMap::new(),
                    }
                });
                entry.live = true;
                FontAtlasSnapshotIdentity {
                    stamp,
                    texture_generation,
                    revision: entry.revision,
                    texture,
                }
            })
            .collect()
    })
}

pub(crate) fn track_font_atlas_texture_operation(
    raw: *mut sys::ImFontAtlas,
    id: SnapshotTextureId,
    operation: &mut Arc<TextureOp>,
) -> u64 {
    let SnapshotTextureId::FontAtlas {
        stamp, generation, ..
    } = id
    else {
        panic!("font atlas operation received a user texture identity");
    };
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        states.get_or_insert(raw);
        let namespace = states
            .renderer_modes
            .get_mut(&(raw as usize))
            .and_then(|mode| match mode {
                FontAtlasRendererMode::Managed {
                    namespace,
                    renderer_reset_committed,
                    ..
                } => {
                    *renderer_reset_committed = false;
                    Some(*namespace)
                }
                FontAtlasRendererMode::Legacy { .. }
                | FontAtlasRendererMode::RendererReleasePending { .. } => None,
            })
            .expect("font atlas managed renderer namespace was not claimed");
        assert_eq!(
            stamp, namespace,
            "font atlas operation belongs to a retired renderer namespace"
        );
        let entry = states
            .texture_ledgers
            .get_mut(&(raw as usize))
            .and_then(|ledger| ledger.entries.get_mut(&generation))
            .expect("font atlas operation must target the current observed texture list");
        assert!(
            entry.live,
            "font atlas operation must target a live texture allocation"
        );
        if entry
            .operation
            .as_deref()
            .is_none_or(|current| current != operation.as_ref())
        {
            entry.revision = entry
                .revision
                .checked_add(1)
                .expect("font atlas texture revision space exhausted");
            entry.operation = Some(Arc::clone(operation));
        } else if let Some(current) = &entry.operation {
            *operation = Arc::clone(current);
        }
        entry.revision
    })
}

pub(crate) fn record_font_atlas_texture_reference(
    raw: *mut sys::ImFontAtlas,
    id: SnapshotTextureId,
    epoch: u64,
) {
    let SnapshotTextureId::FontAtlas {
        context,
        stamp,
        generation,
    } = id
    else {
        return;
    };
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        states.get_or_insert(raw);
        let namespace = states
            .renderer_modes
            .get(&(raw as usize))
            .and_then(|mode| match mode {
                FontAtlasRendererMode::Managed { namespace, .. } => Some(*namespace),
                FontAtlasRendererMode::Legacy { .. }
                | FontAtlasRendererMode::RendererReleasePending { .. } => None,
            })
            .expect("font atlas managed renderer namespace was not claimed");
        assert_eq!(
            stamp, namespace,
            "font atlas request belongs to a retired renderer namespace"
        );
        let entry = states
            .texture_ledgers
            .get_mut(&(raw as usize))
            .and_then(|ledger| ledger.entries.get_mut(&generation))
            .expect("font atlas request identity was not observed");
        entry
            .last_reference_epoch
            .entry(context)
            .and_modify(|last| *last = (*last).max(epoch))
            .or_insert(epoch);
    });
}

pub(crate) fn font_atlas_texture_identity_is_known(
    raw: *mut sys::ImFontAtlas,
    id: SnapshotTextureId,
) -> bool {
    let SnapshotTextureId::FontAtlas {
        stamp, generation, ..
    } = id
    else {
        return false;
    };
    FONT_ATLAS_STATES.with(|states| {
        let states = states.borrow();
        states
            .texture_ledgers
            .get(&(raw as usize))
            .is_some_and(|ledger| {
                states
                    .renderer_modes
                    .get(&(raw as usize))
                    .is_some_and(|mode| {
                        matches!(
                            mode,
                            FontAtlasRendererMode::Managed { namespace, .. }
                                if *namespace == stamp
                        )
                    })
                    && ledger.entries.contains_key(&generation)
            })
    })
}

pub(crate) fn font_atlas_texture_revision_is_current(
    raw: *mut sys::ImFontAtlas,
    id: SnapshotTextureId,
    revision: u64,
) -> bool {
    let SnapshotTextureId::FontAtlas {
        stamp, generation, ..
    } = id
    else {
        return false;
    };
    FONT_ATLAS_STATES.with(|states| {
        let states = states.borrow();
        states
            .texture_ledgers
            .get(&(raw as usize))
            .filter(|_| {
                states
                    .renderer_modes
                    .get(&(raw as usize))
                    .is_some_and(|mode| {
                        matches!(
                            mode,
                            FontAtlasRendererMode::Managed { namespace, .. }
                                if *namespace == stamp
                        )
                    })
            })
            .and_then(|ledger| ledger.entries.get(&generation))
            .is_some_and(|entry| entry.live && entry.revision == revision)
    })
}

pub(crate) fn prune_font_atlas_texture_tombstones(
    raw: *mut sys::ImFontAtlas,
    context: ContextId,
    watermark: u64,
) {
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let Some(ledger) = states.texture_ledgers.get_mut(&(raw as usize)) else {
            return;
        };
        for entry in ledger.entries.values_mut() {
            if entry
                .last_reference_epoch
                .get(&context)
                .is_some_and(|last| *last <= watermark)
            {
                entry.last_reference_epoch.remove(&context);
            }
        }
        ledger
            .entries
            .retain(|_, entry| entry.live || !entry.last_reference_epoch.is_empty());
    });
}

pub(crate) fn mark_font_atlas_renderer_reset(raw: *mut sys::ImFontAtlas) {
    assert!(!raw.is_null(), "renderer reset requires a font atlas");
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        match states.renderer_modes.get_mut(&(raw as usize)) {
            Some(FontAtlasRendererMode::Managed {
                renderer_reset_committed,
                ..
            }) => *renderer_reset_committed = true,
            Some(FontAtlasRendererMode::Legacy { .. }) | None => {
                debug_assert!(false, "renderer reset requires a managed font atlas");
            }
            Some(FontAtlasRendererMode::RendererReleasePending { .. }) => {
                debug_assert!(
                    false,
                    "renderer reset cannot be committed after its Context unregisters"
                );
            }
        }
    });
}

#[derive(Default)]
pub(super) struct FontAtlasStates {
    next_stamp: u64,
    next_renderer_namespace: u64,
    next_custom_rect_nonce: u64,
    by_atlas: HashMap<usize, FontAtlasState>,
    texture_ledgers: HashMap<usize, FontAtlasTextureLedger>,
    renderer_modes: HashMap<usize, FontAtlasRendererMode>,
    contexts_by_atlas: HashMap<usize, HashSet<usize>>,
    custom_rect_nonces: HashMap<usize, HashMap<sys::ImFontAtlasRectId, u64>>,
}

thread_local! {
    static FONT_ATLAS_STATES: RefCell<FontAtlasStates> = RefCell::new(FontAtlasStates {
        next_stamp: 1,
        next_renderer_namespace: 1,
        next_custom_rect_nonce: 1,
        by_atlas: HashMap::new(),
        texture_ledgers: HashMap::new(),
        renderer_modes: HashMap::new(),
        contexts_by_atlas: HashMap::new(),
        custom_rect_nonces: HashMap::new(),
    });
}

impl FontAtlasStates {
    fn enter_managed_renderer(&mut self, atlas_key: usize, context: usize) -> u64 {
        let namespace = self.next_renderer_namespace;
        self.next_renderer_namespace = namespace
            .checked_add(1)
            .expect("font atlas renderer namespace space exhausted");
        self.renderer_modes.insert(
            atlas_key,
            FontAtlasRendererMode::Managed {
                context,
                namespace,
                renderer_reset_committed: true,
            },
        );
        self.texture_ledgers
            .entry(atlas_key)
            .or_default()
            .entries
            .clear();
        namespace
    }

    fn get_or_insert(&mut self, raw: *mut sys::ImFontAtlas) -> &mut FontAtlasState {
        let key = raw as usize;
        if !self.by_atlas.contains_key(&key) {
            let stamp = self.next_stamp;
            self.next_stamp = self
                .next_stamp
                .checked_add(1)
                .expect("font atlas stamp counter overflowed");
            self.by_atlas.insert(
                key,
                FontAtlasState {
                    stamp,
                    generation: 0,
                    custom_rect_generation: 0,
                    texture_borrows: 0,
                },
            );
        }
        self.by_atlas
            .get_mut(&key)
            .expect("font atlas state was just inserted")
    }
}

pub(super) fn font_atlas_state(raw: *mut sys::ImFontAtlas) -> FontAtlasState {
    assert!(!raw.is_null(), "font atlas pointer must not be null");
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        *states.get_or_insert(raw)
    })
}

pub(super) fn bump_font_atlas_generation(raw: *mut sys::ImFontAtlas) -> FontAtlasState {
    assert!(!raw.is_null(), "font atlas pointer must not be null");
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let state = states.get_or_insert(raw);
        state.generation = state
            .generation
            .checked_add(1)
            .expect("font atlas generation counter overflowed");
        *state
    })
}

pub(super) fn bump_custom_rect_generation(raw: *mut sys::ImFontAtlas) -> FontAtlasState {
    assert!(!raw.is_null(), "font atlas pointer must not be null");
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let state = states.get_or_insert(raw);
        state.custom_rect_generation = state
            .custom_rect_generation
            .checked_add(1)
            .expect("custom rectangle generation counter overflowed");
        let state = *state;
        states.custom_rect_nonces.remove(&(raw as usize));
        state
    })
}

pub(super) fn register_custom_rect_nonce(
    atlas: *mut sys::ImFontAtlas,
    raw_id: sys::ImFontAtlasRectId,
) -> (FontAtlasState, u64) {
    assert!(!atlas.is_null(), "font atlas pointer must not be null");
    assert!(raw_id >= 0, "custom rectangle ID must be valid");
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let state = *states.get_or_insert(atlas);
        let nonce = states.next_custom_rect_nonce;
        states.next_custom_rect_nonce = nonce
            .checked_add(1)
            .expect("custom rectangle nonce counter overflowed");
        let previous = states
            .custom_rect_nonces
            .entry(atlas as usize)
            .or_default()
            .insert(raw_id, nonce);
        assert!(
            previous.is_none(),
            "native font atlas reused an active custom rectangle ID"
        );
        (state, nonce)
    })
}

pub(super) fn custom_rect_nonce_is_active(
    atlas: *mut sys::ImFontAtlas,
    raw_id: sys::ImFontAtlasRectId,
    nonce: u64,
) -> bool {
    FONT_ATLAS_STATES.with(|states| {
        states
            .borrow()
            .custom_rect_nonces
            .get(&(atlas as usize))
            .and_then(|nonces| nonces.get(&raw_id))
            .is_some_and(|active| *active == nonce)
    })
}

pub(super) fn unregister_custom_rect_nonce(
    atlas: *mut sys::ImFontAtlas,
    raw_id: sys::ImFontAtlasRectId,
    nonce: u64,
) {
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let atlas_key = atlas as usize;
        let remove_atlas_entry = if let Some(nonces) = states.custom_rect_nonces.get_mut(&atlas_key)
        {
            if nonces.get(&raw_id).is_some_and(|active| *active == nonce) {
                nonces.remove(&raw_id);
            }
            nonces.is_empty()
        } else {
            false
        };
        if remove_atlas_entry {
            states.custom_rect_nonces.remove(&atlas_key);
        }
    });
}

pub(crate) fn register_font_atlas_context(
    atlas: *mut sys::ImFontAtlas,
    context: *mut sys::ImGuiContext,
) {
    assert!(!atlas.is_null(), "font atlas pointer must not be null");
    assert!(!context.is_null(), "ImGui context pointer must not be null");
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        states.get_or_insert(atlas);
        assert!(
            !matches!(
                states.renderer_modes.get(&(atlas as usize)),
                Some(
                    FontAtlasRendererMode::Managed { .. }
                        | FontAtlasRendererMode::RendererReleasePending { .. }
                )
            ),
            "cannot register another Context while a managed renderer owns the font atlas or its release is pending"
        );
        let inserted = states
            .contexts_by_atlas
            .entry(atlas as usize)
            .or_default()
            .insert(context as usize);
        assert!(
            inserted,
            "ImGui context was registered with its font atlas twice"
        );
    });
}

pub(crate) fn unregister_font_atlas_context(
    atlas: *mut sys::ImFontAtlas,
    context: *mut sys::ImGuiContext,
    context_id: ContextId,
) {
    if atlas.is_null() || context.is_null() {
        return;
    }
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let atlas_key = atlas as usize;
        if let Some(ledger) = states.texture_ledgers.get_mut(&atlas_key) {
            for entry in ledger.entries.values_mut() {
                entry.last_reference_epoch.remove(&context_id);
            }
            ledger
                .entries
                .retain(|_, entry| entry.live || !entry.last_reference_epoch.is_empty());
        }
        let remove_atlas_entry =
            if let Some(contexts) = states.contexts_by_atlas.get_mut(&atlas_key) {
                let removed = contexts.remove(&(context as usize));
                debug_assert!(
                    removed,
                    "ImGui context was not registered with this font atlas"
                );
                contexts.is_empty()
            } else {
                debug_assert!(false, "font atlas has no registered ImGui contexts");
                false
            };
        if remove_atlas_entry {
            states.contexts_by_atlas.remove(&atlas_key);
            let mode = states.renderer_modes.get(&atlas_key).copied();
            match mode {
                Some(FontAtlasRendererMode::Managed {
                    context: owner,
                    namespace,
                    renderer_reset_committed,
                }) => {
                    debug_assert_eq!(
                        owner, context as usize,
                        "the last Context did not own its font atlas renderer namespace"
                    );
                    if renderer_reset_committed {
                        states.renderer_modes.remove(&atlas_key);
                        if let Some(ledger) = states.texture_ledgers.get_mut(&atlas_key) {
                            ledger.entries.clear();
                        }
                    } else {
                        states.renderer_modes.insert(
                            atlas_key,
                            FontAtlasRendererMode::RendererReleasePending {
                                _retired_namespace: namespace,
                            },
                        );
                    }
                }
                Some(FontAtlasRendererMode::Legacy { .. }) | None => {
                    if let Some(ledger) = states.texture_ledgers.get_mut(&atlas_key) {
                        ledger.entries.clear();
                    }
                }
                Some(FontAtlasRendererMode::RendererReleasePending { .. }) => {
                    debug_assert!(
                        false,
                        "a renderer release cannot become pending while a Context remains registered"
                    );
                }
            }
        }
    });
}

pub(crate) fn assert_no_open_font_atlas_frames(raw: *mut sys::ImFontAtlas, caller: &str) {
    assert!(!raw.is_null(), "{caller} requires a valid font atlas");
    FONT_ATLAS_STATES.with(|states| {
        let states = states.borrow();
        let Some(contexts) = states.contexts_by_atlas.get(&(raw as usize)) else {
            return;
        };
        for &context in contexts {
            let context = context as *mut sys::ImGuiContext;
            assert!(
                !unsafe { (*context).WithinFrameScope },
                "{caller} cannot modify a font atlas while a registered ImGui context is mid-frame"
            );
        }
    });
}

pub(super) fn acquire_font_atlas_texture_borrow(raw: *mut sys::ImFontAtlas) -> u64 {
    assert!(!raw.is_null(), "font atlas pointer must not be null");
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let state = states.get_or_insert(raw);
        state.texture_borrows = state
            .texture_borrows
            .checked_add(1)
            .expect("font atlas texture borrow counter overflowed");
        state.stamp
    })
}

pub(super) fn release_font_atlas_texture_borrow(raw: *mut sys::ImFontAtlas, stamp: u64) {
    if raw.is_null() {
        return;
    }
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let Some(state) = states.by_atlas.get_mut(&(raw as usize)) else {
            debug_assert!(false, "font atlas texture borrow outlived its atlas state");
            return;
        };
        debug_assert_eq!(
            state.stamp, stamp,
            "font atlas texture borrow belongs to a reused atlas address"
        );
        if state.stamp == stamp {
            debug_assert!(state.texture_borrows > 0);
            state.texture_borrows = state.texture_borrows.saturating_sub(1);
        }
    });
}

pub(crate) fn assert_no_font_atlas_texture_borrows(raw: *mut sys::ImFontAtlas, caller: &str) {
    let state = font_atlas_state(raw);
    assert_eq!(
        state.texture_borrows, 0,
        "{caller} cannot invalidate a borrowed font atlas texture; drop the FontAtlasTexture view first"
    );
}

pub(crate) fn assert_font_atlas_renderer_mode(
    raw: *mut sys::ImFontAtlas,
    renderer_has_textures: bool,
    caller: &str,
) {
    assert!(!raw.is_null(), "{caller} requires a valid font atlas");
    let context = unsafe { sys::igGetCurrentContext() };
    assert!(
        !context.is_null(),
        "{caller} requires an active ImGui context"
    );
    FONT_ATLAS_STATES.with(|states| {
        let states = states.borrow();
        let mode = states.renderer_modes.get(&(raw as usize)).copied();
        if renderer_has_textures {
            match mode {
                Some(FontAtlasRendererMode::Managed { context: owner, .. }) => {
                    assert_eq!(
                        owner, context as usize,
                        "{caller} cannot use another Context's managed font atlas"
                    );
                }
                Some(FontAtlasRendererMode::Legacy { .. }) => {
                    panic!(
                        "{caller} cannot use a legacy font atlas through a managed renderer; clear and repopulate the atlas before creating a renderer consumer"
                    );
                }
                Some(FontAtlasRendererMode::RendererReleasePending { .. }) => {
                    panic!(
                        "{caller} cannot use a font atlas whose prior renderer release was not committed"
                    );
                }
                None => {
                    panic!(
                        "{caller} requires explicit managed font-atlas ownership; create a SynchronousRendererConsumer or DetachedRendererConsumer before starting the frame"
                    );
                }
            }
        } else {
            match mode {
                Some(FontAtlasRendererMode::Legacy { .. }) => {}
                Some(FontAtlasRendererMode::Managed { .. }) => {
                    panic!(
                        "{caller} cannot use a managed font atlas through a legacy renderer"
                    );
                }
                Some(FontAtlasRendererMode::RendererReleasePending { .. }) => {
                    panic!(
                        "{caller} cannot use a font atlas whose prior renderer release was not committed"
                    );
                }
                None => {
                    panic!(
                        "{caller} requires explicit legacy font-atlas ownership; acquire FontAtlas::try_claim_legacy_renderer() before starting the frame"
                    );
                }
            }
        }
    });
    unsafe {
        let builder = (*raw).Builder;
        if renderer_has_textures {
            assert!(
                builder.is_null() || !(*builder).PreloadedAllGlyphsRanges,
                "{caller} cannot switch a legacy-built font atlas to RENDERER_HAS_TEXTURES; clear and repopulate the atlas before changing renderer mode"
            );
        } else {
            assert!(
                (*raw).TexIsBuilt && !builder.is_null() && (*builder).PreloadedAllGlyphsRanges,
                "{caller} requires a font atlas built for legacy glyph preloading; acquire FontAtlas::try_claim_legacy_renderer() and call LegacyFontAtlas::build() after configuring the context"
            );
        }
    }
}

pub(crate) fn forget_font_atlas_generation(raw: *mut sys::ImFontAtlas) {
    if raw.is_null() {
        return;
    }
    FONT_ATLAS_STATES.with(|states| {
        let mut states = states.borrow_mut();
        states.by_atlas.remove(&(raw as usize));
        states.texture_ledgers.remove(&(raw as usize));
        states.renderer_modes.remove(&(raw as usize));
        states.contexts_by_atlas.remove(&(raw as usize));
        states.custom_rect_nonces.remove(&(raw as usize));
    });
}

pub(super) fn font_atlas_contains_font(
    atlas: *mut sys::ImFontAtlas,
    font: *mut sys::ImFont,
) -> bool {
    if atlas.is_null() || font.is_null() {
        return false;
    }
    unsafe {
        let fonts = &(*atlas).Fonts;
        if fonts.Size <= 0 || fonts.Data.is_null() {
            return false;
        }
        for index in 0..fonts.Size {
            if *fonts.Data.add(index as usize) == font {
                return true;
            }
        }
    }
    false
}

pub(super) fn current_context_font_atlas(caller: &str) -> *mut sys::ImFontAtlas {
    unsafe {
        let ctx = sys::igGetCurrentContext();
        assert!(!ctx.is_null(), "{caller} requires an active ImGui context");
        let io = sys::igGetIO_ContextPtr(ctx);
        assert!(!io.is_null(), "{caller} requires a valid ImGui IO");
        let atlas = (*io).Fonts;
        assert!(
            !atlas.is_null(),
            "{caller} requires the current ImGui context to have a font atlas"
        );
        atlas
    }
}
