use std::any::{TypeId, type_name};
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, ThreadId};

use bevy_app::App;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::{
    InternedSystemSet, IntoScheduleConfigs, IntoSystemSet, Schedule, ScheduleConfigs,
    ScheduleLabel, SingleThreadedExecutor, SystemCondition, SystemSet,
};
use bevy_ecs::system::{
    Adapt, AdapterSystem, FunctionSystem, IntoSystem, NonSendMarker, PipeSystem, RunSystemError,
    ScheduleSystem, System, SystemIn, SystemInput, SystemParamValidationError,
};
use bevy_ecs::world::World;
use dear_imgui_rs::{ContextId, Ui};

use super::lifecycle::ImguiAppLifecycle;

static NEXT_REGISTRY_ID: AtomicU64 = AtomicU64::new(1);

/// Type marker for the primary Dear ImGui pass.
pub enum ImguiPrimaryPass {}

/// Failure to create, query, or configure a private Dear ImGui pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ImguiPassError {
    /// Explicit App shutdown permanently closed the integration.
    AppTerminated,
    /// The App previously owned a pass registry that application code removed.
    PassRegistryMissing,
    /// Process-wide pass registry identities were exhausted.
    RegistryIdExhausted,
    /// This App exhausted its private pass identities.
    PassIdExhausted,
    /// The pass handle belongs to another Bevy App.
    ForeignApp {
        /// Rust brand of the rejected handle.
        pass: &'static str,
    },
    /// The handle's Rust type no longer matches its private runtime identity.
    InvalidBrand {
        /// Rust brand of the rejected handle.
        pass: &'static str,
    },
    /// The pass identity is not registered in this App.
    UnknownPass {
        /// Rust brand of the rejected handle.
        pass: &'static str,
    },
    /// A bound system belongs to another runtime pass of the same Rust brand.
    SystemPassMismatch {
        /// Rust brand shared by both runtime passes.
        pass: &'static str,
        /// App-local identity of the target runtime pass.
        expected_runtime: u64,
        /// App-local identity carried by the rejected system configuration.
        actual_runtime: u64,
    },
}

impl fmt::Display for ImguiPassError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AppTerminated => formatter
                .write_str("the Dear ImGui integration is terminal after explicit App shutdown"),
            Self::PassRegistryMissing => formatter.write_str(
                "the App's private Dear ImGui pass registry was removed after it was claimed",
            ),
            Self::RegistryIdExhausted => {
                formatter.write_str("Dear ImGui pass registry identities are exhausted")
            }
            Self::PassIdExhausted => {
                formatter.write_str("Dear ImGui pass identities are exhausted for this App")
            }
            Self::ForeignApp { pass } => {
                write!(formatter, "Dear ImGui pass '{pass}' belongs to another App")
            }
            Self::InvalidBrand { pass } => {
                write!(
                    formatter,
                    "Dear ImGui pass '{pass}' has an invalid Rust brand"
                )
            }
            Self::UnknownPass { pass } => {
                write!(
                    formatter,
                    "Dear ImGui pass '{pass}' is not registered in this App"
                )
            }
            Self::SystemPassMismatch {
                pass,
                expected_runtime,
                actual_runtime,
            } => write!(
                formatter,
                "Dear ImGui system bound to runtime pass {actual_runtime} ('{pass}') cannot be registered in runtime pass {expected_runtime}"
            ),
        }
    }
}

impl std::error::Error for ImguiPassError {}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PassKey(u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PassIdentity {
    registry_id: u64,
    key: PassKey,
    brand: TypeId,
    brand_name: &'static str,
}

impl PassIdentity {
    pub(crate) const fn registry_id(self) -> u64 {
        self.registry_id
    }

    pub(crate) const fn key(self) -> PassKey {
        self.key
    }

    pub(crate) const fn brand_name(self) -> &'static str {
        self.brand_name
    }
}

