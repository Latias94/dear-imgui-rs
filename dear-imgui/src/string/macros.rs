/// Creates an ImString from a string literal at compile time
#[macro_export]
macro_rules! im_str {
    ($e:expr) => {{ $crate::ImString::new($e) }};
}
