use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Type, parse_macro_input, parse_quote};

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
        Data::Enum(data) => derive_for_enum(ident, generics, attrs, data),
        Data::Union(u) => {
            syn::Error::new_spanned(u.union_token, "ImGuiReflect cannot be derived for unions")
                .to_compile_error()
                .into()
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum FieldTypeKind {
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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum NumericWidgetKind {
    /// Use the default ImGuiValue implementation (no special widget).
    Default,
    /// Use an InputScalar-style widget with optional step/step_fast/format.
    Input,
    /// Use a Slider widget with required min/max and optional format/flags.
    Slider,
    /// Use a Drag widget with optional speed/range/format/flags.
    Drag,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum NumericTypeTag {
    I32,
    U32,
    F32,
    F64,
}

fn classify_numeric_type(ty: &Type) -> Option<NumericTypeTag> {
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

fn classify_field_type(ty: &Type) -> FieldTypeKind {
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
        let field_ident = match field.ident {
            Some(id) => id,
            None => continue,
        };

        let mut skip = false;
        let mut label_override: Option<syn::LitStr> = None;
        // Numeric configuration
        let mut slider = false;
        let mut slider_default_range = false;
        let mut as_input = false;
        let mut as_drag = false;
        let mut min_expr: Option<syn::Expr> = None;
        let mut max_expr: Option<syn::Expr> = None;
        let mut format_str: Option<syn::LitStr> = None;
        let mut fmt_hex = false;
        let mut fmt_percentage = false;
        let mut fmt_scientific = false;
        let mut fmt_prefix: Option<syn::LitStr> = None;
        let mut fmt_suffix: Option<syn::LitStr> = None;
        let mut step_expr: Option<syn::Expr> = None;
        let mut step_fast_expr: Option<syn::Expr> = None;
        let mut speed_expr: Option<syn::Expr> = None;
        let mut log_scale = false;
        let mut clamp_manual = false;
        let mut always_clamp_flag = false;
        let mut wrap_around_flag = false;
        let mut no_round_to_format = false;
        let mut no_input = false;
        let mut clamp_on_input = false;
        let mut clamp_zero_range = false;
        let mut no_speed_tweaks = false;
        // Text configuration
        let mut multiline = false;
        let mut lines_expr: Option<syn::Expr> = None;
        let mut hint_str: Option<syn::LitStr> = None;
        let mut read_only = false;
        let mut display_only = false;
        let mut auto_resize = false;
        let mut min_width_expr: Option<syn::Expr> = None;
        // Tuple layout configuration
        let mut tuple_render: Option<String> = None;
        let mut tuple_dropdown = false;
        let mut tuple_columns_expr: Option<syn::Expr> = None;
        let mut tuple_min_width_expr: Option<syn::Expr> = None;
        // Bool configuration
        let mut bool_style: Option<String> = None;
        let mut true_text: Option<syn::LitStr> = None;
        let mut false_text: Option<syn::LitStr> = None;

        for attr in field.attrs.iter().filter(|a| a.path().is_ident("imgui")) {
            let res = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("skip") {
                    skip = true;
                    return Ok(());
                }

                if meta.path.is_ident("name") {
                    let lit: syn::LitStr = meta.value()?.parse()?;
                    label_override = Some(lit);
                    return Ok(());
                }

                if meta.path.is_ident("slider") {
                    slider = true;
                    return Ok(());
                }

                if meta.path.is_ident("slider_default_range") {
                    slider_default_range = true;
                    return Ok(());
                }

                if meta.path.is_ident("as_input") {
                    as_input = true;
                    return Ok(());
                }

                if meta.path.is_ident("as_drag") {
                    as_drag = true;
                    return Ok(());
                }

                if meta.path.is_ident("min") {
                    let expr: syn::Expr = meta.value()?.parse()?;
                    min_expr = Some(expr);
                    return Ok(());
                }

                if meta.path.is_ident("max") {
                    let expr: syn::Expr = meta.value()?.parse()?;
                    max_expr = Some(expr);
                    return Ok(());
                }

                if meta.path.is_ident("format") {
                    let lit: syn::LitStr = meta.value()?.parse()?;
                    format_str = Some(lit);
                    return Ok(());
                }

                if meta.path.is_ident("hex") {
                    fmt_hex = true;
                    return Ok(());
                }

                if meta.path.is_ident("percentage") {
                    fmt_percentage = true;
                    return Ok(());
                }

                if meta.path.is_ident("scientific") {
                    fmt_scientific = true;
                    return Ok(());
                }

                if meta.path.is_ident("prefix") {
                    let lit: syn::LitStr = meta.value()?.parse()?;
                    fmt_prefix = Some(lit);
                    return Ok(());
                }

                if meta.path.is_ident("suffix") {
                    let lit: syn::LitStr = meta.value()?.parse()?;
                    fmt_suffix = Some(lit);
                    return Ok(());
                }

                if meta.path.is_ident("step") {
                    let expr: syn::Expr = meta.value()?.parse()?;
                    step_expr = Some(expr);
                    return Ok(());
                }

                if meta.path.is_ident("step_fast") {
                    let expr: syn::Expr = meta.value()?.parse()?;
                    step_fast_expr = Some(expr);
                    return Ok(());
                }

                if meta.path.is_ident("speed") {
                    let expr: syn::Expr = meta.value()?.parse()?;
                    speed_expr = Some(expr);
                    return Ok(());
                }

                if meta.path.is_ident("log") {
                    log_scale = true;
                    return Ok(());
                }

                if meta.path.is_ident("clamp") {
                    clamp_manual = true;
                    return Ok(());
                }

                if meta.path.is_ident("always_clamp") {
                    always_clamp_flag = true;
                    return Ok(());
                }

                if meta.path.is_ident("wrap_around") {
                    wrap_around_flag = true;
                    return Ok(());
                }

                if meta.path.is_ident("no_round_to_format") {
                    no_round_to_format = true;
                    return Ok(());
                }

                if meta.path.is_ident("no_input") {
                    no_input = true;
                    return Ok(());
                }

                if meta.path.is_ident("clamp_on_input") {
                    clamp_on_input = true;
                    return Ok(());
                }

                if meta.path.is_ident("clamp_zero_range") {
                    clamp_zero_range = true;
                    return Ok(());
                }

                if meta.path.is_ident("no_speed_tweaks") {
                    no_speed_tweaks = true;
                    return Ok(());
                }

                if meta.path.is_ident("multiline") {
                    multiline = true;
                    return Ok(());
                }

                if meta.path.is_ident("lines") {
                    let expr: syn::Expr = meta.value()?.parse()?;
                    lines_expr = Some(expr);
                    return Ok(());
                }

                if meta.path.is_ident("hint") {
                    let lit: syn::LitStr = meta.value()?.parse()?;
                    hint_str = Some(lit);
                    return Ok(());
                }

                if meta.path.is_ident("read_only") {
                    read_only = true;
                    return Ok(());
                }

                if meta.path.is_ident("display_only") {
                    display_only = true;
                    return Ok(());
                }

                if meta.path.is_ident("auto_resize") {
                    auto_resize = true;
                    return Ok(());
                }

                if meta.path.is_ident("min_width") {
                    let expr: syn::Expr = meta.value()?.parse()?;
                    min_width_expr = Some(expr);
                    return Ok(());
                }

                if meta.path.is_ident("bool_style") {
                    let lit: syn::LitStr = meta.value()?.parse()?;
                    bool_style = Some(lit.value());
                    return Ok(());
                }

                if meta.path.is_ident("tuple_render") {
                    let lit: syn::LitStr = meta.value()?.parse()?;
                    let v = lit.value();
                    if v != "line" && v != "grid" {
                        return Err(
                            meta.error("imgui(tuple_render = ...) must be \"line\" or \"grid\"")
                        );
                    }
                    tuple_render = Some(v);
                    return Ok(());
                }

                if meta.path.is_ident("tuple_dropdown") {
                    tuple_dropdown = true;
                    return Ok(());
                }

                if meta.path.is_ident("tuple_columns") {
                    let expr: syn::Expr = meta.value()?.parse()?;
                    tuple_columns_expr = Some(expr);
                    return Ok(());
                }

                if meta.path.is_ident("tuple_min_width") {
                    let expr: syn::Expr = meta.value()?.parse()?;
                    tuple_min_width_expr = Some(expr);
                    return Ok(());
                }

                if meta.path.is_ident("true_text") {
                    let lit: syn::LitStr = meta.value()?.parse()?;
                    true_text = Some(lit);
                    return Ok(());
                }

                if meta.path.is_ident("false_text") {
                    let lit: syn::LitStr = meta.value()?.parse()?;
                    false_text = Some(lit);
                    return Ok(());
                }

                // Ignore unknown keys for forward compatibility.
                Ok(())
            });

            if let Err(err) = res {
                return err.to_compile_error().into();
            }
        }

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
            if let Type::Path(tp) = &ty {
                if let Some(seg) = tp.path.segments.last() {
                    let ident = seg.ident.to_string();
                    match ident.as_str() {
                        "f32" | "f64" => is_float = true,
                        "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64"
                        | "usize" => is_int = true,
                        _ => {}
                    }
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

        if let Some(ref style) = bool_style {
            if style != "checkbox" && style != "button" && style != "radio" && style != "dropdown" {
                return syn::Error::new(
                    field_ident.span(),
                    "imgui(bool_style = ...) must be \"checkbox\", \"button\", \"radio\" or \"dropdown\"",
                )
                .to_compile_error()
                .into();
            }
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
                if bool_style.is_none() && true_text.is_none() && false_text.is_none() {
                    // Use type-level defaults from ReflectSettings when no per-field
                    // attributes are provided, mirroring ImReflect's type_settings<bool>.
                    quote! {
                        {
                            let settings = ::dear_imgui_reflect::current_settings();
                            let bool_settings: ::dear_imgui_reflect::BoolSettings = {
                                if let Some(member) = settings.member::<Self>(#field_name_lit) {
                                    if let Some(ref override_settings) = member.bools {
                                        override_settings.clone()
                                    } else {
                                        settings.bools().clone()
                                    }
                                } else {
                                    settings.bools().clone()
                                }
                            };
                            match bool_settings.style {
                                ::dear_imgui_reflect::BoolStyle::Checkbox => {
                                    __changed |= ui.checkbox(#label, &mut self.#field_ident);
                                }
                                ::dear_imgui_reflect::BoolStyle::Button => {
                                    let state_label = if self.#field_ident { "On" } else { "Off" };
                                    let button_label = format!("{label}: {}", state_label);
                                    if ui.button(&button_label) {
                                        self.#field_ident = !self.#field_ident;
                                        __changed = true;
                                    }
                                }
                                ::dear_imgui_reflect::BoolStyle::Dropdown => {
                                    let true_label = "True";
                                    let false_label = "False";
                                    let mut index: usize = if self.#field_ident { 1 } else { 0 };
                                    let items = [false_label, true_label];
                                    let local_changed =
                                        ui.combo_simple_string(#label, &mut index, &items);
                                    if local_changed {
                                        self.#field_ident = index == 1;
                                    }
                                    __changed |= local_changed;
                                }
                                ::dear_imgui_reflect::BoolStyle::Radio => {
                                    let true_label = "True";
                                    let false_label = "False";
                                    let mut local_changed = false;
                                    let current = self.#field_ident;
                                    if ui.radio_button_bool(true_label, current) {
                                        self.#field_ident = true;
                                        local_changed = true;
                                    }
                                    if ui.radio_button_bool(false_label, !current) {
                                        self.#field_ident = false;
                                        local_changed = true;
                                    }
                                    __changed |= local_changed;
                                }
                            }
                        }
                    }
                } else {
                    let style = bool_style.as_deref().unwrap_or("checkbox");

                    if style == "checkbox" {
                        quote! {
                            __changed |= ui.checkbox(#label, &mut self.#field_ident);
                        }
                    } else if style == "button" {
                        let true_label = true_text
                            .clone()
                            .unwrap_or_else(|| syn::LitStr::new("On", field_ident.span()));
                        let false_label = false_text
                            .clone()
                            .unwrap_or_else(|| syn::LitStr::new("Off", field_ident.span()));

                        quote! {
                            {
                                let state_label = if self.#field_ident {
                                    #true_label
                                } else {
                                    #false_label
                                };
                                let button_label = format!("{label}: {}", state_label);
                                if ui.button(&button_label) {
                                    self.#field_ident = !self.#field_ident;
                                    __changed = true;
                                }
                            }
                        }
                    } else if style == "dropdown" {
                        let true_label = true_text
                            .clone()
                            .unwrap_or_else(|| syn::LitStr::new("True", field_ident.span()));
                        let false_label = false_text
                            .clone()
                            .unwrap_or_else(|| syn::LitStr::new("False", field_ident.span()));

                        quote! {
                            {
                                let mut index: usize = if self.#field_ident { 1 } else { 0 };
                                let items = [#false_label, #true_label];
                                let local_changed = ui.combo_simple_string(#label, &mut index, &items);
                                if local_changed {
                                    self.#field_ident = index == 1;
                                }
                                __changed |= local_changed;
                            }
                        }
                    } else {
                        // "radio"
                        let true_label = true_text
                            .clone()
                            .unwrap_or_else(|| syn::LitStr::new("True", field_ident.span()));
                        let false_label = false_text
                            .clone()
                            .unwrap_or_else(|| syn::LitStr::new("False", field_ident.span()));

                        quote! {
                            {
                                let mut local_changed = false;
                                let current = self.#field_ident;
                                if ui.radio_button_bool(#true_label, current) {
                                    self.#field_ident = true;
                                    local_changed = true;
                                }
                                if ui.radio_button_bool(#false_label, !current) {
                                    self.#field_ident = false;
                                    local_changed = true;
                                }
                                __changed |= local_changed;
                            }
                        }
                    }
                }
            }
            FieldTypeKind::String => {
                let width_prefix = if let Some(width) = min_width_expr.clone() {
                    quote! {
                        let __imgui_reflect_width = ui.push_item_width(#width as f32);
                        let _ = &__imgui_reflect_width;
                    }
                } else if auto_resize {
                    quote! {
                        let __imgui_reflect_width = ui.push_item_width_text(&self.#field_ident);
                        let _ = &__imgui_reflect_width;
                    }
                } else {
                    quote! {}
                };

                if display_only {
                    // Display-only String: show wrapped text with the label and a
                    // simple tooltip on hover, instead of an editable input.
                    quote! {
                        {
                            let text = &self.#field_ident;
                            let display = format!("{}: {}", #label, text);
                            ui.text_wrapped(&display);
                            if ui.is_item_hovered() {
                                ui.set_item_tooltip(text);
                            }
                        }
                    }
                } else if multiline {
                    if hint_str.is_some() {
                        return syn::Error::new(
                            field_ident.span(),
                            "imgui(hint) is not supported together with multiline",
                        )
                        .to_compile_error()
                        .into();
                    }

                    // Multiline String
                    let ro_stmt = if read_only {
                        quote! { builder = builder.read_only(true); }
                    } else {
                        quote! {}
                    };

                    let size_expr = if auto_resize {
                        quote!({
                            let text = &self.#field_ident;
                            let mut lines: usize = 1;
                            for ch in text.chars() {
                                if ch == '\n' {
                                    lines += 1;
                                }
                            }
                            let height = ui.text_line_height_with_spacing() * (lines as f32);
                            [0.0, height]
                        })
                    } else if let Some(lines) = lines_expr.clone() {
                        quote!({
                            let height = ui.text_line_height_with_spacing() * (#lines as f32);
                            [0.0, height]
                        })
                    } else {
                        // ImGui default size (width from item width, height auto)
                        quote!([0.0, 0.0])
                    };

                    quote! {
                        {
                            #width_prefix
                            let size: [f32; 2] = #size_expr;
                            let mut builder = ui.input_text_multiline(#label, &mut self.#field_ident, size);
                            #ro_stmt
                            __changed |= builder.build();
                        }
                    }
                } else if hint_str.is_some() || read_only {
                    let hint = hint_str.clone();
                    let ro = read_only;
                    let hint_stmt = if let Some(h) = hint {
                        quote! { builder = builder.hint(::std::string::String::from(#h)); }
                    } else {
                        quote! {}
                    };
                    let ro_stmt = if ro {
                        quote! { builder = builder.read_only(true); }
                    } else {
                        quote! {}
                    };

                    quote! {
                        {
                            #width_prefix
                            let mut builder = ui.input_text(#label, &mut self.#field_ident);
                            #hint_stmt
                            #ro_stmt
                            __changed |= builder.build();
                        }
                    }
                } else {
                    // Default String handling
                    quote! {
                        __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                            ui,
                            #label,
                            &mut self.#field_ident,
                        );
                    }
                }
            }
            FieldTypeKind::ImString => {
                let width_prefix = if let Some(width) = min_width_expr.clone() {
                    quote! {
                        let __imgui_reflect_width = ui.push_item_width(#width as f32);
                        let _ = &__imgui_reflect_width;
                    }
                } else if auto_resize {
                    quote! {
                        let __imgui_reflect_width = ui.push_item_width_text(&self.#field_ident);
                        let _ = &__imgui_reflect_width;
                    }
                } else {
                    quote! {}
                };

                if display_only {
                    // Display-only ImString: show wrapped text with the label and
                    // a simple tooltip on hover.
                    quote! {
                        {
                            let text = self.#field_ident.as_ref();
                            let display = format!("{}: {}", #label, text);
                            ui.text_wrapped(&display);
                            if ui.is_item_hovered() {
                                ui.set_item_tooltip(text);
                            }
                        }
                    }
                } else if multiline {
                    if hint_str.is_some() {
                        return syn::Error::new(
                            field_ident.span(),
                            "imgui(hint) is not supported together with multiline",
                        )
                        .to_compile_error()
                        .into();
                    }

                    let ro_stmt = if read_only {
                        quote! { builder = builder.read_only(true); }
                    } else {
                        quote! {}
                    };

                    let size_expr = if auto_resize {
                        quote!({
                            let text = &self.#field_ident;
                            let mut lines: usize = 1;
                            for ch in text.chars() {
                                if ch == '\n' {
                                    lines += 1;
                                }
                            }
                            let height = ui.text_line_height_with_spacing() * (lines as f32);
                            [0.0, height]
                        })
                    } else if let Some(lines) = lines_expr.clone() {
                        quote!({
                            let height = ui.text_line_height_with_spacing() * (#lines as f32);
                            [0.0, height]
                        })
                    } else {
                        // ImGui default size (width from item width, height auto)
                        quote!([0.0, 0.0])
                    };

                    quote! {
                        {
                            #width_prefix
                            let size: [f32; 2] = #size_expr;
                            let mut builder = ui.input_text_multiline_imstr(#label, &mut self.#field_ident, size);
                            #ro_stmt
                            __changed |= builder.build();
                        }
                    }
                } else if hint_str.is_some() || read_only {
                    let hint = hint_str.clone();
                    let ro = read_only;
                    let hint_stmt = if let Some(h) = hint {
                        quote! { builder = builder.hint(::std::string::String::from(#h)); }
                    } else {
                        quote! {}
                    };
                    let ro_stmt = if ro {
                        quote! { builder = builder.read_only(true); }
                    } else {
                        quote! {}
                    };

                    quote! {
                        {
                            #width_prefix
                            let mut builder = ui.input_text_imstr(#label, &mut self.#field_ident);
                            #hint_stmt
                            #ro_stmt
                            __changed |= builder.build();
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
                        if let Type::Path(tp) = &ty {
                            if let Some(seg) = tp.path.segments.last() {
                                let ident = seg.ident.to_string();
                                match ident.as_str() {
                                    "f32" | "f64" => is_float_ty = true,
                                    "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16"
                                    | "u32" | "u64" | "usize" => is_int_ty = true,
                                    _ => {}
                                }
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
                    let has_columns = tuple_columns_expr.is_some();
                    let has_min_width = tuple_min_width_expr.is_some();

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

fn derive_for_enum(
    ident: syn::Ident,
    generics: syn::Generics,
    attrs: Vec<syn::Attribute>,
    data: syn::DataEnum,
) -> TokenStream {
    if data.variants.is_empty() {
        return syn::Error::new_spanned(
            ident,
            "ImGuiReflect cannot be derived for enums with no variants",
        )
        .to_compile_error()
        .into();
    }

    let mut enum_style: Option<String> = None;
    for attr in attrs.iter().filter(|a| a.path().is_ident("imgui")) {
        let res = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("enum_style") {
                let lit: syn::LitStr = meta.value()?.parse()?;
                enum_style = Some(lit.value());
                return Ok(());
            }
            Ok(())
        });

        if let Err(err) = res {
            return err.to_compile_error().into();
        }
    }

    if let Some(ref style) = enum_style {
        if style != "dropdown" && style != "radio" {
            return syn::Error::new(
                ident.span(),
                "imgui(enum_style = ...) must be \"dropdown\" or \"radio\"",
            )
            .to_compile_error()
            .into();
        }
    }

    let mut labels: Vec<syn::LitStr> = Vec::new();
    let mut to_index_arms = Vec::new();
    let mut from_index_arms = Vec::new();
    let mut radio_arms = Vec::new();

    for (idx, var) in data.variants.iter().enumerate() {
        let v_ident = &var.ident;

        // Only support C-like enums (no fields).
        match var.fields {
            Fields::Unit => {}
            _ => {
                return syn::Error::new_spanned(
                    v_ident,
                    "ImGuiReflect currently supports only C-like enums (no payload)",
                )
                .to_compile_error()
                .into();
            }
        }

        let mut label_override: Option<syn::LitStr> = None;
        for attr in var.attrs.iter().filter(|a| a.path().is_ident("imgui")) {
            let res = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    let lit: syn::LitStr = meta.value()?.parse()?;
                    label_override = Some(lit);
                    return Ok(());
                }
                Ok(())
            });

            if let Err(err) = res {
                return err.to_compile_error().into();
            }
        }

        let label_lit = if let Some(l) = label_override {
            l
        } else {
            syn::LitStr::new(&v_ident.to_string(), v_ident.span())
        };

        labels.push(label_lit.clone());

        let idx_lit = idx as u32;
        to_index_arms.push(quote! {
            if current == ::core::mem::discriminant(&Self::#v_ident) {
                index = #idx_lit as usize;
            }
        });
        from_index_arms.push(quote! { #idx_lit => Self::#v_ident });

        radio_arms.push(quote! {
            {
                let active = index == #idx_lit as usize;
                if ui.radio_button_bool(#label_lit, active) {
                    index = #idx_lit as usize;
                    changed = true;
                }
            }
        });
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let body_dropdown = quote! {
        let labels: &[&str] = &[#(#labels),*];

        let current = ::core::mem::discriminant(self);
        let mut index: usize = 0;
        #(#to_index_arms)*

        let mut changed = ui.combo_simple_string(label, &mut index, labels);
        if changed {
            let new_value = match index as u32 {
                #(#from_index_arms,)*
                _ => return false,
            };
            *self = new_value;
        }
        changed
    };

    let body_radio = quote! {
        let current = ::core::mem::discriminant(self);
        let mut index: usize = 0;
        #(#to_index_arms)*

        let mut changed = false;
        #(#radio_arms)*

        if changed {
            let new_value = match index as u32 {
                #(#from_index_arms,)*
                _ => return false,
            };
            *self = new_value;
        }
        changed
    };

    let body = if enum_style.as_deref() == Some("radio") {
        body_radio
    } else {
        body_dropdown
    };

    let expanded = quote! {
        impl #impl_generics ::dear_imgui_reflect::ImGuiReflect for #ident #ty_generics #where_clause {
            fn imgui_reflect(
                &mut self,
                ui: &::dear_imgui_reflect::imgui::Ui,
                label: &str,
            ) -> bool {
                #body
            }
        }
    };

    expanded.into()
}
