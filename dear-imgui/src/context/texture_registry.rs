use std::cell::RefCell;

use crate::sys;

use super::binding::{CTX_MUTEX, with_bound_context};
use super::{Context, ContextBinding, ContextLifecycle};

#[derive(Clone)]
struct UserTextureRegistration {
    ctx: *mut sys::ImGuiContext,
    tex: *mut sys::ImTextureData,
    binding: ContextBinding,
}

thread_local! {
    static USER_TEXTURE_REGISTRATIONS: RefCell<Vec<UserTextureRegistration>> = RefCell::new(Vec::new());
}

fn prune_dead_user_texture_registrations(registrations: &mut Vec<UserTextureRegistration>) {
    registrations.retain(|registration| {
        registration.binding.lifecycle() != ContextLifecycle::NativeDestroyed
    });
}

fn is_user_texture_registered(ctx: *mut sys::ImGuiContext, tex: *mut sys::ImTextureData) -> bool {
    USER_TEXTURE_REGISTRATIONS.with(|registrations| {
        let mut registrations = registrations.borrow_mut();
        prune_dead_user_texture_registrations(&mut registrations);
        registrations
            .iter()
            .any(|registration| registration.ctx == ctx && registration.tex == tex)
    })
}

fn track_user_texture_registration(
    ctx: *mut sys::ImGuiContext,
    tex: *mut sys::ImTextureData,
    binding: ContextBinding,
) {
    USER_TEXTURE_REGISTRATIONS.with(|registrations| {
        let mut registrations = registrations.borrow_mut();
        prune_dead_user_texture_registrations(&mut registrations);
        registrations.push(UserTextureRegistration { ctx, tex, binding });
    });
}

fn take_user_texture_registration(
    ctx: *mut sys::ImGuiContext,
    tex: *mut sys::ImTextureData,
) -> Option<UserTextureRegistration> {
    USER_TEXTURE_REGISTRATIONS.with(|registrations| {
        let mut registrations = registrations.borrow_mut();
        prune_dead_user_texture_registrations(&mut registrations);
        registrations
            .iter()
            .position(|registration| registration.ctx == ctx && registration.tex == tex)
            .map(|index| registrations.remove(index))
    })
}

fn unregister_user_texture_registration(registration: UserTextureRegistration) {
    if registration.ctx.is_null() || registration.tex.is_null() {
        return;
    }

    match registration.binding.lifecycle() {
        ContextLifecycle::Alive => {
            let tex = registration.tex;
            let _ = registration
                .binding
                .try_with_bound_context(|| unsafe { sys::igUnregisterUserTexture(tex) });
        }
        ContextLifecycle::Dropping => {
            unregister_user_texture_registration_during_teardown(registration);
        }
        ContextLifecycle::NativeDestroyed => {}
    }
}

fn unregister_user_texture_registration_during_teardown(registration: UserTextureRegistration) {
    if registration.ctx.is_null() || registration.tex.is_null() {
        return;
    }

    unsafe {
        with_bound_context(registration.ctx, || {
            sys::igUnregisterUserTexture(registration.tex);
        });
    }
}

pub(crate) fn unregister_user_texture_from_all_contexts(tex: *mut sys::ImTextureData) {
    if tex.is_null() {
        return;
    }

    let registrations = USER_TEXTURE_REGISTRATIONS.with(|registrations| {
        let mut registrations = registrations.borrow_mut();
        let mut taken = Vec::new();
        let mut index = 0;
        while index < registrations.len() {
            if registrations[index].binding.lifecycle() == ContextLifecycle::NativeDestroyed {
                registrations.remove(index);
            } else if registrations[index].tex == tex {
                taken.push(registrations.remove(index));
            } else {
                index += 1;
            }
        }
        taken
    });

    let _guard = CTX_MUTEX.lock();
    for registration in registrations {
        unregister_user_texture_registration(registration);
    }
}

pub(super) fn unregister_user_textures_for_context(ctx: *mut sys::ImGuiContext) {
    if ctx.is_null() {
        return;
    }

    let registrations = USER_TEXTURE_REGISTRATIONS.with(|registrations| {
        let mut registrations = registrations.borrow_mut();
        let mut taken = Vec::new();
        let mut index = 0;
        while index < registrations.len() {
            if registrations[index].binding.lifecycle() == ContextLifecycle::NativeDestroyed
                || registrations[index].ctx == ctx
            {
                let registration = registrations.remove(index);
                if registration.ctx == ctx {
                    taken.push(registration);
                }
            } else {
                index += 1;
            }
        }
        taken
    });

    for registration in registrations {
        unregister_user_texture_registration_during_teardown(registration);
    }
}

