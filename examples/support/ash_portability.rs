//! Optional Vulkan extensions needed by portability implementations such as MoltenVK.

use ash::{Entry, Instance, khr, vk};
use std::ffi::{CStr, c_char};

/// Add supported portability extensions without duplicating the window system's extensions.
///
/// # Safety
/// Every pointer in `extensions` must reference a live, nul-terminated extension name.
pub unsafe fn enable_instance_extensions(
    entry: &Entry,
    api_version: u32,
    extensions: &mut Vec<*const c_char>,
) -> Result<vk::InstanceCreateFlags, vk::Result> {
    let available = unsafe { entry.enumerate_instance_extension_properties(None)? };
    let mut flags = vk::InstanceCreateFlags::empty();
    for name in [
        khr::portability_enumeration::NAME,
        khr::get_physical_device_properties2::NAME,
    ] {
        if name == khr::get_physical_device_properties2::NAME && api_version >= vk::API_VERSION_1_1
        {
            continue;
        }
        let supported = available
            .iter()
            .any(|extension| unsafe { CStr::from_ptr(extension.extension_name.as_ptr()) == name });
        if supported {
            if !extensions
                .iter()
                .any(|&extension| unsafe { CStr::from_ptr(extension) == name })
            {
                extensions.push(name.as_ptr());
            }
            if name == khr::portability_enumeration::NAME {
                flags |= vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;
            }
        }
    }
    Ok(flags)
}

/// A physical device advertising the portability subset requires it to be enabled.
///
/// # Safety
/// `physical_device` must belong to `instance`, and all extension name pointers must be valid.
pub unsafe fn enable_device_extensions(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
    extensions: &mut Vec<*const c_char>,
) -> Result<(), vk::Result> {
    let available = unsafe { instance.enumerate_device_extension_properties(physical_device)? };
    let name = khr::portability_subset::NAME;
    if available
        .iter()
        .any(|extension| unsafe { CStr::from_ptr(extension.extension_name.as_ptr()) == name })
        && !extensions
            .iter()
            .any(|&extension| unsafe { CStr::from_ptr(extension) == name })
    {
        extensions.push(name.as_ptr());
    }
    Ok(())
}
