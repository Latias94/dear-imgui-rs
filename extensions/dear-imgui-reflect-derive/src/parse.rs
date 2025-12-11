use syn::{Expr, Field, Ident, LitStr, Result};

/// Parsed `#[imgui(...)]` attributes for a single struct field.
#[derive(Default)]
pub struct FieldAttrs {
    pub skip: bool,
    pub label_override: Option<LitStr>,
    // Numeric configuration
    pub slider: bool,
    pub slider_default_range: bool,
    pub as_input: bool,
    pub as_drag: bool,
    pub min_expr: Option<Expr>,
    pub max_expr: Option<Expr>,
    pub format_str: Option<LitStr>,
    pub fmt_hex: bool,
    pub fmt_percentage: bool,
    pub fmt_scientific: bool,
    pub fmt_prefix: Option<LitStr>,
    pub fmt_suffix: Option<LitStr>,
    pub step_expr: Option<Expr>,
    pub step_fast_expr: Option<Expr>,
    pub speed_expr: Option<Expr>,
    pub log_scale: bool,
    pub clamp_manual: bool,
    pub always_clamp_flag: bool,
    pub wrap_around_flag: bool,
    pub no_round_to_format: bool,
    pub no_input: bool,
    pub clamp_on_input: bool,
    pub clamp_zero_range: bool,
    pub no_speed_tweaks: bool,
    // Text configuration
    pub multiline: bool,
    pub lines_expr: Option<Expr>,
    pub hint_str: Option<LitStr>,
    pub read_only: bool,
    pub display_only: bool,
    pub auto_resize: bool,
    pub min_width_expr: Option<Expr>,
    // Tuple layout configuration
    pub tuple_render: Option<String>,
    pub tuple_dropdown: bool,
    pub tuple_columns_expr: Option<Expr>,
    pub tuple_min_width_expr: Option<Expr>,
    // Bool configuration
    pub bool_style: Option<String>,
    pub true_text: Option<LitStr>,
    pub false_text: Option<LitStr>,
}

