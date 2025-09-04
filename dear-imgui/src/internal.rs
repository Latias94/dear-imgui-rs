//! Internal utilities for Dear ImGui
//! 
//! This module provides low-level utilities and type conversions that are used
//! internally by the Dear ImGui binding. These utilities provide safe abstractions
//! over unsafe operations and ensure proper type safety.

use dear_imgui_sys as sys;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::mem;
use std::ptr::NonNull;

/// Data type enumeration for Dear ImGui
/// 
/// This enum represents the different data types that Dear ImGui can work with
/// internally. It's used for type-safe operations and validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum DataType {
    /// Signed 8-bit integer
    S8 = sys::ImGuiDataType_S8 as i32,
    /// Unsigned 8-bit integer  
    U8 = sys::ImGuiDataType_U8 as i32,
    /// Signed 16-bit integer
    S16 = sys::ImGuiDataType_S16 as i32,
    /// Unsigned 16-bit integer
    U16 = sys::ImGuiDataType_U16 as i32,
    /// Signed 32-bit integer
    S32 = sys::ImGuiDataType_S32 as i32,
    /// Unsigned 32-bit integer
    U32 = sys::ImGuiDataType_U32 as i32,
    /// Signed 64-bit integer
    S64 = sys::ImGuiDataType_S64 as i32,
    /// Unsigned 64-bit integer
    U64 = sys::ImGuiDataType_U64 as i32,
    /// 32-bit floating point
    Float = sys::ImGuiDataType_Float as i32,
    /// 64-bit floating point
    Double = sys::ImGuiDataType_Double as i32,
}

impl DataType {
    /// Get the size in bytes of this data type
    pub fn size(self) -> usize {
        match self {
            DataType::S8 | DataType::U8 => 1,
            DataType::S16 | DataType::U16 => 2,
            DataType::S32 | DataType::U32 | DataType::Float => 4,
            DataType::S64 | DataType::U64 | DataType::Double => 8,
        }
    }

    /// Check if this data type is signed
    pub fn is_signed(self) -> bool {
        matches!(self, DataType::S8 | DataType::S16 | DataType::S32 | DataType::S64 | DataType::Float | DataType::Double)
    }

    /// Check if this data type is floating point
    pub fn is_float(self) -> bool {
        matches!(self, DataType::Float | DataType::Double)
    }

    /// Check if this data type is integer
    pub fn is_integer(self) -> bool {
        !self.is_float()
    }
}

/// Trait for types that can be converted to a Dear ImGui data type
/// 
/// This trait provides a safe way to determine the Dear ImGui data type
/// for Rust types, enabling type-safe operations.
pub trait DataTypeKind {
    /// The Dear ImGui data type for this Rust type
    const DATA_TYPE: DataType;
}

impl DataTypeKind for i8 {
    const DATA_TYPE: DataType = DataType::S8;
}

impl DataTypeKind for u8 {
    const DATA_TYPE: DataType = DataType::U8;
}

impl DataTypeKind for i16 {
    const DATA_TYPE: DataType = DataType::S16;
}

impl DataTypeKind for u16 {
    const DATA_TYPE: DataType = DataType::U16;
}

impl DataTypeKind for i32 {
    const DATA_TYPE: DataType = DataType::S32;
}

impl DataTypeKind for u32 {
    const DATA_TYPE: DataType = DataType::U32;
}

impl DataTypeKind for i64 {
    const DATA_TYPE: DataType = DataType::S64;
}

impl DataTypeKind for u64 {
    const DATA_TYPE: DataType = DataType::U64;
}

impl DataTypeKind for f32 {
    const DATA_TYPE: DataType = DataType::Float;
}

impl DataTypeKind for f64 {
    const DATA_TYPE: DataType = DataType::Double;
}

/// Trait for safe casting between types and raw pointers
/// 
/// This trait provides a safe abstraction for converting between Rust types
/// and the raw pointers expected by Dear ImGui's C API.
pub trait RawCast<T> {
    /// Cast to a raw pointer
    fn raw(&self) -> *const T;
    
    /// Cast to a mutable raw pointer
    fn raw_mut(&mut self) -> *mut T;
}

