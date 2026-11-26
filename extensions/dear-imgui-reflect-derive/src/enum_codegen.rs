use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::{Attribute, DataEnum, Error, Fields, Generics, Ident, LitStr, Result, parse_quote};

use crate::attrs::{parse_enum_style, parse_variant_name};
use crate::field_codegen::FieldEditor;
use crate::impl_reflect;

pub fn derive_for_enum(
    ident: &Ident,
    mut generics: Generics,
    attrs: &[Attribute],
    data: DataEnum,
) -> Result<TokenStream> {
    if data.variants.is_empty() {
        return Err(Error::new(
            ident.span(),
            "ImGuiReflect cannot be derived for enums with no variants",
        ));
    }
    let radio = parse_enum_style(attrs)?;
    let mut labels = Vec::new();
    let mut current_arms = Vec::new();
    let mut select_arms = Vec::new();
    let mut payload_arms = Vec::new();

    for (index, variant) in data.variants.iter().enumerate() {
        let name = &variant.ident;
        let name_lit = LitStr::new(&name.to_string(), name.span());
        labels.push(parse_variant_name(&variant.attrs)?.unwrap_or_else(|| name_lit.clone()));

        current_arms.push(match &variant.fields {
            Fields::Unit => quote!(Self::#name => #index,),
            Fields::Unnamed(_) => quote!(Self::#name(..) => #index,),
            Fields::Named(_) => quote!(Self::#name { .. } => #index,),
        });

        let mut bindings = Vec::new();
        let mut defaults = Vec::new();
        let mut editors = Vec::new();
        for (field_index, field) in variant.fields.iter().enumerate() {
            let editor = FieldEditor::parse(field, field_index, Some(name))?;
            let ty = &field.ty;
            if !generics.params.is_empty() {
                generics
                    .make_where_clause()
                    .predicates
                    .push(parse_quote!(#ty: ::core::default::Default));
            }

            let value = if editor.skip() {
                quote!(_)
            } else {
                let binding = format_ident!(
                    "__dear_imgui_reflect_payload_{field_index}",
                    span = Span::mixed_site()
                );
                editors.push(editor.render(&mut generics, quote!(#binding))?);
                quote!(#binding)
            };
            let default = quote!(::core::default::Default::default());
            if let Some(field_name) = &field.ident {
                bindings.push(quote!(#field_name: #value));
                defaults.push(quote!(#field_name: #default));
            } else {
                bindings.push(value);
                defaults.push(default);
            }
        }

        let (pattern, constructor) = match &variant.fields {
            Fields::Unit => (quote!(Self::#name), quote!(Self::#name)),
            Fields::Unnamed(_) => (
                quote!(Self::#name(#(#bindings),*)),
                quote!(Self::#name(#(#defaults),*)),
            ),
            Fields::Named(_) => (
                quote!(Self::#name { #(#bindings),* }),
                quote!(Self::#name { #(#defaults),* }),
            ),
        };
        select_arms.push(quote!(#index => #constructor,));
        payload_arms.push(if editors.is_empty() {
            quote!(#pattern => {},)
        } else {
            quote_spanned! {Span::mixed_site()=>
                #pattern => {
                    let __dear_imgui_reflect_variant_id = __dear_imgui_reflect_ui.push_id(#name_lit);
                    let __dear_imgui_reflect_variant_path =
                        __dear_imgui_reflect_inspector.push_path_static(#name_lit);
                    __dear_imgui_reflect_ui.with_indent(|| {
                        #(#editors)*
                    });
                },
            }
        });
    }

    let receiver = quote!(self);
    let body = quote_spanned! {Span::mixed_site()=>
        let __dear_imgui_reflect_enum_id =
            __dear_imgui_reflect_ui.push_id(__dear_imgui_reflect_label);
        let __dear_imgui_reflect_current = match #receiver {
            #(#current_arms)*
        };
        let __dear_imgui_reflect_index = ::dear_imgui_reflect::__private::render_enum_selection(
            __dear_imgui_reflect_ui,
            __dear_imgui_reflect_label,
            &[#(#labels),*],
            __dear_imgui_reflect_current,
            #radio,
        );
        let mut __dear_imgui_reflect_changed =
            __dear_imgui_reflect_index != __dear_imgui_reflect_current;
        if __dear_imgui_reflect_changed {
            *#receiver = match __dear_imgui_reflect_index {
                #(#select_arms)*
                _ => return false,
            };
        }
        match #receiver {
            #(#payload_arms)*
        }
        __dear_imgui_reflect_changed
    };
    Ok(impl_reflect(ident, &generics, body))
}
