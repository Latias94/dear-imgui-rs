use crate::{Id, sys};

pub(crate) fn assert_nonzero_id(caller: &str, name: &str, id: Id) {
    assert!(id.raw() != 0, "{caller} {name} must be non-zero");
}

pub(super) fn optional_nonzero_id_raw(caller: &str, name: &str, id: Option<Id>) -> sys::ImGuiID {
    id.map_or(0, |id| {
        assert_nonzero_id(caller, name, id);
        id.raw()
    })
}

pub(crate) fn assert_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert!(
        value[0].is_finite() && value[1].is_finite(),
        "{caller} {name} must contain finite values"
    );
}

pub(crate) fn assert_positive_finite_vec2(caller: &str, name: &str, value: [f32; 2]) {
    assert_finite_vec2(caller, name, value);
    assert!(
        value[0] > 0.0 && value[1] > 0.0,
        "{caller} {name} must contain positive values"
    );
}
