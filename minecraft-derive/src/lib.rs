use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{
    parse_macro_input, AngleBracketedGenericArguments, Attribute, Data, DataEnum, DataStruct,
    DeriveInput, Expr, GenericArgument, Ident, Member, Path, PathArguments, PathSegment, Type,
    TypePath,
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

fn derive_minecraft_data_for_struct(name: Ident, data: DataStruct) -> TokenStream {
    let reader_id = format_ident!("reader");
    let writer_id = format_ident!("writer");
    let is_named = matches!(data.fields, syn::Fields::Named(_));
    let members = data.fields.members().collect::<Vec<_>>();
    let mut fields = Vec::new();
    for (f, ident) in data.fields.into_iter().zip(members.iter().cloned()) {
        let cond = if let Some(attr) = f
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("present_if"))
        {
            if optional_type(&f.ty).is_none() {
                return quote! {compile_error!("present_if is only valid on fields of type Option<T>");}.into();
            }
            if !is_named {
                return quote! {compile_error!{"present_if is only valid on structs with named fields"};}.into();
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
        let decode_lines = fields.iter().map(|MyField { ident, cond, ty }| {
            let rvalue = if let Some(cond) = cond {
                quote! {if #cond { Some(#decode_expr) } else { None }}
            } else {
                quote! {#decode_expr}
            };
            quote! {let #ident: #ty = #rvalue;}
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
    let num_bytes_lines = fields.iter().map(|MyField { ident, cond, .. }| match cond {
        Some(_) => quote! {
            if let Some(val) = &self.#ident {
                crate::datatypes::MinecraftData::num_bytes(val)
            } else {
                0
            }
        },
        None => quote! {crate::datatypes::MinecraftData::num_bytes(&self.#ident)},
    });
    let num_bytes_body = if members.len() == 0 {
        quote! {0}
    } else {
        quote! {#(#num_bytes_lines)+*}
    };
    quote!{
        impl crate::datatypes::MinecraftData for #name {
            fn decode<R: ::std::io::Read>(#reader_id: &mut R) -> ::std::result::Result<Self, crate::datatypes::Error> {
                #decode_body
            }

            fn encode<W: ::std::io::Write>(self, #writer_id: &mut W) -> ::std::result::Result<(), crate::datatypes::Error> {
                #(#encode_lines)*
                Ok(())
            }

            fn num_bytes(&self) -> usize {
                #num_bytes_body
            }
        }
    }.into()
}

fn derive_minecraft_data_for_enum(name: Ident, data: DataEnum) -> TokenStream {
    let reader_id = format_ident!("reader");
    let writer_id = format_ident!("writer");
    let mut idents = Vec::new();
    let mut reprs: Vec<Expr> = Vec::new();
    for v in data.variants.into_iter() {
        if !matches!(v.fields, syn::Fields::Unit) {
            return quote!(compile_error!(
                "Can only derive(MinecraftData) on unit-only enum"
            );)
            .into();
        }
        let ident = v.ident;
        // TODO: do something smarter here so that we don't have to specify it literally every time
        let repr = if let Some(attr) = v.attrs.iter().find(|attr| attr.path().is_ident("mc_repr")) {
            match attr.parse_args() {
                Ok(exp) => exp,
                Err(e) => return e.into_compile_error().into(),
            }
        } else {
            return quote!(compile_error!("Each variant needs a repr");).into();
        };

        idents.push(ident);
        reprs.push(repr);
    }

    quote!{
        impl crate::datatypes::MinecraftData for #name {
            fn decode<R: ::std::io::Read>(#reader_id: &mut R) -> ::std::result::Result<Self, crate::datatypes::Error> {
                match crate::datatypes::MinecraftData::decode(#reader_id)? {
                    #(#reprs => Ok(Self::#idents),)*
                    _ => Err(anyhow!("Invalid #name")),
                }
            }

            fn encode<W: ::std::io::Write>(self, #writer_id: &mut W) -> ::std::result::Result<(), crate::datatypes::Error> {
                match self {
                    #(Self::#idents => #reprs,)*
                }.encode(#writer_id)
            }

            fn num_bytes(&self) -> usize {
                match self {
                    #(Self::#idents => #reprs,)*
                }.num_bytes()
            }
        }
    }.into()
}

#[proc_macro_derive(MinecraftData, attributes(present_if, mc_repr))]
pub fn derive_minecraft_data(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match input.data {
        Data::Struct(data_struct) => derive_minecraft_data_for_struct(input.ident, data_struct),
        Data::Enum(data_enum) => derive_minecraft_data_for_enum(input.ident, data_enum),
        Data::Union(_) => {
            quote! {compile_error!{"derive(MinecraftData) does not work on unions"};}.into()
        }
    }
}
