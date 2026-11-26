use std::collections::HashSet;

use syn::{Attribute, Error, Expr, Field, LitStr, Result};

#[derive(Default)]
pub struct FieldAttrs {
    pub skip: bool,
    pub name: Option<LitStr>,
    pub read_only: bool,
    pub numeric: NumericAttrs,
    pub text: TextAttrs,
    pub tuple: TupleAttrs,
    pub boolean: BoolAttrs,
}

#[derive(Default)]
pub struct NumericAttrs {
    pub slider: bool,
    pub slider_default_range: bool,
    pub as_input: bool,
    pub as_drag: bool,
    pub min: Option<Expr>,
    pub max: Option<Expr>,
    pub format: Option<LitStr>,
    pub hex: bool,
    pub percentage: bool,
    pub scientific: bool,
    pub prefix: Option<LitStr>,
    pub suffix: Option<LitStr>,
    pub step: Option<Expr>,
    pub step_fast: Option<Expr>,
    pub speed: Option<Expr>,
    pub log: bool,
    pub clamp: bool,
    pub always_clamp: bool,
    pub wrap_around: bool,
    pub no_round_to_format: bool,
    pub no_input: bool,
    pub clamp_on_input: bool,
    pub clamp_zero_range: bool,
    pub no_speed_tweaks: bool,
}

impl NumericAttrs {
    pub fn has_attrs(&self) -> bool {
        self.slider
            || self.slider_default_range
            || self.as_input
            || self.as_drag
            || self.min.is_some()
            || self.max.is_some()
            || self.step.is_some()
            || self.step_fast.is_some()
            || self.speed.is_some()
            || self.format.is_some()
            || self.has_format_helpers()
            || self.clamp
            || self.has_flags()
    }

    pub fn has_format_helpers(&self) -> bool {
        self.hex
            || self.percentage
            || self.scientific
            || self.prefix.is_some()
            || self.suffix.is_some()
    }

    pub fn has_flags(&self) -> bool {
        self.log
            || self.always_clamp
            || self.wrap_around
            || self.no_round_to_format
            || self.no_input
            || self.clamp_on_input
            || self.clamp_zero_range
            || self.no_speed_tweaks
    }
}

#[derive(Default)]
pub struct TextAttrs {
    pub multiline: bool,
    pub lines: Option<Expr>,
    pub hint: Option<LitStr>,
    pub display_only: bool,
    pub auto_resize: bool,
    pub min_width: Option<Expr>,
}

impl TextAttrs {
    pub fn has_attrs(&self) -> bool {
        self.multiline
            || self.lines.is_some()
            || self.hint.is_some()
            || self.display_only
            || self.auto_resize
            || self.min_width.is_some()
    }
}

#[derive(Default)]
pub struct TupleAttrs {
    pub render: Option<LitStr>,
    pub dropdown: bool,
    pub columns: Option<Expr>,
    pub min_width: Option<Expr>,
}

impl TupleAttrs {
    pub fn has_attrs(&self) -> bool {
        self.render.is_some() || self.dropdown || self.columns.is_some() || self.min_width.is_some()
    }
}

#[derive(Default)]
pub struct BoolAttrs {
    pub style: Option<LitStr>,
    pub true_text: Option<LitStr>,
    pub false_text: Option<LitStr>,
}

impl BoolAttrs {
    pub fn has_attrs(&self) -> bool {
        self.style.is_some() || self.true_text.is_some() || self.false_text.is_some()
    }
}