/// An application-owned handle to one private Dear ImGui pass.
///
/// Handles can only be obtained from [`crate::ImguiAppExt::declare_imgui_pass`] or
/// [`crate::ImguiAppExt::imgui_primary_pass`]. They deliberately do not implement
/// [`ScheduleLabel`], so a pass cannot be run through `World` or `Commands`.
#[derive(Resource)]
pub struct ImguiPass<P: 'static> {
    identity: PassIdentity,
    active: Arc<ActiveFrameControl>,
    _brand: PhantomData<fn() -> P>,
}

impl<P: 'static> ImguiPass<P> {
    fn from_identity(identity: PassIdentity, active: Arc<ActiveFrameControl>) -> Self {
        Self {
            identity,
            active,
            _brand: PhantomData,
        }
    }

    pub(crate) const fn identity(&self) -> PassIdentity {
        self.identity
    }

    /// Bind a frame-input system to this exact private pass.
    ///
    /// Pass the returned configuration to [`crate::ImguiAppExt::add_imgui_systems`]. The sealed
    /// [`IntoImguiSystemConfigs`] trait provides the usual Bevy ordering, set, condition, and
    /// chaining modifiers without allowing the adapted system to enter an ordinary Bevy schedule.
    #[must_use]
    pub fn system<S, M>(&self, system: S) -> ImguiSystemConfigs<P>
    where
        S: IntoSystem<ImguiFrame<'static, P>, (), M> + 'static,
        M: 'static,
    {
        let system = IntoSystem::into_system(system);
        let name = system.name();
        let adapted = AdapterSystem::new(
            ImguiFrameAdapter {
                pass: self.identity,
                active: Arc::clone(&self.active),
                _brand: PhantomData,
            },
            system,
            name.clone(),
        );
        let main_thread: MainThreadGateSystem =
            IntoSystem::into_system(require_main_thread as fn(NonSendMarker));
        let configs = PipeSystem::new(main_thread, adapted, name).into_configs();
        ImguiSystemConfigs {
            identities: vec![self.identity],
            configs,
            _brand: PhantomData,
        }
    }
}

impl<P: 'static> Clone for ImguiPass<P> {
    fn clone(&self) -> Self {
        Self {
            identity: self.identity,
            active: Arc::clone(&self.active),
            _brand: PhantomData,
        }
    }
}

impl<P: 'static> fmt::Debug for ImguiPass<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImguiPass")
            .field("brand", &self.identity.brand_name)
            .finish_non_exhaustive()
    }
}

/// One or more Dear ImGui systems sealed to a private pass.
///
/// This type deliberately does not implement Bevy's [`IntoScheduleConfigs`]. It can only be
/// consumed by [`crate::ImguiAppExt::add_imgui_systems`], which validates its exact App and pass
/// identity before modifying the private runner.
///
/// ```compile_fail
/// use bevy_app::App;
/// use dear_imgui_bevy::ImguiAppExt;
///
/// fn ordinary_bevy_system() {}
///
/// let mut app = App::new();
/// let pass = app.imgui_primary_pass().unwrap();
/// app.add_imgui_systems(&pass, ordinary_bevy_system);
/// ```
pub struct ImguiSystemConfigs<P: 'static> {
    identities: Vec<PassIdentity>,
    configs: ScheduleConfigs<ScheduleSystem>,
    _brand: PhantomData<fn() -> P>,
}

type MainThreadGateSystem = FunctionSystem<fn(NonSendMarker), (), (), fn(NonSendMarker)>;

impl<P: 'static> ImguiSystemConfigs<P> {
    fn map_configs(
        mut self,
        map: impl FnOnce(ScheduleConfigs<ScheduleSystem>) -> ScheduleConfigs<ScheduleSystem>,
    ) -> Self {
        self.configs = map(self.configs);
        self
    }
}

mod sealed {
    pub trait Sealed<P: 'static> {}
}

