use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{
    parse_macro_input, AngleBracketedGenericArguments, Data, DataStruct, DeriveInput, Expr,
    GenericArgument, Member, Path, PathArguments, PathSegment, Type, TypePath,
};

#[derive(Clone)]
struct MyField {
    ident: Member,
    ty: Type,
    cond: Option<Expr>,
}

impl ToTokens for MyField {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        self.ident.to_tokens(tokens);
        self.ty.to_tokens(tokens);
        self.cond.to_tokens(tokens);
    }
}

fn optional_type(ty: &Type) -> Option<&Type> {
    match ty {
        Type::Path(TypePath {
            qself: None,
            path: Path { segments, .. },
        }) => {
            if segments.len() != 1 {
                return None;
            }
            match &segments[0] {
                PathSegment {
                    ident,
                    arguments:
                        PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                            // args: [GenericArgument::Type(inner_ty)],
                            args,
                            ..
                        }),
                } if ident == "Option" => {
                    if args.len() != 1 {
                        return None;
                    }
                    match &args[0] {
                        GenericArgument::Type(inner_ty) => Some(inner_ty),
                        _ => None,
                    }
                }
                _ => None,
            }
        }

        _ => None,
    }
}

#[proc_macro_derive(MinecraftData, attributes(present_if))]
pub fn derive_minecraft_data(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let reader_id = format_ident!("reader");
    let writer_id = format_ident!("writer");
    let name = input.ident;
    let fields_raw = match input.data {
        Data::Struct(DataStruct { fields, .. }) => fields,
        _ => return quote! {compile_error!("derive(MinecraftData) only works on structs");}.into(),
    };
    let is_named = matches!(fields_raw, syn::Fields::Named(_));
    let members = fields_raw.members().collect::<Vec<_>>();
    let mut fields = Vec::new();
    for (f, ident) in fields_raw.into_iter().zip(members.iter().cloned()) {
        let cond = if let Some(attr) = f
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("present_if"))
        {
            if optional_type(&f.ty).is_none() {
                return quote! {compile_error!("present_if is only valid on fields of type Option<T>")}.into();
            }
            if !is_named {
                return quote! {compile_error!{"present_if is only valid on structs with named fields"}}.into();
            }
            match attr.parse_args() {
                Ok(exp) => Some(exp),
                Err(e) => return e.into_compile_error().into(),
            }
        } else {
            None
        };
        fields.push(MyField {
            ident,
            ty: f.ty,
            cond,
        });
    }

    let decode_expr = quote! {crate::datatypes::MinecraftData::decode(#reader_id)?};
    let decode_body = if is_named {
        let decode_lines = fields.iter().map(|MyField { ident, cond, .. }| {
            let rvalue = if let Some(cond) = cond {
                quote! {if #cond { Some(#decode_expr) } else { None }}
            } else {
                quote! {#decode_expr}
            };
            quote! {let #ident = #rvalue;}
        });
        quote! {
            #(#decode_lines)*
            Ok(Self {
                #(#members),*
            })
        }
    } else {
        quote! {
            Ok(Self {
                #(#members: #decode_expr),*

            })
        }
    };
    let encode_lines = fields.iter().map(|MyField { ident, cond, .. }| match cond {
        Some(_) => quote! {
            if let Some(val) = self.#ident {
                crate::datatypes::MinecraftData::encode(val, #writer_id)?;
            }
        },
        None => quote! {crate::datatypes::MinecraftData::encode(self.#ident, #writer_id)?;},
    });
    let len_lines = fields.iter().map(|MyField { ident, cond, .. }| match cond {
        Some(_) => quote! {
            if let Some(val) = &self.#ident {
                crate::datatypes::MinecraftData::len(val)
            } else {
                0
            }
        },
        None => quote! {crate::datatypes::MinecraftData::len(&self.#ident)},
    });
    let len_body = if members.len() == 0 {
        quote! {0}
    } else {
        quote! {#(#len_lines)+*}
    };
    quote!{
        impl crate::datatypes::MinecraftData for #name {
            fn decode<R: ::std::io::Read>(reader: &mut R) -> ::std::result::Result<Self, crate::datatypes::Error> {
                #decode_body
            }

            fn encode<W: ::std::io::Write>(self, writer: &mut W) -> ::std::result::Result<(), crate::datatypes::Error> {
                #(#encode_lines)*
                Ok(())
            }

            fn len(&self) -> usize {
                #len_body
            }
        }
    }.into()
}
