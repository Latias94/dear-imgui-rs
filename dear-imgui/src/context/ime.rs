use super::binding::CTX_MUTEX;
use crate::{Context, Id, sys};

/// Transient IME text owned by the active text widget, separate from its buffer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImePreedit {
    text: String,
    cursor_range: Option<(usize, usize)>,
    owner: ImeOwner,
}

impl ImePreedit {
    pub fn text(&self) -> &str {
        &self.text
    }

    /// UTF-8 byte offsets for the cursor or selected range; `None` hides the cursor.
    /// A reversed range is accepted and retains the cursor direction.
    pub fn cursor_range(&self) -> Option<(usize, usize)> {
        self.cursor_range
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ImePreeditError {
    #[error("IME cursor offsets must be UTF-8 boundaries within the preedit text")]
    InvalidCursorRange,
    #[error("IME preedit text must not contain NUL")]
    InteriorNul,
}

#[derive(Debug, Default)]
pub(crate) struct ImeState {
    pub(crate) preedit: Option<ImePreedit>,
    presentation: Option<InputPresentation>,
    session: ImeSession,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ImeOwner {
    widget: sys::ImGuiID,
    window: sys::ImGuiID,
    viewport: sys::ImGuiID,
}

impl ImeOwner {
    fn current(raw: &sys::ImGuiContext) -> Option<Self> {
        if raw.ActiveId == 0
            || raw.InputTextState.ID != raw.ActiveId
            || raw.ActiveIdWindow.is_null()
            || raw.InputTextState.Flags & sys::ImGuiInputTextFlags_ReadOnly != 0
        {
            return None;
        }
        let window = unsafe { &*raw.ActiveIdWindow };
        if window.Viewport.is_null() {
            return None;
        }
        Some(Self {
            widget: raw.ActiveId,
            window: window.ID,
            viewport: unsafe { (*window.Viewport)._ImGuiViewport.ID },
        })
    }
}

#[derive(Clone, Copy, Debug, Default)]
enum ImeSession {
    #[default]
    Idle,
    Composing(ImeOwner),
    Cancelled {
        owner: ImeOwner,
        reset_requested: bool,
    },
    Disabled,
}

impl ImeState {
    pub(crate) fn cancel(&mut self) {
        self.preedit = None;
        if let ImeSession::Composing(owner) = self.session {
            self.session = ImeSession::Cancelled {
                owner,
                reset_requested: true,
            };
        }
    }

    fn reconcile_owner(&mut self, current: Option<ImeOwner>) {
        if let ImeSession::Composing(owner) = self.session
            && current != Some(owner)
        {
            self.cancel();
        }
    }

    pub(crate) fn session_viewport(&self) -> Option<Id> {
        match self.session {
            ImeSession::Composing(owner) | ImeSession::Cancelled { owner, .. } => {
                Some(Id::from(owner.viewport))
            }
            ImeSession::Idle | ImeSession::Disabled => None,
        }
    }

    pub(crate) fn take_reset_request(&mut self) -> Option<Id> {
        if let ImeSession::Cancelled {
            owner,
            reset_requested,
        } = &mut self.session
            && std::mem::take(reset_requested)
        {
            Some(Id::from(owner.viewport))
        } else {
            None
        }
    }

    pub(crate) fn reset_session(&mut self) {
        self.preedit = None;
        self.session = ImeSession::Idle;
    }
}

#[derive(Clone, Copy, Debug)]
struct InputPresentation {
    frame: i32,
    owner: sys::ImGuiID,
    font: *mut sys::ImFont,
    size: f32,
    density: f32,
}

impl crate::Ui {
    // Capture inside the widget's font scope, never from the frame's final font.
    pub(crate) fn run_input_widget<R>(
        &self,
        label: *const std::ffi::c_char,
        f: impl FnOnce() -> R,
    ) -> R {
        self.run_with_bound_context(|| {
            // A clipped inactive widget may leave LastItemData unchanged.
            let owner = unsafe { sys::igGetID_Str(label) };
            let (font, size, density) = unsafe {
                let font = (*self.ctx).Font;
                (font, (*self.ctx).FontSize, (*font).CurrentRasterizerDensity)
            };
            let result = f();
            unsafe {
                let raw = &*self.ctx;
                if raw.ActiveId == owner
                    && raw.InputTextState.ID == owner
                    && raw.LastItemData.ID == owner
                {
                    self.ime.borrow_mut().presentation = Some(InputPresentation {
                        frame: raw.FrameCount,
                        owner: raw.ActiveId,
                        font,
                        size,
                        density,
                    });
                }
                self.ime
                    .borrow_mut()
                    .reconcile_owner(ImeOwner::current(raw));
            }
            result
        })
    }
}

impl Context {
    /// Replace the active text widget's composition without editing its buffer or undo history.
    ///
    /// Cursor offsets use UTF-8 bytes. Empty text hides the panel while retaining
    /// its owner until commit; `None` hides its cursor. Updates without an active
    /// editable text widget are discarded.
    /// The composition is drawn in a non-interactive foreground panel at the
    /// widget's caret, on its owning viewport. Password input masks its text.
    /// Core single-line, hint, and multiline text inputs capture their presentation;
    /// unsupported custom/native widgets discard composition.
    /// After cancellation or an owner change, a platform must report a new
    /// enabled session through [`Self::notify_ime_enabled`] before more updates.
    pub fn set_ime_preedit(
        &mut self,
        text: &str,
        cursor_range: Option<(usize, usize)>,
    ) -> Result<(), ImePreeditError> {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::set_ime_preedit()");
        if let Some((start, end)) = cursor_range
            && (!text.is_char_boundary(start) || !text.is_char_boundary(end))
        {
            return Err(ImePreeditError::InvalidCursorRange);
        }
        if text.contains('\0') {
            return Err(ImePreeditError::InteriorNul);
        }
        let raw = unsafe { &*self.as_raw() };
        let owner = ImeOwner::current(raw);
        let mut ime = self.state.ime.borrow_mut();
        ime.reconcile_owner(owner);
        if matches!(
            ime.session,
            ImeSession::Cancelled { .. } | ImeSession::Disabled
        ) {
            return Ok(());
        }
        if text.is_empty() {
            ime.preedit = None;
            return Ok(());
        }
        let Some(owner) = owner.filter(|owner| {
            ime.presentation
                .is_some_and(|p| p.owner == owner.widget && p.frame == raw.FrameCount)
        }) else {
            return Ok(());
        };
        ime.session = ImeSession::Composing(owner);
        ime.preedit = Some(ImePreedit {
            text: text.to_owned(),
            cursor_range,
            owner,
        });
        Ok(())
    }

    /// Return an owned snapshot, without borrowing state needed by platform teardown.
    pub fn ime_preedit(&self) -> Option<ImePreedit> {
        self.state.ime.borrow().preedit.clone()
    }

    /// Whether transient composition is active.
    pub fn has_ime_preedit(&self) -> bool {
        self.state.ime.borrow().preedit.is_some()
    }

    /// Whether a composition owns an input, including an empty pre-commit panel.
    pub fn has_ime_composition(&self) -> bool {
        matches!(self.state.ime.borrow().session, ImeSession::Composing(_))
    }

    /// Notify the Context that the platform enabled or disabled its IME session.
    ///
    /// This does not enable the native IME. Enabling starts a fresh session;
    /// disabling rejects preedit and commit events until the next enabled notice.
    pub fn notify_ime_enabled(&mut self, enabled: bool) {
        let mut ime = self.state.ime.borrow_mut();
        if enabled {
            ime.reset_session();
        } else {
            ime.cancel();
            if let ImeSession::Cancelled {
                reset_requested, ..
            } = &mut ime.session
            {
                *reset_requested = false;
            } else {
                ime.session = ImeSession::Disabled;
            }
        }
    }

    /// Commit IME text to its original input, returning whether characters were queued.
    ///
    /// Control characters are ignored. A cancelled session or a changed owner
    /// rejects the commit. Without preedit, native/custom editors requesting text
    /// input can still receive direct commits through the regular IO queue.
    pub fn commit_ime_text(&mut self, text: &str) -> bool {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::commit_ime_text()");
        let raw = unsafe { &*self.as_raw() };
        {
            let mut ime = self.state.ime.borrow_mut();
            ime.reconcile_owner(ImeOwner::current(raw));
            if matches!(
                ime.session,
                ImeSession::Cancelled { .. } | ImeSession::Disabled
            ) || !(raw.PlatformImeData.WantVisible || raw.PlatformImeData.WantTextInput)
            {
                return false;
            }
            ime.reset_session();
        }
        let mut forwarded = false;
        for ch in text.chars().filter(|ch| !ch.is_control()) {
            self.io_mut().add_input_character(ch);
            forwarded = true;
        }
        forwarded
    }

    /// Viewport whose composition is active or awaiting a native reset.
    pub fn ime_session_viewport(&self) -> Option<Id> {
        self.state.ime.borrow().session_viewport()
    }

    /// Cancel transient composition without submitting text.
    ///
    /// The platform must consume its reset request and report a fresh enabled
    /// session before new IME events are accepted.
    pub fn clear_ime_preedit(&mut self) {
        self.state.ime.borrow_mut().cancel();
    }

    /// Viewport currently requesting text input, for native-window event routing.
    pub fn text_input_viewport(&self) -> Option<Id> {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::text_input_viewport()");
        let raw = unsafe { &*self.as_raw() };
        (raw.PlatformImeData.WantVisible || raw.PlatformImeData.WantTextInput)
            .then(|| Id::from(raw.PlatformImeData.ViewportId))
    }

    pub(crate) fn render_ime_preedit(&mut self) {
        let raw = unsafe { &*self.as_raw() };
        {
            let mut state = self.state.ime.borrow_mut();
            // Widget submission may start with inactive inputs. Only the frame
            // boundary can require the owner to have submitted its IME data.
            let current = ImeOwner::current(raw).filter(|owner| {
                raw.PlatformImeData.WantVisible
                    && state
                        .presentation
                        .is_some_and(|p| p.owner == owner.widget && p.frame == raw.FrameCount)
            });
            state.reconcile_owner(current);
        }
        // Do not hold RefCell borrows across font loading or native callbacks.
        let (preedit, presentation) = {
            let state = self.state.ime.borrow();
            let Some(preedit) = state.preedit.clone() else {
                return;
            };
            (preedit, state.presentation)
        };
        // Rendering runs under the Context lock, before native EndFrame/Render.
        let ime = raw.PlatformImeData;
        if raw.IO.AppFocusLost
            || ImeOwner::current(raw) != Some(preedit.owner)
            || !ime.WantVisible
            || !presentation
                .is_some_and(|p| p.owner == preedit.owner.widget && p.frame == raw.FrameCount)
        {
            self.clear_ime_preedit();
            return;
        }
        let viewport = unsafe { sys::igFindViewportByID(ime.ViewportId) };
        let presentation = presentation.expect("validated current-frame input presentation");
        let size = presentation.size;
        if viewport.is_null()
            || presentation.font.is_null()
            || !presentation.density.is_finite()
            || presentation.density <= 0.0
            || !size.is_finite()
            || size <= 0.0
            || !ime.InputPos.x.is_finite()
            || !ime.InputPos.y.is_finite()
        {
            return;
        }
        let password = raw.InputTextState.Flags & sys::ImGuiInputTextFlags_Password != 0;
        let masked;
        let (text, cursor) = if password {
            masked = "*".repeat(preedit.text.chars().count());
            (
                masked.as_str(),
                preedit.cursor_range.map(|(a, b)| {
                    (
                        preedit.text[..a].chars().count(),
                        preedit.text[..b].chars().count(),
                    )
                }),
            )
        } else {
            (preedit.text.as_str(), preedit.cursor_range)
        };
        // Keep geometry transient: no InputText callbacks, text events, or undo edits.
        unsafe {
            let font = presentation.font;
            let previous_density = (*font).CurrentRasterizerDensity;
            (*font).CurrentRasterizerDensity = presentation.density;
            draw_preedit(font, size, viewport, ime.InputPos, text, cursor);
            (*font).CurrentRasterizerDensity = previous_density;
        }
    }
}

unsafe fn draw_preedit(
    font: *mut sys::ImFont,
    size: f32,
    viewport: *mut sys::ImGuiViewport,
    anchor: sys::ImVec2_c,
    text: &str,
    cursor: Option<(usize, usize)>,
) {
    // All pointers belong to the currently bound, live Context and frame.
    unsafe {
        let measure = |text: &str| {
            sys::ImFont_CalcTextSizeA(
                font,
                size,
                f32::MAX,
                0.0,
                text.as_ptr().cast(),
                text.as_ptr().add(text.len()).cast(),
                std::ptr::null_mut(),
            )
        };
        let offset = |byte: usize| {
            let prefix = &text[..byte];
            let line = prefix.rsplit('\n').next().unwrap_or("");
            sys::ImVec2_c {
                x: measure(line).x,
                y: size * prefix.bytes().filter(|&b| b == b'\n').count() as f32,
            }
        };
        let extent = measure(text);
        let vp = &*viewport;
        if !extent.x.is_finite() || !extent.y.is_finite() || vp.Size.x <= 0.0 || vp.Size.y <= 0.0 {
            return;
        }
        let padding = 3.0;
        let width = (extent.x + padding * 2.0).min(vp.Size.x);
        let height = (extent.y.max(size) + padding * 2.0).min(vp.Size.y);
        let left = anchor.x.max(vp.Pos.x).min(vp.Pos.x + vp.Size.x - width);
        let top = anchor.y.max(vp.Pos.y).min(vp.Pos.y + vp.Size.y - height);
        let caret = cursor.map(|(_, end)| offset(end));
        let scroll_x = caret.map_or(0.0, |caret| (caret.x + padding * 2.0 - width).max(0.0));
        let scroll_y = caret.map_or(0.0, |caret| {
            (caret.y + size + padding * 2.0 - height).max(0.0)
        });
        let origin = sys::ImVec2_c {
            x: left + padding - scroll_x,
            y: top + padding - scroll_y,
        };
        let min = sys::ImVec2_c { x: left, y: top };
        let max = sys::ImVec2_c {
            x: left + width,
            y: top + height,
        };
        let draw = sys::igGetForegroundDrawList_ViewportPtr(viewport);
        sys::ImDrawList_PushClipRect(draw, min, max, true);
        sys::ImDrawList_AddRectFilled(
            draw,
            min,
            max,
            sys::igGetColorU32_Col(sys::ImGuiCol_PopupBg, 1.0),
            0.0,
            0,
        );
        if let Some((a, b)) = cursor {
            let start = a.min(b);
            let end = a.max(b);
            let mut byte = 0;
            for line in text.split_inclusive('\n') {
                let line_end = byte + line.len();
                if start != end && start < line_end && end > byte {
                    let from = offset(start.max(byte));
                    let to = offset(end.min(line_end - usize::from(line.ends_with('\n'))));
                    sys::ImDrawList_AddRectFilled(
                        draw,
                        sys::ImVec2_c {
                            x: origin.x + from.x,
                            y: origin.y + from.y,
                        },
                        sys::ImVec2_c {
                            x: origin.x + to.x.max(from.x + 1.0),
                            y: origin.y + from.y + size,
                        },
                        sys::igGetColorU32_Col(sys::ImGuiCol_TextSelectedBg, 1.0),
                        0.0,
                        0,
                    );
                }
                byte = line_end;
            }
        }
        let color = sys::igGetColorU32_Col(sys::ImGuiCol_Text, 1.0);
        sys::ImDrawList_AddText_FontPtr(
            draw,
            font,
            size,
            origin,
            color,
            text.as_ptr().cast(),
            text.as_ptr().add(text.len()).cast(),
            0.0,
            std::ptr::null(),
        );
        sys::ImDrawList_AddLine(
            draw,
            sys::ImVec2_c {
                x: origin.x,
                y: origin.y + extent.y.max(size),
            },
            sys::ImVec2_c {
                x: origin.x + extent.x,
                y: origin.y + extent.y.max(size),
            },
            color,
            1.0,
            0,
        );
        if let Some(caret) = caret {
            let position = sys::ImVec2_c {
                x: origin.x + caret.x,
                y: origin.y + caret.y,
            };
            sys::ImDrawList_AddLine(
                draw,
                position,
                sys::ImVec2_c {
                    x: position.x,
                    y: position.y + size,
                },
                color,
                1.0,
                0,
            );
            (*sys::igGetCurrentContext()).PlatformImeData.InputPos = position;
        }
        sys::ImDrawList_PopClipRect(draw);
    }
}