/// Convert pass-bound systems into one private-runner configuration.
///
/// The trait is sealed so applications cannot forge pass ownership metadata. It intentionally
/// mirrors the common Bevy configuration modifiers while keeping the resulting system out of
/// public schedules.
pub trait IntoImguiSystemConfigs<P: 'static>: sealed::Sealed<P> + Sized {
    #[doc(hidden)]
    fn into_imgui_system_configs(self) -> ImguiSystemConfigs<P>;

    /// Add these systems to `set` inside the private pass runner.
    fn in_set(self, set: impl SystemSet) -> ImguiSystemConfigs<P> {
        self.into_imgui_system_configs()
            .map_configs(|configs| configs.in_set(set))
    }

    /// Run these systems before every system in `set`.
    fn before<M>(self, set: impl IntoSystemSet<M>) -> ImguiSystemConfigs<P> {
        self.into_imgui_system_configs()
            .map_configs(|configs| configs.before(set))
    }

    /// Run these systems after every system in `set`.
    fn after<M>(self, set: impl IntoSystemSet<M>) -> ImguiSystemConfigs<P> {
        self.into_imgui_system_configs()
            .map_configs(|configs| configs.after(set))
    }

    /// Run before `set` without inserting deferred-command barriers.
    fn before_ignore_deferred<M>(self, set: impl IntoSystemSet<M>) -> ImguiSystemConfigs<P> {
        self.into_imgui_system_configs()
            .map_configs(|configs| configs.before_ignore_deferred(set))
    }

    /// Run after `set` without inserting deferred-command barriers.
    fn after_ignore_deferred<M>(self, set: impl IntoSystemSet<M>) -> ImguiSystemConfigs<P> {
        self.into_imgui_system_configs()
            .map_configs(|configs| configs.after_ignore_deferred(set))
    }

    /// Evaluate a cloned condition independently for every contained system.
    fn distributive_run_if<M>(
        self,
        condition: impl SystemCondition<M> + Clone,
    ) -> ImguiSystemConfigs<P> {
        self.into_imgui_system_configs()
            .map_configs(|configs| configs.distributive_run_if(condition))
    }

    /// Evaluate one condition for this complete configuration.
    fn run_if<M>(self, condition: impl SystemCondition<M>) -> ImguiSystemConfigs<P> {
        self.into_imgui_system_configs()
            .map_configs(|configs| configs.run_if(condition))
    }

    /// Suppress ambiguity diagnostics against `set`.
    fn ambiguous_with<M>(self, set: impl IntoSystemSet<M>) -> ImguiSystemConfigs<P> {
        self.into_imgui_system_configs()
            .map_configs(|configs| configs.ambiguous_with(set))
    }

    /// Suppress every ambiguity diagnostic for these systems.
    fn ambiguous_with_all(self) -> ImguiSystemConfigs<P> {
        self.into_imgui_system_configs()
            .map_configs(ScheduleConfigs::ambiguous_with_all)
    }

    /// Chain each successive configuration with normal deferred-command barriers.
    fn chain(self) -> ImguiSystemConfigs<P> {
        self.into_imgui_system_configs()
            .map_configs(ScheduleConfigs::chain)
    }

    /// Chain each successive configuration without deferred-command barriers.
    fn chain_ignore_deferred(self) -> ImguiSystemConfigs<P> {
        self.into_imgui_system_configs()
            .map_configs(ScheduleConfigs::chain_ignore_deferred)
    }
}

impl<P: 'static> sealed::Sealed<P> for ImguiSystemConfigs<P> {}

impl<P: 'static> IntoImguiSystemConfigs<P> for ImguiSystemConfigs<P> {
    fn into_imgui_system_configs(self) -> ImguiSystemConfigs<P> {
        self
    }
}

