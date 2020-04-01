extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro_error::*;
use quote::{format_ident, quote};
use syn::export::{ToTokens, TokenStream2};
use syn::parse::{Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::{parenthesized, token, Ident, Token};
use syn::{parse_macro_input, Type};

struct AbiFunctions {
    functions: Punctuated<AbiFunction, Token![,]>,
}

struct AbiFunction {
    name: Ident,
    _colon_token: Token![:],
    _paren_token: token::Paren,
    params: Punctuated<AbiFunctionParam, Token![,]>,
    _rarrow_token: token::RArrow,
    return_type: Type,
}

struct AbiFunctionParam {
    name: Ident,
    _colon_token: Token![:],
    ty: Type,
}

impl Parse for AbiFunctions {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            functions: input.parse_terminated(AbiFunction::parse)?,
        })
    }
}

impl Parse for AbiFunction {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        Ok(Self {
            name: input.parse()?,
            _colon_token: input.parse()?,
            _paren_token: parenthesized!(content in input),
            params: content.parse_terminated(AbiFunctionParam::parse)?,
            _rarrow_token: input.parse()?,
            return_type: input.parse()?,
        })
    }
}

impl Parse for AbiFunctionParam {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            name: input.parse()?,
            _colon_token: input.parse()?,
            ty: input.parse()?,
        })
    }
}

impl ToTokens for AbiFunctionParam {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let name = &self.name;
        let ty = &self.ty;
        let _ = quote! {
            #name: #ty
        }
        .to_tokens(tokens);
    }
}

#[proc_macro]
#[proc_macro_error]
pub fn abi_functions(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as AbiFunctions);

    let mut trait_funcs = Vec::new();
    let mut glue_functions = Vec::new();
    let mut map_entries = Vec::new();

    for func in input.functions {
        let name = func.name.to_token_stream();
        let return_type = func.return_type;
        let params = func.params.iter();

        trait_funcs.push(quote! {
            #name(&self, #(#params),*) -> WasmStatus
        });

        for param in &func.params {
            match &param.ty {
                Type::Reference(_) | Type::Ptr(_) => {
                    emit_error!(param, "use the wasm pointer type");
                }
                _ => {}
            };
        }

        // Glue code generation
        let glue_name = format_ident!("__abi_{}", func.name);
        let member_func_name = &func.name;
        let param_names = func.params.iter().map(|p| &p.name);
        let params = func.params.iter();
        glue_functions.push(quote! {
            extern "C" fn #glue_name(vmctx: &VmContext, #(#params),*) -> #return_type {
                if let Err(e) = vmctx.#member_func_name(#(#param_names),*) {
                    e
                } else {
                    Errno::Success
                }
            }
        });

        // Map code generation
        let key = syn::LitStr::new(
            &member_func_name.to_token_stream().to_string(),
            member_func_name.span(),
        );
        map_entries.push(quote! {
            map.insert(#key, #glue_name as usize);
        });
    }

    let map_capacity = map_entries.len();

    let result = quote! {
        trait AbiFunctions {
            #(fn #trait_funcs;)*
        }

        #(#glue_functions)*

        lazy_static! {
            static ref ABI_MAP: HashMap<&'static str, usize> = {
                let mut map = HashMap::with_capacity(#map_capacity);
                #(#map_entries)*
                map
            };
        }
    };

    TokenStream::from(result)
}