impl Context {
    /// Register a user-created texture in ImGui's global texture list (ImGui 1.92+).
    ///
    /// Dear ImGui builds `DrawData::textures()` from its internal `PlatformIO.Textures[]` list.
    /// If you create an `OwnedTextureData` yourself, you must register
    /// it for renderer backends (with `BackendFlags::RENDERER_HAS_TEXTURES`) to receive
    /// Create/Update/Destroy requests automatically.
    ///
    /// Note: `RegisterUserTexture()` is currently an experimental ImGui API.
    ///
    /// The registration is tracked by this crate and will be removed automatically when the
    /// `Context` or the `OwnedTextureData` is dropped.
    pub fn register_user_texture(&mut self, texture: &mut crate::texture::OwnedTextureData) {
        self.register_user_texture_ptr(texture.as_mut().as_raw_mut());
    }

    /// Register a borrowed/raw texture data pointer in ImGui's global texture list.
    ///
    /// Prefer [`Context::register_user_texture`] for `OwnedTextureData`.
    ///
    /// # Safety
    /// The caller must guarantee that `texture` remains alive until it is unregistered, the
    /// owning `Context` is dropped, or the texture owner unregisters it from all contexts before
    /// destruction.
    pub unsafe fn register_user_texture_raw(&mut self, texture: &mut crate::texture::TextureData) {
        self.register_user_texture_ptr(texture.as_raw_mut());
    }

    fn register_user_texture_ptr(&mut self, texture: *mut sys::ImTextureData) {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::register_user_texture()");
        assert!(
            !texture.is_null(),
            "Context::register_user_texture() received a null texture"
        );
        if is_user_texture_registered(self.raw, texture) {
            return;
        }
        unsafe {
            sys::igRegisterUserTexture(texture);
        }
        track_user_texture_registration(self.raw, texture, self.binding());
    }

    /// Register a user-created texture and return an RAII token which unregisters on drop.
    ///
    /// This is a convenience wrapper around `register_user_texture()`.
    pub fn register_user_texture_token(
        &mut self,
        texture: &mut crate::texture::OwnedTextureData,
    ) -> RegisteredUserTexture {
        self.register_user_texture(texture);
        RegisteredUserTexture {
            ctx: self.raw,
            tex: texture.as_mut().as_raw_mut(),
            binding: self.binding(),
        }
    }

    /// Unregister a user texture previously registered with `register_user_texture()`.
    ///
    /// This removes the `ImTextureData*` from ImGui's internal texture list.
    pub fn unregister_user_texture(&mut self, texture: &mut crate::texture::OwnedTextureData) {
        self.unregister_user_texture_ptr(texture.as_mut().as_raw_mut());
    }

    /// Unregister a borrowed/raw user texture previously registered with
    /// [`Context::register_user_texture_raw`].
    ///
    /// # Safety
    /// The pointer must refer to the same live `TextureData` object that was previously
    /// registered for this context.
    pub unsafe fn unregister_user_texture_raw(
        &mut self,
        texture: &mut crate::texture::TextureData,
    ) {
        self.unregister_user_texture_ptr(texture.as_raw_mut());
    }

    fn unregister_user_texture_ptr(&mut self, texture: *mut sys::ImTextureData) {
        let _guard = CTX_MUTEX.lock();
        self.assert_current_context("Context::unregister_user_texture()");
        assert!(
            !texture.is_null(),
            "Context::unregister_user_texture() received a null texture"
        );
        if let Some(registration) = take_user_texture_registration(self.raw, texture) {
            unregister_user_texture_registration(registration);
        }
    }
}

/// RAII token returned by `Context::register_user_texture_token()`.
///
/// On drop, this unregisters the corresponding `ImTextureData*` from ImGui's internal user texture
/// list.
#[derive(Debug)]
pub struct RegisteredUserTexture {
    ctx: *mut sys::ImGuiContext,
    tex: *mut sys::ImTextureData,
    binding: ContextBinding,
}

impl Drop for RegisteredUserTexture {
    fn drop(&mut self) {
        if self.ctx.is_null()
            || self.tex.is_null()
            || self.binding.lifecycle() == ContextLifecycle::NativeDestroyed
        {
            return;
        }

        let _guard = CTX_MUTEX.lock();
        if let Some(registration) = take_user_texture_registration(self.ctx, self.tex) {
            unregister_user_texture_registration(registration);
        }
    }
}