macro_rules! impl_imgui_system_config_tuple {
    ($($name:ident),+ $(,)?) => {
        impl<P: 'static, $($name),+> sealed::Sealed<P> for ($($name,)+)
        where
            $($name: IntoImguiSystemConfigs<P>,)+
        {}

        impl<P: 'static, $($name),+> IntoImguiSystemConfigs<P> for ($($name,)+)
        where
            $($name: IntoImguiSystemConfigs<P>,)+
        {
            #[allow(non_snake_case)]
            fn into_imgui_system_configs(self) -> ImguiSystemConfigs<P> {
                let ($($name,)+) = self;
                let mut identities = Vec::new();
                $(
                    let config = $name.into_imgui_system_configs();
                    identities.extend(config.identities.iter().copied());
                    let $name = config.configs;
                )+
                ImguiSystemConfigs {
                    identities,
                    configs: ($($name,)+).into_configs(),
                    _brand: PhantomData,
                }
            }
        }
    };
}

impl_imgui_system_config_tuple!(A);
impl_imgui_system_config_tuple!(A, B);
impl_imgui_system_config_tuple!(A, B, C);
impl_imgui_system_config_tuple!(A, B, C, D);
impl_imgui_system_config_tuple!(A, B, C, D, E);
impl_imgui_system_config_tuple!(A, B, C, D, E, F);
impl_imgui_system_config_tuple!(A, B, C, D, E, F, G);
impl_imgui_system_config_tuple!(A, B, C, D, E, F, G, H);
impl_imgui_system_config_tuple!(A, B, C, D, E, F, G, H, I);
impl_imgui_system_config_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_imgui_system_config_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_imgui_system_config_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_imgui_system_config_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_imgui_system_config_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_imgui_system_config_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_imgui_system_config_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, Q);
impl_imgui_system_config_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, Q, R);
impl_imgui_system_config_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, Q, R, T);
impl_imgui_system_config_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, Q, R, T, U);
impl_imgui_system_config_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, Q, R, T, U, V);

/// Internal unit-input adapter that injects the frame owned by its private pass runner.
struct ImguiFrameAdapter<P: 'static> {
    pass: PassIdentity,
    active: Arc<ActiveFrameControl>,
    _brand: PhantomData<fn() -> P>,
}