pub fn parse_field_attrs(field: &Field) -> Result<FieldAttrs> {
    let mut attrs = FieldAttrs::default();
    let mut seen = HashSet::new();
    for attr in imgui_attrs(&field.attrs) {
        attr.parse_nested_meta(|meta| {
            let Some(key) = meta.path.get_ident().map(ToString::to_string) else {
                return Err(meta.error("unknown imgui field attribute"));
            };
            if !seen.insert(key.clone()) {
                return Err(meta.error("duplicate imgui field attribute"));
            }
            match key.as_str() {
                "skip" => attrs.skip = true,
                "name" => attrs.name = Some(meta.value()?.parse()?),
                "read_only" => attrs.read_only = true,
                "slider" => attrs.numeric.slider = true,
                "slider_default_range" => attrs.numeric.slider_default_range = true,
                "as_input" => attrs.numeric.as_input = true,
                "as_drag" => attrs.numeric.as_drag = true,
                "min" => attrs.numeric.min = Some(meta.value()?.parse()?),
                "max" => attrs.numeric.max = Some(meta.value()?.parse()?),
                "format" => attrs.numeric.format = Some(meta.value()?.parse()?),
                "hex" => attrs.numeric.hex = true,
                "percentage" => attrs.numeric.percentage = true,
                "scientific" => attrs.numeric.scientific = true,
                "prefix" => attrs.numeric.prefix = Some(meta.value()?.parse()?),
                "suffix" => attrs.numeric.suffix = Some(meta.value()?.parse()?),
                "step" => attrs.numeric.step = Some(meta.value()?.parse()?),
                "step_fast" => attrs.numeric.step_fast = Some(meta.value()?.parse()?),
                "speed" => attrs.numeric.speed = Some(meta.value()?.parse()?),
                "log" => attrs.numeric.log = true,
                "clamp" => attrs.numeric.clamp = true,
                "always_clamp" => attrs.numeric.always_clamp = true,
                "wrap_around" => attrs.numeric.wrap_around = true,
                "no_round_to_format" => attrs.numeric.no_round_to_format = true,
                "no_input" => attrs.numeric.no_input = true,
                "clamp_on_input" => attrs.numeric.clamp_on_input = true,
                "clamp_zero_range" => attrs.numeric.clamp_zero_range = true,
                "no_speed_tweaks" => attrs.numeric.no_speed_tweaks = true,
                "multiline" => attrs.text.multiline = true,
                "lines" => attrs.text.lines = Some(meta.value()?.parse()?),
                "hint" => attrs.text.hint = Some(meta.value()?.parse()?),
                "display_only" => attrs.text.display_only = true,
                "auto_resize" => attrs.text.auto_resize = true,
                "min_width" => attrs.text.min_width = Some(meta.value()?.parse()?),
                "tuple_render" => attrs.tuple.render = Some(meta.value()?.parse()?),
                "tuple_dropdown" => attrs.tuple.dropdown = true,
                "tuple_columns" => attrs.tuple.columns = Some(meta.value()?.parse()?),
                "tuple_min_width" => attrs.tuple.min_width = Some(meta.value()?.parse()?),
                "bool_style" => attrs.boolean.style = Some(meta.value()?.parse()?),
                "true_text" => attrs.boolean.true_text = Some(meta.value()?.parse()?),
                "false_text" => attrs.boolean.false_text = Some(meta.value()?.parse()?),
                _ => return Err(meta.error("unknown imgui field attribute")),
            }
            Ok(())
        })?;
    }
    if attrs.skip && seen.len() > 1 {
        return Err(Error::new_spanned(
            &field.ty,
            "imgui(skip) cannot be combined with other field attributes",
        ));
    }
    Ok(attrs)
}

pub fn reject_struct_attrs(attrs: &[Attribute]) -> Result<()> {
    for attr in imgui_attrs(attrs) {
        attr.parse_nested_meta(|meta| Err(meta.error("unknown imgui struct attribute")))?;
    }
    Ok(())
}

pub fn parse_enum_style(attrs: &[Attribute]) -> Result<bool> {
    match parse_scope_attr(attrs, "enum_style", "enum")? {
        Some(style) if style.value() == "radio" => Ok(true),
        Some(style) if style.value() != "dropdown" => Err(Error::new(
            style.span(),
            "imgui(enum_style) must be \"dropdown\" or \"radio\"",
        )),
        _ => Ok(false),
    }
}

pub fn parse_variant_name(attrs: &[Attribute]) -> Result<Option<LitStr>> {
    parse_scope_attr(attrs, "name", "variant")
}

fn parse_scope_attr(attrs: &[Attribute], key: &str, scope: &str) -> Result<Option<LitStr>> {
    let mut value = None;
    for attr in imgui_attrs(attrs) {
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident(key) {
                return Err(meta.error(format!("unknown imgui {scope} attribute")));
            }
            if value.is_some() {
                return Err(meta.error(format!("duplicate imgui {scope} attribute")));
            }
            value = Some(meta.value()?.parse()?);
            Ok(())
        })?;
    }
    Ok(value)
}

fn imgui_attrs(attrs: &[Attribute]) -> impl Iterator<Item = &Attribute> {
    attrs.iter().filter(|attr| attr.path().is_ident("imgui"))
}
