use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dear_imgui_rs::{ContextBinding, Ui};
use dear_imgui_sys as imgui;
use dear_imguizmo_sys as sys;

thread_local! {
    // All live safe Dear ImGui contexts belong to one thread. A new owner thread starts
    // its first frame by replacing the native pointers and ID left by the previous one.
    static CURRENT: RefCell<Option<Rc<GizmoFrame>>> = const { RefCell::new(None) };
}

#[derive(Clone, Copy)]
pub(crate) enum DrawList {
    Frame(*mut imgui::ImDrawList),
    Window(*mut imgui::ImGuiWindow),
}

pub(crate) struct GizmoFrame {
    owner: ContextBinding,
    number: i32,
    target: Cell<DrawList>,
    named_alternative: Cell<Option<imgui::ImGuiID>>,
    // Each value already includes its parent seed. An inner ID can stay active when
    // its outer token is dropped early. No native PushID spans application code.
    ids: RefCell<Vec<Option<u32>>>,
}

impl GizmoFrame {
    pub(crate) fn begin(ui: &Ui) -> Rc<Self> {
        ui.with_bound_context(|| {
            let ctx = unsafe { imgui::igGetCurrentContext() };
            assert!(
                unsafe { (*ctx).WithinFrameScope },
                "ImGuizmo needs an open ImGui frame"
            );
            let number = unsafe { imgui::igGetFrameCount() };
            let mut named_alternative = None;
            let previous = CURRENT.with_borrow(Clone::clone);
            let same_owner = previous
                .as_ref()
                .is_some_and(|frame| frame.owner.id() == ui.context_id());
            if let Some(previous) = previous {
                if same_owner {
                    if previous.number == number {
                        return previous;
                    }
                    named_alternative = previous.named_alternative.get();
                } else {
                    assert!(
                        !previous
                            .owner
                            .try_with_bound_context(|| previous.is_open())
                            .unwrap_or(false),
                        "finish the other ImGuizmo frame before drawing in this ImGui context"
                    );
                }
            }

            let draw_list = unsafe {
                // The native singleton outlives contexts and owner threads. A fresh TLS slot
                // always resets, and a returning thread compares process-unique generations.
                if !same_owner {
                    sys::ImGuizmo_ResetInteraction();
                }
                // BeginFrame opens its own helper window. Keep the application's pending
                // SetNextWindow* data for its next Begin and pin the helper to the main viewport.
                let next_window = std::mem::take(&mut (*ctx).NextWindowData);
                let main = imgui::igGetMainViewport();
                imgui::igSetNextWindowViewport((*main).ID);
                let alternative = named_alternative
                    .map_or(std::ptr::null_mut(), |id| imgui::igFindWindowByID(id));
                sys::ImGuizmo_SetAlternativeWindow(alternative.cast());
                sys::ImGuizmo_SetID(-1);
                sys::ImGuizmo_BeginFrame();
                (*ctx).NextWindowData = next_window;
                let helper = imgui::igFindWindowByName(c"gizmo".as_ptr());
                assert!(
                    !helper.is_null(),
                    "ImGuizmo helper window is missing after BeginFrame"
                );
                (*helper).DrawList
            };
            let frame = Rc::new(Self {
                owner: ui.binding(),
                number,
                target: Cell::new(DrawList::Frame(draw_list)),
                named_alternative: Cell::new(named_alternative),
                ids: RefCell::default(),
            });
            CURRENT.set(Some(Rc::clone(&frame)));
            frame
        })
    }

    // Called with the owning context bound.
    pub(crate) fn is_open(&self) -> bool {
        unsafe {
            (*imgui::igGetCurrentContext()).WithinFrameScope
                && imgui::igGetFrameCount() == self.number
        }
    }

    pub(crate) fn select(&self, target: DrawList) {
        self.target.set(target);
        self.draw_list();
    }

    pub(crate) fn set_alternative(
        &self,
        window: *mut sys::ImGuiWindow,
        id: Option<imgui::ImGuiID>,
    ) {
        self.named_alternative.set(id);
        unsafe { sys::ImGuizmo_SetAlternativeWindow(window) };
    }

    pub(crate) fn draw_list(&self) -> *mut imgui::ImDrawList {
        let list = match self.target.get() {
            DrawList::Frame(list) => list,
            DrawList::Window(window) => unsafe {
                assert_eq!(
                    imgui::igGetCurrentWindowRead(),
                    window,
                    "ImGuizmo needs the selected window to be current"
                );
                imgui::igGetWindowDrawList()
            },
        };
        unsafe { sys::ImGuizmo_SetDrawlist(list) };
        list
    }

    pub(crate) fn current_id(&self) -> u32 {
        self.ids
            .borrow()
            .last()
            .copied()
            .flatten()
            .unwrap_or(u32::MAX)
    }

    pub(crate) fn push_id(&self, id: u32) -> usize {
        let mut ids = self.ids.borrow_mut();
        let slot = ids.len();
        ids.push(Some(id));
        unsafe { sys::ImGuizmo_SetID(id as i32) };
        slot
    }

    pub(crate) fn release_id(&self, slot: usize) {
        let mut ids = self.ids.borrow_mut();
        ids[slot] = None;
        if slot + 1 != ids.len() {
            return;
        }
        while ids.last() == Some(&None) {
            ids.pop();
        }
        let id = ids.last().copied().flatten().unwrap_or(u32::MAX);
        drop(ids);
        let _ = self.owner.try_with_bound_context(|| {
            if self.is_open() {
                unsafe { sys::ImGuizmo_SetID(id as i32) };
            }
        });
    }
}