impl<P, S> Adapt<S> for ImguiFrameAdapter<P>
where
    P: 'static,
    S: System<In = ImguiFrame<'static, P>, Out = ()>,
{
    type In = ();
    type Out = ();

    fn adapt(
        &mut self,
        (): <Self::In as SystemInput>::Inner<'_>,
        run_system: impl FnOnce(SystemIn<'_, S>) -> Result<S::Out, RunSystemError>,
    ) -> Result<Self::Out, RunSystemError> {
        let frame = self.active.snapshot::<P>(self.pass)?;
        let ui_pointer = frame.ui_address as *const Ui;
        // SAFETY: snapshot verified this pass and its owning thread. The private driver retains the
        // active Context and frame guard until this single-threaded schedule returns, and the
        // borrow is consumed by one synchronous invocation.
        let ui = unsafe { &*ui_pointer };
        run_system(ImguiFrame {
            ui,
            context_id: frame.context_id,
            frame_index: frame.frame_index,
            _brand: PhantomData,
            _main_thread: PhantomData,
        })
    }
}

fn require_main_thread(_marker: NonSendMarker) {}

/// Frame-scoped Dear ImGui access injected by a private pass runner.
///
/// `ImguiFrame` is a Bevy [`SystemInput`], not a `SystemParam`. A function that accepts it cannot
/// be added to an ordinary Bevy schedule. Bind it through [`ImguiPass::system`] and register the
/// resulting unit-input system through [`crate::ImguiAppExt::add_imgui_systems`].
///
/// ```compile_fail
/// use bevy_app::{App, Update};
/// use dear_imgui_bevy::ImguiFrame;
///
/// fn draw(_frame: ImguiFrame<'_>) {}
///
/// App::new().add_systems(Update, draw);
/// ```
///
/// ```compile_fail
/// use bevy_ecs::schedule::Schedule;
/// use dear_imgui_bevy::ImguiFrame;
///
/// fn draw(_frame: ImguiFrame<'_>) {}
///
/// Schedule::default().add_systems(draw);
/// ```
///
/// ```compile_fail
/// use bevy_app::App;
/// use dear_imgui_bevy::{ImguiAppExt, ImguiFrame};
///
/// struct PassA;
/// struct PassB;
/// fn draw_b(_frame: ImguiFrame<'_, PassB>) {}
///
/// let mut app = App::new();
/// let pass_a = app.declare_imgui_pass::<PassA>().unwrap();
/// app.add_imgui_systems(&pass_a, pass_a.system(draw_b));
/// ```
///
/// ```compile_fail
/// use bevy_app::{App, Update};
/// use dear_imgui_bevy::{ImguiAppExt, ImguiFrame};
///
/// fn draw(_frame: ImguiFrame<'_>) {}
///
/// let mut app = App::new();
/// let pass = app.imgui_primary_pass().unwrap();
/// app.add_systems(Update, pass.system(draw));
/// ```
pub struct ImguiFrame<'frame, P: 'static = ImguiPrimaryPass> {
    ui: &'frame Ui,
    context_id: ContextId,
    frame_index: u64,
    _brand: PhantomData<fn() -> P>,
    _main_thread: PhantomData<Rc<()>>,
}

impl<P: 'static> SystemInput for ImguiFrame<'_, P> {
    type Param<'input> = ImguiFrame<'input, P>;
    type Inner<'input> = ImguiFrame<'input, P>;

    fn wrap(this: Self::Inner<'_>) -> Self::Param<'_> {
        this
    }
}

impl<P: 'static> ImguiFrame<'_, P> {
    /// Borrow the live Dear ImGui UI for this pass invocation.
    #[must_use]
    pub const fn ui(&self) -> &Ui {
        self.ui
    }

    /// Return the Context identity bound to this pass invocation.
    #[must_use]
    pub const fn context_id(&self) -> ContextId {
        self.context_id
    }

    /// Return the frame index local to this Context.
    #[must_use]
    pub const fn frame_index(&self) -> u64 {
        self.frame_index
    }
}

#[derive(Clone, Copy)]
struct ActiveFrame {
    pass: PassIdentity,
    context_id: ContextId,
    frame_index: u64,
    ui_address: usize,
}

struct ActiveFrameControl {
    owner_thread: ThreadId,
    frame: Mutex<Option<ActiveFrame>>,
}

impl Default for ActiveFrameControl {
    fn default() -> Self {
        Self {
            owner_thread: thread::current().id(),
            frame: Mutex::new(None),
        }
    }
}

impl ActiveFrameControl {
    fn lock(&self) -> MutexGuard<'_, Option<ActiveFrame>> {
        self.frame
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn enter(
        self: &Arc<Self>,
        pass: PassIdentity,
        context_id: ContextId,
        frame_index: u64,
        ui: &Ui,
    ) -> ActiveFrameGuard {
        assert_eq!(
            thread::current().id(),
            self.owner_thread,
            "Dear ImGui pass runner moved away from its owning thread"
        );
        let mut active = self.lock();
        assert!(
            active.is_none(),
            "dear-imgui-bevy attempted to expose two active Context frames"
        );
        *active = Some(ActiveFrame {
            pass,
            context_id,
            frame_index,
            ui_address: std::ptr::from_ref(ui) as usize,
        });
        drop(active);
        ActiveFrameGuard {
            control: Arc::clone(self),
            pass,
        }
    }

    fn snapshot<P: 'static>(&self, expected: PassIdentity) -> Result<ActiveFrame, RunSystemError> {
        if thread::current().id() != self.owner_thread {
            return Err(
                SystemParamValidationError::invalid::<ImguiFrame<'static, P>>(
                    "Dear ImGui pass system ran outside its owning thread",
                )
                .into(),
            );
        }
        let frame = self.lock().as_ref().copied().ok_or_else(|| {
            SystemParamValidationError::invalid::<ImguiFrame<'static, P>>(
                "Dear ImGui pass system ran without an active frame",
            )
        })?;
        if frame.pass != expected {
            return Err(
                SystemParamValidationError::invalid::<ImguiFrame<'static, P>>(format!(
                    "Dear ImGui pass mismatch: expected {}, active {}",
                    expected.brand_name, frame.pass.brand_name
                ))
                .into(),
            );
        }
        Ok(frame)
    }

    fn clear(&self, expected: PassIdentity) {
        debug_assert_eq!(thread::current().id(), self.owner_thread);
        let mut active = self.lock();
        debug_assert!(active.as_ref().is_none_or(|frame| frame.pass == expected));
        *active = None;
    }
}

