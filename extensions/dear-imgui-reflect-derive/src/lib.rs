use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Type, parse_macro_input, parse_quote};

mod codegen;
mod internal;
mod parse;

use crate::internal::{
    FieldTypeKind, NumericTypeTag, NumericWidgetKind, classify_field_type, classify_numeric_type,
};
use crate::parse::{FieldAttrs, parse_field_attrs};

/// Derive macro for [`dear_imgui_reflect::ImGuiReflect`].
///
/// Currently supports:
///
/// - Structs with named fields. Each field must implement
///   [`dear_imgui_reflect::ImGuiValue`], either directly or via a blanket
///   implementation (for example, another type that implements
///   [`dear_imgui_reflect::ImGuiReflect`]).
/// - C-like enums (no payload). Variants are edited via a combo box.
///
/// Supported field attributes:
///
/// - `#[imgui(skip)]` — do not generate any UI for this field.
/// - `#[imgui(name = \"Custom Label\")]` — override the label used for this field.
/// - `#[imgui(slider, min = ..., max = ..., format = \"...\")]` — use a slider
///   with the given range/format for numeric fields.
/// - `#[imgui(multiline, hint = \"...\", read_only)]` — use multiline text
///   widgets for String/ImString fields.
#[proc_macro_derive(ImGuiReflect, attributes(imgui))]
pub fn derive_imgui_reflect(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ident = input.ident;
    let generics = input.generics;
    let attrs = input.attrs;

    match input.data {
        Data::Struct(data) => derive_for_struct(ident, generics, data),
        Data::Enum(data) => codegen::derive_for_enum(ident, generics, attrs, data),
        Data::Union(u) => {
            syn::Error::new_spanned(u.union_token, "ImGuiReflect cannot be derived for unions")
                .to_compile_error()
                .into()
        }
    }
}

