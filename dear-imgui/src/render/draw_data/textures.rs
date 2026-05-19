use super::DrawData;
use crate::sys;
use crate::texture::TextureData;
use std::marker::PhantomData;

impl DrawData {
    /// Returns a shared iterator over textures attached to this draw data.
    ///
    /// Use this for inspection, snapshotting, or read-only request collection. Renderer backends
    /// that need to write `TexID`/`Status` after handling texture requests must use
    /// [`Self::textures_mut`] instead.
    pub fn textures(&self) -> TextureIterator<'_> {
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

    /// Returns a mutable cursor over textures that need to be updated.
    ///
    /// This is used by renderer backends to process texture creation, updates, and destruction.
    /// Each item is an `ImTextureData*` carrying a `Status` which can be one of:
    /// - `OK`: nothing to do.
    /// - `WantCreate`: create a GPU texture and upload all pixels.
    /// - `WantUpdates`: upload specified `UpdateRect` regions.
    /// - `WantDestroy`: destroy the GPU texture (may be delayed until unused).
    /// Most of the time this list has only 1 texture and it doesn't need any update.
    ///
    /// The cursor intentionally does not implement [`Iterator`]. Each mutable item is borrowed
    /// from the cursor itself, so safe Rust cannot hold one texture update guard while asking for
    /// the next one.
    ///
    /// ```compile_fail
    /// use dear_imgui_rs::render::DrawData;
    ///
    /// fn shared_texture_blocks_mutable_cursor(draw_data: &mut DrawData) {
    ///     let shared = draw_data.texture(0);
    ///     let mut textures = draw_data.textures_mut();
    ///     let _first = textures.next();
    ///     drop(shared);
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use dear_imgui_rs::render::DrawData;
    ///
    /// fn texture_guards_cannot_overlap(draw_data: &mut DrawData) {
    ///     let mut textures = draw_data.textures_mut();
    ///     let first = textures.next();
    ///     let second = textures.next();
    ///     drop(first);
    ///     drop(second);
    /// }
    /// ```
    pub fn textures_mut(&mut self) -> TextureMutCursor<'_> {
        unsafe {
            if self.textures.is_null() {
                TextureMutCursor::new(std::ptr::null_mut(), std::ptr::null_mut())
            } else {
                let vector = &mut *self.textures;
                if vector.size <= 0 || vector.data.is_null() {
                    TextureMutCursor::new(std::ptr::null_mut(), std::ptr::null_mut())
                } else {
                    TextureMutCursor::new(vector.data, vector.data.add(vector.size as usize))
                }
            }
        }
    }

    /// Returns the number of textures in the texture list
    pub fn textures_count(&self) -> usize {
        unsafe {
            if self.textures.is_null() {
                0
            } else {
                let vector = &*self.textures;
                if vector.size <= 0 || vector.data.is_null() {
                    0
                } else {
                    vector.size as usize
                }
            }
        }
    }

    /// Get a specific texture by index
    ///
    /// Returns None if the index is out of bounds or no textures are available.
    pub fn texture(&self, index: usize) -> Option<&TextureData> {
        unsafe {
            if self.textures.is_null() {
                return None;
            }
            let vector = &*self.textures;
            let size = usize::try_from(vector.size).ok()?;
            if size == 0 || vector.data.is_null() {
                return None;
            }
            if index >= size {
                return None;
            }
            let texture_ptr = *vector.data.add(index);
            if texture_ptr.is_null() {
                return None;
            }
            Some(TextureData::from_raw_ref(texture_ptr as *const _))
        }
    }

    /// Get a mutable reference to a specific texture by index
    ///
    /// Returns None if the index is out of bounds or no textures are available.
    pub fn texture_mut(&mut self, index: usize) -> Option<&mut TextureData> {
        unsafe {
            if self.textures.is_null() {
                return None;
            }
            let vector = &*self.textures;
            let size = usize::try_from(vector.size).ok()?;
            if size == 0 || vector.data.is_null() {
                return None;
            }
            if index >= size {
                return None;
            }
            let texture_ptr = *vector.data.add(index);
            if texture_ptr.is_null() {
                return None;
            }
            Some(TextureData::from_raw(texture_ptr))
        }
    }
}

/// Iterator over textures in draw data
pub struct TextureIterator<'a> {
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

/// Mutable cursor over a texture list.
///
/// This cursor is the mutable counterpart to [`TextureIterator`]. It is a streaming cursor rather
/// than a standard iterator so each returned [`TextureDataMut`] is tied to the borrow of the cursor
/// used for that single `next()` call.
pub struct TextureMutCursor<'a> {
    ptr: *mut *mut sys::ImTextureData,
    end: *mut *mut sys::ImTextureData,
    _phantom: PhantomData<&'a mut TextureData>,
}

impl<'a> TextureMutCursor<'a> {
    /// Create a new texture cursor from raw pointers.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointers are valid and that the range
    /// [ptr, end) contains valid texture data pointers. The caller must also hold the unique
    /// mutable borrow of the owner texture list for `'a`.
    pub(crate) unsafe fn new(
        ptr: *mut *mut sys::ImTextureData,
        end: *mut *mut sys::ImTextureData,
    ) -> Self {
        Self {
            ptr,
            end,
            _phantom: PhantomData,
        }
    }

    /// Advance to the next non-null texture.
    pub fn next(&mut self) -> Option<TextureDataMut<'_>> {
        while self.ptr < self.end {
            let texture_ptr = unsafe { *self.ptr };
            self.ptr = unsafe { self.ptr.add(1) };
            if texture_ptr.is_null() {
                continue;
            }

            return Some(TextureDataMut {
                raw: texture_ptr,
                _phantom: PhantomData,
            });
        }

        None
    }
}

/// A guarded mutable view of a single `ImTextureData`.
///
/// The guard is created by [`TextureMutCursor::next`] and borrows the cursor for its lifetime, so
/// safe Rust cannot hold multiple mutable texture views from the same list at the same time.
pub struct TextureDataMut<'a> {
    raw: *mut sys::ImTextureData,
    _phantom: PhantomData<&'a mut TextureData>,
}

impl std::ops::Deref for TextureDataMut<'_> {
    type Target = TextureData;

    fn deref(&self) -> &Self::Target {
        unsafe { TextureData::from_raw_ref(self.raw as *const _) }
    }
}

impl std::ops::DerefMut for TextureDataMut<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { TextureData::from_raw(self.raw) }
    }
}