struct ActiveFrameGuard {
    control: Arc<ActiveFrameControl>,
    pass: PassIdentity,
}

impl Drop for ActiveFrameGuard {
    fn drop(&mut self) {
        self.control.clear(self.pass);
    }
}

#[derive(ScheduleLabel, Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct PrivateImguiPassSchedule(PassKey);

struct PassRunner {
    schedule: Schedule,
}

impl PassRunner {
    fn new(key: PassKey) -> Self {
        let mut schedule = Schedule::new(PrivateImguiPassSchedule(key));
        schedule
            .set_executor(SingleThreadedExecutor::default())
            .set_apply_final_deferred(true);
        Self { schedule }
    }

    fn add_systems(&mut self, systems: ScheduleConfigs<ScheduleSystem>) {
        self.schedule.add_systems(systems);
    }

    fn configure_sets<M>(&mut self, sets: impl IntoScheduleConfigs<InternedSystemSet, M>) {
        self.schedule.configure_sets(sets);
    }
}

pub(crate) struct ImguiPassRegistry {
    registry_id: u64,
    next_key: u64,
    primary: PassKey,
    identities: HashMap<PassKey, (TypeId, &'static str)>,
    runners: HashMap<PassKey, PassRunner>,
    active: Arc<ActiveFrameControl>,
    lifecycle: ImguiAppLifecycle,
}

impl ImguiPassRegistry {
    fn new(lifecycle: ImguiAppLifecycle) -> Result<Self, ImguiPassError> {
        let registry_id = NEXT_REGISTRY_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| ImguiPassError::RegistryIdExhausted)?;
        let mut registry = Self {
            registry_id,
            next_key: 1,
            primary: PassKey(0),
            identities: HashMap::new(),
            runners: HashMap::new(),
            active: Arc::new(ActiveFrameControl::default()),
            lifecycle,
        };
        registry.primary = registry.allocate::<ImguiPrimaryPass>()?.identity.key;
        Ok(registry)
    }

    fn ensure_active(&self) -> Result<(), ImguiPassError> {
        if self.lifecycle.is_terminal() {
            Err(ImguiPassError::AppTerminated)
        } else {
            Ok(())
        }
    }

