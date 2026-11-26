use syn::{Ident, Type};

/// C variadic carrier class used to validate numeric display formats.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NumericFormatType {
    Signed32,
    Unsigned32,
    Signed64,
    Unsigned64,
    Float,
    PointerSized,
}

pub fn type_name(ty: &Type) -> Option<&Ident> {
    match ty {
        Type::Path(path) if path.qself.is_none() => {
            path.path.segments.last().map(|segment| &segment.ident)
        }
        Type::Paren(paren) => type_name(&paren.elem),
        Type::Group(group) => type_name(&group.elem),
        _ => None,
    }
}

pub fn numeric_type(ty: &Type) -> Option<(&Ident, NumericFormatType)> {
    let ident = type_name(ty)?;
    let carrier = match ident.to_string().as_str() {
        "i8" | "i16" | "i32" => NumericFormatType::Signed32,
        "u8" | "u16" | "u32" => NumericFormatType::Unsigned32,
        "i64" => NumericFormatType::Signed64,
        "u64" => NumericFormatType::Unsigned64,
        "f32" | "f64" => NumericFormatType::Float,
        "isize" | "usize" => NumericFormatType::PointerSized,
        _ => return None,
    };
    Some((ident, carrier))
}
