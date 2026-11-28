use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{Attribute, DataStruct, Generics, Ident, Index, Result};

use crate::attrs::reject_struct_attrs;
use crate::field_codegen::FieldEditor;
use crate::impl_reflect;

pub fn derive_for_struct(
    ident: &Ident,
    mut generics: Generics,
    attrs: &[Attribute],
    data: DataStruct,
) -> Result<TokenStream> {
    reject_struct_attrs(attrs)?;
    let mut editors = Vec::new();
    for (index, field) in data.fields.iter().enumerate() {
        let editor = FieldEditor::parse(field, index, None)?;
        if editor.skip() {
            continue;
        }
        let member = field.ident.as_ref().map_or_else(
            || {
                let index = Index::from(index);
                quote!(#index)
            },
            |ident| quote!(#ident),
        );
        editors.push(editor.render(&mut generics, quote!(&mut self.#member))?);
    }
    let body = quote_spanned! {Span::mixed_site()=>
        let mut __dear_imgui_reflect_changed = false;
        if let ::core::option::Option::Some(__dear_imgui_reflect_node) =
            __dear_imgui_reflect_ui.tree_node(__dear_imgui_reflect_label)
        {
            #(#editors)*
        }
        __dear_imgui_reflect_changed
    };
    Ok(impl_reflect(ident, &generics, body))
}
