//! Raw Cocoa + Metal integration. Run with features backend-shim-osx,backend-shim-metal.
//! Objective-C objects stay owned by this application; shims borrow their pointers.

#[cfg(target_os = "macos")]
fn main() {
    use dear_imgui_sys::{
        self as sys,
        backend_shim::{metal, osx},
    };
    use objc2::{
        class, msg_send,
        rc::{Retained, autoreleasepool},
        runtime::{AnyObject, Bool},
    };
    use objc2_foundation::{NSDefaultRunLoopMode, NSPoint, NSRect, NSSize, NSString};
    use objc2_metal::MTLClearColor;
    use std::{ffi::c_void, ptr};

    // These classes are used dynamically below. The link attributes also make their Objective-C
    // class definitions available to the runtime, without adding a UI framework to the sys API.
    #[link(name = "Cocoa", kind = "framework")]
    unsafe extern "C" {}
    #[link(name = "QuartzCore", kind = "framework")]
    unsafe extern "C" {}
    #[link(name = "Metal", kind = "framework")]
    unsafe extern "C" {
        fn MTLCreateSystemDefaultDevice() -> *mut AnyObject;
    }
    fn raw(object: &AnyObject) -> *mut c_void {
        ptr::from_ref(object).cast_mut().cast()
    }
    struct Context {
        raw: *mut sys::ImGuiContext,
        platform: bool,
        renderer: bool,
    }
    impl Drop for Context {
        fn drop(&mut self) {
            // SAFETY: application/native objects outlive this guard; command buffers are waited
            // after submission, before the guard can release renderer/platform resources.
            unsafe {
                sys::igSetCurrentContext(self.raw);
                if self.renderer {
                    metal::dear_imgui_backend_metal_shutdown();
                }
                if self.platform {
                    osx::dear_imgui_backend_osx_shutdown();
                }
                sys::igDestroyContext(self.raw);
            }
        }
    }

    assert!(
        objc2::MainThreadMarker::new().is_some(),
        "Cocoa requires the main thread"
    );
    autoreleasepool(|_| {
        // SAFETY: selectors below follow the platform SDK signatures; retained objects are kept
        // alive until backend teardown. The entire Cocoa/Metal loop runs on the main thread.
        unsafe {
            let app: Retained<AnyObject> = msg_send![class!(NSApplication), sharedApplication];
            let _: Bool = msg_send![&app, setActivationPolicy: 0isize];
            let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(960.0, 640.0));
            let allocated: objc2::rc::Allocated<AnyObject> = msg_send![class!(NSWindow), alloc];
            let window: Retained<AnyObject> = msg_send![allocated,
                initWithContentRect: rect, styleMask: 1usize | 2 | 4 | 8, backing: 2usize, defer: Bool::NO];
            let _: () = msg_send![&window, setReleasedWhenClosed: Bool::NO];
            let title = NSString::from_str("Dear ImGui - raw Cocoa / Metal shims");
            let _: () = msg_send![&window, setTitle: &*title];
            let _: () = msg_send![&window, center];
            let view: Retained<AnyObject> = msg_send![&window, contentView];
            let device = Retained::from_raw(MTLCreateSystemDefaultDevice())
                .expect("Metal device unavailable");
            let queue: Retained<AnyObject> = msg_send![&device, newCommandQueue];
            let layer: Retained<AnyObject> = msg_send![class!(CAMetalLayer), layer];
            let _: () = msg_send![&layer, setDevice: &*device];
            let _: () = msg_send![&layer, setPixelFormat: 80usize]; // MTLPixelFormatBGRA8Unorm
            let _: () = msg_send![&view, setWantsLayer: Bool::YES];
            let _: () = msg_send![&view, setLayer: &*layer];
            let _: () = msg_send![&window, makeKeyAndOrderFront: ptr::null::<AnyObject>()];
            let _: () = msg_send![&app, finishLaunching];
            let _: () = msg_send![&app, activateIgnoringOtherApps: Bool::YES];

            let context_raw = sys::igCreateContext(ptr::null_mut());
            assert!(!context_raw.is_null());
            let mut context = Context {
                raw: context_raw,
                platform: false,
                renderer: false,
            };
            (*sys::igGetIO_Nil()).IniFilename = ptr::null();
            (*sys::igGetIO_Nil()).ConfigFlags |= sys::ImGuiConfigFlags_NavEnableKeyboard;
            assert!(osx::dear_imgui_backend_osx_init(raw(&view)));
            context.platform = true;
            assert!(metal::dear_imgui_backend_metal_init(raw(&device)));
            context.renderer = true;
            let limit = std::env::args().find_map(|arg| {
                arg.strip_prefix("--frames=")
                    .and_then(|value| value.parse::<u32>().ok())
            });
            let mut frames = 0;
            loop {
                let visible: Bool = msg_send![&window, isVisible];
                if !visible.as_bool() {
                    break;
                }
                autoreleasepool(|_| {
                    let date: Retained<AnyObject> = msg_send![class!(NSDate), distantPast];
                    loop {
                        let event: Option<Retained<AnyObject>> = msg_send![&app,
                            nextEventMatchingMask: usize::MAX, untilDate: &*date,
                            inMode: NSDefaultRunLoopMode, dequeue: Bool::YES];
                        let Some(event) = event else {
                            break;
                        };
                        let _: () = msg_send![&app, sendEvent: &*event];
                    }
                    let _: () = msg_send![&app, updateWindows];
                    let bounds: NSRect = msg_send![&view, bounds];
                    let scale: f64 = msg_send![&window, backingScaleFactor];
                    let _: () = msg_send![&layer, setDrawableSize:
                        NSSize::new(bounds.size.width * scale, bounds.size.height * scale)];
                    let drawable: Option<Retained<AnyObject>> = msg_send![&layer, nextDrawable];
                    let Some(drawable) = drawable else {
                        return;
                    };
                    let texture: Retained<AnyObject> = msg_send![&drawable, texture];
                    let pass: Retained<AnyObject> =
                        msg_send![class!(MTLRenderPassDescriptor), renderPassDescriptor];
                    let attachments: Retained<AnyObject> = msg_send![&pass, colorAttachments];
                    let attachment: Retained<AnyObject> =
                        msg_send![&attachments, objectAtIndexedSubscript: 0usize];
                    let _: () = msg_send![&attachment, setTexture: &*texture];
                    let _: () = msg_send![&attachment, setLoadAction: 2usize]; // MTLLoadActionClear
                    let _: () = msg_send![&attachment, setStoreAction: 1usize]; // MTLStoreActionStore
                    let _: () = msg_send![&attachment, setClearColor: MTLClearColor { red: 0.08, green: 0.1, blue: 0.14, alpha: 1.0 }];
                    osx::dear_imgui_backend_osx_new_frame(raw(&view));
                    metal::dear_imgui_backend_metal_new_frame(raw(&pass));
                    sys::igNewFrame();
                    if sys::igBegin(c"Native backend shims".as_ptr(), ptr::null_mut(), 0) {
                        sys::igTextUnformatted(
                            c"The application owns Cocoa, Metal, resize, and submission.".as_ptr(),
                            ptr::null(),
                        );
                        sys::igButton(c"Click me".as_ptr(), sys::ImVec2_c { x: 0.0, y: 0.0 });
                    }
                    sys::igEnd();
                    sys::igRender();
                    let buffer: Retained<AnyObject> = msg_send![&queue, commandBuffer];
                    let encoder: Retained<AnyObject> =
                        msg_send![&buffer, renderCommandEncoderWithDescriptor: &*pass];
                    metal::dear_imgui_backend_metal_render_draw_data(
                        sys::igGetDrawData(),
                        raw(&buffer),
                        raw(&encoder),
                    );
                    let _: () = msg_send![&encoder, endEncoding];
                    let _: () = msg_send![&buffer, presentDrawable: &*drawable];
                    let _: () = msg_send![&buffer, commit];
                    // Deliberately serial for a small example; production apps use an in-flight ring.
                    let _: () = msg_send![&buffer, waitUntilCompleted];
                    frames += 1;
                });
                if limit.is_some_and(|limit| frames >= limit) {
                    break;
                }
            }
            drop(context);
            let _: () = msg_send![&window, close];
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("cocoa_metal requires macOS");
}