    fn declare<P: 'static>(&mut self) -> Result<ImguiPass<P>, ImguiPassError> {
        self.ensure_active()?;
        self.allocate::<P>()
    }

    fn primary_pass(&self) -> Result<ImguiPass<ImguiPrimaryPass>, ImguiPassError> {
        self.ensure_active()?;
        let (brand, brand_name) =
            self.identities
                .get(&self.primary)
                .copied()
                .ok_or(ImguiPassError::UnknownPass {
                    pass: type_name::<ImguiPrimaryPass>(),
                })?;
        Ok(ImguiPass::from_identity(
            PassIdentity {
                registry_id: self.registry_id,
                key: self.primary,
                brand,
                brand_name,
            },
            Arc::clone(&self.active),
        ))
    }

    fn allocate<P: 'static>(&mut self) -> Result<ImguiPass<P>, ImguiPassError> {
        let brand = TypeId::of::<P>();
        let key = PassKey(self.next_key);
        self.next_key = self
            .next_key
            .checked_add(1)
            .ok_or(ImguiPassError::PassIdExhausted)?;
        let brand_name = type_name::<P>();
        self.identities.insert(key, (brand, brand_name));
        self.runners.insert(key, PassRunner::new(key));
        Ok(ImguiPass::from_identity(
            PassIdentity {
                registry_id: self.registry_id,
                key,
                brand,
                brand_name,
            },
            Arc::clone(&self.active),
        ))
    }

    fn validate<P: 'static>(&self, pass: &ImguiPass<P>) -> Result<(), ImguiPassError> {
        self.ensure_active()?;
        if pass.identity.registry_id != self.registry_id {
            return Err(ImguiPassError::ForeignApp {
                pass: pass.identity.brand_name,
            });
        }
        if pass.identity.brand != TypeId::of::<P>() {
            return Err(ImguiPassError::InvalidBrand {
                pass: pass.identity.brand_name,
            });
        }
        if self.identities.get(&pass.identity.key)
            != Some(&(pass.identity.brand, pass.identity.brand_name))
        {
            return Err(ImguiPassError::UnknownPass {
                pass: pass.identity.brand_name,
            });
        }
        Ok(())
    }

    fn add_systems<P>(
        &mut self,
        pass: &ImguiPass<P>,
        systems: ImguiSystemConfigs<P>,
    ) -> Result<(), ImguiPassError>
    where
        P: 'static,
    {
        self.validate(pass)?;
        for identity in &systems.identities {
            if identity.registry_id != self.registry_id {
                return Err(ImguiPassError::ForeignApp {
                    pass: identity.brand_name,
                });
            }
            if *identity != pass.identity {
                return Err(ImguiPassError::SystemPassMismatch {
                    pass: pass.identity.brand_name,
                    expected_runtime: pass.identity.key.0,
                    actual_runtime: identity.key.0,
                });
            }
        }
        self.runners
            .get_mut(&pass.identity.key)
            .ok_or(ImguiPassError::UnknownPass {
                pass: pass.identity.brand_name,
            })?
            .add_systems(systems.configs);
        Ok(())
    }

    fn configure_sets<P, M>(
        &mut self,
        pass: &ImguiPass<P>,
        sets: impl IntoScheduleConfigs<InternedSystemSet, M>,
    ) -> Result<(), ImguiPassError>
    where
        P: 'static,
    {
        self.validate(pass)?;
        self.runners
            .get_mut(&pass.identity.key)
            .ok_or(ImguiPassError::UnknownPass {
                pass: pass.identity.brand_name,
            })?
            .configure_sets(sets);
        Ok(())
    }

    fn take_runner(&mut self, pass: PassIdentity) -> (PassRunner, Arc<ActiveFrameControl>) {
        assert_eq!(
            pass.registry_id, self.registry_id,
            "Dear ImGui Context refers to a pass from another App"
        );
        let runner = self
            .runners
            .remove(&pass.key)
            .expect("a configured Dear ImGui pass must retain its private runner");
        (runner, Arc::clone(&self.active))
    }

    fn return_runner(&mut self, pass: PassIdentity, runner: PassRunner) {
        let displaced = self.runners.insert(pass.key, runner);
        assert!(
            displaced.is_none(),
            "Dear ImGui pass runner was replaced while it was active"
        );
    }
}

pub(crate) fn install_pass_registry(app: &mut App) -> Result<(), ImguiPassError> {
    if !app.world().contains_resource::<ImguiAppLifecycle>() {
        app.init_resource::<ImguiAppLifecycle>();
    }
    let lifecycle = app.world().resource::<ImguiAppLifecycle>().clone();
    if lifecycle.is_terminal() {
        return Err(ImguiPassError::AppTerminated);
    }
    if app.world().get_non_send::<ImguiPassRegistry>().is_some() {
        return if lifecycle.pass_registry_claimed() {
            Ok(())
        } else {
            Err(ImguiPassError::PassRegistryMissing)
        };
    }
    if lifecycle.pass_registry_claimed() {
        return Err(ImguiPassError::PassRegistryMissing);
    }
    let registry = ImguiPassRegistry::new(lifecycle.clone())?;
    if !lifecycle.try_claim_pass_registry() {
        return Err(ImguiPassError::PassRegistryMissing);
    }
    app.insert_non_send(registry);
    Ok(())
}