impl<T> RawCast<T> for T {
    fn raw(&self) -> *const T {
        self as *const T
    }

    fn raw_mut(&mut self) -> *mut T {
        self as *mut T
    }
}

/// Extension trait for converting to void pointers
pub trait RawCastVoid {
    /// Cast to a raw void pointer
    fn raw_void(&self) -> *const c_void;

    /// Cast to a mutable raw void pointer
    fn raw_void_mut(&mut self) -> *mut c_void;
}

impl<T> RawCastVoid for T {
    fn raw_void(&self) -> *const c_void {
        self as *const T as *const c_void
    }

    fn raw_void_mut(&mut self) -> *mut c_void {
        self as *mut T as *mut c_void
    }
}

/// Safe wrapper around Dear ImGui's ImVector
///
/// This provides a safe Rust interface to Dear ImGui's internal vector type,
/// which is used throughout the C++ API.
#[derive(Debug)]
pub struct ImVector<T> {
    ptr: NonNull<sys::ImVector<T>>,
    _phantom: PhantomData<T>,
}

impl<T> ImVector<T> {
    /// Create a new ImVector from a raw pointer
    /// 
    /// # Safety
    /// 
    /// The caller must ensure that:
    /// - `ptr` is a valid pointer to an ImVector
    /// - The ImVector contains elements of type T
    /// - The ImVector remains valid for the lifetime of this wrapper
    pub unsafe fn from_raw(ptr: *mut sys::ImVector<T>) -> Option<Self> {
        NonNull::new(ptr).map(|ptr| Self {
            ptr,
            _phantom: PhantomData,
        })
    }

    /// Get the number of elements in the vector
    pub fn len(&self) -> usize {
        unsafe {
            let raw = self.ptr.as_ptr();
            (*raw).Size as usize
        }
    }

    /// Check if the vector is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the capacity of the vector
    pub fn capacity(&self) -> usize {
        unsafe {
            let raw = self.ptr.as_ptr();
            (*raw).Capacity as usize
        }
    }

    /// Get a pointer to the data
    pub fn as_ptr(&self) -> *const T {
        unsafe {
            let raw = self.ptr.as_ptr();
            (*raw).Data as *const T
        }
    }

    /// Get a mutable pointer to the data
    pub fn as_mut_ptr(&mut self) -> *mut T {
        unsafe {
            let raw = self.ptr.as_ptr();
            (*raw).Data as *mut T
        }
    }

    /// Get an element by index
    /// 
    /// # Safety
    /// 
    /// The caller must ensure that `index` is within bounds.
    pub unsafe fn get_unchecked(&self, index: usize) -> &T {
        &*self.as_ptr().add(index)
    }

    /// Get a mutable element by index
    /// 
    /// # Safety
    /// 
    /// The caller must ensure that `index` is within bounds.
    pub unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut T {
        &mut *self.as_mut_ptr().add(index)
    }

    /// Get an element by index with bounds checking
    pub fn get(&self, index: usize) -> Option<&T> {
        if index < self.len() {
            Some(unsafe { self.get_unchecked(index) })
        } else {
            None
        }
    }

    /// Get a mutable element by index with bounds checking
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index < self.len() {
            Some(unsafe { self.get_unchecked_mut(index) })
        } else {
            None
        }
    }

    /// Create an iterator over the vector elements
    pub fn iter(&self) -> ImVectorIter<T> {
        ImVectorIter {
            vector: self,
            index: 0,
        }
    }

    /// Create a mutable iterator over the vector elements
    pub fn iter_mut(&mut self) -> ImVectorIterMut<T> {
        ImVectorIterMut {
            vector: self,
            index: 0,
        }
    }
}

/// Iterator over ImVector elements
pub struct ImVectorIter<'a, T> {
    vector: &'a ImVector<T>,
    index: usize,
}

impl<'a, T> Iterator for ImVectorIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.vector.len() {
            let item = unsafe { self.vector.get_unchecked(self.index) };
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.vector.len().saturating_sub(self.index);
        (remaining, Some(remaining))
    }
}

impl<'a, T> ExactSizeIterator for ImVectorIter<'a, T> {}

