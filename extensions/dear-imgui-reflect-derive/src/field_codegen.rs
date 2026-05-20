use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Expr, Ident, LitStr};

/// Generates code for a boolean field, including support for type-level
/// defaults and per-field bool_style / true_text / false_text overrides.
pub fn gen_bool_field(
    reflect_settings_ident: &Ident,
    field_ident: &Ident,
    field_name_lit: &LitStr,
    label: &TokenStream2,
    bool_style: &Option<String>,
    true_text: &Option<LitStr>,
    false_text: &Option<LitStr>,
) -> syn::Result<TokenStream2> {
    let tokens = if bool_style.is_none() && true_text.is_none() && false_text.is_none() {
        quote! {
            {
                let settings = &#reflect_settings_ident;
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
                        __changed |= ui.checkbox(#label, __field);
                    }
                    ::dear_imgui_reflect::BoolStyle::Button => {
                        let state_label = if *__field { "On" } else { "Off" };
                        let button_label = format!("{label}: {}", state_label);
                        if ui.button(&button_label) {
                            *__field = !*__field;
                            __changed = true;
                        }
                    }
                    ::dear_imgui_reflect::BoolStyle::Dropdown => {
                        let true_label = "True";
                        let false_label = "False";
                        let mut index: usize = if *__field { 1 } else { 0 };
                        let items = [false_label, true_label];
                        let local_changed =
                            ui.combo_simple_string(#label, &mut index, &items);
                        if local_changed {
                            *__field = index == 1;
                        }
                        __changed |= local_changed;
                    }
                    ::dear_imgui_reflect::BoolStyle::Radio => {
                        let true_label = "True";
                        let false_label = "False";
                        let mut local_changed = false;
                        let current = *__field;
                        if ui.radio_button_bool(true_label, current) {
                            *__field = true;
                            local_changed = true;
                        }
                        if ui.radio_button_bool(false_label, !current) {
                            *__field = false;
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
                __changed |= ui.checkbox(#label, __field);
            }
        } else if style == "button" {
            let true_label = true_text
                .clone()
                .unwrap_or_else(|| LitStr::new("On", field_ident.span()));
            let false_label = false_text
                .clone()
                .unwrap_or_else(|| LitStr::new("Off", field_ident.span()));

            quote! {
                {
                    let state_label = if *__field {
                        #true_label
                    } else {
                        #false_label
                    };
                    let button_label = format!("{label}: {}", state_label);
                    if ui.button(&button_label) {
                        *__field = !*__field;
                        __changed = true;
                    }
                }
            }
        } else if style == "dropdown" {
            let true_label = true_text
                .clone()
                .unwrap_or_else(|| LitStr::new("True", field_ident.span()));
            let false_label = false_text
                .clone()
                .unwrap_or_else(|| LitStr::new("False", field_ident.span()));

            quote! {
                {
                    let mut index: usize = if *__field { 1 } else { 0 };
                    let items = [#false_label, #true_label];
                    let local_changed = ui.combo_simple_string(#label, &mut index, &items);
                    if local_changed {
                        *__field = index == 1;
                    }
                    __changed |= local_changed;
                }
            }
        } else {
            // "radio"
            let true_label = true_text
                .clone()
                .unwrap_or_else(|| LitStr::new("True", field_ident.span()));
            let false_label = false_text
                .clone()
                .unwrap_or_else(|| LitStr::new("False", field_ident.span()));

            quote! {
                {
                    let mut local_changed = false;
                    let current = *__field;
                    if ui.radio_button_bool(#true_label, current) {
                        *__field = true;
                        local_changed = true;
                    }
                    if ui.radio_button_bool(#false_label, !current) {
                        *__field = false;
                        local_changed = true;
                    }
                    __changed |= local_changed;
                }
            }
        }
    };

    Ok(tokens)
}

/// Generates code for `String` fields, including multiline, hints, read-only,
/// display-only, and auto-resize behavior.
#[allow(clippy::too_many_arguments)]
pub fn gen_string_field(
    field_ident: &Ident,
    label: &TokenStream2,
    multiline: bool,
    display_only: bool,
    read_only: bool,
    auto_resize: bool,
    min_width_expr: &Option<Expr>,
    lines_expr: &Option<Expr>,
    hint_str: &Option<LitStr>,
) -> syn::Result<TokenStream2> {
    let width_prefix = if let Some(width) = min_width_expr.clone() {
        quote! {
            let __imgui_reflect_width = ui.push_item_width(#width as f32);
            let _ = &__imgui_reflect_width;
        }
    } else if auto_resize {
        quote! {
            let __imgui_reflect_width = ui.push_item_width_text(__field.as_str());
            let _ = &__imgui_reflect_width;
        }
    } else {
        quote! {}
    };

    let tokens = if display_only {
        quote! {
            {
                let text = __field.as_str();
                let display = format!("{}: {}", #label, text);
                ui.text_wrapped(&display);
                if ui.is_item_hovered() {
                    ui.set_item_tooltip(text);
                }
            }
        }
    } else if multiline {
        if hint_str.is_some() {
            return Err(syn::Error::new(
                field_ident.span(),
                "imgui(hint) is not supported together with multiline",
            ));
        }

        let ro_stmt = if read_only {
            quote! { builder = builder.read_only(true); }
        } else {
            quote! {}
        };

        let size_expr = if auto_resize {
            quote!({
                let text = __field.as_str();
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
            quote!([0.0, 0.0])
        };

        quote! {
            {
                #width_prefix
                let size: [f32; 2] = #size_expr;
                let mut builder = ui.input_text_multiline(#label, __field, size);
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
                let mut builder = ui.input_text(#label, __field);
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
                __field,
            );
        }
    };

    Ok(tokens)
}

/// Generates code for `ImString` fields, mirroring the behavior of `String`
/// but using ImString-specific input widgets.
#[allow(clippy::too_many_arguments)]
pub fn gen_imstring_field(
    field_ident: &Ident,
    label: &TokenStream2,
    multiline: bool,
    display_only: bool,
    read_only: bool,
    auto_resize: bool,
    min_width_expr: &Option<Expr>,
    lines_expr: &Option<Expr>,
    hint_str: &Option<LitStr>,
) -> syn::Result<TokenStream2> {
    let width_prefix = if let Some(width) = min_width_expr.clone() {
        quote! {
            let __imgui_reflect_width = ui.push_item_width(#width as f32);
            let _ = &__imgui_reflect_width;
        }
    } else if auto_resize {
        quote! {
            let __imgui_reflect_width = ui.push_item_width_text(__field.as_ref());
            let _ = &__imgui_reflect_width;
        }
    } else {
        quote! {}
    };

    let tokens = if display_only {
        quote! {
            {
                let text = __field.as_ref();
                let display = format!("{}: {}", #label, text);
                ui.text_wrapped(&display);
                if ui.is_item_hovered() {
                    ui.set_item_tooltip(text);
                }
            }
        }
    } else if multiline {
        if hint_str.is_some() {
            return Err(syn::Error::new(
                field_ident.span(),
                "imgui(hint) is not supported together with multiline",
            ));
        }

        let ro_stmt = if read_only {
            quote! { builder = builder.read_only(true); }
        } else {
            quote! {}
        };

        let size_expr = if auto_resize {
            quote!({
                let text = __field.as_ref();
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
            quote!([0.0, 0.0])
        };

        quote! {
            {
                #width_prefix
                let size: [f32; 2] = #size_expr;
                let mut builder = ui.input_text_multiline_imstr(#label, __field, size);
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
                let mut builder = ui.input_text_imstr(#label, __field);
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
                __field,
            );
        }
    };

    Ok(tokens)
}
