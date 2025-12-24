use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::spanned::Spanned;
use syn::{Expr, Fields, Generics, Ident, LitStr, Type, parse_quote};

/// Generates an `ImGuiReflect` implementation for enums.
pub fn derive_for_enum(
    ident: syn::Ident,
    mut generics: Generics,
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

    let use_radio = matches!(enum_style.as_deref(), Some("radio"));

    let reflect_settings_ident =
        syn::Ident::new("__imgui_reflect_settings", proc_macro2::Span::call_site());

    let mut labels: Vec<syn::LitStr> = Vec::new();
    let mut current_index_arms = Vec::new();
    let mut from_index_arms = Vec::new();
    let mut radio_arms = Vec::new();
    let mut payload_match_arms = Vec::new();
    let mut bound_types: Vec<Type> = Vec::new();
    let mut default_types: Vec<Type> = Vec::new();

    for (idx, var) in data.variants.iter().enumerate() {
        let v_ident = &var.ident;
        let variant_segment_lit = syn::LitStr::new(&v_ident.to_string(), v_ident.span());

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

        let idx_usize = idx;
        current_index_arms.push(match &var.fields {
            Fields::Unit => quote! { Self::#v_ident => #idx_usize, },
            Fields::Unnamed(_) => quote! { Self::#v_ident ( .. ) => #idx_usize, },
            Fields::Named(_) => quote! { Self::#v_ident { .. } => #idx_usize, },
        });

        radio_arms.push(quote! {
            {
                let active = index == #idx_usize;
                if ui.radio_button_bool(#label_lit, active) {
                    index = #idx_usize;
                    changed_select = true;
                }
            }
        });

        let from_arm = match &var.fields {
            Fields::Unit => quote! { #idx_usize => Self::#v_ident, },
            Fields::Unnamed(fields) => {
                let defaults: Vec<TokenStream2> = fields
                    .unnamed
                    .iter()
                    .map(|f| {
                        bound_types.push(f.ty.clone());
                        default_types.push(f.ty.clone());
                        quote! { ::core::default::Default::default() }
                    })
                    .collect();
                quote! { #idx_usize => Self::#v_ident( #(#defaults),* ), }
            }
            Fields::Named(fields) => {
                let defaults: Vec<TokenStream2> = fields
                    .named
                    .iter()
                    .filter_map(|f| {
                        let name = f.ident.as_ref()?;
                        bound_types.push(f.ty.clone());
                        default_types.push(f.ty.clone());
                        Some(quote! { #name: ::core::default::Default::default() })
                    })
                    .collect();
                quote! { #idx_usize => Self::#v_ident { #(#defaults),* }, }
            }
        };
        from_index_arms.push(from_arm);

        payload_match_arms.push(match &var.fields {
            Fields::Unit => quote! { Self::#v_ident => {} },
            Fields::Unnamed(fields) => {
                let mut patterns: Vec<TokenStream2> = Vec::new();
                let mut bindings: Vec<Ident> = Vec::new();

                let mut field_stmts = Vec::new();
                for (field_index, field) in fields.unnamed.iter().enumerate() {
                    let mut skip = false;
                    let mut label_override: Option<syn::LitStr> = None;
                    let mut field_read_only = false;
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
                            if meta.path.is_ident("read_only") {
                                field_read_only = true;
                                return Ok(());
                            }
                            Ok(())
                        });
                        if let Err(err) = res {
                            return err.to_compile_error().into();
                        }
                    }
                    if skip {
                        let unused_ident = syn::Ident::new(
                            &format!("_skip_{field_index}"),
                            proc_macro2::Span::call_site(),
                        );
                        patterns.push(quote! { #unused_ident });
                        continue;
                    }

                    let binding_ident = syn::Ident::new(
                        &format!("__field_{field_index}"),
                        proc_macro2::Span::call_site(),
                    );
                    patterns.push(quote! { #binding_ident });
                    bindings.push(binding_ident.clone());

                    let label_lit = label_override.unwrap_or_else(|| {
                        syn::LitStr::new(&field_index.to_string(), field.span())
                    });
                    let field_segment_lit = syn::LitStr::new(
                        &field_index.to_string(),
                        proc_macro2::Span::call_site(),
                    );
                    let member_key_lit = syn::LitStr::new(
                        &format!("{}.{}", v_ident, field_index),
                        v_ident.span(),
                    );

                    field_stmts.push(quote! {
                        let local_changed = ::dear_imgui_reflect::with_field_path_static(#field_segment_lit, || {
                            let __member_read_only = {
                                let settings = &#reflect_settings_ident;
                                if let Some(member) = settings.member::<Self>(#member_key_lit) {
                                    member.read_only
                                } else {
                                    false
                                }
                            };
                            if #field_read_only || __member_read_only {
                                let _disabled = ui.begin_disabled();
                                let changed = ::dear_imgui_reflect::ImGuiValue::imgui_value(ui, #label_lit, #binding_ident);
                                drop(_disabled);
                                changed
                            } else {
                                ::dear_imgui_reflect::ImGuiValue::imgui_value(ui, #label_lit, #binding_ident)
                            }
                        });
                        __changed |= local_changed;
                    });
                }

                quote! {
                    Self::#v_ident( #(#patterns),* ) => {
                        ::dear_imgui_reflect::with_field_path_static(#variant_segment_lit, || {
                            ui.indent();
                            #(#field_stmts)*
                            ui.unindent();
                        });
                    }
                }
            }
            Fields::Named(fields) => {
                let mut field_stmts = Vec::new();
                let mut bindings: Vec<Ident> = Vec::new();
                for field in fields.named.iter() {
                    let Some(name) = field.ident.as_ref() else {
                        continue;
                    };

                    let mut skip = false;
                    let mut label_override: Option<syn::LitStr> = None;
                    let mut field_read_only = false;
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
                            if meta.path.is_ident("read_only") {
                                field_read_only = true;
                                return Ok(());
                            }
                            Ok(())
                    });
                    if let Err(err) = res {
                        return err.to_compile_error().into();
                    }
                }
                if skip {
                    continue;
                }

                bindings.push(name.clone());

                let label_lit = label_override
                    .unwrap_or_else(|| syn::LitStr::new(&name.to_string(), name.span()));
                let field_segment_lit =
                    syn::LitStr::new(&name.to_string(), proc_macro2::Span::call_site());
                let member_key_lit =
                    syn::LitStr::new(&format!("{}.{}", v_ident, name), v_ident.span());

                    field_stmts.push(quote! {
                        let local_changed = ::dear_imgui_reflect::with_field_path_static(#field_segment_lit, || {
                            let __member_read_only = {
                                let settings = &#reflect_settings_ident;
                                if let Some(member) = settings.member::<Self>(#member_key_lit) {
                                    member.read_only
                                } else {
                                    false
                                }
                            };
                            if #field_read_only || __member_read_only {
                                let _disabled = ui.begin_disabled();
                                let changed = ::dear_imgui_reflect::ImGuiValue::imgui_value(ui, #label_lit, #name);
                                drop(_disabled);
                                changed
                            } else {
                                ::dear_imgui_reflect::ImGuiValue::imgui_value(ui, #label_lit, #name)
                            }
                        });
                        __changed |= local_changed;
                    });
                }

                let match_pat = if bindings.is_empty() {
                    quote! { Self::#v_ident { .. } }
                } else {
                    quote! { Self::#v_ident { #(#bindings),*, .. } }
                };

                quote! {
                    #match_pat => {
                        ::dear_imgui_reflect::with_field_path_static(#variant_segment_lit, || {
                            ui.indent();
                            #(#field_stmts)*
                            ui.unindent();
                        });
                    }
                }
            }
        });
    }

    {
        let where_clause = generics.make_where_clause();
        for ty in bound_types {
            where_clause
                .predicates
                .push(parse_quote!(#ty: ::dear_imgui_reflect::ImGuiValue));
        }
        for ty in default_types {
            where_clause
                .predicates
                .push(parse_quote!(#ty: ::core::default::Default));
        }
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let labels_decl = if use_radio {
        quote! {}
    } else {
        quote! {
            let labels: &[&str] = &[#(#labels),*];
        }
    };

    let select_widget = if use_radio {
        quote! {
            let mut changed_select = false;
            ui.text(label);
            ui.indent();
            #(#radio_arms)*
            ui.unindent();
        }
    } else {
        quote! {
            let mut changed_select = ui.combo_simple_string(label, &mut index, labels);
        }
    };

    let body = quote! {
        let #reflect_settings_ident = ::dear_imgui_reflect::current_settings();
        #labels_decl

        let mut index: usize = match self {
            #(#current_index_arms)*
        };

        #select_widget

        let mut __changed = false;
        if changed_select {
            let new_value = match index {
                #(#from_index_arms)*
                _ => return false,
            };
            *self = new_value;
            __changed = true;
        }

        match self {
            #(#payload_match_arms,)*
        }

        __changed
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
