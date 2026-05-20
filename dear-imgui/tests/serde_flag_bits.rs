#![cfg(feature = "serde")]

use dear_imgui_rs as imgui;
use serde::de::{
    DeserializeOwned,
    value::{Error as ValueError, I32Deserializer},
};

fn deserialize_i32<T: DeserializeOwned>(bits: i32) -> T {
    T::deserialize(I32Deserializer::<ValueError>::new(bits)).unwrap()
}

fn first_unknown_bit(known_bits: i32) -> i32 {
    (0..31)
        .map(|shift| 1_i32 << shift)
        .find(|candidate| known_bits & candidate == 0)
        .expect("test requires at least one spare positive flag bit")
}

macro_rules! assert_deserializes_without_truncating_unknown_bits {
    ($flag:ty) => {{
        let unknown = first_unknown_bit(<$flag>::all().bits());
        let bits = <$flag>::all().bits() | unknown;
        let flags: $flag = deserialize_i32(bits);
        assert_eq!(flags.bits(), bits);
    }};
}

#[test]
fn serde_keeps_unknown_bits_for_public_flag_wrappers() {
    assert_deserializes_without_truncating_unknown_bits!(imgui::ConfigFlags);
    assert_deserializes_without_truncating_unknown_bits!(imgui::BackendFlags);
    assert_deserializes_without_truncating_unknown_bits!(imgui::ViewportFlags);
    assert_deserializes_without_truncating_unknown_bits!(imgui::WindowFlags);
    assert_deserializes_without_truncating_unknown_bits!(imgui::TableFlags);
    assert_deserializes_without_truncating_unknown_bits!(imgui::TableColumnFlags);
    assert_deserializes_without_truncating_unknown_bits!(imgui::TableColumnStateFlags);
    assert_deserializes_without_truncating_unknown_bits!(imgui::TableRowFlags);
}
