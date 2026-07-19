use super::DrawData;
use crate::sys;
use crate::texture::TextureData;
use std::marker::PhantomData;

impl DrawData {
    /// Returns a shared iterator over textures attached to this draw data.
    ///
    /// This crate-internal view is used only while collecting Context-owned renderer requests.
    pub(crate) fn textures(&self) -> TextureIterator<'_> {
        unsafe {
            if self.textures.is_null() {
                TextureIterator::new(std::ptr::null(), std::ptr::null())
            } else {
                let vector = &*self.textures;
                if vector.size <= 0 || vector.data.is_null() {
                    TextureIterator::new(std::ptr::null(), std::ptr::null())
                } else {
                    TextureIterator::new(vector.data, vector.data.add(vector.size as usize))
                }
            }
        }
    }
}

/// Iterator over textures in draw data
pub(crate) struct TextureIterator<'a> {
    ptr: *const *mut sys::ImTextureData,
    end: *const *mut sys::ImTextureData,
    _phantom: PhantomData<&'a TextureData>,
}

impl<'a> TextureIterator<'a> {
    /// Create a new texture iterator from raw pointers
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointers are valid and that the range
    /// [ptr, end) contains valid texture data pointers.
    pub(crate) unsafe fn new(
        ptr: *const *mut sys::ImTextureData,
        end: *const *mut sys::ImTextureData,
    ) -> Self {
        Self {
            ptr,
            end,
            _phantom: PhantomData,
        }
    }
}

impl<'a> Iterator for TextureIterator<'a> {
    type Item = &'a TextureData;

    fn next(&mut self) -> Option<Self::Item> {
        while self.ptr < self.end {
            let texture_ptr = unsafe { *self.ptr };
            self.ptr = unsafe { self.ptr.add(1) };
            if texture_ptr.is_null() {
                continue;
            }

            return Some(unsafe { TextureData::from_raw_ref(texture_ptr as *const _) });
        }

        None
    }
}

impl<'a> std::iter::FusedIterator for TextureIterator<'a> {}
