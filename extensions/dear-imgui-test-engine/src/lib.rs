//! Dear ImGui Test Engine bindings for `dear-imgui-rs`.
//!
//! This crate wraps `dear-imgui-test-engine-sys` with a small safe API for
//! engine lifetime management and per-frame UI integration.

mod attachment;
mod config;
mod counts;
mod engine;
mod error;
mod results;
mod runner;
mod script;
mod state;

#[cfg(test)]
mod tests;

pub use config::{InputMode, RunFlags, RunSpeed, TestGroup, VerboseLevel};
pub use counts::{ScriptCount, ScriptLimit};
pub use engine::TestEngine;
pub use error::{TestEngineError, TestEngineResult, TestEngineStatus};
pub use results::{ResultSummary, RunOutcome, RunReport};
pub use runner::{RunnerCallbackStage, RunnerControl, RunnerError, TestRunner};
pub use script::ScriptTest;
pub use state::{AttachmentState, RunState};

pub use dear_imgui_test_engine_sys as raw;

pub(crate) use script::Script;
