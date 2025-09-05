use crate::sys;

/// A primary data type
#[repr(i32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DataType {
    I8 = sys::ImGuiDataType_S8,
    U8 = sys::ImGuiDataType_U8,
    I16 = sys::ImGuiDataType_S16,
    U16 = sys::ImGuiDataType_U16,
    I32 = sys::ImGuiDataType_S32,
    U32 = sys::ImGuiDataType_U32,
    I64 = sys::ImGuiDataType_S64,
    U64 = sys::ImGuiDataType_U64,
    F32 = sys::ImGuiDataType_Float,
    F64 = sys::ImGuiDataType_Double,
}

/// Primitive type marker.
///
/// If this trait is implemented for a type, it is assumed to have *exactly* the same
/// representation in memory as the primitive value described by the associated `KIND` constant.
///
/// # Safety
/// The `DataType` *must* have the same representation as the primitive value of `KIND`.
pub unsafe trait DataTypeKind: Copy {
    const KIND: DataType;
}

unsafe impl DataTypeKind for i8 {
    const KIND: DataType = DataType::I8;
}
unsafe impl DataTypeKind for u8 {
    const KIND: DataType = DataType::U8;
}
unsafe impl DataTypeKind for i16 {
    const KIND: DataType = DataType::I16;
}
unsafe impl DataTypeKind for u16 {
    const KIND: DataType = DataType::U16;
}
unsafe impl DataTypeKind for i32 {
    const KIND: DataType = DataType::I32;
}
unsafe impl DataTypeKind for u32 {
    const KIND: DataType = DataType::U32;
}
unsafe impl DataTypeKind for i64 {
    const KIND: DataType = DataType::I64;
}
unsafe impl DataTypeKind for u64 {
    const KIND: DataType = DataType::U64;
}
unsafe impl DataTypeKind for f32 {
    const KIND: DataType = DataType::F32;
}
unsafe impl DataTypeKind for f64 {
    const KIND: DataType = DataType::F64;
}

unsafe impl DataTypeKind for usize {
    #[cfg(target_pointer_width = "16")]
    const KIND: DataType = DataType::U16;

    #[cfg(target_pointer_width = "32")]
    const KIND: DataType = DataType::U32;

    #[cfg(target_pointer_width = "64")]
    const KIND: DataType = DataType::U64;

    // Fallback for when we are on a weird system width
    //
    #[cfg(not(any(
        target_pointer_width = "16",
        target_pointer_width = "32",
        target_pointer_width = "64"
    )))]
    compile_error!("cannot impl DataTypeKind for usize: unsupported target pointer width. supported values are 16, 32, 64");
}

unsafe impl DataTypeKind for isize {
    #[cfg(target_pointer_width = "16")]
    const KIND: DataType = DataType::I16;

    #[cfg(target_pointer_width = "32")]
    const KIND: DataType = DataType::I32;

    #[cfg(target_pointer_width = "64")]
    const KIND: DataType = DataType::I64;

    // Fallback for when we are on a weird system width
    //
    #[cfg(not(any(
        target_pointer_width = "16",
        target_pointer_width = "32",
        target_pointer_width = "64"
    )))]
    compile_error!("cannot impl DataTypeKind for isize: unsupported target pointer width. supported values are 16, 32, 64");
}
