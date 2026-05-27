use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, ToTokens};
use syn::{
    parse_macro_input, AngleBracketedGenericArguments, Data, DataEnum, DataStruct, DeriveInput,
    Expr, Fields, GenericArgument, Ident, Member, Path, PathArguments, PathSegment, Type, TypePath,
};

#[derive(Clone)]
struct MyField {
    ident: Ident,
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

fn derive_minecraft_data_for_fields(
    reader_id: &Ident,
    writer_id: &Ident,
    raw_fields: Fields,
    constructor: TokenStream2,
) -> Result<(TokenStream2, TokenStream2, TokenStream2, TokenStream2), TokenStream2> {
    let is_named = matches!(raw_fields, syn::Fields::Named(_));
    let members = raw_fields.members().collect::<Vec<_>>();
    let mut fields = Vec::new();
    let mut idents = Vec::new();
    for (f, member) in raw_fields.clone().into_iter().zip(members.iter().cloned()) {
        let cond = if let Some(attr) = f
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("present_if"))
        {
            if optional_type(&f.ty).is_none() {
                return Err(quote! {compile_error!("present_if is only valid on fields of type Option<T>");}.into());
            }
            if !is_named {
                return Err(quote! {compile_error!{"present_if is only valid on structs with named fields"};}.into());
            }
            match attr.parse_args() {
                Ok(exp) => Some(exp),
                Err(e) => return Err(e.into_compile_error().into()),
            }
        } else {
            None
        };
        let ident = match member {
            Member::Named(i) => i,
            Member::Unnamed(i) => format_ident!("__field{}", i),
        };
        idents.push(ident.clone());
        fields.push(MyField {
            ident,
            ty: f.ty,
            cond,
        });
    }

    let decode_expr = quote! {crate::datatypes::MinecraftData::decode(#reader_id)?};
    let decode_block = if is_named {
        let decode_lines = fields.iter().map(|MyField { ident, cond, ty }| {
            let rvalue = if let Some(cond) = cond {
                quote! {if #cond { Some(#decode_expr) } else { None }}
            } else {
                quote! {#decode_expr}
            };
            quote! {let #ident: #ty = #rvalue;}
        });
        quote! {
            {
                #(#decode_lines)*
                #constructor {
                    #(#members),*
                }
            }
        }
    } else {
        quote! {
            {
                #constructor {
                    #(#members: #decode_expr),*
                }
            }
        }
    };
    let match_arm = match &raw_fields {
        Fields::Unit => quote! {#constructor},
        Fields::Unnamed(_) => quote! {#constructor(#(#idents),*)},
        Fields::Named(_) => quote! {#constructor{#(#idents),*}},
    };
    let encode_lines = fields.iter().map(|MyField { ident, cond, .. }| match cond {
        Some(_) => quote! {
            if let Some(val) = #ident {
                crate::datatypes::MinecraftData::encode(val, #writer_id)?;
            }
        },
        None => quote! {crate::datatypes::MinecraftData::encode(#ident, #writer_id)?;},
    });
    let num_bytes_lines = fields.iter().map(|MyField { ident, cond, .. }| match cond {
        Some(_) => quote! {
            if let Some(val) = #ident {
                crate::datatypes::MinecraftData::num_bytes(val)
            } else {
                0
            }
        },
        None => quote! {crate::datatypes::MinecraftData::num_bytes(#ident)},
    });
    let num_bytes_expr = if members.len() == 0 {
        quote! {0}
    } else {
        quote! {(#(#num_bytes_lines)+*)}
    };
    return Ok((
        match_arm,
        decode_block,
        quote! {#(#encode_lines)*},
        num_bytes_expr,
    ));
}

fn derive_minecraft_data_for_struct(name: Ident, data: DataStruct) -> TokenStream {
    let reader_id = format_ident!("reader");
    let writer_id = format_ident!("writer");

    match derive_minecraft_data_for_fields(&reader_id, &writer_id, data.fields, quote! {Self}) {
        Err(msg) => return msg.into(),
        Ok((match_arm, decode_block, encode_expr, num_bytes_expr)) => {
            let decode_body = quote! {Ok(#decode_block)};
            quote!{
                impl crate::datatypes::MinecraftData for #name {
                    fn decode<R: ::std::io::Read>(#reader_id: &mut R) -> ::std::result::Result<Self, crate::datatypes::Error> {
                        #decode_body
                    }

                    fn encode<W: ::std::io::Write>(self, #writer_id: &mut W) -> ::std::result::Result<(), crate::datatypes::Error> {
                        match self {
                            #match_arm => { #encode_expr }
                        }
                        Ok(())
                    }

                    fn num_bytes(&self) -> usize {
                        match self {
                            #match_arm => #num_bytes_expr
                        }
                    }
                }
            }.into()
        }
    }
}

fn derive_minecraft_data_for_enum(name: Ident, data: DataEnum) -> TokenStream {
    let reader_id = format_ident!("reader");
    let writer_id = format_ident!("writer");
    let mut idents = Vec::new();
    let mut reprs: Vec<Expr> = Vec::new();
    let mut match_arms = Vec::new();
    let mut decode_blocks = Vec::new();
    let mut encode_exprs = Vec::new();
    let mut num_bytes_exprs = Vec::new();
    for v in data.variants.into_iter() {
        // TODO: delete
        // if !matches!(v.fields, syn::Fields::Unit) {
        //     return quote!(compile_error!(
        //         "Can only derive(MinecraftData) on unit-only enum"
        //     );)
        //     .into();
        // }
        let ident = v.ident;
        let repr = if let Some(attr) = v.attrs.iter().find(|attr| attr.path().is_ident("mc_repr")) {
            match attr.parse_args() {
                Ok(exp) => exp,
                Err(e) => return e.into_compile_error().into(),
            }
        } else {
            return quote!(compile_error!("Each variant needs a repr");).into();
        };

        match derive_minecraft_data_for_fields(
            &reader_id,
            &writer_id,
            v.fields,
            quote! {Self::#ident},
        ) {
            Err(msg) => return msg.into(),
            Ok((match_arm, decode_block, encode_expr, num_bytes_expr)) => {
                match_arms.push(match_arm);
                decode_blocks.push(decode_block);
                encode_exprs.push(encode_expr);
                num_bytes_exprs.push(num_bytes_expr);
            }
        }

        idents.push(ident);
        reprs.push(repr);
    }

    quote!{
        impl crate::datatypes::MinecraftData for #name {
            fn decode<R: ::std::io::Read>(#reader_id: &mut R) -> ::std::result::Result<Self, crate::datatypes::Error> {
                match crate::datatypes::MinecraftData::decode(#reader_id)? {
                    #(#reprs => Ok(#decode_blocks),)*
                    _ => Err(anyhow!("Invalid #name")),
                }
            }

            fn encode<W: ::std::io::Write>(self, #writer_id: &mut W) -> ::std::result::Result<(), crate::datatypes::Error> {
                match self {
                    #(#match_arms => { crate::datatypes::MinecraftData::encode(#reprs, #writer_id)?; #encode_exprs })*
                };
                Ok(())
            }

            fn num_bytes(&self) -> usize {
                match self {
                    #(#match_arms => crate::datatypes::MinecraftData::num_bytes(&#reprs) + #num_bytes_exprs,)*
                }
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
