use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{Error, LitStr, Result, Type};

use crate::attrs::NumericAttrs;
use crate::internal::{NumericFormatType, numeric_type};
use crate::numeric_format::validate_and_normalize;
use crate::option_tokens;

enum Widget {
    Input,
    Slider,
    Drag,
}

impl Widget {
    fn select(attrs: &NumericAttrs, ty: &Type) -> Result<Self> {
        let error = |message| Error::new_spanned(ty, message);
        let selectors = [
            attrs.as_input,
            attrs.as_drag,
            attrs.slider,
            attrs.slider_default_range,
        ];
        if selectors.into_iter().filter(|enabled| *enabled).count() > 1 {
            return Err(error(
                "imgui numeric widget selectors are mutually exclusive",
            ));
        }
        let has_range = attrs.min.is_some() && attrs.max.is_some();
        if attrs.min.is_some() != attrs.max.is_some() {
            return Err(error("imgui(min, max) must specify both bounds"));
        }
        if attrs.slider && !has_range {
            return Err(error("imgui(slider) requires both min and max"));
        }
        if attrs.slider_default_range && has_range {
            return Err(error(
                "imgui(slider_default_range) cannot be combined with min or max",
            ));
        }

        let widget = if attrs.as_input {
            Self::Input
        } else if attrs.as_drag {
            Self::Drag
        } else if has_range || attrs.slider_default_range {
            Self::Slider
        } else if attrs.speed.is_some() {
            Self::Drag
        } else {
            Self::Input
        };

        if matches!(widget, Self::Input) && has_range {
            return Err(error("imgui(min, max) cannot be combined with as_input"));
        }
        if !matches!(widget, Self::Input) && (attrs.step.is_some() || attrs.step_fast.is_some()) {
            return Err(error("imgui(step, step_fast) require an input widget"));
        }
        if !matches!(widget, Self::Drag) && attrs.speed.is_some() {
            return Err(error("imgui(speed) requires a drag widget"));
        }
        if attrs.clamp && !(has_range || attrs.slider_default_range) {
            return Err(error("imgui(clamp) requires a slider or drag with a range"));
        }
        if matches!(widget, Self::Input) && attrs.has_flags() {
            return Err(error("imgui numeric widget flags require a slider or drag"));
        }
        if !matches!(widget, Self::Drag) && attrs.wrap_around {
            return Err(error("imgui(wrap_around) requires a drag widget"));
        }
        if attrs.clamp && attrs.wrap_around {
            return Err(error("imgui(clamp) cannot be combined with wrap_around"));
        }
        Ok(widget)
    }

