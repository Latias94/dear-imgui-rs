use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{Error, Field, Generics, Ident, Index, LitStr, Result, Type, parse_quote};

use crate::attrs::{BoolAttrs, FieldAttrs, TextAttrs, TupleAttrs, parse_field_attrs};
use crate::internal::type_name;
use crate::numeric_codegen::render_numeric;
use crate::option_tokens;

pub struct FieldEditor<'a> {
    field: &'a Field,
    attrs: FieldAttrs,
    segment: LitStr,
    key: LitStr,
}

impl<'a> FieldEditor<'a> {
    pub fn parse(field: &'a Field, index: usize, variant: Option<&Ident>) -> Result<Self> {
        let attrs = parse_field_attrs(field)?;
        let name = field
            .ident
            .as_ref()
            .map_or_else(|| index.to_string(), ToString::to_string);
        let key = variant.map_or_else(|| name.clone(), |variant| format!("{variant}.{name}"));
        Ok(Self {
            field,
            attrs,
            segment: LitStr::new(&name, field.span()),
            key: LitStr::new(&key, field.span()),
        })
    }

    pub fn skip(&self) -> bool {
        self.attrs.skip
    }

    pub fn render(&self, generics: &mut Generics, value: TokenStream) -> Result<TokenStream> {
        let ty = &self.field.ty;
        let label = self.attrs.name.as_ref().unwrap_or(&self.segment);
        let typed =
            generics.lifetimes().next().is_none() && generics.type_params().next().is_none();
        let mut tuple = ty;
        while let Type::Paren(paren) = tuple {
            tuple = &paren.elem;
        }

        let body = if self.attrs.numeric.has_attrs() {
            render_numeric(&self.attrs.numeric, ty, label, &value)?
        } else if self.attrs.text.has_attrs() {
            if !type_name(ty).is_some_and(|name| name == "String") {
                return Err(Error::new_spanned(
                    ty,
                    "imgui text attributes require a String field",
                ));
            }
            render_text(&self.attrs.text, ty, label, &value)?
        } else if self.attrs.boolean.has_attrs() || type_name(ty).is_some_and(|name| name == "bool")
        {
            if !type_name(ty).is_some_and(|name| name == "bool") {
                return Err(Error::new_spanned(
                    ty,
                    "imgui boolean attributes require a bool field",
                ));
            }
            render_bool(&self.attrs.boolean, &self.segment, label, &value)?
        } else if let Type::Tuple(tuple) = tuple {
            for elem in &tuple.elems {
                add_value_bound(generics, elem);
            }
            render_tuple(&self.attrs.tuple, &self.key, label, &value, tuple, typed)?
        } else {
            add_value_bound(generics, ty);
            render_value(label, &value)
        };
        if self.attrs.tuple.has_attrs() && !matches!(tuple, Type::Tuple(_)) {
            return Err(Error::new_spanned(
                ty,
                "imgui tuple attributes require a tuple field",
            ));
        }
        let groups = [
            self.attrs.numeric.has_attrs(),
            self.attrs.text.has_attrs(),
            self.attrs.boolean.has_attrs(),
            self.attrs.tuple.has_attrs(),
        ];
        if groups.into_iter().filter(|enabled| *enabled).count() > 1 {
            return Err(Error::new_spanned(
                ty,
                "imgui widget attributes from different groups cannot be combined",
            ));
        }

        let segment = &self.segment;
        let render = with_member(&self.key, typed, self.attrs.read_only, &body);
        Ok(quote_spanned! {Span::mixed_site()=> {
            let __dear_imgui_reflect_path =
                __dear_imgui_reflect_inspector.push_path_static(#segment);
            __dear_imgui_reflect_changed |= #render;
        }})
    }
}

fn add_value_bound(generics: &mut Generics, ty: &Type) {
    if !generics.params.is_empty() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(#ty: ::dear_imgui_reflect::ImGuiValue));
    }
}

fn with_member(key: &LitStr, typed: bool, read_only: bool, body: &TokenStream) -> TokenStream {
    let member = if typed {
        quote_spanned! {Span::mixed_site()=>
            __dear_imgui_reflect_inspector.settings().member::<Self>(#key)
        }
    } else {
        option_tokens::<TokenStream>(None)
    };
    quote_spanned! {Span::mixed_site()=>
        __dear_imgui_reflect_inspector.with_member(
            #member,
            #read_only,
            |__dear_imgui_reflect_inspector, __dear_imgui_reflect_member| #body,
        )
    }
}

fn render_value(label: &LitStr, value: &TokenStream) -> TokenStream {
    quote_spanned! {Span::mixed_site()=>
        ::dear_imgui_reflect::ImGuiValue::imgui_value_with_member(
            __dear_imgui_reflect_inspector,
            #label,
            #value,
            __dear_imgui_reflect_member,
        )
    }
}

fn render_bool(
    attrs: &BoolAttrs,
    segment: &LitStr,
    label: &LitStr,
    value: &TokenStream,
) -> Result<TokenStream> {
    let style = if let Some(literal) = &attrs.style {
        Some(match literal.value().as_str() {
            "checkbox" if attrs.true_text.is_some() || attrs.false_text.is_some() => {
                return Err(Error::new(
                    literal.span(),
                    "imgui true_text and false_text do not apply to checkboxes",
                ));
            }
            "checkbox" => quote!(::dear_imgui_reflect::BoolStyle::Checkbox),
            "button" => quote!(::dear_imgui_reflect::BoolStyle::Button),
            "dropdown" => quote!(::dear_imgui_reflect::BoolStyle::Dropdown),
            "radio" => quote!(::dear_imgui_reflect::BoolStyle::Radio),
            _ => {
                return Err(Error::new(
                    literal.span(),
                    "imgui(bool_style) must be \"checkbox\", \"button\", \"dropdown\" or \"radio\"",
                ));
            }
        })
    } else {
        None
    };
    let style = option_tokens(style);
    let true_text = option_tokens(attrs.true_text.as_ref());
    let false_text = option_tokens(attrs.false_text.as_ref());
    Ok(quote_spanned! {Span::mixed_site()=>
        ::dear_imgui_reflect::__private::render_bool(
            __dear_imgui_reflect_inspector,
            #label,
            #value,
            __dear_imgui_reflect_member,
            #segment,
            #style,
            [#false_text, #true_text],
        )
    })
}

fn render_text(
    attrs: &TextAttrs,
    ty: &Type,
    label: &LitStr,
    value: &TokenStream,
) -> Result<TokenStream> {
    let error = |message| Error::new_spanned(ty, message);
    if attrs.auto_resize && attrs.min_width.is_some() {
        return Err(error(
            "imgui(auto_resize) cannot be combined with min_width",
        ));
    }
    if attrs.auto_resize && attrs.lines.is_some() {
        return Err(error("imgui(auto_resize) cannot be combined with lines"));
    }
    if !attrs.multiline && (attrs.auto_resize || attrs.lines.is_some()) {
        return Err(error("imgui(auto_resize, lines) require multiline"));
    }
    if attrs.multiline && attrs.hint.is_some() {
        return Err(error("imgui(hint) cannot be combined with multiline"));
    }
    if attrs.display_only && (attrs.multiline || attrs.hint.is_some() || attrs.min_width.is_some())
    {
        return Err(error(
            "imgui(display_only) cannot be combined with text input attributes",
        ));
    }

    let widget = quote!(::dear_imgui_reflect::__private::TextWidget);
    let widget = if attrs.display_only {
        quote!(#widget::Display)
    } else if attrs.multiline {
        let lines = option_tokens(attrs.lines.as_ref().map(|lines| quote!((#lines) as f32)));
        let auto_resize = attrs.auto_resize;
        quote!(#widget::Multiline { lines: #lines, auto_resize: #auto_resize })
    } else {
        let hint = option_tokens(attrs.hint.as_ref());
        quote!(#widget::Input { hint: #hint })
    };
    let width = option_tokens(
        attrs
            .min_width
            .as_ref()
            .map(|width| quote!((#width) as f32)),
    );
    Ok(quote_spanned! {Span::mixed_site()=> {
        let __dear_imgui_reflect_widget = #widget;
        let __dear_imgui_reflect_width = #width;
        ::dear_imgui_reflect::__private::render_text(
            __dear_imgui_reflect_inspector,
            #label,
            #value,
            __dear_imgui_reflect_widget,
            __dear_imgui_reflect_width,
        )
    }})
}

fn render_tuple(
    attrs: &TupleAttrs,
    key: &LitStr,
    label: &LitStr,
    value: &TokenStream,
    tuple: &syn::TypeTuple,
    typed: bool,
) -> Result<TokenStream> {
    let len = tuple.elems.len();
    let arms = (0..len).map(|index| {
        let element_label = LitStr::new(&format!("##{index}"), label.span());
        let element_key = LitStr::new(&format!("{}[{index}]", key.value()), key.span());
        let element_index = Index::from(index);
        let element_value = quote_spanned! {Span::mixed_site()=>
            &mut __dear_imgui_reflect_tuple.#element_index
        };
        let body = render_value(&element_label, &element_value);
        let render = with_member(&element_key, typed, false, &body);
        quote!(#index => #render,)
    });

    let render_mode = if let Some(style) = &attrs.render {
        let mode = match style.value().as_str() {
            "line" if attrs.columns.is_some() || attrs.min_width.is_some() => {
                return Err(Error::new(
                    style.span(),
                    "imgui tuple_columns and tuple_min_width do not apply in line mode",
                ));
            }
            "line" => quote!(::dear_imgui_reflect::TupleRenderMode::Line),
            "grid" => quote!(::dear_imgui_reflect::TupleRenderMode::Grid),
            _ => {
                return Err(Error::new(
                    style.span(),
                    "imgui(tuple_render) must be \"line\" or \"grid\"",
                ));
            }
        };
        quote_spanned! {Span::mixed_site()=> __dear_imgui_reflect_tuple_settings.render_mode = #mode; }
    } else {
        TokenStream::new()
    };
    let dropdown = attrs.dropdown.then(|| {
        quote_spanned! {Span::mixed_site()=>
            __dear_imgui_reflect_tuple_settings.dropdown = true;
        }
    });
    let columns = attrs.columns.as_ref().map(|columns| {
        quote_spanned! {Span::mixed_site()=>
            __dear_imgui_reflect_tuple_settings.columns = (#columns) as usize;
        }
    });
    let width = attrs.min_width.as_ref().map(|width| quote_spanned! {Span::mixed_site()=>
        __dear_imgui_reflect_tuple_settings.min_width = ::core::option::Option::Some((#width) as f32);
    });
    let settings = if attrs.has_attrs() {
        quote_spanned! {Span::mixed_site()=>
            let mut __dear_imgui_reflect_tuple_settings = __dear_imgui_reflect_tuple_settings.clone();
            #render_mode
            #dropdown
            #columns
            #width
            let __dear_imgui_reflect_tuple_settings = &__dear_imgui_reflect_tuple_settings;
        }
    } else {
        TokenStream::new()
    };

    Ok(quote_spanned! {Span::mixed_site()=> {
        let __dear_imgui_reflect_tuple_settings = __dear_imgui_reflect_member
            .and_then(|__dear_imgui_reflect_member| __dear_imgui_reflect_member.tuples.as_ref())
            .unwrap_or_else(|| __dear_imgui_reflect_inspector.settings().tuples());
        #settings
        let __dear_imgui_reflect_tuple = #value;
        ::dear_imgui_reflect::imgui_tuple_body(
            __dear_imgui_reflect_inspector,
            #label,
            #len,
            __dear_imgui_reflect_tuple_settings,
            |__dear_imgui_reflect_inspector, __dear_imgui_reflect_index| {
                match __dear_imgui_reflect_index {
                    #(#arms)*
                    _ => false,
                }
            },
        )
    }})
}
