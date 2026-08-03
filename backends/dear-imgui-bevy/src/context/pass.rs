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
    InternedSystemSet, IntoScheduleConfigs, Schedule, ScheduleLabel, SingleThreadedExecutor,
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

    /// Bind a frame-input system to this pass so Bevy can configure it like any other system.
    ///
    /// Pass the returned system to [`crate::ImguiAppExt::add_imgui_systems`]. Tuples and every
    /// standard [`IntoScheduleConfigs`] modifier remain available after binding.
    #[must_use]
    pub fn system<S>(&self, system: S) -> ImguiSystem<P, S> {
        ImguiSystem {
            pass: self.identity,
            active: Arc::clone(&self.active),
            system,
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

/// A Dear ImGui frame-input system bound to one private pass.
///
/// This implements [`IntoSystem`] with unit input, so callers can use Bevy's normal system
/// configuration API before registering it with [`crate::ImguiAppExt::add_imgui_systems`].
pub struct ImguiSystem<P: 'static, S> {
    pass: PassIdentity,
    active: Arc<ActiveFrameControl>,
    system: S,
    _brand: PhantomData<fn() -> P>,
}

#[doc(hidden)]
pub struct ImguiSystemMarker<P, M>(PhantomData<fn() -> (P, M)>);

impl<P, S, M> IntoSystem<(), (), ImguiSystemMarker<P, M>> for ImguiSystem<P, S>
where
    P: 'static,
    S: IntoSystem<ImguiFrame<'static, P>, (), M> + 'static,
    M: 'static,
{
    type System = PipeSystem<
        FunctionSystem<fn(NonSendMarker), (), (), fn(NonSendMarker)>,
        AdapterSystem<ImguiFrameAdapter<P>, S::System>,
    >;

    fn into_system(this: Self) -> Self::System {
        let system = IntoSystem::into_system(this.system);
        let name = system.name();
        let adapted = AdapterSystem::new(
            ImguiFrameAdapter {
                pass: this.pass,
                active: this.active,
                _brand: PhantomData,
            },
            system,
            name.clone(),
        );
        let main_thread: FunctionSystem<fn(NonSendMarker), (), (), fn(NonSendMarker)> =
            IntoSystem::into_system(require_main_thread as fn(NonSendMarker));
        PipeSystem::new(main_thread, adapted, name)
    }
}

/// Internal unit-input adapter that injects the frame owned by its private pass runner.
#[doc(hidden)]
pub struct ImguiFrameAdapter<P: 'static> {
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
/// let pass_a = app.declare_imgui_pass::<PassA>();
/// app.add_imgui_systems(&pass_a, pass_a.system(draw_b));
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

    fn add_systems<M>(&mut self, systems: impl IntoScheduleConfigs<ScheduleSystem, M>) {
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
    fn new(lifecycle: ImguiAppLifecycle) -> Self {
        let registry_id = NEXT_REGISTRY_ID.fetch_add(1, Ordering::Relaxed);
        assert_ne!(
            registry_id,
            u64::MAX,
            "Dear ImGui pass registry IDs exhausted"
        );
        let mut registry = Self {
            registry_id,
            next_key: 1,
            primary: PassKey(0),
            identities: HashMap::new(),
            runners: HashMap::new(),
            active: Arc::new(ActiveFrameControl::default()),
            lifecycle,
        };
        registry.primary = registry.allocate::<ImguiPrimaryPass>().identity.key;
        registry
    }

    fn declare<P: 'static>(&mut self) -> ImguiPass<P> {
        self.allocate::<P>()
    }

    fn primary_pass(&self) -> ImguiPass<ImguiPrimaryPass> {
        let (brand, brand_name) = self
            .identities
            .get(&self.primary)
            .copied()
            .expect("the pass registry must retain its primary pass");
        ImguiPass::from_identity(
            PassIdentity {
                registry_id: self.registry_id,
                key: self.primary,
                brand,
                brand_name,
            },
            Arc::clone(&self.active),
        )
    }

    fn allocate<P: 'static>(&mut self) -> ImguiPass<P> {
        let brand = TypeId::of::<P>();
        let key = PassKey(self.next_key);
        self.next_key = self
            .next_key
            .checked_add(1)
            .expect("Dear ImGui pass IDs exhausted");
        let brand_name = type_name::<P>();
        self.identities.insert(key, (brand, brand_name));
        self.runners.insert(key, PassRunner::new(key));
        ImguiPass::from_identity(
            PassIdentity {
                registry_id: self.registry_id,
                key,
                brand,
                brand_name,
            },
            Arc::clone(&self.active),
        )
    }

    fn validate<P: 'static>(&self, pass: &ImguiPass<P>) {
        assert_eq!(
            pass.identity.registry_id, self.registry_id,
            "Dear ImGui pass handle belongs to another App"
        );
        assert_eq!(
            pass.identity.brand,
            TypeId::of::<P>(),
            "Dear ImGui pass handle has an invalid type brand"
        );
        assert_eq!(
            self.identities.get(&pass.identity.key),
            Some(&(pass.identity.brand, pass.identity.brand_name)),
            "Dear ImGui pass handle is not registered in this App"
        );
    }

    fn add_systems<P, M>(
        &mut self,
        pass: &ImguiPass<P>,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) where
        P: 'static,
    {
        assert!(
            !self.lifecycle.is_terminal(),
            "Dear ImGui App lifecycle is terminal"
        );
        self.validate(pass);
        self.runners
            .get_mut(&pass.identity.key)
            .expect("a declared Dear ImGui pass must retain its private runner")
            .add_systems(systems);
    }

    fn configure_sets<P, M>(
        &mut self,
        pass: &ImguiPass<P>,
        sets: impl IntoScheduleConfigs<InternedSystemSet, M>,
    ) where
        P: 'static,
    {
        assert!(
            !self.lifecycle.is_terminal(),
            "Dear ImGui App lifecycle is terminal"
        );
        self.validate(pass);
        self.runners
            .get_mut(&pass.identity.key)
            .expect("a declared Dear ImGui pass must retain its private runner")
            .configure_sets(sets);
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

pub(crate) fn install_pass_registry(app: &mut App) {
    if !app.world().contains_resource::<ImguiAppLifecycle>() {
        app.init_resource::<ImguiAppLifecycle>();
    }
    let lifecycle = app.world().resource::<ImguiAppLifecycle>().clone();
    assert!(
        !lifecycle.is_terminal(),
        "Dear ImGui App lifecycle is terminal"
    );
    if app.world().get_non_send::<ImguiPassRegistry>().is_none() {
        app.insert_non_send(ImguiPassRegistry::new(lifecycle));
    }
}

pub(crate) fn declare_pass<P: 'static>(app: &mut App) -> ImguiPass<P> {
    install_pass_registry(app);
    app.world_mut()
        .get_non_send_mut::<ImguiPassRegistry>()
        .expect("Dear ImGui pass registry was just installed")
        .declare::<P>()
}

