use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{Fields, Type, parse_quote};

use crate::attrs::{FieldAttrs, parse_field_attrs};
use crate::field_codegen;
use crate::internal::{
    FieldTypeKind, NumericFormatType, NumericTypeTag, NumericWidgetKind, classify_field_type,
    classify_numeric_format_type, classify_numeric_type,
};
use crate::numeric_format::validate_and_normalize;
use crate::settings_codegen::reflect_settings_ident;

fn escape_format_literal(text: &str) -> String {
    text.replace('%', "%%")
}

pub(crate) fn derive_for_struct(
    ident: syn::Ident,
    mut generics: syn::Generics,
    data: syn::DataStruct,
) -> TokenStream {
    let reflect_settings_ident = reflect_settings_ident();
    enum FieldAccess {
        Named(syn::Ident),
        Unnamed(syn::Index),
    }

    let mut field_stmts = Vec::new();
    let mut bound_types: Vec<Type> = Vec::new();
    let mut default_range_types: Vec<Type> = Vec::new();

    let fields: Vec<(syn::Field, FieldAccess, syn::Ident, syn::LitStr)> = match data.fields {
        Fields::Named(named) => named
            .named
            .into_iter()
            .filter_map(|field| {
                let ident = field.ident.clone()?;
                let key = ident.to_string();
                Some((
                    field,
                    FieldAccess::Named(ident.clone()),
                    ident,
                    syn::LitStr::new(&key, Span::call_site()),
                ))
            })
            .collect(),
        Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .into_iter()
            .enumerate()
            .map(|(index, field)| {
                let idx = syn::Index::from(index);
                let field_ident_for_errors =
                    syn::Ident::new(&format!("field_{index}"), Span::call_site());
                let key = index.to_string();
                (
                    field,
                    FieldAccess::Unnamed(idx),
                    field_ident_for_errors,
                    syn::LitStr::new(&key, Span::call_site()),
                )
            })
            .collect(),
        Fields::Unit => Vec::new(),
    };

    for (field, field_access, field_ident, field_name_lit) in fields {
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
            let numeric_format_type = classify_numeric_format_type(&ty);

            let fmt_style_count = (fmt_hex as u8) + (fmt_percentage as u8) + (fmt_scientific as u8);
            if fmt_style_count > 1 {
                return syn::Error::new(
                    field_ident.span(),
                    "imgui(hex/percentage/scientific) are mutually exclusive; use at most one on the same field",
                )
                .to_compile_error()
                .into();
            }

            if fmt_hex
                && !matches!(
                    numeric_format_type,
                    Some(NumericFormatType::Unsigned32 | NumericFormatType::Unsigned64)
                )
            {
                return syn::Error::new(
                    field_ident.span(),
                    "imgui(hex) is only supported on unsigned fixed-width integer fields",
                )
                .to_compile_error()
                .into();
            }

            if (fmt_percentage || fmt_scientific)
                && numeric_format_type != Some(NumericFormatType::Float)
            {
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
            quote! { #field_name_lit }
        };

        let field_access_expr = match field_access {
            FieldAccess::Named(ident) => quote! { self.#ident },
            FieldAccess::Unnamed(index) => quote! { self.#index },
        };

        bound_types.push(ty.clone());
        if slider_default_range {
            default_range_types.push(ty.clone());
        }

        // Decide how to render this field based on attributes and type.
        let inner_stmt = match kind {
            FieldTypeKind::Bool => {
                match field_codegen::gen_bool_field(
                    &reflect_settings_ident,
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
                match field_codegen::gen_string_field(
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
                match field_codegen::gen_imstring_field(
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

                // A field-level format is an explicit widget override. When
                // no other selector is present, render it with InputScalar
                // instead of silently dropping the format in the settings path.
                if matches!(widget_kind, NumericWidgetKind::Default)
                    && (format_str.is_some()
                        || fmt_hex
                        || fmt_percentage
                        || fmt_scientific
                        || fmt_prefix.is_some()
                        || fmt_suffix.is_some())
                {
                    widget_kind = NumericWidgetKind::Input;
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

                if wrap_around_flag && matches!(widget_kind, NumericWidgetKind::Slider) {
                    return syn::Error::new(
                        field_ident.span(),
                        "imgui(wrap_around) is only supported for drag widgets; use imgui(as_drag, wrap_around) instead of slider",
                    )
                    .to_compile_error()
                    .into();
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

                // Compute and validate the exact typed numeric format at macro
                // expansion time. Generated code constructs NumericFormat<T>
                // instead of passing a raw string to a widget.
                let mut effective_format_lit: Option<syn::LitStr> = {
                    if let Some(lit) = format_str.clone() {
                        Some(lit)
                    } else if fmt_hex
                        || fmt_percentage
                        || fmt_scientific
                        || fmt_prefix.is_some()
                        || fmt_suffix.is_some()
                    {
                        let Some(numeric_format_type) = classify_numeric_format_type(&ty) else {
                            return syn::Error::new(
                                field_ident.span(),
                                "numeric format attributes require a supported primitive numeric field",
                            )
                            .to_compile_error()
                            .into();
                        };

                        let base = match (
                            fmt_hex,
                            fmt_percentage,
                            fmt_scientific,
                            numeric_format_type,
                        ) {
                            (true, _, _, NumericFormatType::Unsigned32) => "%#x",
                            (true, _, _, NumericFormatType::Unsigned64) => "%#llx",
                            (_, true, _, NumericFormatType::Float) => "%.2f%%",
                            (_, _, true, NumericFormatType::Float) => "%e",
                            (false, false, false, NumericFormatType::Signed32) => "%d",
                            (false, false, false, NumericFormatType::Unsigned32) => "%u",
                            (false, false, false, NumericFormatType::Signed64) => "%lld",
                            (false, false, false, NumericFormatType::Unsigned64) => "%llu",
                            (false, false, false, NumericFormatType::Float) => "%.3f",
                            (_, _, _, NumericFormatType::PointerSized) => {
                                return syn::Error::new(
                                    field_ident.span(),
                                    "custom formats for isize/usize are target-width dependent; use a fixed-width numeric field",
                                )
                                .to_compile_error()
                                .into();
                            }
                            _ => {
                                return syn::Error::new(
                                    field_ident.span(),
                                    "numeric format helper does not match the field type",
                                )
                                .to_compile_error()
                                .into();
                            }
                        };

                        let prefix_val = fmt_prefix
                            .as_ref()
                            .map(|literal| escape_format_literal(&literal.value()))
                            .unwrap_or_default();
                        let suffix_val = fmt_suffix
                            .as_ref()
                            .map(|literal| escape_format_literal(&literal.value()))
                            .unwrap_or_default();
                        let combined = format!("{prefix_val}{base}{suffix_val}");
                        Some(syn::LitStr::new(&combined, field_ident.span()))
                    } else {
                        None
                    }
                };

                if let Some(format) = effective_format_lit.clone() {
                    let Some(numeric_format_type) = classify_numeric_format_type(&ty) else {
                        return syn::Error::new(
                            format.span(),
                            "numeric format requires a supported primitive numeric field",
                        )
                        .to_compile_error()
                        .into();
                    };
                    let normalized =
                        match validate_and_normalize(&format.value(), numeric_format_type) {
                            Ok(normalized) => normalized,
                            Err(message) => {
                                return syn::Error::new(format.span(), message)
                                    .to_compile_error()
                                    .into();
                            }
                        };
                    effective_format_lit = Some(syn::LitStr::new(&normalized, format.span()));
                }

                let typed_format_expr = effective_format_lit.as_ref().map(|format| {
                    quote! {
                        ::dear_imgui_reflect::imgui::NumericFormat::<#ty>::new(#format)
                            .expect("ImGuiReflect derive validated this numeric format")
                    }
                });

                match widget_kind {
                    NumericWidgetKind::Input => {
                        let step = step_expr.clone();
                        let step_fast = step_fast_expr.clone();

                        let fmt_stmt = if let Some(format) = typed_format_expr.clone() {
                            quote! { let mut builder = builder.display_format(#format); }
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
                                let mut builder = ui.input_scalar(#label, __field);
                                #fmt_stmt
                                #step_stmt
                                #step_fast_stmt
                                __changed |= builder.build();
                            }
                        }
                    }
                    NumericWidgetKind::Slider => {
                        let fmt_stmt = if let Some(format) = typed_format_expr.clone() {
                            quote! { let mut slider = slider.display_format(#format); }
                        } else {
                            quote! {}
                        };

                        let flags_stmt = if log_scale
                            || always_clamp_flag
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
                                    let mut local_changed = slider.build(__field);
                                    if #clamp_manual {
                                        if *__field < min {
                                            *__field = min;
                                            local_changed = true;
                                        }
                                        if *__field > max {
                                            *__field = max;
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
                                    let mut local_changed = slider.build(__field);
                                    if #clamp_manual {
                                        if *__field < #min {
                                            *__field = #min;
                                            local_changed = true;
                                        }
                                        if *__field > #max {
                                            *__field = #max;
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

                        let fmt_stmt = if let Some(format) = typed_format_expr.clone() {
                            quote! { let mut drag = drag.display_format(#format); }
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
                                    flags |= ::dear_imgui_reflect::imgui::DragFlags::LOGARITHMIC;
                                }
                            } else {
                                quote! {}
                            };
                            let clamp_stmt = if always_clamp_flag {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::DragFlags::ALWAYS_CLAMP;
                                }
                            } else {
                                quote! {}
                            };
                            let wrap_stmt = if wrap_around_flag {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::DragFlags::WRAP_AROUND;
                                }
                            } else {
                                quote! {}
                            };
                            let no_round_stmt = if no_round_to_format {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::DragFlags::NO_ROUND_TO_FORMAT;
                                }
                            } else {
                                quote! {}
                            };
                            let no_input_stmt = if no_input {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::DragFlags::NO_INPUT;
                                }
                            } else {
                                quote! {}
                            };
                            let clamp_on_input_stmt = if clamp_on_input {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::DragFlags::CLAMP_ON_INPUT;
                                }
                            } else {
                                quote! {}
                            };
                            let clamp_zero_range_stmt = if clamp_zero_range {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::DragFlags::CLAMP_ZERO_RANGE;
                                }
                            } else {
                                quote! {}
                            };
                            let no_speed_tweaks_stmt = if no_speed_tweaks {
                                quote! {
                                    flags |= ::dear_imgui_reflect::imgui::DragFlags::NO_SPEED_TWEAKS;
                                }
                            } else {
                                quote! {}
                            };

                            quote! {
                                let mut flags = ::dear_imgui_reflect::imgui::DragFlags::NONE;
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
                                let local_changed = drag.build(ui, __field);
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
                                        let settings = &#reflect_settings_ident;
                                        let numeric = settings
                                            .member::<Self>(#field_name_lit)
                                            .and_then(|member| member.numerics_i32.as_ref())
                                            .unwrap_or_else(|| settings.numerics_i32());
                                        match numeric.widget {
                                            __NumKind::Input => {
                                                let mut builder = ui.input_scalar(#label, __field);
                                                if let Some(step) = numeric.step {
                                                    builder = builder.step(step as _);
                                                }
                                                if let Some(step_fast) = numeric.step_fast {
                                                    builder = builder.step_fast(step_fast as _);
                                                }
                                                __changed |= if let Some(ref fmt) = numeric.format {
                                                    builder.display_format(fmt.borrowed()).build()
                                                } else {
                                                    builder.build()
                                                };
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
                                                slider = slider.flags(numeric.slider_flags());
                                                let mut local_changed = if let Some(ref fmt) = numeric.format {
                                                    slider.display_format(fmt.borrowed()).build(__field)
                                                } else {
                                                    slider.build(__field)
                                                };
                                                if numeric.clamp {
                                                    if *__field < min {
                                                        *__field = min;
                                                        local_changed = true;
                                                    }
                                                    if *__field > max {
                                                        *__field = max;
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
                                                drag = drag.flags(numeric.drag_flags());
                                                let local_changed = if let Some(ref fmt) = numeric.format {
                                                    drag.display_format(fmt.borrowed()).build(ui, __field)
                                                } else {
                                                    drag.build(ui, __field)
                                                };
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
                                        let settings = &#reflect_settings_ident;
                                        let numeric = settings
                                            .member::<Self>(#field_name_lit)
                                            .and_then(|member| member.numerics_u32.as_ref())
                                            .unwrap_or_else(|| settings.numerics_u32());
                                        match numeric.widget {
                                            __NumKind::Input => {
                                                let mut builder = ui.input_scalar(#label, __field);
                                                if let Some(step) = numeric.step {
                                                    builder = builder.step(step as _);
                                                }
                                                if let Some(step_fast) = numeric.step_fast {
                                                    builder = builder.step_fast(step_fast as _);
                                                }
                                                __changed |= if let Some(ref fmt) = numeric.format {
                                                    builder.display_format(fmt.borrowed()).build()
                                                } else {
                                                    builder.build()
                                                };
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
                                                slider = slider.flags(numeric.slider_flags());
                                                let mut local_changed = if let Some(ref fmt) = numeric.format {
                                                    slider.display_format(fmt.borrowed()).build(__field)
                                                } else {
                                                    slider.build(__field)
                                                };
                                                if numeric.clamp {
                                                    if *__field < min {
                                                        *__field = min;
                                                        local_changed = true;
                                                    }
                                                    if *__field > max {
                                                        *__field = max;
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
                                                drag = drag.flags(numeric.drag_flags());
                                                let local_changed = if let Some(ref fmt) = numeric.format {
                                                    drag.display_format(fmt.borrowed()).build(ui, __field)
                                                } else {
                                                    drag.build(ui, __field)
                                                };
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
                                        let settings = &#reflect_settings_ident;
                                        let numeric = settings
                                            .member::<Self>(#field_name_lit)
                                            .and_then(|member| member.numerics_f32.as_ref())
                                            .unwrap_or_else(|| settings.numerics_f32());
                                        match numeric.widget {
                                            __NumKind::Input => {
                                                let mut builder = ui.input_scalar(#label, __field);
                                                if let Some(step) = numeric.step {
                                                    builder = builder.step(step as _);
                                                }
                                                if let Some(step_fast) = numeric.step_fast {
                                                    builder = builder.step_fast(step_fast as _);
                                                }
                                                __changed |= if let Some(ref fmt) = numeric.format {
                                                    builder.display_format(fmt.borrowed()).build()
                                                } else {
                                                    builder.build()
                                                };
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
                                                slider = slider.flags(numeric.slider_flags());
                                                let mut local_changed = if let Some(ref fmt) = numeric.format {
                                                    slider.display_format(fmt.borrowed()).build(__field)
                                                } else {
                                                    slider.build(__field)
                                                };
                                                if numeric.clamp {
                                                    if *__field < min {
                                                        *__field = min;
                                                        local_changed = true;
                                                    }
                                                    if *__field > max {
                                                        *__field = max;
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
                                                drag = drag.flags(numeric.drag_flags());
                                                let local_changed = if let Some(ref fmt) = numeric.format {
                                                    drag.display_format(fmt.borrowed()).build(ui, __field)
                                                } else {
                                                    drag.build(ui, __field)
                                                };
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
                                        let settings = &#reflect_settings_ident;
                                        let numeric = settings
                                            .member::<Self>(#field_name_lit)
                                            .and_then(|member| member.numerics_f64.as_ref())
                                            .unwrap_or_else(|| settings.numerics_f64());
                                        match numeric.widget {
                                            __NumKind::Input => {
                                                let mut builder = ui.input_scalar(#label, __field);
                                                if let Some(step) = numeric.step {
                                                    builder = builder.step(step as _);
                                                }
                                                if let Some(step_fast) = numeric.step_fast {
                                                    builder = builder.step_fast(step_fast as _);
                                                }
                                                __changed |= if let Some(ref fmt) = numeric.format {
                                                    builder.display_format(fmt.borrowed()).build()
                                                } else {
                                                    builder.build()
                                                };
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
                                                slider = slider.flags(numeric.slider_flags());
                                                let mut local_changed = if let Some(ref fmt) = numeric.format {
                                                    slider.display_format(fmt.borrowed()).build(__field)
                                                } else {
                                                    slider.build(__field)
                                                };
                                                if numeric.clamp {
                                                    if *__field < min {
                                                        *__field = min;
                                                        local_changed = true;
                                                    }
                                                    if *__field > max {
                                                        *__field = max;
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
                                                drag = drag.flags(numeric.drag_flags());
                                                let local_changed = if let Some(ref fmt) = numeric.format {
                                                    drag.display_format(fmt.borrowed()).build(ui, __field)
                                                } else {
                                                    drag.build(ui, __field)
                                                };
                                                __changed |= local_changed;
                                            }
                                        }
                                    }
                                }
                            }
                            None => {
                                quote! {
                                    __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                                        inspector,
                                        #label,
                                        __field,
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
                            inspector,
                            #label,
                            __field,
                        );
                    }
                } else {
                    // Build TupleSettings for this member by layering:
                    //  - session ReflectSettings::tuples()
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
                                &format!("{}[{}]", field_name_lit.value(), index),
                                field_name_lit.span(),
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
                                            let settings = &#reflect_settings_ident;
                                            let numeric = settings
                                                .member::<Self>(#element_member_name)
                                                .and_then(|member| member.numerics_i32.as_ref())
                                                .unwrap_or_else(|| settings.numerics_i32());
                                            match numeric.widget {
                                                __NumKind::Input => {
                                                    let mut builder =
                                                        ui.input_scalar(#element_label, &mut __field.#idx);
                                                    if let Some(step) = numeric.step {
                                                        builder = builder.step(step as _);
                                                    }
                                                    if let Some(step_fast) = numeric.step_fast {
                                                        builder = builder.step_fast(step_fast as _);
                                                    }
                                                    if let Some(ref fmt) = numeric.format {
                                                        builder.display_format(fmt.borrowed()).build()
                                                    } else {
                                                        builder.build()
                                                    }
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
                                                    slider = slider.flags(numeric.slider_flags());
                                                    let mut local_changed = if let Some(ref fmt) = numeric.format {
                                                        slider
                                                            .display_format(fmt.borrowed())
                                                            .build(&mut __field.#idx)
                                                    } else {
                                                        slider.build(&mut __field.#idx)
                                                    };
                                                    if numeric.clamp {
                                                        if __field.#idx < min {
                                                            __field.#idx = min;
                                                            local_changed = true;
                                                        }
                                                        if __field.#idx > max {
                                                            __field.#idx = max;
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
                                                    drag = drag.flags(numeric.drag_flags());
                                                    if let Some(ref fmt) = numeric.format {
                                                        drag
                                                            .display_format(fmt.borrowed())
                                                            .build(ui, &mut __field.#idx)
                                                    } else {
                                                        drag.build(ui, &mut __field.#idx)
                                                    }
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
                                            let settings = &#reflect_settings_ident;
                                            let numeric = settings
                                                .member::<Self>(#element_member_name)
                                                .and_then(|member| member.numerics_u32.as_ref())
                                                .unwrap_or_else(|| settings.numerics_u32());
                                            match numeric.widget {
                                                __NumKind::Input => {
                                                    let mut builder =
                                                        ui.input_scalar(#element_label, &mut __field.#idx);
                                                    if let Some(step) = numeric.step {
                                                        builder = builder.step(step as _);
                                                    }
                                                    if let Some(step_fast) = numeric.step_fast {
                                                        builder = builder.step_fast(step_fast as _);
                                                    }
                                                    if let Some(ref fmt) = numeric.format {
                                                        builder.display_format(fmt.borrowed()).build()
                                                    } else {
                                                        builder.build()
                                                    }
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
                                                    slider = slider.flags(numeric.slider_flags());
                                                    let mut local_changed = if let Some(ref fmt) = numeric.format {
                                                        slider
                                                            .display_format(fmt.borrowed())
                                                            .build(&mut __field.#idx)
                                                    } else {
                                                        slider.build(&mut __field.#idx)
                                                    };
                                                    if numeric.clamp {
                                                        if __field.#idx < min {
                                                            __field.#idx = min;
                                                            local_changed = true;
                                                        }
                                                        if __field.#idx > max {
                                                            __field.#idx = max;
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
                                                    drag = drag.flags(numeric.drag_flags());
                                                    if let Some(ref fmt) = numeric.format {
                                                        drag
                                                            .display_format(fmt.borrowed())
                                                            .build(ui, &mut __field.#idx)
                                                    } else {
                                                        drag.build(ui, &mut __field.#idx)
                                                    }
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
                                            let settings = &#reflect_settings_ident;
                                            let numeric = settings
                                                .member::<Self>(#element_member_name)
                                                .and_then(|member| member.numerics_f32.as_ref())
                                                .unwrap_or_else(|| settings.numerics_f32());
                                            match numeric.widget {
                                                __NumKind::Input => {
                                                    let mut builder =
                                                        ui.input_scalar(#element_label, &mut __field.#idx);
                                                    if let Some(step) = numeric.step {
                                                        builder = builder.step(step as _);
                                                    }
                                                    if let Some(step_fast) = numeric.step_fast {
                                                        builder = builder.step_fast(step_fast as _);
                                                    }
                                                    if let Some(ref fmt) = numeric.format {
                                                        builder.display_format(fmt.borrowed()).build()
                                                    } else {
                                                        builder.build()
                                                    }
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
                                                    slider = slider.flags(numeric.slider_flags());
                                                    let mut local_changed = if let Some(ref fmt) = numeric.format {
                                                        slider
                                                            .display_format(fmt.borrowed())
                                                            .build(&mut __field.#idx)
                                                    } else {
                                                        slider.build(&mut __field.#idx)
                                                    };
                                                    if numeric.clamp {
                                                        if __field.#idx < min {
                                                            __field.#idx = min;
                                                            local_changed = true;
                                                        }
                                                        if __field.#idx > max {
                                                            __field.#idx = max;
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
                                                    drag = drag.flags(numeric.drag_flags());
                                                    if let Some(ref fmt) = numeric.format {
                                                        drag
                                                            .display_format(fmt.borrowed())
                                                            .build(ui, &mut __field.#idx)
                                                    } else {
                                                        drag.build(ui, &mut __field.#idx)
                                                    }
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
                                            let settings = &#reflect_settings_ident;
                                            let numeric = settings
                                                .member::<Self>(#element_member_name)
                                                .and_then(|member| member.numerics_f64.as_ref())
                                                .unwrap_or_else(|| settings.numerics_f64());
                                            match numeric.widget {
                                                __NumKind::Input => {
                                                    let mut builder =
                                                        ui.input_scalar(#element_label, &mut __field.#idx);
                                                    if let Some(step) = numeric.step {
                                                        builder = builder.step(step as _);
                                                    }
                                                    if let Some(step_fast) = numeric.step_fast {
                                                        builder = builder.step_fast(step_fast as _);
                                                    }
                                                    if let Some(ref fmt) = numeric.format {
                                                        builder.display_format(fmt.borrowed()).build()
                                                    } else {
                                                        builder.build()
                                                    }
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
                                                    slider = slider.flags(numeric.slider_flags());
                                                    let mut local_changed = if let Some(ref fmt) = numeric.format {
                                                        slider
                                                            .display_format(fmt.borrowed())
                                                            .build(&mut __field.#idx)
                                                    } else {
                                                        slider.build(&mut __field.#idx)
                                                    };
                                                    if numeric.clamp {
                                                        if (__field.#idx as f64) < min {
                                                            __field.#idx = min as _;
                                                            local_changed = true;
                                                        }
                                                        if (__field.#idx as f64) > max {
                                                            __field.#idx = max as _;
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
                                                    drag = drag.flags(numeric.drag_flags());
                                                    if let Some(ref fmt) = numeric.format {
                                                        drag
                                                            .display_format(fmt.borrowed())
                                                            .build(ui, &mut __field.#idx)
                                                    } else {
                                                        drag.build(ui, &mut __field.#idx)
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                None => {
                                    quote! {
                                        ::dear_imgui_reflect::ImGuiValue::imgui_value(
                                            inspector,
                                            #element_label,
                                            &mut __field.#idx,
                                        )
                                    }
                                }
                            };

                            per_element_arms.push(quote! {
                                #index => {
                                    let __element_read_only = {
                                        let settings = &#reflect_settings_ident;
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
                            let settings = &#reflect_settings_ident;
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
                                inspector,
                                #label,
                                #len,
                                &tuple_settings,
                                |inspector, index| {
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
                // For Vec<T> fields, layer per-member VecSettings on top of session
                // defaults and call the shared helper so insertable/removable/
                // reorderable/dropdown flags can be customized per field.
                match &ty {
                    Type::Path(tp) => {
                        if let Some(seg) = tp.path.segments.last() {
                            if seg.ident == "Vec" {
                                quote! {
                                    {
                                        let settings = &#reflect_settings_ident;
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
                                            inspector,
                                            #label,
                                            __field,
                                            &vec_settings,
                                        );
                                    }
                                }
                            } else {
                                quote! {
                                    __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                                        inspector,
                                        #label,
                                        __field,
                                    );
                                }
                            }
                        } else {
                            quote! {
                                __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                                    inspector,
                                    #label,
                                    __field,
                                );
                            }
                        }
                    }
                    _ => {
                        quote! {
                            __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                                inspector,
                                #label,
                                __field,
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
                                let settings = &#reflect_settings_ident;
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
                                    inspector,
                                    #label,
                                    __field,
                                    &arr_settings,
                                );
                            }
                        }
                    }
                    _ => {
                        quote! {
                            __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                                inspector,
                                #label,
                                __field,
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
                                        let settings = &#reflect_settings_ident;
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
                                            inspector,
                                            #label,
                                            __field,
                                            &map_settings,
                                        );
                                    }
                                }
                            } else if ident_str == "BTreeMap" {
                                quote! {
                                    {
                                        let settings = &#reflect_settings_ident;
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
                                            inspector,
                                            #label,
                                            __field,
                                            &map_settings,
                                        );
                                    }
                                }
                            } else {
                                quote! {
                                    __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                                        inspector,
                                        #label,
                                        __field,
                                    );
                                }
                            }
                        } else {
                            quote! {
                                __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                                    inspector,
                                    #label,
                                    __field,
                                );
                            }
                        }
                    }
                    _ => {
                        quote! {
                            __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                                inspector,
                                #label,
                                __field,
                            );
                        }
                    }
                }
            }
            FieldTypeKind::Other => {
                quote! {
                    __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                        inspector,
                        #label,
                        __field,
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
                let __field_path = inspector.push_path_static(#field_name_lit);
                let __field = &mut #field_access_expr;
                let __member_read_only = {
                    let settings = &#reflect_settings_ident;
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
                drop(__field_path);
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
                inspector: &mut ::dear_imgui_reflect::Inspector<'_, '_>,
                label: &str,
            ) -> bool {
                let ui = inspector.ui();
                let #reflect_settings_ident = inspector.settings();
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
