use proc_macro::TokenStream;
use std::ops::Deref;

use proc_macro2::{Ident, Span};
use quote::quote;
use syn::{FnArg, ItemFn, Pat, Signature};

/// Register vm's ABI for handling wasm callbacks.
/// format description: `#[vm_handle(wasmy_abi::Method)]`
/// example:
/// ```
/// #[vm_handle(123)]
/// fn xxx<A: wasmy_abi::Message, R: wasmy_abi::Message>(args: A) -> wasmy_abi::Result<R> {todo!()}
/// ```
/// or with context
/// ```
/// #[vm_handle(123)]
/// fn yyy<C: wasmy_abi::Message, A: wasmy_abi::Message, R: wasmy_abi::Message>(ctx: Option<&C>, args: A) -> wasmy_abi::Result<R> {todo!()}
/// ```
/// command to check expanded code: `cargo +nightly rustc -- -Zunstable-options --pretty=expanded`
#[proc_macro_attribute]
#[cfg(not(test))] // Work around for rust-lang/rust#62127
pub fn vm_handle(args: TokenStream, item: TokenStream) -> TokenStream {
    let method = args.to_string().parse::<i32>().expect("expect #[vm_handle(wasmy_abi::VmMessage)]");
    if method < 0 {
        panic!("vm_handle: VmMessage({})<0", method);
    }
    let raw_item = proc_macro2::TokenStream::from(item.clone());
    let raw_sig = syn::parse_macro_input!(item as ItemFn).sig;
    let has_ctx = raw_sig.inputs.len() == 2;
    let raw_ident = raw_sig.ident;
    let new_ident = Ident::new(&format!("_vm_handle_{}", method), Span::call_site());

    let new_item = if has_ctx {
        quote! {
            #raw_item


            #[allow(redundant_semicolons)]
            fn #new_ident(ctx_ptr: usize, args: &::wasmy_vm::Any) -> ::wasmy_vm::Result<::wasmy_vm::Any> {
                #raw_ident(unsafe{::wasmy_vm::VmHandlerApi::try_as(ctx_ptr)}, ::wasmy_vm::VmHandlerApi::unpack_any(args)?).and_then(|res|::wasmy_vm::VmHandlerApi::pack_any(res))
            }
            ::wasmy_vm::submit_handler!{
               ::wasmy_vm::VmHandlerApi::new(#method, #new_ident)
            }
        }
    } else {
        quote! {
            #raw_item


            #[allow(redundant_semicolons)]
            fn #new_ident(_ctx_ptr: usize, args: &::wasmy_vm::Any) -> ::wasmy_vm::Result<::wasmy_vm::Any> {
                #raw_ident(::wasmy_vm::VmHandlerApi::unpack_any(args)?).and_then(|res|::wasmy_vm::VmHandlerApi::pack_any(res))
            }
            ::wasmy_vm::submit_handler!{
               ::wasmy_vm::VmHandlerApi::new(#method, #new_ident)
            }
        }
    };

    #[cfg(debug_assertions)] println!("{}", new_item);
    TokenStream::from(new_item)
}


/// Register wasm's ABI for handling requests.
/// format description: `#[wasm_handle(wasmy_abi::Method)]`
/// example:
/// ```
/// #[vm_handle(123)]
/// fn xxx<C: wasmy_abi::Message, A: wasmy_abi::Message, R: wasmy_abi::Message>(ctx: wasmy_abi::WasmCtx<C>, args: A) -> wasmy_abi::Result<R> {todo!()}
/// ```
/// command to check expanded code: `cargo +nightly rustc -- -Zunstable-options --pretty=expanded`
#[proc_macro_attribute]
#[cfg(not(test))] // Work around for rust-lang/rust#62127
pub fn wasm_handle(args: TokenStream, item: TokenStream) -> TokenStream {
    let method = args.to_string().parse::<i32>().expect("expect #[wasm_handle(wasmy_abi::WasmMessage)]");
    if method < 0 {
        panic!("wasm_handle: WasmMessage({})<0", method);
    }
    let mut new_item = item.clone();
    let raw_sig = syn::parse_macro_input!(item as ItemFn).sig;
    let (inner_ident, inner_item) = wasm_gen_inner(raw_sig);
    let outer_ident = Ident::new(&format!("_wasm_handle_{}", method), Span::call_site());
    let outer_item = quote! {
        #[allow(redundant_semicolons)]
        #[inline]
        #[no_mangle]
        pub extern "C" fn #outer_ident(ctx_size: i32, args_size: i32) {
            #inner_item;
            ::wasmy_abi::wasm_handle(ctx_size, args_size, #inner_ident)
        }
    };
    new_item.extend(TokenStream::from(outer_item));

    #[cfg(debug_assertions)] println!("{}", new_item);

    new_item
}

fn wasm_gen_inner(raw_sig: Signature) -> (Ident, proc_macro2::TokenStream) {
    let inner_ident = Ident::new("_inner", Span::call_site());
    let raw_ident = raw_sig.ident.clone();
    let raw_first_input = raw_sig.inputs.first().unwrap();
    let fn_args;
    if let FnArg::Typed(a) = raw_first_input {
        if let Pat::Ident(ident) = a.pat.deref() {
            fn_args = ident.ident.clone();
        } else {
            unreachable!()
        }
    } else {
        unreachable!()
    }
    (inner_ident.clone(), quote! {
        #[allow(unused_mut)]
        #[inline]
        fn #inner_ident(#raw_first_input, args: ::wasmy_abi::InArgs) -> ::wasmy_abi::Result<::wasmy_abi::Any> {
           ::wasmy_abi::pack_any(#raw_ident(#fn_args, args.get_args()?)?)
        }
    })
}

/// Register the ABI for wasm load-time initialization state.
/// Register wasm's ABI for handling requests.
/// format description: `#[wasm_onload]`
/// example:
/// ```
/// #[wasm_onload]
/// fn xxx() {}
/// ```
/// command to check expanded code: `cargo +nightly rustc -- -Zunstable-options --pretty=expanded`
#[proc_macro_attribute]
#[cfg(not(test))] // Work around for rust-lang/rust#62127
pub fn wasm_onload(_args: TokenStream, item: TokenStream) -> TokenStream {
    let raw_item = proc_macro2::TokenStream::from(item.clone());
    let raw_ident = syn::parse_macro_input!(item as syn::ItemFn).sig.ident;
    let new_ident = Ident::new("_wasm_onload", Span::call_site());
    let new_item = quote! {
        #[allow(redundant_semicolons)]
        #[inline]
        #[no_mangle]
        pub extern "C" fn #new_ident() {
            #raw_item;
            #raw_ident();
        }
    };
    #[cfg(debug_assertions)] println!("{}", new_item);
    TokenStream::from(new_item)
}