pub(crate) fn primary_pass(app: &mut App) -> ImguiPass<ImguiPrimaryPass> {
    install_pass_registry(app);
    app.world()
        .get_non_send::<ImguiPassRegistry>()
        .expect("Dear ImGui pass registry was just installed")
        .primary_pass()
}

pub(crate) fn add_systems<P, M>(
    app: &mut App,
    pass: &ImguiPass<P>,
    systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
) where
    P: 'static,
{
    install_pass_registry(app);
    app.world_mut()
        .get_non_send_mut::<ImguiPassRegistry>()
        .expect("Dear ImGui pass registry was just installed")
        .add_systems(pass, systems);
}

pub(crate) fn configure_sets<P, M>(
    app: &mut App,
    pass: &ImguiPass<P>,
    sets: impl IntoScheduleConfigs<InternedSystemSet, M>,
) where
    P: 'static,
{
    install_pass_registry(app);
    app.world_mut()
        .get_non_send_mut::<ImguiPassRegistry>()
        .expect("Dear ImGui pass registry was just installed")
        .configure_sets(pass, sets);
}

pub(crate) fn registry_id(world: &World) -> u64 {
    world
        .get_non_send::<ImguiPassRegistry>()
        .expect("ImguiPlugin must install the pass registry")
        .registry_id
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
