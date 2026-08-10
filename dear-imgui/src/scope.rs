//! Internal native-scope bookkeeping.
//!
//! Dear ImGui has several independent native stacks plus window- and table-local scopes. A safe
//! Rust token must therefore prove both resource order and provenance before calling a matching
//! `Pop`/`End`. This registry rejects invalid strict-LIFO use before FFI and can defer cleanup
//! until the original window or table scope is current again.

use std::collections::HashMap;
use std::fmt;

use crate::sys;
use crate::ui::Ui;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct WindowScope {
    frame: i32,
    window: usize,
    stack_depth: i32,
    owner: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct TableScope {
    window: WindowScope,
    table: usize,
    instance: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ScopeSnapshot {
    window: Option<WindowScope>,
    table: Option<TableScope>,
    row: i32,
    column: i32,
}

impl ScopeSnapshot {
    unsafe fn current_raw(ui: &Ui) -> Self {
        debug_assert_eq!(unsafe { sys::igGetCurrentContext() }, ui.context_raw());
        let context = unsafe { &*ui.context_raw() };
        let frame = context.FrameCount;
        let window = unsafe { sys::igGetCurrentWindowRead() };
        let window = (!window.is_null()).then(|| WindowScope {
            frame,
            window: window as usize,
            stack_depth: context.CurrentWindowStack.Size,
            owner: None,
        });
        let table = unsafe { sys::igGetCurrentTable() };
        if table.is_null() {
            return Self {
                window,
                table: None,
                row: -1,
                column: -1,
            };
        }
        let window = window.expect("an active Dear ImGui table must have a current window");
        Self {
            window: Some(window),
            table: Some(TableScope {
                window,
                table: table as usize,
                instance: unsafe { (*table).InstanceCurrent },
            }),
            row: unsafe { (*table).CurrentRow },
            column: unsafe { (*table).CurrentColumn },
        }
    }

    fn with_window_owner(mut self, owner: Option<u64>) -> Self {
        if let Some(window) = self.window.as_mut() {
            window.owner = owner;
        }
        if let Some(table) = self.table.as_mut() {
            table.window.owner = owner;
        }
        self
    }

    pub(crate) fn window(self) -> Option<WindowScope> {
        self.window
    }

    pub(crate) fn table(self) -> Option<TableScope> {
        self.table
    }
}

impl WindowScope {
    fn has_same_native_position(self, other: Self) -> bool {
        self.frame == other.frame
            && self.window == other.window
            && self.stack_depth == other.stack_depth
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum NativeScopeKey {
    WindowStack,
    StyleColor,
    StyleVar,
    AlphaMutation,
    Font,
    Id(WindowScope),
    FocusScope,
    ItemFlag,
    ItemWidth(WindowScope),
    TextWrap(WindowScope),
    Group(WindowScope),
    Indent(WindowScope),
    MenuBar(WindowScope),
    Tab(WindowScope),
    DragDrop(WindowScope),
    #[cfg(feature = "stack-layout")]
    StackLayout(WindowScope),
    DrawListClip(usize),
    DrawListTexture(usize),
    TableStack,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum NativeScopePop {
    EndWindow,
    EndChild,
    EndCombo,
    EndListBox,
    EndMainMenuBar,
    EndMenuBar,
    EndMenu,
    EndDisabled {
        restores_alpha: bool,
    },
    EndPopup,
    EndTooltip,
    EndTabBar,
    EndTabItem,
    TreePop,
    EndTable,
    EndGroup,
    EndDragDropSource,
    EndDragDropTarget,
    EndMultiSelect {
        range_source_reset: bool,
    },
    PopStyleColor,
    PopStyleVar {
        is_alpha: bool,
    },
    PopFont,
    PopId,
    PopFocusScope,
    PopItemFlag,
    PopItemWidth,
    PopTextWrap,
    Unindent(f32),
    #[cfg(feature = "stack-layout")]
    EndHorizontal,
    #[cfg(feature = "stack-layout")]
    EndVertical,
    #[cfg(feature = "stack-layout")]
    ResumeLayout,
    PopUiClipRect(*mut sys::ImDrawList),
    DrawListPopClipRect {
        draw_list: *mut sys::ImDrawList,
        window_scoped: bool,
    },
    DrawListPopTexture {
        draw_list: *mut sys::ImDrawList,
        window_scoped: bool,
    },
    PopTableBackgroundChannel,
    PopTableColumnChannel,
}

impl NativeScopePop {
    fn keys(self, scope: ScopeSnapshot) -> NativeScopeKeys {
        let window = || {
            scope
                .window
                .expect("native scope operation requires an active Dear ImGui window")
        };
        let primary = match self {
            Self::EndWindow
            | Self::EndChild
            | Self::EndCombo
            | Self::EndListBox
            | Self::EndMainMenuBar
            | Self::EndMenu
            | Self::EndPopup
            | Self::EndTooltip => NativeScopeKey::WindowStack,
            Self::EndMenuBar => NativeScopeKey::MenuBar(window()),
            Self::EndDisabled { .. } | Self::PopItemFlag => NativeScopeKey::ItemFlag,
            Self::EndTabBar | Self::EndTabItem => NativeScopeKey::Tab(window()),
            Self::TreePop | Self::PopId => NativeScopeKey::Id(window()),
            Self::EndTable | Self::PopTableBackgroundChannel | Self::PopTableColumnChannel => {
                NativeScopeKey::TableStack
            }
            Self::EndGroup => NativeScopeKey::Group(window()),
            Self::EndDragDropSource | Self::EndDragDropTarget => NativeScopeKey::DragDrop(window()),
            Self::EndMultiSelect { .. } | Self::PopFocusScope => NativeScopeKey::FocusScope,
            Self::PopStyleColor => NativeScopeKey::StyleColor,
            Self::PopStyleVar { .. } => NativeScopeKey::StyleVar,
            Self::PopFont => NativeScopeKey::Font,
            Self::PopItemWidth => NativeScopeKey::ItemWidth(window()),
            Self::PopTextWrap => NativeScopeKey::TextWrap(window()),
            Self::Unindent(_) => NativeScopeKey::Indent(window()),
            #[cfg(feature = "stack-layout")]
            Self::EndHorizontal | Self::EndVertical | Self::ResumeLayout => {
                NativeScopeKey::StackLayout(window())
            }
            Self::PopUiClipRect(draw_list)
            | Self::DrawListPopClipRect {
                draw_list,
                window_scoped: _,
            } => NativeScopeKey::DrawListClip(draw_list as usize),
            Self::DrawListPopTexture {
                draw_list,
                window_scoped: _,
            } => NativeScopeKey::DrawListTexture(draw_list as usize),
        };
        let secondary = match self {
            Self::EndDisabled {
                restores_alpha: true,
            }
            | Self::PopStyleVar { is_alpha: true } => Some(NativeScopeKey::AlphaMutation),
            _ => None,
        };
        NativeScopeKeys { primary, secondary }
    }

    fn requires_window(self) -> bool {
        match self {
            Self::EndWindow
            | Self::EndChild
            | Self::EndCombo
            | Self::EndListBox
            | Self::EndMainMenuBar
            | Self::EndMenuBar
            | Self::EndMenu
            | Self::EndDisabled { .. }
            | Self::EndPopup
            | Self::EndTooltip
            | Self::EndTabBar
            | Self::EndTabItem
            | Self::TreePop
            | Self::EndGroup
            | Self::EndDragDropSource
            | Self::EndDragDropTarget
            | Self::EndMultiSelect { .. }
            | Self::PopStyleColor
            | Self::PopStyleVar { .. }
            | Self::PopFont
            | Self::PopId
            | Self::PopFocusScope
            | Self::PopItemFlag
            | Self::PopItemWidth
            | Self::PopTextWrap
            | Self::Unindent(_)
            | Self::PopUiClipRect(_) => true,
            Self::DrawListPopClipRect {
                window_scoped: true,
                ..
            }
            | Self::DrawListPopTexture {
                window_scoped: true,
                ..
            } => true,
            #[cfg(feature = "stack-layout")]
            Self::EndHorizontal | Self::EndVertical | Self::ResumeLayout => true,
            _ => false,
        }
    }

    fn requires_table(self) -> bool {
        matches!(
            self,
            Self::EndTable | Self::PopTableBackgroundChannel | Self::PopTableColumnChannel
        )
    }

    fn requires_table_cell(self) -> bool {
        matches!(
            self,
            Self::PopTableBackgroundChannel | Self::PopTableColumnChannel
        )
    }

    fn ends_window(self) -> bool {
        matches!(
            self,
            Self::EndWindow
                | Self::EndChild
                | Self::EndCombo
                | Self::EndListBox
                | Self::EndMainMenuBar
                | Self::EndMenu
                | Self::EndPopup
                | Self::EndTooltip
        )
    }

    fn ends_table(self) -> bool {
        matches!(self, Self::EndTable)
    }

    fn counts_for_window_end(self) -> bool {
        !self.ends_window()
            && !matches!(
                self,
                Self::DrawListPopClipRect {
                    window_scoped: false,
                    ..
                } | Self::DrawListPopTexture {
                    window_scoped: false,
                    ..
                }
            )
    }

    fn counts_for_table_end(self) -> bool {
        self.counts_for_window_end()
    }

    pub(crate) unsafe fn finish_multi_select(self) -> *mut sys::ImGuiMultiSelectIO {
        let Self::EndMultiSelect { range_source_reset } = self else {
            panic!("finish_multi_select() requires an EndMultiSelect action");
        };

        unsafe {
            if range_source_reset {
                let context = sys::igGetCurrentContext();
                let active = context.as_ref().and_then(|context| {
                    context
                        .CurrentMultiSelect
                        .cast::<sys::ImGuiMultiSelectTempData>()
                        .as_mut()
                });
                if let Some(active) = active {
                    active.IO.RangeSrcReset = true;
                }
            }
            sys::igEndMultiSelect()
        }
    }

    unsafe fn run(self) {
        unsafe {
            match self {
                Self::EndWindow => sys::igEnd(),
                Self::EndChild => sys::igEndChild(),
                Self::EndCombo => sys::igEndCombo(),
                Self::EndListBox => sys::igEndListBox(),
                Self::EndMainMenuBar => sys::igEndMainMenuBar(),
                Self::EndMenuBar => sys::igEndMenuBar(),
                Self::EndMenu => sys::igEndMenu(),
                Self::EndDisabled { .. } => sys::igEndDisabled(),
                Self::EndPopup => sys::igEndPopup(),
                Self::EndTooltip => sys::igEndTooltip(),
                Self::EndTabBar => sys::igEndTabBar(),
                Self::EndTabItem => sys::igEndTabItem(),
                Self::TreePop => sys::igTreePop(),
                Self::EndTable => sys::igEndTable(),
                Self::EndGroup => sys::igEndGroup(),
                Self::EndDragDropSource => sys::igEndDragDropSource(),
                Self::EndDragDropTarget => sys::igEndDragDropTarget(),
                Self::EndMultiSelect { .. } => {
                    let _ = self.finish_multi_select();
                }
                Self::PopStyleColor => sys::igPopStyleColor(1),
                Self::PopStyleVar { .. } => sys::igPopStyleVar(1),
                Self::PopFont => sys::igPopFont(),
                Self::PopId => sys::igPopID(),
                Self::PopFocusScope => sys::igPopFocusScope(),
                Self::PopItemFlag => sys::igPopItemFlag(),
                Self::PopItemWidth => sys::igPopItemWidth(),
                Self::PopTextWrap => sys::igPopTextWrapPos(),
                Self::Unindent(width) => sys::igUnindent(width),
                #[cfg(feature = "stack-layout")]
                Self::EndHorizontal => sys::ImGuiStack_EndHorizontal(),
                #[cfg(feature = "stack-layout")]
                Self::EndVertical => sys::ImGuiStack_EndVertical(),
                #[cfg(feature = "stack-layout")]
                Self::ResumeLayout => sys::ImGuiStack_ResumeLayout(),
                Self::PopUiClipRect(_) => sys::igPopClipRect(),
                Self::DrawListPopClipRect { draw_list, .. } => {
                    sys::ImDrawList_PopClipRect(draw_list)
                }
                Self::DrawListPopTexture { draw_list, .. } => sys::ImDrawList_PopTexture(draw_list),
                Self::PopTableBackgroundChannel => sys::igTablePopBackgroundChannel(),
                Self::PopTableColumnChannel => sys::igTablePopColumnChannel(),
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeScopeKeys {
    primary: NativeScopeKey,
    secondary: Option<NativeScopeKey>,
}

impl NativeScopeKeys {
    fn iter(self) -> impl Iterator<Item = NativeScopeKey> {
        std::iter::once(self.primary).chain(self.secondary)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeScopeOrder {
    StrictLifo,
    ProvenanceOnly,
}

#[derive(Debug, Default)]
enum NativeScopeStack {
    #[default]
    Empty,
    One(u64),
    Many(Vec<u64>),
}

impl NativeScopeStack {
    fn push(&mut self, id: u64) {
        *self = match std::mem::take(self) {
            Self::Empty => Self::One(id),
            Self::One(previous) => Self::Many(vec![previous, id]),
            Self::Many(mut ids) => {
                ids.push(id);
                Self::Many(ids)
            }
        };
    }

    fn last(&self) -> Option<u64> {
        match self {
            Self::Empty => None,
            Self::One(id) => Some(*id),
            Self::Many(ids) => ids.last().copied(),
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::One(_) => 1,
            Self::Many(ids) => ids.len(),
        }
    }

    fn position(&self, id: u64) -> Option<usize> {
        match self {
            Self::Empty => None,
            Self::One(candidate) => (*candidate == id).then_some(0),
            Self::Many(ids) => ids.iter().position(|candidate| *candidate == id),
        }
    }

    fn remove(&mut self, position: usize) -> bool {
        *self = match std::mem::take(self) {
            Self::Empty => panic!("native scope resource stack is empty"),
            Self::One(_) if position == 0 => Self::Empty,
            Self::One(_) => panic!("native scope resource stack position is invalid"),
            Self::Many(mut ids) => {
                ids.remove(position);
                match ids.as_slice() {
                    [] => Self::Empty,
                    [id] => Self::One(*id),
                    _ => Self::Many(ids),
                }
            }
        };
        matches!(self, Self::Empty)
    }
}

#[derive(Clone, Copy, Debug)]
struct NativeScopeEntry {
    id: u64,
    label: &'static str,
    pop: NativeScopePop,
    keys: NativeScopeKeys,
    scope: ScopeSnapshot,
    order: NativeScopeOrder,
    abandoned: bool,
}

#[derive(Debug, Default)]
pub(crate) struct NativeScopeTracker {
    next_id: u64,
    stacks: HashMap<NativeScopeKey, NativeScopeStack>,
    entries: HashMap<u64, NativeScopeEntry>,
    pending_recovery: bool,
}

#[derive(Debug)]
struct ScopeOrderError {
    label: &'static str,
    reason: String,
}

impl fmt::Display for ScopeOrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native scope `{}` {}", self.label, self.reason)
    }
}

impl NativeScopeTracker {
    fn begin(
        &mut self,
        pop: NativeScopePop,
        label: &'static str,
        order: NativeScopeOrder,
        raw_scope: ScopeSnapshot,
    ) -> (NativeScopeKeys, u64) {
        assert!(
            !self.pending_recovery,
            "cannot enter `{label}` after a native scope order violation; finish the remaining scopes first"
        );
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("native scope identity space is exhausted");
        let mut scope = self.resolve_scope(raw_scope);
        if pop.ends_window() {
            scope = scope.with_window_owner(Some(id));
        }
        let keys = pop.keys(scope);
        for key in keys.iter() {
            self.stacks.entry(key).or_default().push(id);
        }
        self.entries.insert(
            id,
            NativeScopeEntry {
                id,
                label,
                pop,
                keys,
                scope,
                order,
                abandoned: false,
            },
        );
        (keys, id)
    }

    fn prepare_pop(
        &mut self,
        keys: NativeScopeKeys,
        id: u64,
        label: &'static str,
        raw_current: ScopeSnapshot,
    ) -> Result<bool, ScopeOrderError> {
        let current = self.resolve_scope(raw_current);
        let Some(entry) = self.entries.get(&id).copied() else {
            self.pending_recovery = true;
            return Err(ScopeOrderError {
                label,
                reason: "was ended more than once".to_owned(),
            });
        };
        debug_assert_eq!(entry.keys, keys);
        if let Some(reason) = self.violation(entry, current) {
            self.entries
                .get_mut(&id)
                .expect("native scope entry disappeared")
                .abandoned = true;
            self.refresh_pending_recovery();
            return match entry.order {
                NativeScopeOrder::StrictLifo => Err(ScopeOrderError { label, reason }),
                NativeScopeOrder::ProvenanceOnly => Ok(false),
            };
        }
        self.remove_ready(entry);
        self.refresh_pending_recovery();
        Ok(true)
    }

    fn violation(&self, entry: NativeScopeEntry, current: ScopeSnapshot) -> Option<String> {
        if entry.order == NativeScopeOrder::StrictLifo {
            for key in entry.keys.iter() {
                let stack = self
                    .stacks
                    .get(&key)
                    .expect("native scope resource stack disappeared");
                if stack.last() != Some(entry.id) {
                    let top = stack
                        .last()
                        .and_then(|id| self.entries.get(&id))
                        .map_or("unknown scope", |entry| entry.label);
                    return Some(format!("was dropped before the active inner scope `{top}`"));
                }
            }
        }
        if entry.pop.requires_window() && entry.scope.window != current.window {
            return Some(
                "was dropped outside its original frame and window Begin scope".to_owned(),
            );
        }
        if entry.pop.requires_table() && entry.scope.table != current.table {
            return Some("was dropped outside its original table instance".to_owned());
        }
        if entry.pop.requires_table_cell()
            && (entry.scope.row != current.row || entry.scope.column != current.column)
        {
            return Some("was dropped after the current table cell changed".to_owned());
        }
        if entry.pop.ends_window() {
            if let Some(blocker) = self
                .entries
                .values()
                .filter(|other| {
                    other.id > entry.id
                        && other.pop.counts_for_window_end()
                        && other.scope.window == entry.scope.window
                })
                .max_by_key(|other| other.id)
            {
                return Some(format!(
                    "was dropped before the window-local scope `{}`",
                    blocker.label
                ));
            }
        }
        if entry.pop.ends_table() {
            if let Some(blocker) = self
                .entries
                .values()
                .filter(|other| {
                    other.id > entry.id
                        && other.pop.counts_for_table_end()
                        && other.scope.table == entry.scope.table
                })
                .max_by_key(|other| other.id)
            {
                return Some(format!(
                    "was dropped before the table-local scope `{}`",
                    blocker.label
                ));
            }
        }
        None
    }

    fn replace_pop(&mut self, id: u64, keys: NativeScopeKeys, pop: NativeScopePop) {
        let entry = self
            .entries
            .get_mut(&id)
            .expect("native scope entry disappeared before its cleanup action was updated");
        assert_eq!(
            entry.keys, keys,
            "native scope token keys changed unexpectedly"
        );
        assert_eq!(
            pop.keys(entry.scope),
            keys,
            "replacement native cleanup action uses different resource stacks"
        );
        entry.pop = pop;
    }

    fn assert_no_table_channel(&self, current: ScopeSnapshot, operation: &'static str) {
        let current = self.resolve_scope(current);
        let Some(table) = current.table else {
            return;
        };
        let active = self
            .entries
            .values()
            .filter(|entry| {
                entry.scope.table == Some(table)
                    && matches!(
                        entry.pop,
                        NativeScopePop::PopTableBackgroundChannel
                            | NativeScopePop::PopTableColumnChannel
                    )
            })
            .max_by_key(|entry| entry.id);
        assert!(
            active.is_none(),
            "{operation} cannot change the current table cell while `{}` is active",
            active.map_or("table channel", |entry| entry.label)
        );
    }

    fn remove_ready(&mut self, entry: NativeScopeEntry) {
        for key in entry.keys.iter() {
            let remove_stack = {
                let stack = self
                    .stacks
                    .get_mut(&key)
                    .expect("native scope resource stack disappeared");
                let position = stack
                    .position(entry.id)
                    .expect("native scope resource stack lost its active entry");
                if entry.order == NativeScopeOrder::StrictLifo {
                    assert_eq!(
                        position,
                        stack.len() - 1,
                        "strict native scope was removed out of order"
                    );
                }
                stack.remove(position)
            };
            if remove_stack {
                self.stacks.remove(&key);
            }
        }
        self.entries
            .remove(&entry.id)
            .expect("native scope entry disappeared");
    }

    fn take_ready_abandoned(&mut self, raw_current: ScopeSnapshot) -> Option<NativeScopePop> {
        let current = self.resolve_scope(raw_current);
        let ready = self
            .entries
            .values()
            .filter(|entry| entry.abandoned && self.violation(**entry, current).is_none())
            .max_by_key(|entry| entry.id)
            .copied()?;
        self.remove_ready(ready);
        self.refresh_pending_recovery();
        Some(ready.pop)
    }

    fn refresh_pending_recovery(&mut self) {
        self.pending_recovery = self
            .entries
            .values()
            .any(|entry| entry.abandoned && entry.order == NativeScopeOrder::StrictLifo);
    }

    fn assert_usable(&self) {
        assert!(
            !self.pending_recovery,
            "native scope order was violated; use closure-based scopes or finish the remaining native tokens before calling Dear ImGui again"
        );
    }

    fn resolve_scope(&self, raw_scope: ScopeSnapshot) -> ScopeSnapshot {
        let Some(raw_window) = raw_scope.window else {
            return raw_scope;
        };
        let owner = self
            .entries
            .values()
            .filter(|entry| entry.pop.ends_window())
            .filter_map(|entry| {
                entry
                    .scope
                    .window
                    .filter(|window| window.has_same_native_position(raw_window))
                    .map(|_| entry.id)
            })
            .max();
        raw_scope.with_window_owner(owner)
    }
}

#[derive(Debug)]
pub(crate) struct NativeScopeToken<'ui> {
    ui: &'ui Ui,
    keys: NativeScopeKeys,
    id: u64,
    label: &'static str,
    pop: NativeScopePop,
    finished: bool,
}

impl<'ui> NativeScopeToken<'ui> {
    pub(crate) fn new(ui: &'ui Ui, pop: NativeScopePop, label: &'static str) -> Self {
        Self::new_with_order(ui, pop, label, NativeScopeOrder::StrictLifo)
    }

    fn new_with_order(
        ui: &'ui Ui,
        pop: NativeScopePop,
        label: &'static str,
        order: NativeScopeOrder,
    ) -> Self {
        let binding = ui.ctx_binding.clone();
        let (keys, id) = binding.with_bound_context(|| {
            let scope = unsafe { ScopeSnapshot::current_raw(ui) };
            ui.native_scopes
                .borrow_mut()
                .begin(pop, label, order, scope)
        });
        Self {
            ui,
            keys,
            id,
            label,
            pop,
            finished: false,
        }
    }

    pub(crate) fn finish_with(&mut self, current_pop: impl FnOnce()) {
        if self.finished {
            return;
        }
        self.finished = true;
        let binding = self.ui.ctx_binding.clone();
        let _ = binding.try_with_bound_context(|| {
            let current = unsafe { ScopeSnapshot::current_raw(self.ui) };
            let should_pop = match self
                .ui
                .native_scopes
                .borrow_mut()
                .prepare_pop(self.keys, self.id, self.label, current)
            {
                Ok(should_pop) => should_pop,
                Err(error) => {
                    if !std::thread::panicking() {
                        panic!("native scope order violation: {error}");
                    }
                    return;
                }
            };
            if !should_pop {
                return;
            }

            current_pop();
            loop {
                let current = unsafe { ScopeSnapshot::current_raw(self.ui) };
                let Some(pop) = self
                    .ui
                    .native_scopes
                    .borrow_mut()
                    .take_ready_abandoned(current)
                else {
                    break;
                };
                unsafe { pop.run() };
            }
        });
    }

    pub(crate) fn replace_pop(&mut self, pop: NativeScopePop) {
        assert!(
            !self.finished,
            "cannot update a finished native scope token"
        );
        self.ui
            .native_scopes
            .borrow_mut()
            .replace_pop(self.id, self.keys, pop);
        self.pop = pop;
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.finished
    }

    pub(crate) fn finish(&mut self) {
        let pop = self.pop;
        self.finish_with(|| unsafe { pop.run() });
    }
}

impl Ui {
    pub(crate) fn begin_native_scope(
        &self,
        pop: NativeScopePop,
        label: &'static str,
    ) -> NativeScopeToken<'_> {
        NativeScopeToken::new(self, pop, label)
    }

    pub(crate) fn begin_provenance_native_scope(
        &self,
        pop: NativeScopePop,
        label: &'static str,
    ) -> NativeScopeToken<'_> {
        NativeScopeToken::new_with_order(self, pop, label, NativeScopeOrder::ProvenanceOnly)
    }

    pub(crate) fn assert_native_scopes_usable(&self) {
        self.native_scopes.borrow().assert_usable();
    }

    pub(crate) fn native_scope_recovery_pending(&self) -> bool {
        self.native_scopes.borrow().pending_recovery
    }

    pub(crate) fn assert_no_active_table_channel(&self, operation: &'static str) {
        let current = unsafe { ScopeSnapshot::current_raw(self) };
        self.native_scopes
            .borrow()
            .assert_no_table_channel(current, operation);
    }

    pub(crate) fn current_native_scope(&self) -> ScopeSnapshot {
        let current = unsafe { ScopeSnapshot::current_raw(self) };
        self.native_scopes.borrow().resolve_scope(current)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ending_a_top_level_window_does_not_mark_the_fallback_window_written() {
        let mut ctx = crate::Context::create();
        ctx.io_mut().set_display_size([128.0, 128.0]);
        ctx.io_mut().set_delta_time(1.0 / 60.0);
        ctx.font_atlas()
            .try_claim_legacy_renderer()
            .expect("legacy renderer font atlas should be available")
            .build();

        let ui = ctx.frame();
        ui.window("Regression").build(|| {});

        let fallback = unsafe { crate::sys::igGetCurrentWindowRead() };
        assert!(!fallback.is_null());
        assert!(
            !unsafe { (*fallback).WriteAccessed },
            "ending a top-level window must not mark Dear ImGui's implicit \
             fallback window as written-to"
        );
    }
}