    fn tokens(&self, attrs: &NumericAttrs) -> TokenStream {
        let widget = quote!(::dear_imgui_reflect::__private::NumericWidget);
        let range = match (&attrs.min, &attrs.max) {
            (Some(min), Some(max)) => option_tokens(Some(quote!((#min, #max)))),
            _ => option_tokens::<TokenStream>(None),
        };
        let clamp = attrs.clamp;
        match self {
            Self::Input => {
                let step = option_tokens(attrs.step.as_ref());
                let step_fast = option_tokens(attrs.step_fast.as_ref());
                quote!(#widget::Input { step: #step, step_fast: #step_fast })
            }
            Self::Slider => {
                let flags = widget_flags(attrs, quote!(::dear_imgui_reflect::imgui::SliderFlags));
                quote!(#widget::Slider { range: #range, flags: #flags, clamp: #clamp })
            }
            Self::Drag => {
                let flags = widget_flags(attrs, quote!(::dear_imgui_reflect::imgui::DragFlags));
                let speed = option_tokens(attrs.speed.as_ref());
                quote!(#widget::Drag { range: #range, speed: #speed, flags: #flags, clamp: #clamp })
            }
        }
    }
}

fn widget_flags(attrs: &NumericAttrs, kind: TokenStream) -> TokenStream {
    let flags = [
        (attrs.log, quote!(LOGARITHMIC)),
        (attrs.always_clamp, quote!(ALWAYS_CLAMP)),
        (attrs.wrap_around, quote!(WRAP_AROUND)),
        (attrs.no_round_to_format, quote!(NO_ROUND_TO_FORMAT)),
        (attrs.no_input, quote!(NO_INPUT)),
        (attrs.clamp_on_input, quote!(CLAMP_ON_INPUT)),
        (attrs.clamp_zero_range, quote!(CLAMP_ZERO_RANGE)),
        (attrs.no_speed_tweaks, quote!(NO_SPEED_TWEAKS)),
    ];
    let flags = flags
        .into_iter()
        .filter_map(|(enabled, flag)| enabled.then_some(flag));
    quote!(#kind::NONE #(| #kind::#flags)*)
}

fn effective_format(
    attrs: &NumericAttrs,
    ty: &Type,
    carrier: NumericFormatType,
) -> Result<Option<LitStr>> {
    let error = |message| Error::new_spanned(ty, message);
    let literal = if let Some(literal) = &attrs.format {
        if attrs.has_format_helpers() {
            return Err(Error::new(
                literal.span(),
                "imgui(format) cannot be combined with format helpers",
            ));
        }
        literal.clone()
    } else if attrs.has_format_helpers() {
        if [attrs.hex, attrs.percentage, attrs.scientific]
            .into_iter()
            .filter(|enabled| *enabled)
            .count()
            > 1
        {
            return Err(error(
                "imgui(hex, percentage, scientific) are mutually exclusive",
            ));
        }
        let base = match carrier {
            NumericFormatType::Unsigned32 if attrs.hex => "%#x",
            NumericFormatType::Unsigned64 if attrs.hex => "%#llx",
            _ if attrs.hex => {
                return Err(error("imgui(hex) requires an unsigned fixed-width integer"));
            }
            NumericFormatType::Float if attrs.percentage => "%.2f%%",
            NumericFormatType::Float if attrs.scientific => "%e",
            _ if attrs.percentage || attrs.scientific => {
                return Err(error("imgui(percentage, scientific) require a float"));
            }
            NumericFormatType::Signed32 => "%d",
            NumericFormatType::Unsigned32 => "%u",
            NumericFormatType::Signed64 => "%lld",
            NumericFormatType::Unsigned64 => "%llu",
            NumericFormatType::Float => "%.3f",
            NumericFormatType::PointerSized => {
                return Err(error(
                    "numeric formats for isize/usize are target-width dependent",
                ));
            }
        };
        let decorate = |text: &Option<LitStr>| {
            text.as_ref()
                .map(|literal| literal.value().replace('%', "%%"))
                .unwrap_or_default()
        };
        let formatted = format!(
            "{}{base}{}",
            decorate(&attrs.prefix),
            decorate(&attrs.suffix)
        );
        LitStr::new(&formatted, syn::spanned::Spanned::span(ty))
    } else {
        return Ok(None);
    };

    let validated = validate_and_normalize(&literal.value(), carrier)
        .map_err(|message| Error::new(literal.span(), message))?;
    Ok(Some(LitStr::new(&validated, literal.span())))
}

pub fn render_numeric(
    attrs: &NumericAttrs,
    ty: &Type,
    label: &LitStr,
    value: &TokenStream,
) -> Result<TokenStream> {
    let (primitive, carrier) = numeric_type(ty).ok_or_else(|| {
        Error::new_spanned(
            ty,
            "imgui numeric attributes require a primitive numeric field",
        )
    })?;
    let widget = Widget::select(attrs, ty)?.tokens(attrs);
    let format = effective_format(attrs, ty, carrier)?.map(|literal| {
        quote_spanned! {Span::mixed_site()=> {
            static __DEAR_IMGUI_REFLECT_FORMAT: ::std::sync::LazyLock<
                ::dear_imgui_reflect::imgui::NumericFormat<'static, ::core::primitive::#primitive>,
            > = ::std::sync::LazyLock::new(|| {
                ::dear_imgui_reflect::imgui::NumericFormat::new(#literal)
                    .expect("ImGuiReflect derive validated this numeric format")
            });
            __DEAR_IMGUI_REFLECT_FORMAT.borrowed()
        }}
    });
    let format = option_tokens(format);
    Ok(quote_spanned! {Span::mixed_site()=> {
        let __dear_imgui_reflect_widget = #widget;
        ::dear_imgui_reflect::__private::render_numeric(
            __dear_imgui_reflect_inspector,
            #label,
            #value,
            __dear_imgui_reflect_widget,
            #format,
        )
    }})
}
