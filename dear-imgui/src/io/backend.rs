use crate::io::{BackendFlags, Io, validate_backend_flags};
use std::ffi::{CStr, c_void};

impl Io {
    /// Returns the backend platform name, if set.
    ///
    /// Note: to set this safely, use `Context::set_platform_name()`.
    #[doc(alias = "BackendPlatformName")]
    pub fn backend_platform_name(&self) -> Option<&CStr> {
        let ptr = self.inner().BackendPlatformName;
        unsafe { (!ptr.is_null()).then(|| CStr::from_ptr(ptr)) }
    }

    /// Returns the backend renderer name, if set.
    ///
    /// Note: to set this safely, use `Context::set_renderer_name()`.
    #[doc(alias = "BackendRendererName")]
    pub fn backend_renderer_name(&self) -> Option<&CStr> {
        let ptr = self.inner().BackendRendererName;
        unsafe { (!ptr.is_null()).then(|| CStr::from_ptr(ptr)) }
    }

    /// Returns the backend platform user data pointer.
    #[doc(alias = "BackendPlatformUserData")]
    pub fn backend_platform_user_data(&self) -> *mut c_void {
        self.inner().BackendPlatformUserData
    }

    /// Set the backend platform user data pointer.
    #[doc(alias = "BackendPlatformUserData")]
    pub fn set_backend_platform_user_data(&mut self, user_data: *mut c_void) {
        self.inner_mut().BackendPlatformUserData = user_data;
    }

    /// Returns the backend renderer user data pointer.
    #[doc(alias = "BackendRendererUserData")]
    pub fn backend_renderer_user_data(&self) -> *mut c_void {
        self.inner().BackendRendererUserData
    }

    /// Set the backend renderer user data pointer.
    #[doc(alias = "BackendRendererUserData")]
    pub fn set_backend_renderer_user_data(&mut self, user_data: *mut c_void) {
        self.inner_mut().BackendRendererUserData = user_data;
    }

    /// Returns the backend language user data pointer.
    #[doc(alias = "BackendLanguageUserData")]
    pub fn backend_language_user_data(&self) -> *mut c_void {
        self.inner().BackendLanguageUserData
    }

    /// Set the backend language user data pointer.
    #[doc(alias = "BackendLanguageUserData")]
    pub fn set_backend_language_user_data(&mut self, user_data: *mut c_void) {
        self.inner_mut().BackendLanguageUserData = user_data;
    }

    /// Backend flags
    pub fn backend_flags(&self) -> BackendFlags {
        BackendFlags::from_bits_retain(self.inner().BackendFlags)
    }

    /// Set backend flags
    pub fn set_backend_flags(&mut self, flags: BackendFlags) {
        validate_backend_flags("Io::set_backend_flags()", flags);
        self.inner_mut().BackendFlags = flags.bits();
    }
}