/// Parses all `#[imgui(...)]` attributes on a field into a `FieldAttrs` struct.
///
/// This keeps attribute parsing separate from type-based validation and
/// code generation, which remain in `lib.rs`.
pub fn parse_field_attrs(_field_ident: &Ident, field: &Field) -> Result<FieldAttrs> {
    let mut attrs = FieldAttrs::default();

    for attr in field.attrs.iter().filter(|a| a.path().is_ident("imgui")) {
        let res = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") {
                attrs.skip = true;
                return Ok(());
            }

            if meta.path.is_ident("name") {
                let lit: LitStr = meta.value()?.parse()?;
                attrs.label_override = Some(lit);
                return Ok(());
            }

            if meta.path.is_ident("slider") {
                attrs.slider = true;
                return Ok(());
            }

            if meta.path.is_ident("slider_default_range") {
                attrs.slider_default_range = true;
                return Ok(());
            }

            if meta.path.is_ident("as_input") {
                attrs.as_input = true;
                return Ok(());
            }

            if meta.path.is_ident("as_drag") {
                attrs.as_drag = true;
                return Ok(());
            }

            if meta.path.is_ident("min") {
                let expr: Expr = meta.value()?.parse()?;
                attrs.min_expr = Some(expr);
                return Ok(());
            }

            if meta.path.is_ident("max") {
                let expr: Expr = meta.value()?.parse()?;
                attrs.max_expr = Some(expr);
                return Ok(());
            }

            if meta.path.is_ident("format") {
                let lit: LitStr = meta.value()?.parse()?;
                attrs.format_str = Some(lit);
                return Ok(());
            }

            if meta.path.is_ident("hex") {
                attrs.fmt_hex = true;
                return Ok(());
            }

            if meta.path.is_ident("percentage") {
                attrs.fmt_percentage = true;
                return Ok(());
            }

            if meta.path.is_ident("scientific") {
                attrs.fmt_scientific = true;
                return Ok(());
            }

            if meta.path.is_ident("prefix") {
                let lit: LitStr = meta.value()?.parse()?;
                attrs.fmt_prefix = Some(lit);
                return Ok(());
            }

            if meta.path.is_ident("suffix") {
                let lit: LitStr = meta.value()?.parse()?;
                attrs.fmt_suffix = Some(lit);
                return Ok(());
            }

            if meta.path.is_ident("step") {
                let expr: Expr = meta.value()?.parse()?;
                attrs.step_expr = Some(expr);
                return Ok(());
            }

            if meta.path.is_ident("step_fast") {
                let expr: Expr = meta.value()?.parse()?;
                attrs.step_fast_expr = Some(expr);
                return Ok(());
            }

            if meta.path.is_ident("speed") {
                let expr: Expr = meta.value()?.parse()?;
                attrs.speed_expr = Some(expr);
                return Ok(());
            }

            if meta.path.is_ident("log") {
                attrs.log_scale = true;
                return Ok(());
            }

            if meta.path.is_ident("clamp") {
                attrs.clamp_manual = true;
                return Ok(());
            }

            if meta.path.is_ident("always_clamp") {
                attrs.always_clamp_flag = true;
                return Ok(());
            }

            if meta.path.is_ident("wrap_around") {
                attrs.wrap_around_flag = true;
                return Ok(());
            }

            if meta.path.is_ident("no_round_to_format") {
                attrs.no_round_to_format = true;
                return Ok(());
            }

            if meta.path.is_ident("no_input") {
                attrs.no_input = true;
                return Ok(());
            }

            if meta.path.is_ident("clamp_on_input") {
                attrs.clamp_on_input = true;
                return Ok(());
            }

            if meta.path.is_ident("clamp_zero_range") {
                attrs.clamp_zero_range = true;
                return Ok(());
            }

            if meta.path.is_ident("no_speed_tweaks") {
                attrs.no_speed_tweaks = true;
                return Ok(());
            }

            if meta.path.is_ident("multiline") {
                attrs.multiline = true;
                return Ok(());
            }

            if meta.path.is_ident("lines") {
                let expr: Expr = meta.value()?.parse()?;
                attrs.lines_expr = Some(expr);
                return Ok(());
            }

            if meta.path.is_ident("hint") {
                let lit: LitStr = meta.value()?.parse()?;
                attrs.hint_str = Some(lit);
                return Ok(());
            }

            if meta.path.is_ident("read_only") {
                attrs.read_only = true;
                return Ok(());
            }

            if meta.path.is_ident("display_only") {
                attrs.display_only = true;
                return Ok(());
            }

            if meta.path.is_ident("auto_resize") {
                attrs.auto_resize = true;
                return Ok(());
            }

            if meta.path.is_ident("min_width") {
                let expr: Expr = meta.value()?.parse()?;
                attrs.min_width_expr = Some(expr);
                return Ok(());
            }

            if meta.path.is_ident("bool_style") {
                let lit: LitStr = meta.value()?.parse()?;
                attrs.bool_style = Some(lit.value());
                return Ok(());
            }

            if meta.path.is_ident("tuple_render") {
                let lit: LitStr = meta.value()?.parse()?;
                let v = lit.value();
                if v != "line" && v != "grid" {
                    return Err(
                        meta.error("imgui(tuple_render = ...) must be \"line\" or \"grid\"")
                    );
                }
                attrs.tuple_render = Some(v);
                return Ok(());
            }

            if meta.path.is_ident("tuple_dropdown") {
                attrs.tuple_dropdown = true;
                return Ok(());
            }

            if meta.path.is_ident("tuple_columns") {
                let expr: Expr = meta.value()?.parse()?;
                attrs.tuple_columns_expr = Some(expr);
                return Ok(());
            }

            if meta.path.is_ident("tuple_min_width") {
                let expr: Expr = meta.value()?.parse()?;
                attrs.tuple_min_width_expr = Some(expr);
                return Ok(());
            }

            if meta.path.is_ident("true_text") {
                let lit: LitStr = meta.value()?.parse()?;
                attrs.true_text = Some(lit);
                return Ok(());
            }

            if meta.path.is_ident("false_text") {
                let lit: LitStr = meta.value()?.parse()?;
                attrs.false_text = Some(lit);
                return Ok(());
            }

            // Ignore unknown keys for forward compatibility.
            Ok(())
        });

        if let Err(err) = res {
            return Err(err);
        }
    }

    // For now there is no additional validation that depends only on attributes
    // and not on the Rust field type; type-based validation remains in lib.rs.

    Ok(attrs)
}
