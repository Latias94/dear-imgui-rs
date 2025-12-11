use syn::Type;

/// High-level classification of field types for ImGuiReflect derive.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FieldTypeKind {
    Bool,
    Numeric,
    String,
    ImString,
    Tuple,
    Vec,
    Array,
    Map,
    Other,
}

/// Desired widget kind for numeric fields at the derive level.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NumericWidgetKind {
    /// Use the default ImGuiValue implementation (no special widget).
    Default,
    /// Use an InputScalar-style widget with optional step/step_fast/format.
    Input,
    /// Use a Slider widget with required min/max and optional format/flags.
    Slider,
    /// Use a Drag widget with optional speed/range/format/flags.
    Drag,
}

/// Tag for primitive numeric types that have dedicated type-level settings.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NumericTypeTag {
    I32,
    U32,
    F32,
    F64,
}

/// Classifies a Rust type as one of the primitive numeric types we support.
pub fn classify_numeric_type(ty: &Type) -> Option<NumericTypeTag> {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            match seg.ident.to_string().as_str() {
                "i32" => Some(NumericTypeTag::I32),
                "u32" => Some(NumericTypeTag::U32),
                "f32" => Some(NumericTypeTag::F32),
                "f64" => Some(NumericTypeTag::F64),
                _ => None,
            }
        } else {
            None
        }
    } else {
        None
    }
}

/// Roughly classifies a field's Rust type to drive high-level widget selection.
pub fn classify_field_type(ty: &Type) -> FieldTypeKind {
    match ty {
        Type::Tuple(_) => FieldTypeKind::Tuple,
        Type::Array(_) => FieldTypeKind::Array,
        Type::Path(tp) => {
            if let Some(seg) = tp.path.segments.last() {
                let ident = seg.ident.to_string();
                match ident.as_str() {
                    "bool" => FieldTypeKind::Bool,
                    // Primitive numeric types we commonly care about
                    "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64"
                    | "usize" | "f32" | "f64" => FieldTypeKind::Numeric,
                    "String" => FieldTypeKind::String,
                    "ImString" => FieldTypeKind::ImString,
                    "Vec" => FieldTypeKind::Vec,
                    "HashMap" | "BTreeMap" => FieldTypeKind::Map,
                    _ => FieldTypeKind::Other,
                }
            } else {
                FieldTypeKind::Other
            }
        }
        _ => FieldTypeKind::Other,
    }
}
