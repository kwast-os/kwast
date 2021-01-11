extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro_error::*;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::{parenthesized, token, Ident, Token};
use syn::{parse_macro_input, Type};
use quote::ToTokens;

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
        let name = input.parse()?;
        let _colon_token = input.parse()?;
        let _paren_token = parenthesized!(content in input);
        let params = content.parse_terminated(AbiFunctionParam::parse)?;
        let _rarrow_token = input.parse()?;
        let return_type = input.parse()?;
        Ok(Self {
            name,
            _colon_token,
            _paren_token,
            params,
            _rarrow_token,
            return_type,
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
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.name;
        let ty = &self.ty;
        (quote! {
            #name: #ty
        })
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
        let mut translated_types = Vec::new();

        let errno_return = match &return_type {
            Type::Path(p) => {
                assert_eq!(p.path.segments.len(), 1);
                match p.path.segments[0].ident.to_string().as_str() {
                    "Errno" => true,
                    _ => {
                        emit_error!(return_type, "unexpected return type");
                        false
                    }
                }
            }
            Type::Tuple(t) => {
                if !t.elems.is_empty() {
                    emit_error!(t, "unexpected tuple");
                }
                false
            }
            _ => {
                emit_error!(
                    return_type,
                    "unexpected return type {:?}",
                    return_type.to_token_stream()
                );
                false
            }
        };

        let trait_return_type = if errno_return {
            quote! { WasmStatus }
        } else {
            quote! { () }
        };

        trait_funcs.push(quote! {
            #name(&self, #(#params),*) -> #trait_return_type
        });

        for param in &func.params {
            translated_types.push(match &param.ty {
                Type::Slice(_) | Type::Reference(_) | Type::Ptr(_) => {
                    emit_error!(param, "use the wasm pointer type");
                    quote! {} // Just here for the compiler
                }
                Type::Path(p) => {
                    assert_eq!(p.path.segments.len(), 1);
                    match p.path.segments[0].ident.to_string().as_str() {
                        "i64" | "u64" | "Rights" => quote! { types::I64 },
                        "u32" | "i32" | "Fd" | "ExitCode" | "WasmPtr" | "Size" | "LookupFlags" | "OFlags" | "FdFlags" => quote! { types::I32 },
                        "i16" | "u16" => quote! { types::I16 },
                        "i8" | "u8" => quote! { types::I8 },
                        _ => unimplemented!("{:?}", p.path.to_token_stream()),
                    }
                }
                _ => unimplemented!(),
            });
        }

        // Glue code generation
        let glue_name = format_ident!("__abi_{}", func.name);
        let member_func_name = &func.name;
        let param_names = func.params.iter().map(|p| &p.name);
        let params = func.params.iter();

        let function_body_call = quote! {
            vmctx.#member_func_name(#(#param_names),*)
        };

        let function_body = if errno_return {
            quote! {
                if let Err(e) = #function_body_call {
                    e
                } else {
                    Errno::Success
                }
            }
        } else {
            function_body_call
        };

        let sig_returns = if errno_return {
            quote! { AbiParam::new(types::I32) }
        } else {
            quote! {}
        };

        glue_functions.push(quote! {
            extern "C" fn #glue_name(vmctx: &VmContext, #(#params),*) -> #return_type {
                #function_body
            }
        });

        // Map code generation
        let key = syn::LitStr::new(
            &member_func_name.to_token_stream().to_string(),
            member_func_name.span(),
        );
        map_entries.push(quote! {
            map.insert(#key, (VirtAddr::new(#glue_name as usize), Signature {
                params: vec![AbiParam::special(WASM_VMCTX_TYPE, ArgumentPurpose::VMContext), #(AbiParam::new(#translated_types)),*],
                returns: vec![#sig_returns],
                call_conv: WASM_CALL_CONV,
            }));
        });
    }

    let result = quote! {
        trait AbiFunctions {
            #(fn #trait_funcs;)*
        }

        #(#glue_functions)*

        lazy_static! {
            static ref ABI_MAP: BTreeMap<&'static str, (VirtAddr, Signature)> = {
                let mut map = BTreeMap::new();
                #(#map_entries)*
                map
            };
        }
    };

    TokenStream::from(result)
}
