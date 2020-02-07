use syn::export::TokenStream2;

use crate::info_extractor::{ArgInfo, AttrSigInfo, BindgenArgType, SerializerType};
use quote::quote;

impl AttrSigInfo {
    /// Create struct representing input arguments.
    /// Each argument is getting converted to a field in a struct. Specifically argument:
    /// `ATTRIBUTES ref mut binding @ SUBPATTERN : TYPE` is getting converted to:
    /// `binding: SUBTYPE,` where `TYPE` is one of the following: `& SUBTYPE`, `&mut SUBTYPE`, `SUBTYPE`,
    /// and `SUBTYPE` is one of the following: `[T; n]`, path like
    /// `std::collections::HashMap<SUBTYPE, SUBTYPE>`, or tuple `(SUBTYPE0, SUBTYPE1, ...)`.
    /// # Example
    /// ```
    /// struct Input {
    ///   arg0: Vec<String>,
    ///   arg1: [u64; 10],
    ///   arg2: (u64, Vec<String>),
    /// }
    /// ```
    pub fn input_struct(&self) -> TokenStream2 {
        let args: Vec<_> = self.input_args().collect();
        assert!(
            !args.is_empty(),
            "Can only generate input struct for when input args are specified"
        );
        let attribute = match &self.input_serializer {
            SerializerType::JSON => quote! {#[derive(serde::Deserialize)]},
            SerializerType::Borsh => quote! {#[derive(borsh::BorshDeserialize)]},
        };
        let mut fields = TokenStream2::new();
        for arg in args {
            let ArgInfo { ty, ident, .. } = &arg;
            fields.extend(quote! {
                #ident: #ty,
            });
        }
        quote! {
            #attribute
            struct Input {
                #fields
            }
        }
    }

    /// Create pattern that decomposes input struct using correct mutability modifiers.
    /// # Example:
    /// ```
    /// Input {
    ///     arg0,
    ///     mut arg1,
    ///     arg2
    /// }
    /// ```
    pub fn decomposition_pattern(&self) -> TokenStream2 {
        let args: Vec<_> = self.input_args().collect();
        assert!(
            !args.is_empty(),
            "Can only generate decomposition pattern for when input args are specified."
        );
        let mut fields = TokenStream2::new();
        for arg in args {
            let ArgInfo { mutability, ident, .. } = &arg;
            fields.extend(quote! {
            #mutability #ident,
            });
        }
        quote! {
            Input {
                #fields
            }
        }
    }

    /// Create a sequence of arguments that can be used to call the method or the function
    /// of the smart contract.
    ///
    /// # Example:
    /// ```
    /// a, &b, &mut c,
    /// ```
    pub fn arg_list(&self) -> TokenStream2 {
        let mut result = TokenStream2::new();
        for arg in &self.args {
            let ArgInfo { reference, mutability, ident, .. } = &arg;
            result.extend(quote! {
                #reference #mutability #ident,
            });
        }
        result
    }

    /// Create code that deserializes arguments that were decorated with `#[callback]`
    pub fn callback_deserialization(&self) -> TokenStream2 {
        self
            .args
            .iter()
            .filter(|arg| match arg.bindgen_ty {
                BindgenArgType::CallbackArg => true,
                _ => false,
            })
            .enumerate()
            .fold(TokenStream2::new(), |acc, (idx, arg)| {
                let ArgInfo { mutability, ident, ty, .. } = arg;
                let read_data = quote! {
                let data: Vec<u8> = match near_bindgen::env::promise_result(#idx) {
                    near_bindgen::PromiseResult::Successful(x) => x,
                    _ => panic!("Callback computation {} was not successful", #idx)
                };
            };
                let invocation = match arg.serializer_ty {
                    SerializerType::JSON => quote! {
                    serde_json::from_slice(&data).expect("Failed to deserialize callback using JSON")
                },
                    SerializerType::Borsh => quote! {
                    borsh::Deserialize::try_from_slice(&data).expect("Failed to deserialize callback using JSON")
                },
                };
                quote! {
                #acc
                #read_data
                let #mutability #ident: #ty = #invocation;
            }
            })
    }

    /// Create code that deserializes arguments that were decorated with `#[callback_vec]`.
    pub fn callback_vec_deserialization(&self) -> TokenStream2 {
        self
            .args
            .iter()
            .filter(|arg| match arg.bindgen_ty {
                BindgenArgType::CallbackArgVec => true,
                _ => false,
            })
            .fold(TokenStream2::new(), |acc, arg| {
                let ArgInfo { mutability, ident, ty, .. } = arg;
                let invocation = match arg.serializer_ty {
                    SerializerType::JSON => quote! {
                    serde_json::from_slice(&data).expect("Failed to deserialize callback using JSON")
                },
                    SerializerType::Borsh => quote! {
                    borsh::Deserialize::try_from_slice(&data).expect("Failed to deserialize callback using JSON")
                },
                };
                quote! {
                #acc
                let #mutability #ident: #ty = (0..near_bindgen::env::promise_results_count())
                .map(|i| {
                    let data: Vec<u8> = match near_bindgen::env::promise_result(i) {
                        near_bindgen::PromiseResult::Successful(x) => x,
                        _ => panic!("Callback computation {} was not successful", i)
                    };
                    #invocation
                }).collect();
            }
            })
    }
}