//! Bevy image texture leases for the Bevy backend.
//!
//! A lease keeps the main-world registration alive while its owner can submit the corresponding
//! Dear ImGui texture reference. Lease destruction only records an event. The registry waits for
//! render-world extraction and cleanup confirmation before reusing the legacy texture ID.

#[cfg(feature = "render")]
mod render {
    use bevy_app::{App, PreUpdate};
    use bevy_asset::{AssetId, Handle};
    use bevy_ecs::prelude::{Res, ResMut, Resource};
    use bevy_image::Image;
    use dear_imgui_rs as imgui;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, MutexGuard};

    const BEVY_IMAGE_TEXTURE_NAMESPACE: u64 = 0x8000_0000_0000_0000;

    /// A cloneable lease for a Bevy image submitted to Dear ImGui.
    ///
    /// Prefer passing `&ImguiTexture` directly to `Ui::image` and draw-list image APIs. The
    /// resulting `TextureRef` borrows this lease, so normal UI construction cannot outlive it.
    /// `id` is available for APIs that require a legacy texture ID; retain the lease for as long
    /// as that raw ID can still appear in submitted ImGui draw data.
    #[derive(Debug)]
    #[must_use = "dropping an ImguiTexture immediately begins retirement once its final clone is gone"]
    pub struct ImguiTexture {
        identity: ImguiTextureLeaseIdentity,
        asset_id: AssetId<Image>,
        kind: ImguiTextureLeaseKind,
        events: ImguiTextureLeaseEvents,
        // Each strong lease retains its own Bevy asset handle. The registry also keeps one guard
        // until render-world extraction publishes the final strong submission.
        strong_asset: Option<Handle<Image>>,
    }

    impl Clone for ImguiTexture {
        fn clone(&self) -> Self {
            self.events.push(ImguiTextureLeaseEvent::Acquired {
                identity: self.identity,
                kind: self.kind,
            });
            Self {
                identity: self.identity,
                asset_id: self.asset_id,
                kind: self.kind,
                events: self.events.clone(),
                strong_asset: self.strong_asset.clone(),
            }
        }
    }

    impl Drop for ImguiTexture {
        fn drop(&mut self) {
            self.events.push(ImguiTextureLeaseEvent::Released {
                identity: self.identity,
                kind: self.kind,
            });
        }
    }

    impl ImguiTexture {
        fn new(
            identity: ImguiTextureLeaseIdentity,
            asset_id: AssetId<Image>,
            kind: ImguiTextureLeaseKind,
            events: ImguiTextureLeaseEvents,
            strong_asset: Option<Handle<Image>>,
        ) -> Self {
            Self {
                identity,
                asset_id,
                kind,
                events,
                strong_asset,
            }
        }

        /// Dear ImGui legacy texture ID associated with this lease.
        #[must_use]
        pub const fn id(&self) -> imgui::TextureId {
            self.identity.texture_id
        }

        /// Bevy image asset represented by this lease.
        #[must_use]
        pub const fn asset_id(&self) -> AssetId<Image> {
            self.asset_id
        }

        /// Whether this lease retains a strong Bevy image handle.
        #[must_use]
        pub const fn is_strong(&self) -> bool {
            matches!(self.kind, ImguiTextureLeaseKind::Strong)
        }

        /// Whether this lease adds no Bevy image asset retention.
        ///
        /// If the asset is unavailable to the renderer, ImGui uses its fallback texture until it
        /// becomes available again. A shared mapping can temporarily retain a guard until the final
        /// strong lease's submitted frame reaches render-world extraction.
        #[must_use]
        pub const fn is_weak(&self) -> bool {
            matches!(self.kind, ImguiTextureLeaseKind::Weak)
        }
    }

    impl From<&ImguiTexture> for imgui::TextureId {
        fn from(texture: &ImguiTexture) -> Self {
            texture.id()
        }
    }

    impl<'texture> From<&'texture ImguiTexture> for imgui::texture::TextureRef<'texture> {
        fn from(texture: &'texture ImguiTexture) -> Self {
            texture.id().into()
        }
    }

    /// Error returned when a strong texture lease cannot retain its Bevy image asset.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[non_exhaustive]
    pub enum ImguiTextureRegistrationError {
        /// Bevy UUID handles are stable identifiers, not retaining asset references.
        HandleDoesNotRetainAsset { asset_id: AssetId<Image> },
    }

    impl std::fmt::Display for ImguiTextureRegistrationError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::HandleDoesNotRetainAsset { asset_id } => write!(
                    formatter,
                    "cannot create a strong ImGui texture lease from non-retaining Bevy image handle {asset_id:?}"
                ),
            }
        }
    }

    impl std::error::Error for ImguiTextureRegistrationError {}

    /// Main-world registry for leased Bevy image textures.
    ///
    /// The registry is initialized by [`crate::ImguiPlugin`]. It is renderer-global: the same
    /// lease can be used from any owned Dear ImGui Context. Strong registration retains one
    /// slot-level Bevy handle until the final strong submission is extracted. Remaining weak
    /// leases keep the mapping live without retaining the asset afterward.
    #[derive(Resource, Debug)]
    pub struct ImguiBevyTextures {
        by_asset: HashMap<AssetId<Image>, ImguiTextureSlot>,
        by_texture: HashMap<imgui::TextureId, AssetId<Image>>,
        free_texture_ids: Vec<imgui::TextureId>,
        generations: HashMap<imgui::TextureId, u64>,
        next_texture_id: u64,
        events: ImguiTextureLeaseEvents,
    }

    impl Default for ImguiBevyTextures {
        fn default() -> Self {
            Self {
                by_asset: HashMap::new(),
                by_texture: HashMap::new(),
                free_texture_ids: Vec::new(),
                generations: HashMap::new(),
                next_texture_id: BEVY_IMAGE_TEXTURE_NAMESPACE,
                events: ImguiTextureLeaseEvents::default(),
            }
        }
    }

    impl ImguiBevyTextures {
        /// Register a Bevy image with a lease that retains its asset handle.
        ///
        /// Repeated registrations of the same image share one Dear ImGui texture ID while each
        /// returned lease independently keeps the image asset alive. The shared mapping retains
        /// one handle until the final strong submission is extracted or the full mapping retires.
        ///
        /// # Errors
        ///
        /// Returns [`ImguiTextureRegistrationError::HandleDoesNotRetainAsset`] when `image` is a
        /// Bevy UUID handle. UUID handles are stable identifiers but cannot keep an asset alive;
        /// use [`Self::register_weak`] for that explicit non-retaining behavior.
        pub fn register_strong(
            &mut self,
            image: Handle<Image>,
        ) -> Result<ImguiTexture, ImguiTextureRegistrationError> {
            let asset_id = image.id();
            if !image.is_strong() {
                return Err(ImguiTextureRegistrationError::HandleDoesNotRetainAsset { asset_id });
            }
            Ok(self.register(asset_id, ImguiTextureLeaseKind::Strong, Some(image)))
        }

        /// Register an externally owned Bevy image by asset ID.
        ///
        /// The lease does not introduce asset retention. If no strong lease remains and Bevy removes
        /// or has not yet uploaded the image, the renderer safely uses its fallback texture and
        /// resumes sampling if the image returns. A guard from the final strong lease is retained
        /// only until its already-submitted frame reaches render-world extraction.
        pub fn register_weak(&mut self, asset_id: AssetId<Image>) -> ImguiTexture {
            self.register(asset_id, ImguiTextureLeaseKind::Weak, None)
        }

        /// Number of live or retiring Bevy image texture registrations.
        #[must_use]
        pub fn len(&self) -> usize {
            self.by_asset.len()
        }

        /// Whether no live or retiring Bevy image texture registrations remain.
        #[must_use]
        pub fn is_empty(&self) -> bool {
            self.by_asset.is_empty()
        }

        fn register(
            &mut self,
            asset_id: AssetId<Image>,
            kind: ImguiTextureLeaseKind,
            strong_asset: Option<Handle<Image>>,
        ) -> ImguiTexture {
            if let Some(slot) = self.by_asset.get_mut(&asset_id) {
                slot.acquire(kind);
                if let Some(strong_asset) = strong_asset.as_ref() {
                    slot.retain_strong_asset(strong_asset);
                }
                return ImguiTexture::new(
                    slot.identity,
                    asset_id,
                    kind,
                    self.events.clone(),
                    strong_asset,
                );
            }

            let identity = self.allocate_identity();
            let mut slot = ImguiTextureSlot::new(identity);
            slot.acquire(kind);
            if let Some(strong_asset) = strong_asset.as_ref() {
                slot.retain_strong_asset(strong_asset);
            }
            self.by_texture.insert(identity.texture_id, asset_id);
            self.by_asset.insert(asset_id, slot);
            ImguiTexture::new(identity, asset_id, kind, self.events.clone(), strong_asset)
        }

        pub(crate) fn extract_for_render(&self) -> ImguiBevyTextureExtraction {
            let mut textures = Vec::with_capacity(self.by_asset.len());
            let mut retirements = Vec::new();

            for (asset_id, slot) in &self.by_asset {
                match slot.state {
                    ImguiTextureSlotState::Live => {
                        textures.push((slot.identity.texture_id, *asset_id));
                        match slot.asset_retention_state {
                            ImguiTextureAssetRetentionState::None
                            | ImguiTextureAssetRetentionState::Live => {}
                            ImguiTextureAssetRetentionState::AwaitingPublication(generation) => {
                                self.events
                                    .push(ImguiTextureLeaseEvent::AssetRetentionPublished {
                                        identity: slot.identity,
                                        generation,
                                    });
                            }
                        }
                    }
                    ImguiTextureSlotState::AwaitingPublication => {
                        textures.push((slot.identity.texture_id, *asset_id));
                        self.events
                            .push(ImguiTextureLeaseEvent::Published(slot.identity));
                    }
                    ImguiTextureSlotState::AwaitingAcknowledgement => {
                        retirements.push(slot.identity);
                    }
                }
            }

            textures.sort_by_key(|(texture_id, _)| texture_id.id());
            retirements.sort_by_key(|identity| identity.texture_id.id());
            ImguiBevyTextureExtraction {
                textures,
                retirements,
                events: Some(self.events.clone()),
            }
        }

        fn maintain(&mut self, render_integration_installed: bool) {
            for event in self.events.drain() {
                self.apply_event(event);
            }

            if !render_integration_installed {
                self.release_zero_strong_asset_guards();
                self.recycle_zero_lease_slots();
            }
        }

        fn apply_event(&mut self, event: ImguiTextureLeaseEvent) {
            match event {
                ImguiTextureLeaseEvent::Acquired { identity, kind } => {
                    if let Some(slot) = self.slot_mut(identity) {
                        slot.acquire(kind);
                    }
                }
                ImguiTextureLeaseEvent::Released { identity, kind } => {
                    if let Some(slot) = self.slot_mut(identity) {
                        slot.release(kind);
                    }
                }
                ImguiTextureLeaseEvent::Published(identity) => {
                    if let Some(slot) = self.slot_mut(identity)
                        && slot.total_leases() == 0
                        && matches!(slot.state, ImguiTextureSlotState::AwaitingPublication)
                    {
                        slot.state = ImguiTextureSlotState::AwaitingAcknowledgement;
                    }
                }
                ImguiTextureLeaseEvent::AssetRetentionPublished {
                    identity,
                    generation,
                } => {
                    if let Some(slot) = self.slot_mut(identity) {
                        slot.release_asset_retention_if_published(generation);
                    }
                }
                ImguiTextureLeaseEvent::Acknowledged(identity) => {
                    self.recycle_if_acknowledged(identity);
                }
            }
        }

        fn slot_mut(
            &mut self,
            identity: ImguiTextureLeaseIdentity,
        ) -> Option<&mut ImguiTextureSlot> {
            let asset_id = *self.by_texture.get(&identity.texture_id)?;
            let slot = self.by_asset.get_mut(&asset_id)?;
            (slot.identity == identity).then_some(slot)
        }

        fn recycle_if_acknowledged(&mut self, identity: ImguiTextureLeaseIdentity) {
            let Some(asset_id) = self.by_texture.get(&identity.texture_id).copied() else {
                return;
            };
            let Some(slot) = self.by_asset.get(&asset_id) else {
                return;
            };
            if slot.identity != identity
                || slot.total_leases() != 0
                || !matches!(slot.state, ImguiTextureSlotState::AwaitingAcknowledgement)
            {
                return;
            }

            self.remove_slot(asset_id, identity.texture_id);
        }

        fn release_zero_strong_asset_guards(&mut self) {
            for slot in self.by_asset.values_mut() {
                if slot.strong_leases == 0 {
                    slot.release_asset_retention_without_render();
                }
            }
        }

        fn recycle_zero_lease_slots(&mut self) {
            let stale_slots = self
                .by_asset
                .iter()
                .filter_map(|(asset_id, slot)| {
                    (slot.total_leases() == 0).then_some((*asset_id, slot.identity.texture_id))
                })
                .collect::<Vec<_>>();
            for (asset_id, texture_id) in stale_slots {
                self.remove_slot(asset_id, texture_id);
            }
        }

        fn remove_slot(&mut self, asset_id: AssetId<Image>, texture_id: imgui::TextureId) {
            let removed = self.by_asset.remove(&asset_id);
            debug_assert!(
                removed
                    .as_ref()
                    .is_some_and(|slot| slot.identity.texture_id == texture_id),
                "texture registry reverse mapping must remain synchronized"
            );
            self.by_texture.remove(&texture_id);
            self.free_texture_ids.push(texture_id);
        }

        fn allocate_identity(&mut self) -> ImguiTextureLeaseIdentity {
            if let Some(texture_id) = self.free_texture_ids.pop() {
                let generation = self.next_generation(texture_id);
                return ImguiTextureLeaseIdentity {
                    texture_id,
                    generation,
                };
            }

            if self.next_texture_id < BEVY_IMAGE_TEXTURE_NAMESPACE {
                self.next_texture_id = BEVY_IMAGE_TEXTURE_NAMESPACE;
            }

            loop {
                let texture_id = imgui::TextureId::new(self.next_texture_id);
                self.next_texture_id = self.next_texture_id.wrapping_add(1);
                if self.next_texture_id < BEVY_IMAGE_TEXTURE_NAMESPACE {
                    self.next_texture_id = BEVY_IMAGE_TEXTURE_NAMESPACE;
                }
                if !texture_id.is_null() && !self.by_texture.contains_key(&texture_id) {
                    let generation = self.next_generation(texture_id);
                    return ImguiTextureLeaseIdentity {
                        texture_id,
                        generation,
                    };
                }
            }
        }

        fn next_generation(&mut self, texture_id: imgui::TextureId) -> u64 {
            let generation = self.generations.entry(texture_id).or_insert(0);
            *generation = generation
                .checked_add(1)
                .expect("Bevy image texture generation space exhausted");
            *generation
        }
    }

    /// Install main-world texture lease maintenance.
    pub(crate) fn install_texture_leases(app: &mut App) {
        app.init_resource::<ImguiBevyTextures>()
            .add_systems(PreUpdate, maintain_imgui_texture_leases);
    }

    fn maintain_imgui_texture_leases(
        mut textures: ResMut<ImguiBevyTextures>,
        backend_runtime: Option<Res<crate::context::ownership::ImguiBackendRuntime>>,
    ) {
        let render_integration_installed =
            backend_runtime.is_some_and(|runtime| runtime.render_integration_installed());
        textures.maintain(render_integration_installed);
    }

    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    pub(crate) struct ImguiTextureLeaseIdentity {
        texture_id: imgui::TextureId,
        generation: u64,
    }

    impl ImguiTextureLeaseIdentity {
        #[must_use]
        pub(crate) const fn texture_id(self) -> imgui::TextureId {
            self.texture_id
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ImguiTextureLeaseKind {
        Strong,
        Weak,
    }

    #[derive(Clone, Copy, Debug)]
    enum ImguiTextureLeaseEvent {
        Acquired {
            identity: ImguiTextureLeaseIdentity,
            kind: ImguiTextureLeaseKind,
        },
        Released {
            identity: ImguiTextureLeaseIdentity,
            kind: ImguiTextureLeaseKind,
        },
        Published(ImguiTextureLeaseIdentity),
        AssetRetentionPublished {
            identity: ImguiTextureLeaseIdentity,
            generation: u64,
        },
        Acknowledged(ImguiTextureLeaseIdentity),
    }

    #[derive(Clone, Debug, Default)]
    pub(crate) struct ImguiTextureLeaseEvents {
        events: Arc<Mutex<Vec<ImguiTextureLeaseEvent>>>,
    }

    impl ImguiTextureLeaseEvents {
        fn state(&self) -> MutexGuard<'_, Vec<ImguiTextureLeaseEvent>> {
            self.events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        }

        fn push(&self, event: ImguiTextureLeaseEvent) {
            self.state().push(event);
        }

        fn drain(&self) -> Vec<ImguiTextureLeaseEvent> {
            std::mem::take(&mut *self.state())
        }

        pub(crate) fn acknowledge(&self, identity: ImguiTextureLeaseIdentity) {
            self.push(ImguiTextureLeaseEvent::Acknowledged(identity));
        }
    }

    #[derive(Debug)]
    struct ImguiTextureSlot {
        identity: ImguiTextureLeaseIdentity,
        strong_leases: usize,
        weak_leases: usize,
        // Retain one actual Bevy handle until the final strong submission or the complete mapping
        // retirement has received a render-world acknowledgement.
        retained_asset: Option<Handle<Image>>,
        asset_retention_state: ImguiTextureAssetRetentionState,
        next_asset_retention_generation: u64,
        state: ImguiTextureSlotState,
    }

    impl ImguiTextureSlot {
        fn new(identity: ImguiTextureLeaseIdentity) -> Self {
            Self {
                identity,
                strong_leases: 0,
                weak_leases: 0,
                retained_asset: None,
                asset_retention_state: ImguiTextureAssetRetentionState::None,
                next_asset_retention_generation: 0,
                state: ImguiTextureSlotState::Live,
            }
        }

        fn retain_strong_asset(&mut self, asset: &Handle<Image>) {
            if self.retained_asset.is_none() {
                self.retained_asset = Some(asset.clone());
            }
            self.asset_retention_state = ImguiTextureAssetRetentionState::Live;
        }

        fn acquire(&mut self, kind: ImguiTextureLeaseKind) {
            let leases = match kind {
                ImguiTextureLeaseKind::Strong => &mut self.strong_leases,
                ImguiTextureLeaseKind::Weak => &mut self.weak_leases,
            };
            *leases = leases
                .checked_add(1)
                .expect("Bevy image texture lease count exhausted");
            if matches!(kind, ImguiTextureLeaseKind::Strong) && self.retained_asset.is_some() {
                self.asset_retention_state = ImguiTextureAssetRetentionState::Live;
            }
            self.state = ImguiTextureSlotState::Live;
        }

        fn release(&mut self, kind: ImguiTextureLeaseKind) {
            let leases = match kind {
                ImguiTextureLeaseKind::Strong => &mut self.strong_leases,
                ImguiTextureLeaseKind::Weak => &mut self.weak_leases,
            };
            debug_assert!(
                *leases > 0,
                "texture lease release must match one acquisition"
            );
            let Some(next) = leases.checked_sub(1) else {
                return;
            };
            *leases = next;
            if matches!(kind, ImguiTextureLeaseKind::Strong) && self.strong_leases == 0 {
                self.begin_asset_retention();
            }
            if self.total_leases() == 0 {
                self.state = ImguiTextureSlotState::AwaitingPublication;
            }
        }

        fn begin_asset_retention(&mut self) {
            debug_assert!(
                self.retained_asset.is_some(),
                "the final strong lease must leave one registry asset guard"
            );
            if self.retained_asset.is_none() {
                self.asset_retention_state = ImguiTextureAssetRetentionState::None;
                return;
            }

            self.next_asset_retention_generation = self
                .next_asset_retention_generation
                .checked_add(1)
                .expect("Bevy image asset retention generation space exhausted");
            self.asset_retention_state = ImguiTextureAssetRetentionState::AwaitingPublication(
                self.next_asset_retention_generation,
            );
        }

        fn release_asset_retention_if_published(&mut self, generation: u64) {
            if self.strong_leases == 0
                && matches!(
                    self.asset_retention_state,
                    ImguiTextureAssetRetentionState::AwaitingPublication(current)
                        if current == generation
                )
            {
                self.retained_asset = None;
                self.asset_retention_state = ImguiTextureAssetRetentionState::None;
            }
        }

        fn release_asset_retention_without_render(&mut self) {
            debug_assert_eq!(self.strong_leases, 0);
            self.retained_asset = None;
            self.asset_retention_state = ImguiTextureAssetRetentionState::None;
        }

        fn total_leases(&self) -> usize {
            self.strong_leases
                .checked_add(self.weak_leases)
                .expect("Bevy image texture lease count exhausted")
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ImguiTextureSlotState {
        Live,
        AwaitingPublication,
        AwaitingAcknowledgement,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ImguiTextureAssetRetentionState {
        None,
        Live,
        AwaitingPublication(u64),
    }

    #[derive(Clone, Debug, Default)]
    pub(crate) struct ImguiBevyTextureExtraction {
        pub(crate) textures: Vec<(imgui::TextureId, AssetId<Image>)>,
        pub(crate) retirements: Vec<ImguiTextureLeaseIdentity>,
        pub(crate) events: Option<ImguiTextureLeaseEvents>,
    }

    impl ImguiBevyTextureExtraction {
        #[cfg(test)]
        pub(crate) fn from_textures(textures: Vec<(imgui::TextureId, AssetId<Image>)>) -> Self {
            Self {
                textures,
                ..Self::default()
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use bevy_asset::Assets;

        #[test]
        fn leases_share_an_id_and_last_drop_waits_for_render_acknowledgement() {
            let asset_id = AssetId::<Image>::default();
            let mut textures = ImguiBevyTextures::default();
            let first = textures.register_weak(asset_id);
            let second = first.clone();
            let identity = first.identity;

            assert_eq!(first.id(), second.id());
            assert_eq!(textures.len(), 1);

            drop(first);
            textures.maintain(true);
            assert_eq!(textures.extract_for_render().textures.len(), 1);

            drop(second);
            textures.maintain(true);
            let published = textures.extract_for_render();
            assert_eq!(published.textures.len(), 1);

            textures.maintain(true);
            let retiring = textures.extract_for_render();
            assert!(retiring.textures.is_empty());
            assert_eq!(retiring.retirements, vec![identity]);

            retiring
                .events
                .as_ref()
                .expect("main-world extraction must retain the acknowledgement queue")
                .acknowledge(identity);
            textures.maintain(true);
            assert!(textures.is_empty());
        }

        #[test]
        fn stale_generation_acknowledgements_cannot_reclaim_a_reused_slot() {
            let first_asset = AssetId::<Image>::default();
            let second_asset = AssetId::<Image>::invalid();
            let mut textures = ImguiBevyTextures::default();
            let first = textures.register_weak(first_asset);
            let stale_identity = first.identity;

            drop(first);
            textures.maintain(false);
            assert!(textures.is_empty());

            let replacement = textures.register_weak(second_asset);
            assert_eq!(replacement.id(), stale_identity.texture_id);
            assert_ne!(replacement.identity, stale_identity);

            textures.events.acknowledge(stale_identity);
            textures.maintain(true);
            assert_eq!(textures.len(), 1);
            assert_eq!(replacement.asset_id(), second_asset);
        }

        #[test]
        fn stale_acknowledgements_cannot_reclaim_a_reactivated_registration() {
            let asset_id = AssetId::<Image>::default();
            let mut textures = ImguiBevyTextures::default();
            let original = textures.register_weak(asset_id);
            let identity = original.identity;

            drop(original);
            textures.maintain(true);
            let publication = textures.extract_for_render();
            textures.maintain(true);

            let reactivated = textures.register_weak(asset_id);
            assert_eq!(reactivated.identity, identity);

            publication
                .events
                .as_ref()
                .expect("main-world extraction must retain the acknowledgement queue")
                .acknowledge(identity);
            textures.maintain(true);

            assert_eq!(textures.len(), 1);
            assert_eq!(reactivated.asset_id(), asset_id);
            assert_eq!(
                textures.extract_for_render().textures,
                vec![(reactivated.id(), asset_id)]
            );
        }

        #[test]
        fn strong_lease_retains_the_handle_and_weak_lease_does_not() {
            let mut textures = ImguiBevyTextures::default();
            let mut images = Assets::<Image>::default();
            let strong = textures
                .register_strong(images.add(Image::default()))
                .expect("Assets::add should return a retaining Bevy handle");
            let weak = textures.register_weak(AssetId::invalid());

            assert!(strong.is_strong());
            assert!(strong.strong_asset.is_some());
            assert!(weak.is_weak());
            assert!(weak.strong_asset.is_none());
            assert!(
                textures
                    .by_asset
                    .get(&strong.asset_id())
                    .is_some_and(|slot| slot.retained_asset.is_some())
            );
            assert!(
                textures
                    .by_asset
                    .get(&weak.asset_id())
                    .is_some_and(|slot| slot.retained_asset.is_none())
            );
        }

        #[test]
        fn strong_lease_keeps_its_asset_until_render_retirement_is_acknowledged() {
            let mut textures = ImguiBevyTextures::default();
            let mut images = Assets::<Image>::default();
            let handle = images.add(Image::default());
            let asset_id = handle.id();
            let lease = textures
                .register_strong(handle)
                .expect("Assets::add should return a retaining Bevy handle");
            let identity = lease.identity;

            drop(lease);
            textures.maintain(true);
            let publication = textures.extract_for_render();
            assert_eq!(publication.textures, vec![(identity.texture_id, asset_id)]);
            assert!(
                textures
                    .by_asset
                    .get(&asset_id)
                    .is_some_and(|slot| slot.retained_asset.is_some()),
                "the slot must retain the Bevy image while the publication frame is in flight"
            );

            textures.maintain(true);
            let retirement = textures.extract_for_render();
            assert_eq!(retirement.retirements, vec![identity]);
            assert!(
                textures
                    .by_asset
                    .get(&asset_id)
                    .is_some_and(|slot| slot.retained_asset.is_some()),
                "the slot must retain the Bevy image until the renderer confirms cleanup"
            );

            retirement
                .events
                .as_ref()
                .expect("retirement extraction must retain the acknowledgement queue")
                .acknowledge(identity);
            textures.maintain(true);
            assert!(textures.is_empty());
        }

        #[test]
        fn final_strong_guard_retires_while_shared_weak_mapping_stays_live() {
            let mut textures = ImguiBevyTextures::default();
            let mut images = Assets::<Image>::default();
            let handle = images.add(Image::default());
            let asset_id = handle.id();
            let strong = textures
                .register_strong(handle)
                .expect("Assets::add should return a retaining Bevy handle");
            let weak = textures.register_weak(asset_id);
            let identity = strong.identity;

            assert!(weak.strong_asset.is_none());
            drop(strong);
            textures.maintain(true);
            let publication = textures.extract_for_render();
            assert_eq!(publication.textures, vec![(identity.texture_id, asset_id)]);
            assert!(publication.retirements.is_empty());
            let slot = textures
                .by_asset
                .get(&asset_id)
                .expect("the weak lease should keep the shared slot live");
            assert_eq!(slot.strong_leases, 0);
            assert_eq!(slot.weak_leases, 1);
            assert!(
                slot.retained_asset.is_some(),
                "a pre-existing strong registration must guard mixed-lease snapshots"
            );

            textures.maintain(true);
            let slot = textures
                .by_asset
                .get(&asset_id)
                .expect("the weak lease should keep the shared slot live");
            assert_eq!(slot.strong_leases, 0);
            assert_eq!(slot.weak_leases, 1);
            assert!(
                slot.retained_asset.is_none(),
                "the asset guard must release after its publication reaches the render world"
            );
            assert_eq!(
                slot.asset_retention_state,
                ImguiTextureAssetRetentionState::None
            );
            assert_eq!(slot.state, ImguiTextureSlotState::Live);
            let weak_extraction = textures.extract_for_render();
            assert_eq!(
                weak_extraction.textures,
                vec![(identity.texture_id, asset_id)]
            );
            assert!(weak_extraction.retirements.is_empty());

            drop(weak);
            textures.maintain(true);
            let mapping_publication = textures.extract_for_render();
            assert_eq!(
                mapping_publication.textures,
                vec![(identity.texture_id, asset_id)]
            );
            textures.maintain(true);
            let retirement = textures.extract_for_render();
            assert_eq!(retirement.retirements, vec![identity]);

            retirement
                .events
                .as_ref()
                .expect("mapping retirement extraction must retain the acknowledgement queue")
                .acknowledge(identity);
            textures.maintain(true);
            assert!(textures.is_empty());
        }

        #[test]
        fn stale_asset_retention_publication_cannot_release_a_new_strong_guard() {
            let mut textures = ImguiBevyTextures::default();
            let mut images = Assets::<Image>::default();
            let handle = images.add(Image::default());
            let asset_id = handle.id();
            let first_strong = textures
                .register_strong(handle.clone())
                .expect("Assets::add should return a retaining Bevy handle");
            let weak = textures.register_weak(asset_id);
            let identity = first_strong.identity;

            drop(first_strong);
            textures.maintain(true);
            let _ = textures.extract_for_render();
            let stale_generation = match textures
                .by_asset
                .get(&asset_id)
                .expect("the weak lease should keep the slot live")
                .asset_retention_state
            {
                ImguiTextureAssetRetentionState::AwaitingPublication(generation) => generation,
                state => panic!("expected pending asset publication, got {state:?}"),
            };

            let reactivated = textures
                .register_strong(handle)
                .expect("the retaining handle should reactivate strong ownership");
            textures.maintain(true);
            let slot = textures
                .by_asset
                .get(&asset_id)
                .expect("the reactivated strong lease should keep the slot live");
            assert_eq!(slot.strong_leases, 1);
            assert!(slot.retained_asset.is_some());
            assert_eq!(
                slot.asset_retention_state,
                ImguiTextureAssetRetentionState::Live
            );

            drop(reactivated);
            textures.maintain(true);
            let current_generation = match textures
                .by_asset
                .get(&asset_id)
                .expect("the weak lease should keep the slot live")
                .asset_retention_state
            {
                ImguiTextureAssetRetentionState::AwaitingPublication(generation) => generation,
                state => panic!("expected renewed asset publication, got {state:?}"),
            };
            assert_ne!(current_generation, stale_generation);

            textures
                .events
                .push(ImguiTextureLeaseEvent::AssetRetentionPublished {
                    identity,
                    generation: stale_generation,
                });
            textures.maintain(true);
            let slot = textures
                .by_asset
                .get(&asset_id)
                .expect("the weak lease should keep the slot live");
            assert!(slot.retained_asset.is_some());
            assert_eq!(
                slot.asset_retention_state,
                ImguiTextureAssetRetentionState::AwaitingPublication(current_generation)
            );

            let current_publication = textures.extract_for_render();
            assert_eq!(
                current_publication.textures,
                vec![(identity.texture_id, asset_id)]
            );
            assert!(current_publication.retirements.is_empty());
            textures.maintain(true);
            assert!(
                textures
                    .by_asset
                    .get(&asset_id)
                    .is_some_and(|slot| slot.retained_asset.is_none())
            );

            drop(weak);
        }

        #[test]
        fn no_render_integration_releases_the_final_strong_guard_immediately() {
            let mut textures = ImguiBevyTextures::default();
            let mut images = Assets::<Image>::default();
            let handle = images.add(Image::default());
            let asset_id = handle.id();
            let strong = textures
                .register_strong(handle)
                .expect("Assets::add should return a retaining Bevy handle");
            let weak = textures.register_weak(asset_id);

            drop(strong);
            textures.maintain(false);
            let slot = textures
                .by_asset
                .get(&asset_id)
                .expect("the weak lease should keep the mapping live");
            assert_eq!(slot.weak_leases, 1);
            assert!(slot.retained_asset.is_none());
            assert_eq!(
                slot.asset_retention_state,
                ImguiTextureAssetRetentionState::None
            );

            drop(weak);
        }

        #[test]
        fn strong_registration_rejects_non_retaining_uuid_handles() {
            let mut textures = ImguiBevyTextures::default();
            let asset_id = Handle::<Image>::default().id();

            let error = textures
                .register_strong(Handle::default())
                .expect_err("UUID handles cannot satisfy a strong lease");

            assert_eq!(
                error,
                ImguiTextureRegistrationError::HandleDoesNotRetainAsset { asset_id }
            );
        }
    }
}

#[cfg(feature = "render")]
pub(crate) use render::{
    ImguiBevyTextureExtraction, ImguiTextureLeaseEvents, ImguiTextureLeaseIdentity,
    install_texture_leases,
};
#[cfg(feature = "render")]
pub use render::{ImguiBevyTextures, ImguiTexture, ImguiTextureRegistrationError};
