use crate::{InputScalarFlags, InputTextFlags, InputTextMultilineFlags};

fn assert_no_unsupported_bits(caller: &str, ty: &str, bits: i32, all: i32) {
    let unsupported = bits & !all;
    assert!(
        unsupported == 0,
        "{caller} received unsupported {ty} bits: 0x{unsupported:X}"
    );
}

pub(in crate::widget::input) fn validate_input_text_flags(caller: &str, flags: InputTextFlags) {
    assert_no_unsupported_bits(
        caller,
        "InputTextFlags",
        flags.bits(),
        InputTextFlags::all().bits(),
    );
    assert!(
        !flags.contains(InputTextFlags::CALLBACK_COMPLETION)
            || !flags.contains(InputTextFlags::ALLOW_TAB_INPUT),
        "{caller} cannot combine CALLBACK_COMPLETION with ALLOW_TAB_INPUT"
    );
}

pub(in crate::widget::input) fn validate_input_multiline_flags(
    caller: &str,
    flags: InputTextMultilineFlags,
) {
    assert_no_unsupported_bits(
        caller,
        "InputTextMultilineFlags",
        flags.bits(),
        InputTextMultilineFlags::all().bits(),
    );
}

pub(in crate::widget::input) fn validate_input_scalar_flags(caller: &str, flags: InputScalarFlags) {
    assert_no_unsupported_bits(
        caller,
        "InputScalarFlags",
        flags.bits(),
        InputScalarFlags::all().bits(),
    );
}