/// Mutable iterator over ImVector elements
pub struct ImVectorIterMut<'a, T> {
    vector: &'a mut ImVector<T>,
    index: usize,
}

impl<'a, T> Iterator for ImVectorIterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.vector.len() {
            let item = unsafe {
                // SAFETY: We're creating a mutable reference with a longer lifetime
                // This is safe because we're consuming self and the vector outlives the iterator
                &mut *(self.vector.as_mut_ptr().add(self.index))
            };
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.vector.len().saturating_sub(self.index);
        (remaining, Some(remaining))
    }
}

impl<'a, T> ExactSizeIterator for ImVectorIterMut<'a, T> {}

/// Utility function to check if two types have the same memory layout
/// 
/// This is used to ensure that Rust types can be safely cast to/from
/// their C counterparts.
pub const fn assert_same_layout<T, U>() {
    assert!(mem::size_of::<T>() == mem::size_of::<U>());
    assert!(mem::align_of::<T>() == mem::align_of::<U>());
}

/// Utility function to safely transmute between types with the same layout
/// 
/// # Safety
/// 
/// The caller must ensure that T and U have the same memory layout and
/// that the transmutation is semantically valid.
pub unsafe fn transmute_same_layout<T, U>(value: T) -> U {
    debug_assert_eq!(mem::size_of::<T>(), mem::size_of::<U>());
    debug_assert_eq!(mem::align_of::<T>(), mem::align_of::<U>());
    
    let result = mem::transmute_copy(&value);
    mem::forget(value);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_type_properties() {
        assert_eq!(DataType::S8.size(), 1);
        assert_eq!(DataType::U8.size(), 1);
        assert_eq!(DataType::S16.size(), 2);
        assert_eq!(DataType::U16.size(), 2);
        assert_eq!(DataType::S32.size(), 4);
        assert_eq!(DataType::U32.size(), 4);
        assert_eq!(DataType::S64.size(), 8);
        assert_eq!(DataType::U64.size(), 8);
        assert_eq!(DataType::Float.size(), 4);
        assert_eq!(DataType::Double.size(), 8);

        assert!(DataType::S8.is_signed());
        assert!(!DataType::U8.is_signed());
        assert!(DataType::Float.is_float());
        assert!(!DataType::S32.is_float());
        assert!(DataType::S32.is_integer());
        assert!(!DataType::Float.is_integer());
    }

    #[test]
    fn test_data_type_kind() {
        assert_eq!(i8::DATA_TYPE, DataType::S8);
        assert_eq!(u8::DATA_TYPE, DataType::U8);
        assert_eq!(i16::DATA_TYPE, DataType::S16);
        assert_eq!(u16::DATA_TYPE, DataType::U16);
        assert_eq!(i32::DATA_TYPE, DataType::S32);
        assert_eq!(u32::DATA_TYPE, DataType::U32);
        assert_eq!(i64::DATA_TYPE, DataType::S64);
        assert_eq!(u64::DATA_TYPE, DataType::U64);
        assert_eq!(f32::DATA_TYPE, DataType::Float);
        assert_eq!(f64::DATA_TYPE, DataType::Double);
    }

    #[test]
    fn test_raw_cast() {
        let mut value = 42i32;
        
        let ptr = value.raw();
        assert_eq!(unsafe { *ptr }, 42);
        
        let mut_ptr = value.raw_mut();
        unsafe { *mut_ptr = 84 };
        assert_eq!(value, 84);
        
        let void_ptr = value.raw() as *const c_void;
        assert!(!void_ptr.is_null());
    }

    #[test]
    fn test_layout_assertions() {
        // These should compile without panicking
        assert_same_layout::<i32, i32>();
        assert_same_layout::<f32, u32>();
        
        // Test that different sized types would fail
        // assert_same_layout::<i32, i64>(); // This would panic at compile time
    }

    #[test]
    fn test_transmute_same_layout() {
        let float_val = 3.14f32;
        let int_bits = unsafe { transmute_same_layout::<f32, u32>(float_val) };
        
        // Convert back to verify
        let back_to_float = unsafe { transmute_same_layout::<u32, f32>(int_bits) };
        assert_eq!(back_to_float, 3.14f32);
    }
}
