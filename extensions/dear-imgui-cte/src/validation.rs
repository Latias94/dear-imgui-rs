use crate::{CteError, CteResult};
use dear_imgui_rs::{ChildFlags, WindowFlags};
use std::time::Duration;

pub(crate) fn validate_finite_vec2(
    operation: &'static str,
    parameter: &'static str,
    value: [f32; 2],
) -> CteResult<()> {
    if !value.into_iter().all(f32::is_finite) {
        return Err(CteError::NonFinite {
            operation,
            parameter,
        });
    }
    Ok(())
}

pub(crate) fn validate_finite_f32(
    operation: &'static str,
    parameter: &'static str,
    value: f32,
) -> CteResult<()> {
    if !value.is_finite() {
        return Err(CteError::NonFinite {
            operation,
            parameter,
        });
    }
    Ok(())
}

pub(crate) fn validate_nonzero_usize(
    operation: &'static str,
    parameter: &'static str,
    value: usize,
) -> CteResult<()> {
    if value == 0 {
        return Err(CteError::InvalidValue {
            operation,
            parameter,
            requirement: "greater than zero",
        });
    }
    Ok(())
}

pub(crate) fn validate_render_flags(
    operation: &'static str,
    child_flags: ChildFlags,
    window_flags: WindowFlags,
) -> CteResult<()> {
    if !ChildFlags::all().contains(child_flags) {
        return Err(CteError::InvalidValue {
            operation,
            parameter: "child_flags",
            requirement: "a supported ChildFlags combination",
        });
    }
    if !WindowFlags::all().contains(window_flags) {
        return Err(CteError::InvalidValue {
            operation,
            parameter: "window_flags",
            requirement: "a supported WindowFlags combination",
        });
    }
    Ok(())
}

pub(crate) fn duration_millis_i32(
    operation: &'static str,
    parameter: &'static str,
    value: Duration,
) -> CteResult<i32> {
    i32::try_from(value.as_millis()).map_err(|_| CteError::InvalidValue {
        operation,
        parameter,
        requirement: "at most i32::MAX milliseconds",
    })
}
