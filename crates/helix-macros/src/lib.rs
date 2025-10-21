use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::Parse, parse_macro_input, Ident, LitStr, Token};

struct HelixQueryDefinition {
    name: Ident,
    source: LitStr,
}

impl Parse for HelixQueryDefinition {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let keyword: Ident = input.parse()?;
        if keyword != "QUERY" {
            return Err(syn::Error::new(keyword.span(), "expected QUERY"));
        }
        let name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let source: LitStr = input.parse()?;
        Ok(Self { name, source })
    }
}

#[proc_macro]
pub fn helix_query(tokens: TokenStream) -> TokenStream {
    let def = parse_macro_input!(tokens as HelixQueryDefinition);
    let name = def.name;
    let source = def.source;
    TokenStream::from(quote! {
        ::helix_db::HelixQueryLiteral {
            name: stringify!(#name),
            source: #source,
        }
    })
}
