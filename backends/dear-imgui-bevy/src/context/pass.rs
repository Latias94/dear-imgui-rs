use std::any::{TypeId, type_name};
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use bevy_app::App;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::{Schedule, ScheduleLabel, SingleThreadedExecutor};
use bevy_ecs::system::{
    Adapt, AdapterSystem, IntoSystem, NonSendMarker, PipeSystem, RunSystemError, System, SystemIn,
    SystemInput, SystemParamValidationError,
};
use bevy_ecs::world::World;
use dear_imgui_rs::{ContextId, Ui};

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
    _brand: PhantomData<fn() -> P>,
}

impl<P: 'static> ImguiPass<P> {
    pub(crate) const fn from_identity(identity: PassIdentity) -> Self {
        Self {
            identity,
            _brand: PhantomData,
        }
    }

    pub(crate) const fn identity(&self) -> PassIdentity {
        self.identity
    }
}

impl<P: 'static> Clone for ImguiPass<P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P: 'static> Copy for ImguiPass<P> {}

impl<P: 'static> fmt::Debug for ImguiPass<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImguiPass")
            .field("brand", &self.identity.brand_name)
            .finish_non_exhaustive()
    }
}

/// Frame-scoped Dear ImGui access injected by a private pass runner.
///
/// `ImguiFrame` is a Bevy [`SystemInput`], not a `SystemParam`. A function that accepts it cannot
/// be added to an ordinary Bevy schedule. Register the function through
/// [`crate::ImguiAppExt::add_imgui_system`].
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
/// app.add_imgui_system(&pass_a, draw_b);
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

#[derive(Default)]
struct ActiveFrameControl {
    frame: Mutex<Option<ActiveFrame>>,
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

struct FrameInputAdapter<P: 'static> {
    pass: PassIdentity,
    active: Arc<ActiveFrameControl>,
    _brand: PhantomData<fn() -> P>,
}

impl<P: 'static, S> Adapt<S> for FrameInputAdapter<P>
where
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
        // SAFETY: the private driver owns the native frame and keeps ActiveFrameGuard alive until
        // this private schedule returns. This adapter runs synchronously on the main thread, and
        // the constructed borrow is passed directly into one system invocation.
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

    fn add_system<P, S, M>(
        &mut self,
        pass: PassIdentity,
        active: Arc<ActiveFrameControl>,
        system: S,
    ) where
        P: 'static,
        S: IntoSystem<ImguiFrame<'static, P>, (), M> + 'static,
        M: 'static,
    {
        let inner = IntoSystem::into_system(system);
        let name = inner.name();
        let adapted = AdapterSystem::new(
            FrameInputAdapter::<P> {
                pass,
                active,
                _brand: PhantomData,
            },
            inner,
            name.clone(),
        );
        let main_thread = IntoSystem::into_system(require_main_thread);
        let bound = PipeSystem::new(main_thread, adapted, name);
        self.schedule.add_systems(bound);
    }
}

pub(crate) struct ImguiPassRegistry {
    registry_id: u64,
    next_key: u64,
    primary: PassKey,
    identities: HashMap<PassKey, (TypeId, &'static str)>,
    runners: HashMap<PassKey, PassRunner>,
    active: Arc<ActiveFrameControl>,
}

impl ImguiPassRegistry {
    fn new() -> Self {
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
        ImguiPass::from_identity(PassIdentity {
            registry_id: self.registry_id,
            key: self.primary,
            brand,
            brand_name,
        })
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
        ImguiPass::from_identity(PassIdentity {
            registry_id: self.registry_id,
            key,
            brand,
            brand_name,
        })
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

    fn add_system<P, S, M>(&mut self, pass: &ImguiPass<P>, system: S)
    where
        P: 'static,
        S: IntoSystem<ImguiFrame<'static, P>, (), M> + 'static,
        M: 'static,
    {
        self.validate(pass);
        let active = Arc::clone(&self.active);
        self.runners
            .get_mut(&pass.identity.key)
            .expect("a declared Dear ImGui pass must retain its private runner")
            .add_system::<P, _, M>(pass.identity, active, system);
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
    if app.world().get_non_send::<ImguiPassRegistry>().is_none() {
        app.insert_non_send(ImguiPassRegistry::new());
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

pub(crate) fn add_system<P, S, M>(app: &mut App, pass: &ImguiPass<P>, system: S)
where
    P: 'static,
    S: IntoSystem<ImguiFrame<'static, P>, (), M> + 'static,
    M: 'static,
{
    install_pass_registry(app);
    app.world_mut()
        .get_non_send_mut::<ImguiPassRegistry>()
        .expect("Dear ImGui pass registry was just installed")
        .add_system::<P, _, M>(pass, system);
}

pub(crate) fn registry_id(world: &World) -> u64 {
    world
        .get_non_send::<ImguiPassRegistry>()
        .expect("ImguiPlugin must install the pass registry")
        .registry_id
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
