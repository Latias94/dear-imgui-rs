#[inline]
pub(crate) fn assert_finite_f32(caller: &str, name: &str, value: f32) {
    assert!(value.is_finite(), "{caller} {name} must be finite");
}

#[inline]
pub(crate) fn assert_finite_vec2(caller: &str, name: &str, [x, y]: [f32; 2]) {
    assert!(
        x.is_finite() && y.is_finite(),
        "{caller} {name} must contain finite values"
    );
}

#[inline]
pub(crate) fn assert_finite_vec4(caller: &str, name: &str, value: [f32; 4]) {
    assert!(
        value.iter().all(|component| component.is_finite()),
        "{caller} {name} must contain finite values"
    );
}

#[cfg(feature = "serde")]
macro_rules! impl_serde_i32_flags {
    ($($flags:ty),+ $(,)?) => {
        $(
            impl serde::Serialize for $flags {
                fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                    serializer.serialize_i32(self.bits())
                }
            }

            impl<'de> serde::Deserialize<'de> for $flags {
                fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                    let bits = <i32 as serde::Deserialize>::deserialize(deserializer)?;
                    Self::from_bits(bits).ok_or_else(|| {
                        serde::de::Error::custom(format_args!(
                            "{} has unsupported bits: 0x{:08X}",
                            stringify!($flags),
                            bits & !Self::all().bits(),
                        ))
                    })
                }
            }
        )+
    };
}

#[cfg(feature = "serde")]
pub(crate) use impl_serde_i32_flags;