pub(crate) fn declare_pass<P: 'static>(app: &mut App) -> Result<ImguiPass<P>, ImguiPassError> {
    install_pass_registry(app)?;
    app.world_mut()
        .get_non_send_mut::<ImguiPassRegistry>()
        .expect("Dear ImGui pass registry was just installed")
        .declare::<P>()
}

pub(crate) fn primary_pass(app: &mut App) -> Result<ImguiPass<ImguiPrimaryPass>, ImguiPassError> {
    install_pass_registry(app)?;
    app.world()
        .get_non_send::<ImguiPassRegistry>()
        .expect("Dear ImGui pass registry was just installed")
        .primary_pass()
}

pub(crate) fn add_systems<P>(
    app: &mut App,
    pass: &ImguiPass<P>,
    systems: impl IntoImguiSystemConfigs<P>,
) -> Result<(), ImguiPassError>
where
    P: 'static,
{
    let systems = systems.into_imgui_system_configs();
    install_pass_registry(app)?;
    app.world_mut()
        .get_non_send_mut::<ImguiPassRegistry>()
        .expect("Dear ImGui pass registry was just installed")
        .add_systems(pass, systems)
}

pub(crate) fn configure_sets<P, M>(
    app: &mut App,
    pass: &ImguiPass<P>,
    sets: impl IntoScheduleConfigs<InternedSystemSet, M>,
) -> Result<(), ImguiPassError>
where
    P: 'static,
{
    install_pass_registry(app)?;
    app.world_mut()
        .get_non_send_mut::<ImguiPassRegistry>()
        .expect("Dear ImGui pass registry was just installed")
        .configure_sets(pass, sets)
}

pub(crate) fn remove_pass_registry(app: &mut App) {
    drop(app.world_mut().remove_non_send::<ImguiPassRegistry>());
}

pub(crate) fn existing_registry_id(world: &World) -> Option<u64> {
    world
        .get_non_send::<ImguiPassRegistry>()
        .map(|registry| registry.registry_id)
}

pub(crate) fn lifecycle(world: &World) -> ImguiAppLifecycle {
    world.resource::<ImguiAppLifecycle>().clone()
}

pub(crate) fn run_pass(
    world: &mut World,
    pass: PassIdentity,
    context_id: ContextId,
    frame_index: u64,
    ui: &Ui,
) {
    let (mut runner, active) = world
        .get_non_send_mut::<ImguiPassRegistry>()
        .expect("ImguiPlugin must retain the pass registry")
        .take_runner(pass);
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let _guard = active.enter(pass, context_id, frame_index, ui);
        runner.schedule.run(world);
    }));
    world
        .get_non_send_mut::<ImguiPassRegistry>()
        .expect("ImguiPlugin must retain the pass registry")
        .return_runner(pass, runner);
    if let Err(payload) = result {
        panic::resume_unwind(payload);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn active_frame_control_rejects_foreign_threads_before_frame_lookup() {
        let control = Arc::new(ActiveFrameControl::default());
        let identity = PassIdentity {
            registry_id: 1,
            key: PassKey(1),
            brand: TypeId::of::<ImguiPrimaryPass>(),
            brand_name: type_name::<ImguiPrimaryPass>(),
        };

        let rejected = thread::spawn(move || {
            let error = match control.snapshot::<ImguiPrimaryPass>(identity) {
                Err(error) => error,
                Ok(_) => panic!("a foreign thread must not receive an active UI pointer"),
            };
            error.to_string().contains("outside its owning thread")
        })
        .join()
        .unwrap();

        assert!(rejected);
    }
}
