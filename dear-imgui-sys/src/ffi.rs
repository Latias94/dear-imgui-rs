// This module only exists to include the generated bindings at the module root,
// which allows inner attributes (#![allow(...)] emitted by bindgen) to be valid.
#![allow(unsafe_op_in_unsafe_fn)]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