fn derive_for_struct(
    ident: syn::Ident,
    mut generics: syn::Generics,
    data: syn::DataStruct,
) -> TokenStream {
    let fields = match data.fields {
        Fields::Named(named) => named.named,
        _ => {
            return syn::Error::new_spanned(
                ident,
                "ImGuiReflect currently supports only structs with named fields",
            )
            .to_compile_error()
            .into();
        }
    };

    let mut field_stmts = Vec::new();
    let mut bound_types: Vec<Type> = Vec::new();
    let mut default_range_types: Vec<Type> = Vec::new();

    for field in fields {
        let field_ident = match &field.ident {
            Some(id) => id.clone(),
            None => continue,
        };

        let parsed: FieldAttrs = match parse_field_attrs(&field_ident, &field) {
            Ok(attrs) => attrs,
            Err(err) => return err.to_compile_error().into(),
        };

        let FieldAttrs {
            skip,
            label_override,
            slider,
            slider_default_range,
            as_input,
            as_drag,
            min_expr,
            max_expr,
            format_str,
            fmt_hex,
            fmt_percentage,
            fmt_scientific,
            fmt_prefix,
            fmt_suffix,
            step_expr,
            step_fast_expr,
            speed_expr,
            log_scale,
            clamp_manual,
            always_clamp_flag,
            wrap_around_flag,
            no_round_to_format,
            no_input,
            clamp_on_input,
            clamp_zero_range,
            no_speed_tweaks,
            multiline,
            lines_expr,
            hint_str,
            read_only,
            display_only,
            auto_resize,
            min_width_expr,
            tuple_render,
            tuple_dropdown,
            tuple_columns_expr,
            tuple_min_width_expr,
            bool_style,
            true_text,
            false_text,
        } = parsed;

        if skip {
            continue;
        }

        // Validate combinations
        if (min_expr.is_some() && max_expr.is_none()) || (min_expr.is_none() && max_expr.is_some())
        {
            return syn::Error::new(
                field_ident.span(),
                "imgui(min = ..., max = ...) must specify both min and max",
            )
            .to_compile_error()
            .into();
        }

        let ty = field.ty.clone();
        let kind = classify_field_type(&ty);

        // Additional validation for numeric-format helpers: they are restricted
        // to appropriate primitive numeric types.
        if matches!(kind, FieldTypeKind::Numeric) {
            // Determine whether this numeric type is integral or floating-point.
            let (mut is_float, mut is_int) = (false, false);
            if let Type::Path(tp) = &ty
                && let Some(seg) = tp.path.segments.last()
            {
                let ident = seg.ident.to_string();
                match ident.as_str() {
                    "f32" | "f64" => is_float = true,
                    "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64"
                    | "usize" => is_int = true,
                    _ => {}
                }
            }

            let fmt_style_count = (fmt_hex as u8) + (fmt_percentage as u8) + (fmt_scientific as u8);
            if fmt_style_count > 1 {
                return syn::Error::new(
                    field_ident.span(),
                    "imgui(hex/percentage/scientific) are mutually exclusive; use at most one on the same field",
                )
                .to_compile_error()
                .into();
            }

            if fmt_hex && !is_int {
                return syn::Error::new(
                    field_ident.span(),
                    "imgui(hex) is only supported on integral numeric types",
                )
                .to_compile_error()
                .into();
            }

            if (fmt_percentage || fmt_scientific) && !is_float {
                return syn::Error::new(
                    field_ident.span(),
                    "imgui(percentage/scientific) are only supported on floating-point numeric types",
                )
                .to_compile_error()
                .into();
            }
        }

        // Text-only attributes on non-text fields (read_only is handled separately
        // and allowed on all field kinds).
        if (multiline || hint_str.is_some() || auto_resize || min_width_expr.is_some())
            && !matches!(kind, FieldTypeKind::String | FieldTypeKind::ImString)
        {
            return syn::Error::new(
                field_ident.span(),
                "imgui(text attributes like multiline/hint/auto_resize/min_width are only supported on String/ImString fields",
            )
            .to_compile_error()
            .into();
        }

        // display_only is restricted to text fields.
        if display_only && !matches!(kind, FieldTypeKind::String | FieldTypeKind::ImString) {
            return syn::Error::new(
                field_ident.span(),
                "imgui(display_only) is only supported on String/ImString fields",
            )
            .to_compile_error()
            .into();
        }

        // Tuple-layout attributes on non-tuple fields.
        if (tuple_render.is_some()
            || tuple_dropdown
            || tuple_columns_expr.is_some()
            || tuple_min_width_expr.is_some())
            && !matches!(kind, FieldTypeKind::Tuple)
        {
            return syn::Error::new(
                field_ident.span(),
                "imgui(tuple_render/tuple_dropdown/tuple_columns/tuple_min_width) are only supported on tuple fields",
            )
            .to_compile_error()
            .into();
        }

        if lines_expr.is_some() && !multiline {
            return syn::Error::new(
                field_ident.span(),
                "imgui(lines = ...) currently requires multiline to be set",
            )
            .to_compile_error()
            .into();
        }

        if auto_resize && !multiline {
            return syn::Error::new(
                field_ident.span(),
                "imgui(auto_resize) currently requires multiline to be set",
            )
            .to_compile_error()
            .into();
        }

        if auto_resize && lines_expr.is_some() {
            return syn::Error::new(
                field_ident.span(),
                "imgui(auto_resize) and imgui(lines = ...) cannot currently be used together",
            )
            .to_compile_error()
            .into();
        }

        if auto_resize && min_width_expr.is_some() {
            return syn::Error::new(
                field_ident.span(),
                "imgui(auto_resize) and imgui(min_width = ...) cannot currently be used together",
            )
            .to_compile_error()
            .into();
        }

        // Range/slider/format attributes on obviously non-numeric fields
        if (slider
            || as_input
            || as_drag
            || slider_default_range
            || min_expr.is_some()
            || max_expr.is_some()
            || format_str.is_some()
            || fmt_hex
            || fmt_percentage
            || fmt_scientific
            || fmt_prefix.is_some()
            || fmt_suffix.is_some()
            || step_expr.is_some()
            || step_fast_expr.is_some()
            || speed_expr.is_some()
            || log_scale
            || clamp_manual
            || always_clamp_flag
            || wrap_around_flag
            || no_round_to_format
            || no_input
            || clamp_on_input
            || clamp_zero_range
            || no_speed_tweaks)
            && !matches!(kind, FieldTypeKind::Numeric)
        {
            return syn::Error::new(
                field_ident.span(),
                "imgui(slider/slider_default_range/as_input/as_drag/min/max/format/step/step_fast/speed/log/clamp/always_clamp/wrap_around) attributes are only supported on numeric fields",
            )
            .to_compile_error()
            .into();
        }

        // Bool-only attributes
        if (bool_style.is_some() || true_text.is_some() || false_text.is_some())
            && !matches!(kind, FieldTypeKind::Bool)
        {
            return syn::Error::new(
                field_ident.span(),
                "imgui(bool_style/true_text/false_text) attributes are only supported on bool fields",
            )
            .to_compile_error()
            .into();
        }

        if let Some(ref style) = bool_style
            && style != "checkbox"
            && style != "button"
            && style != "radio"
            && style != "dropdown"
        {
            return syn::Error::new(
                field_ident.span(),
                "imgui(bool_style = ...) must be \"checkbox\", \"button\", \"radio\" or \"dropdown\"",
            )
            .to_compile_error()
            .into();
        }

        let label = if let Some(lit) = label_override {
            quote! { #lit }
        } else {
            let name = field_ident.to_string();
            let lit = syn::LitStr::new(&name, field_ident.span());
            quote! { #lit }
        };

        // Use the original Rust field name (not the label override) as the
        // stable identifier for member-level settings, similar to
        // ImSettings::push_member<&T::field>() in ImReflect.
        let field_name_lit = syn::LitStr::new(&field_ident.to_string(), field_ident.span());

        bound_types.push(ty.clone());
        if slider_default_range {
            default_range_types.push(ty.clone());
        }

        // Decide how to render this field based on attributes and type.
        let inner_stmt = match kind {
            FieldTypeKind::Bool => {
                match codegen::gen_bool_field(
                    &field_ident,
                    &field_name_lit,
                    &label,
                    &bool_style,
                    &true_text,
                    &false_text,
                ) {
                    Ok(tokens) => tokens,
                    Err(err) => return err.to_compile_error().into(),
                }
            }
            FieldTypeKind::String => {
                match codegen::gen_string_field(
                    &field_ident,
                    &label,
                    multiline,
                    display_only,
                    read_only,
                    auto_resize,
                    &min_width_expr,
                    &lines_expr,
                    &hint_str,
                ) {
                    Ok(tokens) => tokens,
                    Err(err) => return err.to_compile_error().into(),
                }
            }
            FieldTypeKind::ImString => {
                match codegen::gen_imstring_field(
                    &field_ident,
                    &label,
                    multiline,
                    display_only,
                    read_only,
                    auto_resize,
                    &min_width_expr,
                    &lines_expr,
                    &hint_str,
                ) {
                    Ok(tokens) => tokens,
                    Err(err) => return err.to_compile_error().into(),
                }
            }
            FieldTypeKind::Numeric => {
                // Basic validation for slider-style hints
                if slider && min_expr.is_none() && max_expr.is_none() {
                    return syn::Error::new(
                        field_ident.span(),
                        "imgui(slider) currently requires both min = ... and max = ... on numeric fields",
                    )
                    .to_compile_error()
                    .into();
                }

                // Decide which numeric widget style to use.
                let mut widget_kind = NumericWidgetKind::Default;

                if as_input {
                    widget_kind = NumericWidgetKind::Input;
                }

                if as_drag {
                    if !matches!(widget_kind, NumericWidgetKind::Default) {
                        return syn::Error::new(
                            field_ident.span(),
                            "imgui(as_drag) cannot be combined with other numeric widget selectors (as_input/slider/min/max)",
                        )
                        .to_compile_error()
                        .into();
                    }
                    widget_kind = NumericWidgetKind::Drag;
                }

                if slider {
                    if !matches!(widget_kind, NumericWidgetKind::Default) {
                        return syn::Error::new(
                            field_ident.span(),
                            "imgui(slider) cannot be combined with imgui(as_input) or imgui(as_drag) on the same field",
                        )
                        .to_compile_error()
                        .into();
                    }
                    widget_kind = NumericWidgetKind::Slider;
                }

                // Slider with default range (no explicit min/max).
                if slider_default_range {
                    if !matches!(widget_kind, NumericWidgetKind::Default) {
                        return syn::Error::new(
                            field_ident.span(),
                            "imgui(slider_default_range) cannot be combined with imgui(as_input) or imgui(as_drag) on the same field",
                        )
                        .to_compile_error()
                        .into();
                    }
                    if min_expr.is_some() || max_expr.is_some() {
                        return syn::Error::new(
                            field_ident.span(),
                            "imgui(slider_default_range) cannot be combined with imgui(min = ...) or imgui(max = ...)",
                        )
                        .to_compile_error()
                        .into();
                    }
                    widget_kind = NumericWidgetKind::Slider;
                }

                // If only a range is provided, default to a slider widget.
                if matches!(widget_kind, NumericWidgetKind::Default)
                    && (min_expr.is_some() || max_expr.is_some())
                {
                    widget_kind = NumericWidgetKind::Slider;
                }

                // Input-style numeric widgets can opt into step / step_fast.
                if step_expr.is_some() || step_fast_expr.is_some() {
                    match widget_kind {
                        NumericWidgetKind::Default => {
                            widget_kind = NumericWidgetKind::Input;
                        }
                        NumericWidgetKind::Input => {}
                        _ => {
                            return syn::Error::new(
                                field_ident.span(),
                                "imgui(step/step_fast) are only supported for input-style widgets (as_input or default); remove slider/as_drag on this field",
                            )
                            .to_compile_error()
                            .into();
                        }
                    }
                }

                // Drag-style widgets can configure a drag speed.
                if speed_expr.is_some() {
                    match widget_kind {
                        NumericWidgetKind::Default => {
                            widget_kind = NumericWidgetKind::Drag;
                        }
                        NumericWidgetKind::Drag => {}
                        _ => {
                            return syn::Error::new(
                                field_ident.span(),
                                "imgui(speed = ...) is only supported for drag-style widgets (as_drag)",
                            )
                            .to_compile_error()
                            .into();
                        }
                    }
                }

                // Slider flags are only meaningful for slider/drag widgets.
                if log_scale
                    || always_clamp_flag
                    || wrap_around_flag
                    || no_round_to_format
                    || no_input
                    || clamp_on_input
                    || clamp_zero_range
                    || no_speed_tweaks
                {
                    match widget_kind {
                        NumericWidgetKind::Slider | NumericWidgetKind::Drag => {}
                        _ => {
                            return syn::Error::new(
                                field_ident.span(),
                                "imgui(log/always_clamp/wrap_around/no_round_to_format/no_input/...) require a slider or drag widget; combine with `slider`/`min`/`max` or `as_drag`",
                            )
                            .to_compile_error()
                            .into();
                        }
                    }
                }

                // Slider widgets always require a range: either explicit min/max
                // or a default numeric range.
                if matches!(widget_kind, NumericWidgetKind::Slider)
                    && !slider_default_range
                    && (min_expr.is_none() || max_expr.is_none())
                {
                    return syn::Error::new(
                        field_ident.span(),
                        "slider widgets currently require both imgui(min = ...) and imgui(max = ...) or imgui(slider_default_range)",
                    )
                    .to_compile_error()
                    .into();
                }

                // Manual clamp currently requires a numeric range: either explicit
                // min/max or a default numeric range for sliders.
                if clamp_manual
                    && !(slider_default_range || (min_expr.is_some() && max_expr.is_some()))
                {
                    return syn::Error::new(
                        field_ident.span(),
                        "imgui(clamp) currently requires either imgui(slider_default_range) or both imgui(min = ...) and imgui(max = ...)",
                    )
                    .to_compile_error()
                    .into();
                }

                // Precompute helpers for numeric format attributes (hex/percentage/scientific,
                // prefix/suffix). These are used by all numeric widget kinds.
                // Compute an effective numeric format string, if any. Explicit
                // `format = "..."` takes precedence; otherwise hex/percentage/
                // scientific/prefix/suffix are combined into a single printf-
                // style format string at compile time.
                let effective_format_lit: Option<syn::LitStr> = {
                    if let Some(lit) = format_str.clone() {
                        Some(lit)
                    } else if fmt_hex
                        || fmt_percentage
                        || fmt_scientific
                        || fmt_prefix.is_some()
                        || fmt_suffix.is_some()
                    {
                        let (mut is_float_ty, mut is_int_ty) = (false, false);
                        if let Type::Path(tp) = &ty
                            && let Some(seg) = tp.path.segments.last()
                        {
                            let ident = seg.ident.to_string();
                            match ident.as_str() {
                                "f32" | "f64" => is_float_ty = true,
                                "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32"
                                | "u64" | "usize" => is_int_ty = true,
                                _ => {}
                            }
                        }

                        let base = if fmt_hex {
                            "%#x"
                        } else if fmt_percentage {
                            "%.2f%%"
                        } else if fmt_scientific {
                            "%e"
                        } else if is_int_ty {
                            "%d"
                        } else if is_float_ty {
                            "%.3f"
                        } else {
                            "%g"
                        };

                        let prefix_val = fmt_prefix.as_ref().map(|l| l.value()).unwrap_or_default();
                        let suffix_val = fmt_suffix.as_ref().map(|l| l.value()).unwrap_or_default();
                        let combined = format!("{prefix_val}{base}{suffix_val}");
                        Some(syn::LitStr::new(&combined, field_ident.span()))
                    } else {
                        None
                    }
                };

                match widget_kind {
                    NumericWidgetKind::Input => {
                        let step = step_expr.clone();
                        let step_fast = step_fast_expr.clone();

                        let fmt_stmt = if let Some(f) = effective_format_lit.clone() {
                            quote! { builder = builder.display_format(#f); }
                        } else {
                            quote! {}
                        };
                        let step_stmt = if let Some(s) = step {
                            quote! { builder = builder.step(#s); }
                        } else {
                            quote! {}
                        };
                        let step_fast_stmt = if let Some(sf) = step_fast {
                            quote! { builder = builder.step_fast(#sf); }
                        } else {
                            quote! {}
                        };

                        quote! {
                            {
                                let mut builder = ui.input_scalar(#label, &mut self.#field_ident);
                                #fmt_stmt
                                #step_stmt
                                #step_fast_stmt
                                __changed |= builder.build();
                            }
                        }
                    }
                    NumericWidgetKind::Slider => {
                        let fmt_stmt = if let Some(f) = effective_format_lit.clone() {
                            quote! { slider = slider.display_format(#f); }
                        } else {
                            quote! {}
                        };

                        let flags_stmt = if log_scale
                            || always_clamp_flag
                            || wrap_around_flag
                            || no_round_to_format
                            || no_input
                            || clamp_on_input
                            || clamp_zero_range
                            || no_speed_tweaks
                        {
                            let log_stmt = if log_scale {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::SliderFlags::LOGARITHMIC;
                                }
                            } else {
                                quote! {}
                            };
                            let clamp_stmt = if always_clamp_flag {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::SliderFlags::ALWAYS_CLAMP;
                                }
                            } else {
                                quote! {}
                            };
                            let wrap_stmt = if wrap_around_flag {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::SliderFlags::WRAP_AROUND;
                                }
                            } else {
                                quote! {}
                            };
                            let no_round_stmt = if no_round_to_format {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_ROUND_TO_FORMAT;
                                }
                            } else {
                                quote! {}
                            };
                            let no_input_stmt = if no_input {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_INPUT;
                                }
                            } else {
                                quote! {}
                            };
                            let clamp_on_input_stmt = if clamp_on_input {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ON_INPUT;
                                }
                            } else {
                                quote! {}
                            };
                            let clamp_zero_range_stmt = if clamp_zero_range {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ZERO_RANGE;
                                }
                            } else {
                                quote! {}
                            };
                            let no_speed_tweaks_stmt = if no_speed_tweaks {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_SPEED_TWEAKS;
                                }
                            } else {
                                quote! {}
                            };

                            quote! {
                                let mut flags = ::dear_imgui_reflect::imgui::SliderFlags::NONE;
                                #log_stmt
                                #clamp_stmt
                                #wrap_stmt
                                #no_round_stmt
                                #no_input_stmt
                                #clamp_on_input_stmt
                                #clamp_zero_range_stmt
                                #no_speed_tweaks_stmt
                                slider = slider.flags(flags);
                            }
                        } else {
                            quote! {}
                        };

                        if slider_default_range {
                            // Use type-level default numeric range via NumericDefaultRange.
                            quote! {
                                {
                                    let min = <#ty as ::dear_imgui_reflect::NumericDefaultRange>::default_min();
                                    let max = <#ty as ::dear_imgui_reflect::NumericDefaultRange>::default_max();
                                    let mut slider = ui.slider_config(#label, min, max);
                                    #fmt_stmt
                                    #flags_stmt
                                    let mut local_changed = slider.build(&mut self.#field_ident);
                                    if #clamp_manual {
                                        if self.#field_ident < min {
                                            self.#field_ident = min;
                                            local_changed = true;
                                        }
                                        if self.#field_ident > max {
                                            self.#field_ident = max;
                                            local_changed = true;
                                        }
                                    }
                                    __changed |= local_changed;
                                }
                            }
                        } else {
                            // Explicit min/max range must have been provided at this point.
                            let (min, max) = (min_expr.clone().unwrap(), max_expr.clone().unwrap());

                            quote! {
                                {
                                    let mut slider = ui.slider_config(#label, #min, #max);
                                    #fmt_stmt
                                    #flags_stmt
                                    let mut local_changed = slider.build(&mut self.#field_ident);
                                    if #clamp_manual {
                                        if self.#field_ident < #min {
                                            self.#field_ident = #min;
                                            local_changed = true;
                                        }
                                        if self.#field_ident > #max {
                                            self.#field_ident = #max;
                                            local_changed = true;
                                        }
                                    }
                                    __changed |= local_changed;
                                }
                            }
                        }
                    }
                    NumericWidgetKind::Drag => {
                        let speed = speed_expr.clone();
                        let min_opt = min_expr.clone();
                        let max_opt = max_expr.clone();

                        // Range is optional for drags; only set when both min and max are present.
                        let range_stmt = if let (Some(min), Some(max)) = (min_opt, max_opt) {
                            quote! { drag = drag.range(#min, #max); }
                        } else {
                            quote! {}
                        };

                        let speed_stmt = if let Some(s) = speed {
                            quote! { drag = drag.speed(#s); }
                        } else {
                            quote! {}
                        };

                        let fmt_stmt = if let Some(f) = effective_format_lit.clone() {
                            quote! { drag = drag.display_format(#f); }
                        } else {
                            quote! {}
                        };

                        let flags_stmt = if log_scale
                            || always_clamp_flag
                            || wrap_around_flag
                            || no_round_to_format
                            || no_input
                            || clamp_on_input
                            || clamp_zero_range
                            || no_speed_tweaks
                        {
                            let log_stmt = if log_scale {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::SliderFlags::LOGARITHMIC;
                                }
                            } else {
                                quote! {}
                            };
                            let clamp_stmt = if always_clamp_flag {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::SliderFlags::ALWAYS_CLAMP;
                                }
                            } else {
                                quote! {}
                            };
                            let wrap_stmt = if wrap_around_flag {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::SliderFlags::WRAP_AROUND;
                                }
                            } else {
                                quote! {}
                            };
                            let no_round_stmt = if no_round_to_format {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_ROUND_TO_FORMAT;
                                }
                            } else {
                                quote! {}
                            };
                            let no_input_stmt = if no_input {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_INPUT;
                                }
                            } else {
                                quote! {}
                            };
                            let clamp_on_input_stmt = if clamp_on_input {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ON_INPUT;
                                }
                            } else {
                                quote! {}
                            };
                            let clamp_zero_range_stmt = if clamp_zero_range {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ZERO_RANGE;
                                }
                            } else {
                                quote! {}
                            };
                            let no_speed_tweaks_stmt = if no_speed_tweaks {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_SPEED_TWEAKS;
                                }
                            } else {
                                quote! {}
                            };

                            quote! {
                                let mut flags = ::dear_imgui_reflect::imgui::SliderFlags::NONE;
                                #log_stmt
                                #clamp_stmt
                                #wrap_stmt
                                #no_round_stmt
                                #no_input_stmt
                                #clamp_on_input_stmt
                                #clamp_zero_range_stmt
                                #no_speed_tweaks_stmt
                                drag = drag.flags(flags);
                            }
                        } else {
                            quote! {}
                        };

                        quote! {
                            {
                                let mut drag = ui.drag_config(#label);
                                #range_stmt
                                #speed_stmt
                                #fmt_stmt
                                #flags_stmt
                                let local_changed = drag.build(ui, &mut self.#field_ident);
                                __changed |= local_changed;
                            }
                        }
                    }
                    NumericWidgetKind::Default => {
                        match classify_numeric_type(&ty) {
                            Some(NumericTypeTag::I32) => {
                                quote! {
                                    {
                                        use ::dear_imgui_reflect::{
                                            NumericWidgetKind as __NumKind,
                                            NumericRange as __NumRange,
                                        };
                                        let settings = ::dear_imgui_reflect::current_settings();
                                        let numeric: ::dear_imgui_reflect::NumericTypeSettings = {
                                            if let Some(member) = settings.member::<Self>(#field_name_lit) {
                                                if let Some(ref override_settings) = member.numerics_i32 {
                                                    override_settings.clone()
                                                } else {
                                                    settings.numerics_i32().clone()
                                                }
                                            } else {
                                                settings.numerics_i32().clone()
                                            }
                                        };
                                        match numeric.widget {
                                            __NumKind::Input => {
                                                let mut builder = ui.input_scalar(#label, &mut self.#field_ident);
                                                if let Some(ref fmt) = numeric.format {
                                                    builder = builder.display_format(fmt);
                                                }
                                                if let Some(step) = numeric.step {
                                                    builder = builder.step(step as _);
                                                }
                                                if let Some(step_fast) = numeric.step_fast {
                                                    builder = builder.step_fast(step_fast as _);
                                                }
                                                __changed |= builder.build();
                                            }
                                            __NumKind::Slider => {
                                                let (min, max) = match numeric.range {
                                                    __NumRange::Explicit { min, max } => (min as i32, max as i32),
                                                    __NumRange::DefaultSlider | __NumRange::None => {
                                                        let min = <i32 as ::dear_imgui_reflect::NumericDefaultRange>::default_min();
                                                        let max = <i32 as ::dear_imgui_reflect::NumericDefaultRange>::default_max();
                                                        (min, max)
                                                    }
                                                };
                                                let mut slider = ui.slider_config(#label, min, max);
                                                if let Some(ref fmt) = numeric.format {
                                                    slider = slider.display_format(fmt);
                                                }
                                                if numeric.log
                                                    || numeric.always_clamp
                                                    || numeric.wrap_around
                                                    || numeric.no_round_to_format
                                                    || numeric.no_input
                                                    || numeric.clamp_on_input
                                                    || numeric.clamp_zero_range
                                                    || numeric.no_speed_tweaks
                                                {
                                                    let mut flags = ::dear_imgui_reflect::imgui::SliderFlags::NONE;
                                                    if numeric.log {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::LOGARITHMIC;
                                                    }
                                                    if numeric.always_clamp {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::ALWAYS_CLAMP;
                                                    }
                                                    if numeric.wrap_around {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::WRAP_AROUND;
                                                    }
                                                    if numeric.no_round_to_format {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_ROUND_TO_FORMAT;
                                                    }
                                                    if numeric.no_input {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_INPUT;
                                                    }
                                                    if numeric.clamp_on_input {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ON_INPUT;
                                                    }
                                                    if numeric.clamp_zero_range {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ZERO_RANGE;
                                                    }
                                                    if numeric.no_speed_tweaks {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_SPEED_TWEAKS;
                                                    }
                                                    slider = slider.flags(flags);
                                                }
                                                let mut local_changed = slider.build(&mut self.#field_ident);
                                                if numeric.clamp {
                                                    if self.#field_ident < min {
                                                        self.#field_ident = min;
                                                        local_changed = true;
                                                    }
                                                    if self.#field_ident > max {
                                                        self.#field_ident = max;
                                                        local_changed = true;
                                                    }
                                                }
                                                __changed |= local_changed;
                                            }
                                            __NumKind::Drag => {
                                                let mut drag = ui.drag_config(#label);
                                                if let Some(speed) = numeric.speed {
                                                    drag = drag.speed(speed as _);
                                                }
                                                // Optional range for drags
                                                match numeric.range {
                                                    __NumRange::Explicit { min, max } => {
                                                        drag = drag.range(min as i32, max as i32);
                                                    }
                                                    __NumRange::DefaultSlider | __NumRange::None => {}
                                                }
                                                if let Some(ref fmt) = numeric.format {
                                                    drag = drag.display_format(fmt);
                                                }
                                                if numeric.log
                                                    || numeric.always_clamp
                                                    || numeric.wrap_around
                                                    || numeric.no_round_to_format
                                                    || numeric.no_input
                                                    || numeric.clamp_on_input
                                                    || numeric.clamp_zero_range
                                                    || numeric.no_speed_tweaks
                                                {
                                                    let mut flags = ::dear_imgui_reflect::imgui::SliderFlags::NONE;
                                                    if numeric.log {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::LOGARITHMIC;
                                                    }
                                                    if numeric.always_clamp {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::ALWAYS_CLAMP;
                                                    }
                                                    if numeric.wrap_around {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::WRAP_AROUND;
                                                    }
                                                    if numeric.no_round_to_format {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_ROUND_TO_FORMAT;
                                                    }
                                                    if numeric.no_input {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_INPUT;
                                                    }
                                                    if numeric.clamp_on_input {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ON_INPUT;
                                                    }
                                                    if numeric.clamp_zero_range {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ZERO_RANGE;
                                                    }
                                                    if numeric.no_speed_tweaks {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_SPEED_TWEAKS;
                                                    }
                                                    drag = drag.flags(flags);
                                                }
                                                let local_changed = drag.build(ui, &mut self.#field_ident);
                                                __changed |= local_changed;
                                            }
                                        }
                                    }
                                }
                            }
                            Some(NumericTypeTag::U32) => {
                                quote! {
                                    {
                                        use ::dear_imgui_reflect::{
                                            NumericWidgetKind as __NumKind,
                                            NumericRange as __NumRange,
                                        };
                                        let settings = ::dear_imgui_reflect::current_settings();
                                        let numeric: ::dear_imgui_reflect::NumericTypeSettings = {
                                            if let Some(member) = settings.member::<Self>(#field_name_lit) {
                                                if let Some(ref override_settings) = member.numerics_u32 {
                                                    override_settings.clone()
                                                } else {
                                                    settings.numerics_u32().clone()
                                                }
                                            } else {
                                                settings.numerics_u32().clone()
                                            }
                                        };
                                        match numeric.widget {
                                            __NumKind::Input => {
                                                let mut builder = ui.input_scalar(#label, &mut self.#field_ident);
                                                if let Some(ref fmt) = numeric.format {
                                                    builder = builder.display_format(fmt);
                                                }
                                                if let Some(step) = numeric.step {
                                                    builder = builder.step(step as _);
                                                }
                                                if let Some(step_fast) = numeric.step_fast {
                                                    builder = builder.step_fast(step_fast as _);
                                                }
                                                __changed |= builder.build();
                                            }
                                            __NumKind::Slider => {
                                                let (min, max) = match numeric.range {
                                                    __NumRange::Explicit { min, max } => (min as u32, max as u32),
                                                    __NumRange::DefaultSlider | __NumRange::None => {
                                                        let min = <u32 as ::dear_imgui_reflect::NumericDefaultRange>::default_min();
                                                        let max = <u32 as ::dear_imgui_reflect::NumericDefaultRange>::default_max();
                                                        (min, max)
                                                    }
                                                };
                                                let mut slider = ui.slider_config(#label, min, max);
                                                if let Some(ref fmt) = numeric.format {
                                                    slider = slider.display_format(fmt);
                                                }
                                                if numeric.log
                                                    || numeric.always_clamp
                                                    || numeric.wrap_around
                                                    || numeric.no_round_to_format
                                                    || numeric.no_input
                                                    || numeric.clamp_on_input
                                                    || numeric.clamp_zero_range
                                                    || numeric.no_speed_tweaks
                                                {
                                                    let mut flags = ::dear_imgui_reflect::imgui::SliderFlags::NONE;
                                                    if numeric.log {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::LOGARITHMIC;
                                                    }
                                                    if numeric.always_clamp {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::ALWAYS_CLAMP;
                                                    }
                                                    if numeric.wrap_around {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::WRAP_AROUND;
                                                    }
                                                    if numeric.no_round_to_format {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_ROUND_TO_FORMAT;
                                                    }
                                                    if numeric.no_input {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_INPUT;
                                                    }
                                                    if numeric.clamp_on_input {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ON_INPUT;
                                                    }
                                                    if numeric.clamp_zero_range {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ZERO_RANGE;
                                                    }
                                                    if numeric.no_speed_tweaks {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_SPEED_TWEAKS;
                                                    }
                                                    slider = slider.flags(flags);
                                                }
                                                let mut local_changed = slider.build(&mut self.#field_ident);
                                                if numeric.clamp {
                                                    if self.#field_ident < min {
                                                        self.#field_ident = min;
                                                        local_changed = true;
                                                    }
                                                    if self.#field_ident > max {
                                                        self.#field_ident = max;
                                                        local_changed = true;
                                                    }
                                                }
                                                __changed |= local_changed;
                                            }
                                            __NumKind::Drag => {
                                                let mut drag = ui.drag_config(#label);
                                                if let Some(speed) = numeric.speed {
                                                    drag = drag.speed(speed as _);
                                                }
                                                match numeric.range {
                                                    __NumRange::Explicit { min, max } => {
                                                        drag = drag.range(min as u32, max as u32);
                                                    }
                                                    __NumRange::DefaultSlider | __NumRange::None => {}
                                                }
                                                if let Some(ref fmt) = numeric.format {
                                                    drag = drag.display_format(fmt);
                                                }
                                                if numeric.log
                                                    || numeric.always_clamp
                                                    || numeric.wrap_around
                                                    || numeric.no_round_to_format
                                                    || numeric.no_input
                                                    || numeric.clamp_on_input
                                                    || numeric.clamp_zero_range
                                                    || numeric.no_speed_tweaks
                                                {
                                                    let mut flags = ::dear_imgui_reflect::imgui::SliderFlags::NONE;
                                                    if numeric.log {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::LOGARITHMIC;
                                                    }
                                                    if numeric.always_clamp {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::ALWAYS_CLAMP;
                                                    }
                                                    if numeric.wrap_around {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::WRAP_AROUND;
                                                    }
                                                    if numeric.no_round_to_format {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_ROUND_TO_FORMAT;
                                                    }
                                                    if numeric.no_input {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_INPUT;
                                                    }
                                                    if numeric.clamp_on_input {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ON_INPUT;
                                                    }
                                                    if numeric.clamp_zero_range {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ZERO_RANGE;
                                                    }
                                                    if numeric.no_speed_tweaks {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_SPEED_TWEAKS;
                                                    }
                                                    drag = drag.flags(flags);
                                                }
                                                let local_changed = drag.build(ui, &mut self.#field_ident);
                                                __changed |= local_changed;
                                            }
                                        }
                                    }
                                }
                            }
                            Some(NumericTypeTag::F32) => {
                                quote! {
                                    {
                                        use ::dear_imgui_reflect::{
                                            NumericWidgetKind as __NumKind,
                                            NumericRange as __NumRange,
                                        };
                                        let settings = ::dear_imgui_reflect::current_settings();
                                        let numeric: ::dear_imgui_reflect::NumericTypeSettings = {
                                            if let Some(member) = settings.member::<Self>(#field_name_lit) {
                                                if let Some(ref override_settings) = member.numerics_f32 {
                                                    override_settings.clone()
                                                } else {
                                                    settings.numerics_f32().clone()
                                                }
                                            } else {
                                                settings.numerics_f32().clone()
                                            }
                                        };
                                        match numeric.widget {
                                            __NumKind::Input => {
                                                let mut builder = ui.input_scalar(#label, &mut self.#field_ident);
                                                if let Some(ref fmt) = numeric.format {
                                                    builder = builder.display_format(fmt);
                                                }
                                                if let Some(step) = numeric.step {
                                                    builder = builder.step(step as _);
                                                }
                                                if let Some(step_fast) = numeric.step_fast {
                                                    builder = builder.step_fast(step_fast as _);
                                                }
                                                __changed |= builder.build();
                                            }
                                            __NumKind::Slider => {
                                                let (min, max) = match numeric.range {
                                                    __NumRange::Explicit { min, max } => (min as f32, max as f32),
                                                    __NumRange::DefaultSlider | __NumRange::None => {
                                                        let min = <f32 as ::dear_imgui_reflect::NumericDefaultRange>::default_min();
                                                        let max = <f32 as ::dear_imgui_reflect::NumericDefaultRange>::default_max();
                                                        (min, max)
                                                    }
                                                };
                                                let mut slider = ui.slider_config(#label, min, max);
                                                if let Some(ref fmt) = numeric.format {
                                                    slider = slider.display_format(fmt);
                                                }
                                                if numeric.log
                                                    || numeric.always_clamp
                                                    || numeric.wrap_around
                                                    || numeric.no_round_to_format
                                                    || numeric.no_input
                                                    || numeric.clamp_on_input
                                                    || numeric.clamp_zero_range
                                                    || numeric.no_speed_tweaks
                                                {
                                                    let mut flags = ::dear_imgui_reflect::imgui::SliderFlags::NONE;
                                                    if numeric.log {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::LOGARITHMIC;
                                                    }
                                                    if numeric.always_clamp {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::ALWAYS_CLAMP;
                                                    }
                                                    if numeric.wrap_around {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::WRAP_AROUND;
                                                    }
                                                    if numeric.no_round_to_format {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_ROUND_TO_FORMAT;
                                                    }
                                                    if numeric.no_input {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_INPUT;
                                                    }
                                                    if numeric.clamp_on_input {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ON_INPUT;
                                                    }
                                                    if numeric.clamp_zero_range {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ZERO_RANGE;
                                                    }
                                                    if numeric.no_speed_tweaks {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_SPEED_TWEAKS;
                                                    }
                                                    slider = slider.flags(flags);
                                                }
                                                let mut local_changed = slider.build(&mut self.#field_ident);
                                                if numeric.clamp {
                                                    if self.#field_ident < min {
                                                        self.#field_ident = min;
                                                        local_changed = true;
                                                    }
                                                    if self.#field_ident > max {
                                                        self.#field_ident = max;
                                                        local_changed = true;
                                                    }
                                                }
                                                __changed |= local_changed;
                                            }
                                            __NumKind::Drag => {
                                                let mut drag = ui.drag_config(#label);
                                                if let Some(speed) = numeric.speed {
                                                    drag = drag.speed(speed as _);
                                                }
                                                match numeric.range {
                                                    __NumRange::Explicit { min, max } => {
                                                        drag = drag.range(min as f32, max as f32);
                                                    }
                                                    __NumRange::DefaultSlider | __NumRange::None => {}
                                                }
                                                if let Some(ref fmt) = numeric.format {
                                                    drag = drag.display_format(fmt);
                                                }
                                                if numeric.log
                                                    || numeric.always_clamp
                                                    || numeric.wrap_around
                                                    || numeric.no_round_to_format
                                                    || numeric.no_input
                                                    || numeric.clamp_on_input
                                                    || numeric.clamp_zero_range
                                                    || numeric.no_speed_tweaks
                                                {
                                                    let mut flags = ::dear_imgui_reflect::imgui::SliderFlags::NONE;
                                                    if numeric.log {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::LOGARITHMIC;
                                                    }
                                                    if numeric.always_clamp {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::ALWAYS_CLAMP;
                                                    }
                                                    if numeric.wrap_around {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::WRAP_AROUND;
                                                    }
                                                    if numeric.no_round_to_format {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_ROUND_TO_FORMAT;
                                                    }
                                                    if numeric.no_input {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_INPUT;
                                                    }
                                                    if numeric.clamp_on_input {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ON_INPUT;
                                                    }
                                                    if numeric.clamp_zero_range {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ZERO_RANGE;
                                                    }
                                                    if numeric.no_speed_tweaks {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_SPEED_TWEAKS;
                                                    }
                                                    drag = drag.flags(flags);
                                                }
                                                let local_changed = drag.build(ui, &mut self.#field_ident);
                                                __changed |= local_changed;
                                            }
                                        }
                                    }
                                }
                            }
                            Some(NumericTypeTag::F64) => {
                                quote! {
                                    {
                                        use ::dear_imgui_reflect::{
                                            NumericWidgetKind as __NumKind,
                                            NumericRange as __NumRange,
                                        };
                                        let settings = ::dear_imgui_reflect::current_settings();
                                        let numeric: ::dear_imgui_reflect::NumericTypeSettings = {
                                            if let Some(member) = settings.member::<Self>(#field_name_lit) {
                                                if let Some(ref override_settings) = member.numerics_f64 {
                                                    override_settings.clone()
                                                } else {
                                                    settings.numerics_f64().clone()
                                                }
                                            } else {
                                                settings.numerics_f64().clone()
                                            }
                                        };
                                        match numeric.widget {
                                            __NumKind::Input => {
                                                let mut builder = ui.input_scalar(#label, &mut self.#field_ident);
                                                if let Some(ref fmt) = numeric.format {
                                                    builder = builder.display_format(fmt);
                                                }
                                                if let Some(step) = numeric.step {
                                                    builder = builder.step(step as _);
                                                }
                                                if let Some(step_fast) = numeric.step_fast {
                                                    builder = builder.step_fast(step_fast as _);
                                                }
                                                __changed |= builder.build();
                                            }
                                            __NumKind::Slider => {
                                                let (min, max) = match numeric.range {
                                                    __NumRange::Explicit { min, max } => (min, max),
                                                    __NumRange::DefaultSlider | __NumRange::None => {
                                                        let min = <f64 as ::dear_imgui_reflect::NumericDefaultRange>::default_min();
                                                        let max = <f64 as ::dear_imgui_reflect::NumericDefaultRange>::default_max();
                                                        (min, max)
                                                    }
                                                };
                                                let mut slider = ui.slider_config(#label, min, max);
                                                if let Some(ref fmt) = numeric.format {
                                                    slider = slider.display_format(fmt);
                                                }
                                                if numeric.log
                                                    || numeric.always_clamp
                                                    || numeric.wrap_around
                                                    || numeric.no_round_to_format
                                                    || numeric.no_input
                                                    || numeric.clamp_on_input
                                                    || numeric.clamp_zero_range
                                                    || numeric.no_speed_tweaks
                                                {
                                                    let mut flags = ::dear_imgui_reflect::imgui::SliderFlags::NONE;
                                                    if numeric.log {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::LOGARITHMIC;
                                                    }
                                                    if numeric.always_clamp {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::ALWAYS_CLAMP;
                                                    }
                                                    if numeric.wrap_around {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::WRAP_AROUND;
                                                    }
                                                    if numeric.no_round_to_format {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_ROUND_TO_FORMAT;
                                                    }
                                                    if numeric.no_input {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_INPUT;
                                                    }
                                                    if numeric.clamp_on_input {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ON_INPUT;
                                                    }
                                                    if numeric.clamp_zero_range {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ZERO_RANGE;
                                                    }
                                                    if numeric.no_speed_tweaks {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_SPEED_TWEAKS;
                                                    }
                                                    slider = slider.flags(flags);
                                                }
                                                let mut local_changed = slider.build(&mut self.#field_ident);
                                                if numeric.clamp {
                                                    if self.#field_ident < min {
                                                        self.#field_ident = min;
                                                        local_changed = true;
                                                    }
                                                    if self.#field_ident > max {
                                                        self.#field_ident = max;
                                                        local_changed = true;
                                                    }
                                                }
                                                __changed |= local_changed;
                                            }
                                            __NumKind::Drag => {
                                                let mut drag = ui.drag_config(#label);
                                                if let Some(speed) = numeric.speed {
                                                    drag = drag.speed(speed as _);
                                                }
                                                match numeric.range {
                                                    __NumRange::Explicit { min, max } => {
                                                        drag = drag.range(min, max);
                                                    }
                                                    __NumRange::DefaultSlider | __NumRange::None => {}
                                                }
                                                if let Some(ref fmt) = numeric.format {
                                                    drag = drag.display_format(fmt);
                                                }
                                                if numeric.log
                                                    || numeric.always_clamp
                                                    || numeric.wrap_around
                                                    || numeric.no_round_to_format
                                                    || numeric.no_input
                                                    || numeric.clamp_on_input
                                                    || numeric.clamp_zero_range
                                                    || numeric.no_speed_tweaks
                                                {
                                                    let mut flags = ::dear_imgui_reflect::imgui::SliderFlags::NONE;
                                                    if numeric.log {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::LOGARITHMIC;
                                                    }
                                                    if numeric.always_clamp {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::ALWAYS_CLAMP;
                                                    }
                                                    if numeric.wrap_around {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::WRAP_AROUND;
                                                    }
                                                    if numeric.no_round_to_format {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_ROUND_TO_FORMAT;
                                                    }
                                                    if numeric.no_input {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_INPUT;
                                                    }
                                                    if numeric.clamp_on_input {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ON_INPUT;
                                                    }
                                                    if numeric.clamp_zero_range {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ZERO_RANGE;
                                                    }
                                                    if numeric.no_speed_tweaks {
                                                        flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_SPEED_TWEAKS;
                                                    }
                                                    drag = drag.flags(flags);
                                                }
                                                let local_changed = drag.build(ui, &mut self.#field_ident);
                                                __changed |= local_changed;
                                            }
                                        }
                                    }
                                }
                            }
                            None => {
                                quote! {
                                    __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                                        ui,
                                        #label,
                                        &mut self.#field_ident,
                                    );
                                }
                            }
                        }
                    }
                }
            }
            FieldTypeKind::Tuple => {
                let len = match &ty {
                    Type::Tuple(tup) => tup.elems.len(),
                    _ => 0,
                };

                if len == 0 {
                    // Fallback: use the generic ImGuiValue implementation.
                    quote! {
                        __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                            ui,
                            #label,
                            &mut self.#field_ident,
                        );
                    }
                } else {
                    // Build TupleSettings for this member by layering:
                    //  - global ReflectSettings::tuples()
                    //  - optional MemberSettings::tuples for this field
                    //  - optional per-field attributes overriding mode/dropdown/columns/min_width.
                    let _has_columns = tuple_columns_expr.is_some();
                    let _has_min_width = tuple_min_width_expr.is_some();

                    let render_mode_stmt = if let Some(mode) = tuple_render.clone() {
                        if mode == "grid" {
                            quote! {
                                tuple_settings.render_mode =
                                    ::dear_imgui_reflect::TupleRenderMode::Grid;
                            }
                        } else {
                            quote! {
                                tuple_settings.render_mode =
                                    ::dear_imgui_reflect::TupleRenderMode::Line;
                            }
                        }
                    } else {
                        quote! {}
                    };

                    let dropdown_stmt = if tuple_dropdown {
                        quote! {
                            tuple_settings.dropdown = true;
                        }
                    } else {
                        quote! {}
                    };

                    let columns_stmt = if let Some(expr) = tuple_columns_expr.clone() {
                        quote! {
                            tuple_settings.columns = (#expr) as usize;
                        }
                    } else {
                        quote! {}
                    };

                    let min_width_stmt = if let Some(expr) = tuple_min_width_expr.clone() {
                        quote! {
                            tuple_settings.min_width = Some(#expr as f32);
                        }
                    } else {
                        quote! {}
                    };

                    // Generate per-element match arms based on tuple length. Each element
                    // can be controlled independently via member-level settings using
                    // a path of the form `"field_name[index]"`, allowing per-element
                    // read_only and numeric semantics similar to ImReflect.
                    let arms: proc_macro2::TokenStream = if let Type::Tuple(tup) = &ty {
                        let mut per_element_arms = Vec::new();
                        for (index, elem_ty) in tup.elems.iter().enumerate() {
                            let idx = syn::Index::from(index);
                            let element_label =
                                syn::LitStr::new(&format!("##{}", index), field_ident.span());
                            let element_member_name = syn::LitStr::new(
                                &format!("{}[{}]", field_ident, index),
                                field_ident.span(),
                            );

                            // Decide whether this element should use numeric type-level
                            // settings (plus optional per-element overrides) or fall back
                            // to the generic ImGuiValue implementation.
                            let element_body = match classify_numeric_type(elem_ty) {
                                Some(NumericTypeTag::I32) => {
                                    quote! {
                                        {
                                            use ::dear_imgui_reflect::{
                                                NumericWidgetKind as __NumKind,
                                                NumericRange as __NumRange,
                                            };
                                            let settings = ::dear_imgui_reflect::current_settings();
                                            let numeric: ::dear_imgui_reflect::NumericTypeSettings = {
                                                if let Some(member) = settings.member::<Self>(#element_member_name) {
                                                    if let Some(ref override_settings) = member.numerics_i32 {
                                                        override_settings.clone()
                                                    } else {
                                                        settings.numerics_i32().clone()
                                                    }
                                                } else {
                                                    settings.numerics_i32().clone()
                                                }
                                            };
                                            match numeric.widget {
                                                __NumKind::Input => {
                                                    let mut builder = ui.input_scalar(#element_label, &mut self.#field_ident.#idx);
                                                    if let Some(ref fmt) = numeric.format {
                                                        builder = builder.display_format(fmt);
                                                    }
                                                    if let Some(step) = numeric.step {
                                                        builder = builder.step(step as _);
                                                    }
                                                    if let Some(step_fast) = numeric.step_fast {
                                                        builder = builder.step_fast(step_fast as _);
                                                    }
                                                    builder.build()
                                                }
                                                __NumKind::Slider => {
                                                    let (min, max) = match numeric.range {
                                                        __NumRange::Explicit { min, max } => (min as i32, max as i32),
                                                        __NumRange::DefaultSlider | __NumRange::None => {
                                                            let min = <i32 as ::dear_imgui_reflect::NumericDefaultRange>::default_min();
                                                            let max = <i32 as ::dear_imgui_reflect::NumericDefaultRange>::default_max();
                                                            (min, max)
                                                        }
                                                    };
                                                    let mut slider = ui.slider_config(#element_label, min, max);
                                                    if let Some(ref fmt) = numeric.format {
                                                        slider = slider.display_format(fmt);
                                                    }
                                                    if numeric.log
                                                        || numeric.always_clamp
                                                        || numeric.wrap_around
                                                        || numeric.no_round_to_format
                                                        || numeric.no_input
                                                        || numeric.clamp_on_input
                                                        || numeric.clamp_zero_range
                                                        || numeric.no_speed_tweaks
                                                    {
                                                        let mut flags = ::dear_imgui_reflect::imgui::SliderFlags::NONE;
                                                        if numeric.log {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::LOGARITHMIC;
                                                        }
                                                        if numeric.always_clamp {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::ALWAYS_CLAMP;
                                                        }
                                                        if numeric.wrap_around {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::WRAP_AROUND;
                                                        }
                                                        if numeric.no_round_to_format {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_ROUND_TO_FORMAT;
                                                        }
                                                        if numeric.no_input {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_INPUT;
                                                        }
                                                        if numeric.clamp_on_input {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ON_INPUT;
                                                        }
                                                        if numeric.clamp_zero_range {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ZERO_RANGE;
                                                        }
                                                        if numeric.no_speed_tweaks {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_SPEED_TWEAKS;
                                                        }
                                                        slider = slider.flags(flags);
                                                    }
                                                    let mut local_changed = slider.build(&mut self.#field_ident.#idx);
                                                    if numeric.clamp {
                                                        if self.#field_ident.#idx < min {
                                                            self.#field_ident.#idx = min;
                                                            local_changed = true;
                                                        }
                                                        if self.#field_ident.#idx > max {
                                                            self.#field_ident.#idx = max;
                                                            local_changed = true;
                                                        }
                                                    }
                                                    local_changed
                                                }
                                                __NumKind::Drag => {
                                                    let mut drag = ui.drag_config(#element_label);
                                                    if let Some(speed) = numeric.speed {
                                                        drag = drag.speed(speed as _);
                                                    }
                                                    match numeric.range {
                                                        __NumRange::Explicit { min, max } => {
                                                            drag = drag.range(min as i32, max as i32);
                                                        }
                                                        __NumRange::DefaultSlider | __NumRange::None => {}
                                                    }
                                                    if let Some(ref fmt) = numeric.format {
                                                        drag = drag.display_format(fmt);
                                                    }
                                                    if numeric.log
                                                        || numeric.always_clamp
                                                        || numeric.wrap_around
                                                        || numeric.no_round_to_format
                                                        || numeric.no_input
                                                        || numeric.clamp_on_input
                                                        || numeric.clamp_zero_range
                                                        || numeric.no_speed_tweaks
                                                    {
                                                        let mut flags = ::dear_imgui_reflect::imgui::SliderFlags::NONE;
                                                        if numeric.log {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::LOGARITHMIC;
                                                        }
                                                        if numeric.always_clamp {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::ALWAYS_CLAMP;
                                                        }
                                                        if numeric.wrap_around {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::WRAP_AROUND;
                                                        }
                                                        if numeric.no_round_to_format {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_ROUND_TO_FORMAT;
                                                        }
                                                        if numeric.no_input {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_INPUT;
                                                        }
                                                        if numeric.clamp_on_input {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ON_INPUT;
                                                        }
                                                        if numeric.clamp_zero_range {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ZERO_RANGE;
                                                        }
                                                        if numeric.no_speed_tweaks {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_SPEED_TWEAKS;
                                                        }
                                                        drag = drag.flags(flags);
                                                    }
                                                    drag.build(ui, &mut self.#field_ident.#idx)
                                                }
                                            }
                                        }
                                    }
                                }
                                Some(NumericTypeTag::U32) => {
                                    quote! {
                                        {
                                            use ::dear_imgui_reflect::{
                                                NumericWidgetKind as __NumKind,
                                                NumericRange as __NumRange,
                                            };
                                            let settings = ::dear_imgui_reflect::current_settings();
                                            let numeric: ::dear_imgui_reflect::NumericTypeSettings = {
                                                if let Some(member) = settings.member::<Self>(#element_member_name) {
                                                    if let Some(ref override_settings) = member.numerics_u32 {
                                                        override_settings.clone()
                                                    } else {
                                                        settings.numerics_u32().clone()
                                                    }
                                                } else {
                                                    settings.numerics_u32().clone()
                                                }
                                            };
                                            match numeric.widget {
                                                __NumKind::Input => {
                                                    let mut builder = ui.input_scalar(#element_label, &mut self.#field_ident.#idx);
                                                    if let Some(ref fmt) = numeric.format {
                                                        builder = builder.display_format(fmt);
                                                    }
                                                    if let Some(step) = numeric.step {
                                                        builder = builder.step(step as _);
                                                    }
                                                    if let Some(step_fast) = numeric.step_fast {
                                                        builder = builder.step_fast(step_fast as _);
                                                    }
                                                    builder.build()
                                                }
                                                __NumKind::Slider => {
                                                    let (min, max) = match numeric.range {
                                                        __NumRange::Explicit { min, max } => (min as u32, max as u32),
                                                        __NumRange::DefaultSlider | __NumRange::None => {
                                                            let min = <u32 as ::dear_imgui_reflect::NumericDefaultRange>::default_min();
                                                            let max = <u32 as ::dear_imgui_reflect::NumericDefaultRange>::default_max();
                                                            (min, max)
                                                        }
                                                    };
                                                    let mut slider = ui.slider_config(#element_label, min, max);
                                                    if let Some(ref fmt) = numeric.format {
                                                        slider = slider.display_format(fmt);
                                                    }
                                                    if numeric.log
                                                        || numeric.always_clamp
                                                        || numeric.wrap_around
                                                        || numeric.no_round_to_format
                                                        || numeric.no_input
                                                        || numeric.clamp_on_input
                                                        || numeric.clamp_zero_range
                                                        || numeric.no_speed_tweaks
                                                    {
                                                        let mut flags = ::dear_imgui_reflect::imgui::SliderFlags::NONE;
                                                        if numeric.log {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::LOGARITHMIC;
                                                        }
                                                        if numeric.always_clamp {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::ALWAYS_CLAMP;
                                                        }
                                                        if numeric.wrap_around {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::WRAP_AROUND;
                                                        }
                                                        if numeric.no_round_to_format {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_ROUND_TO_FORMAT;
                                                        }
                                                        if numeric.no_input {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_INPUT;
                                                        }
                                                        if numeric.clamp_on_input {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ON_INPUT;
                                                        }
                                                        if numeric.clamp_zero_range {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ZERO_RANGE;
                                                        }
                                                        if numeric.no_speed_tweaks {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_SPEED_TWEAKS;
                                                        }
                                                        slider = slider.flags(flags);
                                                    }
                                                    let mut local_changed = slider.build(&mut self.#field_ident.#idx);
                                                    if numeric.clamp {
                                                        if self.#field_ident.#idx < min {
                                                            self.#field_ident.#idx = min;
                                                            local_changed = true;
                                                        }
                                                        if self.#field_ident.#idx > max {
                                                            self.#field_ident.#idx = max;
                                                            local_changed = true;
                                                        }
                                                    }
                                                    local_changed
                                                }
                                                __NumKind::Drag => {
                                                    let mut drag = ui.drag_config(#element_label);
                                                    if let Some(speed) = numeric.speed {
                                                        drag = drag.speed(speed as _);
                                                    }
                                                    match numeric.range {
                                                        __NumRange::Explicit { min, max } => {
                                                            drag = drag.range(min as u32, max as u32);
                                                        }
                                                        __NumRange::DefaultSlider | __NumRange::None => {}
                                                    }
                                                    if let Some(ref fmt) = numeric.format {
                                                        drag = drag.display_format(fmt);
                                                    }
                                                    if numeric.log
                                                        || numeric.always_clamp
                                                        || numeric.wrap_around
                                                        || numeric.no_round_to_format
                                                        || numeric.no_input
                                                        || numeric.clamp_on_input
                                                        || numeric.clamp_zero_range
                                                        || numeric.no_speed_tweaks
                                                    {
                                                        let mut flags = ::dear_imgui_reflect::imgui::SliderFlags::NONE;
                                                        if numeric.log {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::LOGARITHMIC;
                                                        }
                                                        if numeric.always_clamp {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::ALWAYS_CLAMP;
                                                        }
                                                        if numeric.wrap_around {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::WRAP_AROUND;
                                                        }
                                                        if numeric.no_round_to_format {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_ROUND_TO_FORMAT;
                                                        }
                                                        if numeric.no_input {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_INPUT;
                                                        }
                                                        if numeric.clamp_on_input {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ON_INPUT;
                                                        }
                                                        if numeric.clamp_zero_range {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ZERO_RANGE;
                                                        }
                                                        if numeric.no_speed_tweaks {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_SPEED_TWEAKS;
                                                        }
                                                        drag = drag.flags(flags);
                                                    }
                                                    drag.build(ui, &mut self.#field_ident.#idx)
                                                }
                                            }
                                        }
                                    }
                                }
                                Some(NumericTypeTag::F32) => {
                                    quote! {
                                        {
                                            use ::dear_imgui_reflect::{
                                                NumericWidgetKind as __NumKind,
                                                NumericRange as __NumRange,
                                            };
                                            let settings = ::dear_imgui_reflect::current_settings();
                                            let numeric: ::dear_imgui_reflect::NumericTypeSettings = {
                                                if let Some(member) = settings.member::<Self>(#element_member_name) {
                                                    if let Some(ref override_settings) = member.numerics_f32 {
                                                        override_settings.clone()
                                                    } else {
                                                        settings.numerics_f32().clone()
                                                    }
                                                } else {
                                                    settings.numerics_f32().clone()
                                                }
                                            };
                                            match numeric.widget {
                                                __NumKind::Input => {
                                                    let mut builder = ui.input_scalar(#element_label, &mut self.#field_ident.#idx);
                                                    if let Some(ref fmt) = numeric.format {
                                                        builder = builder.display_format(fmt);
                                                    }
                                                    if let Some(step) = numeric.step {
                                                        builder = builder.step(step as _);
                                                    }
                                                    if let Some(step_fast) = numeric.step_fast {
                                                        builder = builder.step_fast(step_fast as _);
                                                    }
                                                    builder.build()
                                                }
                                                __NumKind::Slider => {
                                                    let (min, max) = match numeric.range {
                                                        __NumRange::Explicit { min, max } => (min as f32, max as f32),
                                                        __NumRange::DefaultSlider | __NumRange::None => {
                                                            let min = <f32 as ::dear_imgui_reflect::NumericDefaultRange>::default_min();
                                                            let max = <f32 as ::dear_imgui_reflect::NumericDefaultRange>::default_max();
                                                            (min, max)
                                                        }
                                                    };
                                                    let mut slider = ui.slider_config(#element_label, min, max);
                                                    if let Some(ref fmt) = numeric.format {
                                                        slider = slider.display_format(fmt);
                                                    }
                                                    if numeric.log
                                                        || numeric.always_clamp
                                                        || numeric.wrap_around
                                                        || numeric.no_round_to_format
                                                        || numeric.no_input
                                                        || numeric.clamp_on_input
                                                        || numeric.clamp_zero_range
                                                        || numeric.no_speed_tweaks
                                                    {
                                                        let mut flags = ::dear_imgui_reflect::imgui::SliderFlags::NONE;
                                                        if numeric.log {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::LOGARITHMIC;
                                                        }
                                                        if numeric.always_clamp {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::ALWAYS_CLAMP;
                                                        }
                                                        if numeric.wrap_around {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::WRAP_AROUND;
                                                        }
                                                        if numeric.no_round_to_format {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_ROUND_TO_FORMAT;
                                                        }
                                                        if numeric.no_input {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_INPUT;
                                                        }
                                                        if numeric.clamp_on_input {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ON_INPUT;
                                                        }
                                                        if numeric.clamp_zero_range {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ZERO_RANGE;
                                                        }
                                                        if numeric.no_speed_tweaks {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_SPEED_TWEAKS;
                                                        }
                                                        slider = slider.flags(flags);
                                                    }
                                                    let mut local_changed = slider.build(&mut self.#field_ident.#idx);
                                                    if numeric.clamp {
                                                        if self.#field_ident.#idx < min {
                                                            self.#field_ident.#idx = min;
                                                            local_changed = true;
                                                        }
                                                        if self.#field_ident.#idx > max {
                                                            self.#field_ident.#idx = max;
                                                            local_changed = true;
                                                        }
                                                    }
                                                    local_changed
                                                }
                                                __NumKind::Drag => {
                                                    let mut drag = ui.drag_config(#element_label);
                                                    if let Some(speed) = numeric.speed {
                                                        drag = drag.speed(speed as _);
                                                    }
                                                    match numeric.range {
                                                        __NumRange::Explicit { min, max } => {
                                                            drag = drag.range(min as f32, max as f32);
                                                        }
                                                        __NumRange::DefaultSlider | __NumRange::None => {}
                                                    }
                                                    if let Some(ref fmt) = numeric.format {
                                                        drag = drag.display_format(fmt);
                                                    }
                                                    if numeric.log
                                                        || numeric.always_clamp
                                                        || numeric.wrap_around
                                                        || numeric.no_round_to_format
                                                        || numeric.no_input
                                                        || numeric.clamp_on_input
                                                        || numeric.clamp_zero_range
                                                        || numeric.no_speed_tweaks
                                                    {
                                                        let mut flags = ::dear_imgui_reflect::imgui::SliderFlags::NONE;
                                                        if numeric.log {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::LOGARITHMIC;
                                                        }
                                                        if numeric.always_clamp {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::ALWAYS_CLAMP;
                                                        }
                                                        if numeric.wrap_around {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::WRAP_AROUND;
                                                        }
                                                        if numeric.no_round_to_format {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_ROUND_TO_FORMAT;
                                                        }
                                                        if numeric.no_input {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_INPUT;
                                                        }
                                                        if numeric.clamp_on_input {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ON_INPUT;
                                                        }
                                                        if numeric.clamp_zero_range {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ZERO_RANGE;
                                                        }
                                                        if numeric.no_speed_tweaks {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_SPEED_TWEAKS;
                                                        }
                                                        drag = drag.flags(flags);
                                                    }
                                                    drag.build(ui, &mut self.#field_ident.#idx)
                                                }
                                            }
                                        }
                                    }
                                }
                                Some(NumericTypeTag::F64) => {
                                    quote! {
                                        {
                                            use ::dear_imgui_reflect::{
                                                NumericWidgetKind as __NumKind,
                                                NumericRange as __NumRange,
                                            };
                                            let settings = ::dear_imgui_reflect::current_settings();
                                            let numeric: ::dear_imgui_reflect::NumericTypeSettings = {
                                                if let Some(member) = settings.member::<Self>(#element_member_name) {
                                                    if let Some(ref override_settings) = member.numerics_f64 {
                                                        override_settings.clone()
                                                    } else {
                                                        settings.numerics_f64().clone()
                                                    }
                                                } else {
                                                    settings.numerics_f64().clone()
                                                }
                                            };
                                            match numeric.widget {
                                                __NumKind::Input => {
                                                    let mut builder = ui.input_scalar(#element_label, &mut self.#field_ident.#idx);
                                                    if let Some(ref fmt) = numeric.format {
                                                        builder = builder.display_format(fmt);
                                                    }
                                                    if let Some(step) = numeric.step {
                                                        builder = builder.step(step as _);
                                                    }
                                                    if let Some(step_fast) = numeric.step_fast {
                                                        builder = builder.step_fast(step_fast as _);
                                                    }
                                                    builder.build()
                                                }
                                                __NumKind::Slider => {
                                                    let (min, max) = match numeric.range {
                                                        __NumRange::Explicit { min, max } => (min, max),
                                                        __NumRange::DefaultSlider | __NumRange::None => {
                                                            let min = <f64 as ::dear_imgui_reflect::NumericDefaultRange>::default_min();
                                                            let max = <f64 as ::dear_imgui_reflect::NumericDefaultRange>::default_max();
                                                            (min, max)
                                                        }
                                                    };
                                                    let mut slider = ui.slider_config(#element_label, min, max);
                                                    if let Some(ref fmt) = numeric.format {
                                                        slider = slider.display_format(fmt);
                                                    }
                                                    if numeric.log
                                                        || numeric.always_clamp
                                                        || numeric.wrap_around
                                                        || numeric.no_round_to_format
                                                        || numeric.no_input
                                                        || numeric.clamp_on_input
                                                        || numeric.clamp_zero_range
                                                        || numeric.no_speed_tweaks
                                                    {
                                                        let mut flags = ::dear_imgui_reflect::imgui::SliderFlags::NONE;
                                                        if numeric.log {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::LOGARITHMIC;
                                                        }
                                                        if numeric.always_clamp {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::ALWAYS_CLAMP;
                                                        }
                                                        if numeric.wrap_around {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::WRAP_AROUND;
                                                        }
                                                        if numeric.no_round_to_format {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_ROUND_TO_FORMAT;
                                                        }
                                                        if numeric.no_input {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_INPUT;
                                                        }
                                                        if numeric.clamp_on_input {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ON_INPUT;
                                                        }
                                                        if numeric.clamp_zero_range {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ZERO_RANGE;
                                                        }
                                                        if numeric.no_speed_tweaks {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_SPEED_TWEAKS;
                                                        }
                                                        slider = slider.flags(flags);
                                                    }
                                                    let mut local_changed = slider.build(&mut self.#field_ident.#idx);
                                                    if numeric.clamp {
                                                        if (self.#field_ident.#idx as f64) < min {
                                                            self.#field_ident.#idx = min as _;
                                                            local_changed = true;
                                                        }
                                                        if (self.#field_ident.#idx as f64) > max {
                                                            self.#field_ident.#idx = max as _;
                                                            local_changed = true;
                                                        }
                                                    }
                                                    local_changed
                                                }
                                                __NumKind::Drag => {
                                                    let mut drag = ui.drag_config(#element_label);
                                                    if let Some(speed) = numeric.speed {
                                                        drag = drag.speed(speed as _);
                                                    }
                                                    match numeric.range {
                                                        __NumRange::Explicit { min, max } => {
                                                            drag = drag.range(min as f64, max as f64);
                                                        }
                                                        __NumRange::DefaultSlider | __NumRange::None => {}
                                                    }
                                                    if let Some(ref fmt) = numeric.format {
                                                        drag = drag.display_format(fmt);
                                                    }
                                                    if numeric.log
                                                        || numeric.always_clamp
                                                        || numeric.wrap_around
                                                        || numeric.no_round_to_format
                                                        || numeric.no_input
                                                        || numeric.clamp_on_input
                                                        || numeric.clamp_zero_range
                                                        || numeric.no_speed_tweaks
                                                    {
                                                        let mut flags = ::dear_imgui_reflect::imgui::SliderFlags::NONE;
                                                        if numeric.log {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::LOGARITHMIC;
                                                        }
                                                        if numeric.always_clamp {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::ALWAYS_CLAMP;
                                                        }
                                                        if numeric.wrap_around {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::WRAP_AROUND;
                                                        }
                                                        if numeric.no_round_to_format {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_ROUND_TO_FORMAT;
                                                        }
                                                        if numeric.no_input {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_INPUT;
                                                        }
                                                        if numeric.clamp_on_input {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ON_INPUT;
                                                        }
                                                        if numeric.clamp_zero_range {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::CLAMP_ZERO_RANGE;
                                                        }
                                                        if numeric.no_speed_tweaks {
                                                            flags |= ::dear_imgui_reflect::imgui::SliderFlags::NO_SPEED_TWEAKS;
                                                        }
                                                        drag = drag.flags(flags);
                                                    }
                                                    drag.build(ui, &mut self.#field_ident.#idx)
                                                }
                                            }
                                        }
                                    }
                                }
                                None => {
                                    quote! {
                                        ::dear_imgui_reflect::ImGuiValue::imgui_value(
                                            ui,
                                            #element_label,
                                            &mut self.#field_ident.#idx,
                                        )
                                    }
                                }
                            };

                            per_element_arms.push(quote! {
                                #index => {
                                    let __element_read_only = {
                                        let settings = ::dear_imgui_reflect::current_settings();
                                        if let Some(member) = settings.member::<Self>(#element_member_name) {
                                            member.read_only
                                        } else {
                                            false
                                        }
                                    };
                                    if __element_read_only {
                                        let _disabled = ui.begin_disabled();
                                        let changed = #element_body;
                                        drop(_disabled);
                                        changed
                                    } else {
                                        #element_body
                                    }
                                }
                            });
                        }
                        quote! {
                            #(#per_element_arms,)*
                            _ => false,
                        }
                    } else {
                        quote! { _ => false }
                    };

                    quote! {
                        {
                            let settings = ::dear_imgui_reflect::current_settings();
                            let mut tuple_settings =
                                settings.tuples().clone();
                            if let Some(member) = settings.member::<Self>(#field_name_lit) {
                                if let Some(ref override_settings) = member.tuples {
                                    tuple_settings = override_settings.clone();
                                }
                            }
                            #dropdown_stmt
                            #render_mode_stmt
                            #columns_stmt
                            #min_width_stmt

                            let local_changed = ::dear_imgui_reflect::imgui_tuple_body(
                                ui,
                                #label,
                                #len,
                                &tuple_settings,
                                |ui, index| {
                                    match index {
                                        #arms
                                    }
                                },
                            );
                            __changed |= local_changed;
                        }
                    }
                }
            }
            FieldTypeKind::Vec => {
                // For Vec<T> fields, layer per-member VecSettings on top of global
                // defaults and call the shared helper so insertable/removable/
                // reorderable/dropdown flags can be customized per field.
                match &ty {
                    Type::Path(tp) => {
                        if let Some(seg) = tp.path.segments.last() {
                            if seg.ident == "Vec" {
                                quote! {
                                    {
                                        let settings = ::dear_imgui_reflect::current_settings();
                                        let vec_settings: ::dear_imgui_reflect::VecSettings = {
                                            if let Some(member) = settings.member::<Self>(#field_name_lit) {
                                                if let Some(ref override_settings) = member.vec {
                                                    override_settings.clone()
                                                } else {
                                                    settings.vec().clone()
                                                }
                                            } else {
                                                settings.vec().clone()
                                            }
                                        };
                                        __changed |= ::dear_imgui_reflect::imgui_vec_with_settings(
                                            ui,
                                            #label,
                                            &mut self.#field_ident,
                                            &vec_settings,
                                        );
                                    }
                                }
                            } else {
                                quote! {
                                    __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                                        ui,
                                        #label,
                                        &mut self.#field_ident,
                                    );
                                }
                            }
                        } else {
                            quote! {
                                __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                                    ui,
                                    #label,
                                    &mut self.#field_ident,
                                );
                            }
                        }
                    }
                    _ => {
                        quote! {
                            __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                                ui,
                                #label,
                                &mut self.#field_ident,
                            );
                        }
                    }
                }
            }
            FieldTypeKind::Array => {
                // For fixed-size arrays, use per-member ArraySettings when available.
                match &ty {
                    Type::Array(_) => {
                        quote! {
                            {
                                let settings = ::dear_imgui_reflect::current_settings();
                                let arr_settings: ::dear_imgui_reflect::ArraySettings = {
                                    if let Some(member) = settings.member::<Self>(#field_name_lit) {
                                        if let Some(ref override_settings) = member.arrays {
                                            override_settings.clone()
                                        } else {
                                            settings.arrays().clone()
                                        }
                                    } else {
                                        settings.arrays().clone()
                                    }
                                };
                                __changed |= ::dear_imgui_reflect::imgui_array_with_settings(
                                    ui,
                                    #label,
                                    &mut self.#field_ident,
                                    &arr_settings,
                                );
                            }
                        }
                    }
                    _ => {
                        quote! {
                            __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                                ui,
                                #label,
                                &mut self.#field_ident,
                            );
                        }
                    }
                }
            }
            FieldTypeKind::Map => {
                // For supported string-key maps, use per-member MapSettings when
                // available and delegate to the shared helpers.
                match &ty {
                    Type::Path(tp) => {
                        if let Some(seg) = tp.path.segments.last() {
                            let ident_str = seg.ident.to_string();
                            if ident_str == "HashMap" {
                                quote! {
                                    {
                                        let settings = ::dear_imgui_reflect::current_settings();
                                        let map_settings: ::dear_imgui_reflect::MapSettings = {
                                            if let Some(member) = settings.member::<Self>(#field_name_lit) {
                                                if let Some(ref override_settings) = member.maps {
                                                    override_settings.clone()
                                                } else {
                                                    settings.maps().clone()
                                                }
                                            } else {
                                                settings.maps().clone()
                                            }
                                        };
                                        __changed |= ::dear_imgui_reflect::imgui_hash_map_with_settings(
                                            ui,
                                            #label,
                                            &mut self.#field_ident,
                                            &map_settings,
                                        );
                                    }
                                }
                            } else if ident_str == "BTreeMap" {
                                quote! {
                                    {
                                        let settings = ::dear_imgui_reflect::current_settings();
                                        let map_settings: ::dear_imgui_reflect::MapSettings = {
                                            if let Some(member) = settings.member::<Self>(#field_name_lit) {
                                                if let Some(ref override_settings) = member.maps {
                                                    override_settings.clone()
                                                } else {
                                                    settings.maps().clone()
                                                }
                                            } else {
                                                settings.maps().clone()
                                            }
                                        };
                                        __changed |= ::dear_imgui_reflect::imgui_btree_map_with_settings(
                                            ui,
                                            #label,
                                            &mut self.#field_ident,
                                            &map_settings,
                                        );
                                    }
                                }
                            } else {
                                quote! {
                                    __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                                        ui,
                                        #label,
                                        &mut self.#field_ident,
                                    );
                                }
                            }
                        } else {
                            quote! {
                                __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                                    ui,
                                    #label,
                                    &mut self.#field_ident,
                                );
                            }
                        }
                    }
                    _ => {
                        quote! {
                            __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                                ui,
                                #label,
                                &mut self.#field_ident,
                            );
                        }
                    }
                }
            }
            FieldTypeKind::Other => {
                quote! {
                    __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                        ui,
                        #label,
                        &mut self.#field_ident,
                    );
                }
            }
        };
        // Wrap field rendering in a disabled scope when either the field-level
        // `#[imgui(read_only)]` attribute is present or a member-level
        // `MemberSettings::read_only` override is active, allowing read-only
        // behavior on any field type (including tuples, maps, containers, etc.).
        let field_read_only = read_only;
        let stmt = quote! {
            {
                let __member_read_only = {
                    let settings = ::dear_imgui_reflect::current_settings();
                    if let Some(member) = settings.member::<Self>(#field_name_lit) {
                        member.read_only
                    } else {
                        false
                    }
                };
                if #field_read_only || __member_read_only {
                    let _disabled = ui.begin_disabled();
                    #inner_stmt
                    drop(_disabled);
                } else {
                    #inner_stmt
                }
            }
        };

        field_stmts.push(stmt);
    }

    {
        let where_clause = generics.make_where_clause();
        for ty in bound_types {
            where_clause
                .predicates
                .push(parse_quote!(#ty: ::dear_imgui_reflect::ImGuiValue));
        }
        for ty in default_range_types {
            where_clause
                .predicates
                .push(parse_quote!(#ty: ::dear_imgui_reflect::NumericDefaultRange));
        }
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics ::dear_imgui_reflect::ImGuiReflect for #ident #ty_generics #where_clause {
            fn imgui_reflect(
                &mut self,
                ui: &::dear_imgui_reflect::imgui::Ui,
                label: &str,
            ) -> bool {
                let mut __changed = false;
                if let Some(__node) = ui.tree_node(label) {
                    let _ = __node;
                    #(#field_stmts)*
                }
                __changed
            }
        }
    };

    expanded.into()
}
