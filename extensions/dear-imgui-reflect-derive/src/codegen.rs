use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Expr, Fields, Generics, Ident, LitStr};

/// Generates an `ImGuiReflect` implementation for C-like enums.
pub fn derive_for_enum(
    ident: syn::Ident,
    generics: Generics,
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

    if let Some(ref style) = enum_style
        && style != "dropdown"
        && style != "radio"
    {
        return syn::Error::new(
            ident.span(),
            "imgui(enum_style = ...) must be \"dropdown\" or \"radio\"",
        )
        .to_compile_error()
        .into();
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

/// Generates code for a boolean field, including support for type-level
/// defaults and per-field bool_style / true_text / false_text overrides.
pub fn gen_bool_field(
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
                .unwrap_or_else(|| LitStr::new("On", field_ident.span()));
            let false_label = false_text
                .clone()
                .unwrap_or_else(|| LitStr::new("Off", field_ident.span()));

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
                .unwrap_or_else(|| LitStr::new("True", field_ident.span()));
            let false_label = false_text
                .clone()
                .unwrap_or_else(|| LitStr::new("False", field_ident.span()));

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
                .unwrap_or_else(|| LitStr::new("True", field_ident.span()));
            let false_label = false_text
                .clone()
                .unwrap_or_else(|| LitStr::new("False", field_ident.span()));

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
            let __imgui_reflect_width = ui.push_item_width_text(&self.#field_ident);
            let _ = &__imgui_reflect_width;
        }
    } else {
        quote! {}
    };

    let tokens = if display_only {
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
        quote! {
            __changed |= ::dear_imgui_reflect::ImGuiValue::imgui_value(
                ui,
                #label,
                &mut self.#field_ident,
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
            let __imgui_reflect_width = ui.push_item_width_text(&self.#field_ident);
            let _ = &__imgui_reflect_width;
        }
    } else {
        quote! {}
    };

    let tokens = if display_only {
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
    };

    Ok(tokens)
}
