use super::DragFlags;

pub(super) fn validate_drag_flags(caller: &str, flags: DragFlags) {
    let unsupported = flags.bits() & !DragFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiSliderFlags bits: 0x{unsupported:X}"
    );
}
