//! Minimal runner scaffold for future shared utilities.
//! Not used yet by the single-file examples; intended for gradual adoption.

#![allow(dead_code)]

use dear_imgui_rs::Context;
use dear_imgui_winit::WinitPlatform;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::WindowId,
};

pub trait ExampleApp {
    fn new(event_loop: &ActiveEventLoop) -> anyhow::Result<Self>
    where
        Self: Sized;
    fn resize(&mut self, _new_size: winit::dpi::PhysicalSize<u32>) {}
    fn render(&mut self) -> anyhow::Result<()>;
}

pub fn run<E: ExampleApp + Default + 'static>() -> anyhow::Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = E::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}

pub struct ImguiBundle {
    pub ctx: Context,
    pub platform: WinitPlatform,
}

