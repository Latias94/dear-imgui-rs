use crate::InputTextFlags;

pub(in crate::widget::input) fn validate_input_text_flags(
    caller: &str,
    flags: InputTextFlags,
    multiline: bool,
) {
    let unsupported = flags.bits() & !InputTextFlags::all().bits();
    assert!(
        unsupported == 0,
        "{caller} received unsupported ImGuiInputTextFlags bits: 0x{unsupported:X}"
    );
    assert!(
        !flags.contains(InputTextFlags::CALLBACK_COMPLETION)
            || !flags.contains(InputTextFlags::ALLOW_TAB_INPUT),
        "{caller} cannot combine CALLBACK_COMPLETION with ALLOW_TAB_INPUT"
    );
    assert!(
        !flags.contains(InputTextFlags::WORD_WRAP) || !flags.contains(InputTextFlags::PASSWORD),
        "{caller} cannot combine WORD_WRAP with PASSWORD"
    );
    if multiline {
        assert!(
            !flags.contains(InputTextFlags::CALLBACK_HISTORY),
            "{caller} cannot combine CALLBACK_HISTORY with multiline inputs"
        );
        assert!(
            !flags.contains(InputTextFlags::ELIDE_LEFT),
            "{caller} cannot combine ELIDE_LEFT with multiline inputs"
        );
    } else {
        assert!(
            !flags.contains(InputTextFlags::WORD_WRAP),
            "{caller} cannot use WORD_WRAP with single-line inputs"
        );
    }
}

pub(in crate::widget::input) fn validate_input_scalar_flags(caller: &str, flags: InputTextFlags) {
    validate_input_text_flags(caller, flags, false);
    assert!(
        !flags.contains(InputTextFlags::ENTER_RETURNS_TRUE),
        "{caller} does not support ENTER_RETURNS_TRUE"
    );
    let callback_flags = InputTextFlags::CALLBACK_COMPLETION
        | InputTextFlags::CALLBACK_HISTORY
        | InputTextFlags::CALLBACK_ALWAYS
        | InputTextFlags::CALLBACK_CHAR_FILTER
        | InputTextFlags::CALLBACK_RESIZE
        | InputTextFlags::CALLBACK_EDIT;
    assert!(
        !flags.intersects(callback_flags),
        "{caller} does not support input text callback flags"
    );
}
