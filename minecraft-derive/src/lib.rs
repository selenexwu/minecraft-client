use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DataStruct, DeriveInput};

#[proc_macro_derive(MinecraftData)]
pub fn derive_minecraft_data(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let reader_id = format_ident!("reader");
    let writer_id = format_ident!("writer");
    let name = input.ident;
    let members = match input.data {
        Data::Struct(DataStruct { fields, .. }) => fields,
        _ => return quote! {compile_error!("derive(MinecraftData) only works on structs");}.into(),
    }
    .members()
    .collect::<Vec<_>>();

    let len_body = if members.len() == 0 {
        quote! {0}
    } else {
        quote! {#(crate::datatypes::MinecraftData::len(&self.#members))+*}
    };
    quote!{
        impl crate::datatypes::MinecraftData for #name {
            fn decode<R: ::std::io::Read>(reader: &mut R) -> ::std::result::Result<Self, crate::datatypes::Error> {
                Ok(Self {
                    #(#members: crate::datatypes::MinecraftData::decode(#reader_id)?),*
                })
            }

            fn encode<W: ::std::io::Write>(self, writer: &mut W) -> ::std::result::Result<(), crate::datatypes::Error> {
                #(crate::datatypes::MinecraftData::encode(self.#members, #writer_id)?;)*
                Ok(())
            }

            fn len(&self) -> usize {
                #len_body
            }
        }
    }.into()
}
