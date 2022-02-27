use proc_macro::TokenStream;

use proc_macro2::{Ident, Span};
use quote::quote;

/// Entry pointer of function, take function handler as argument.
///
/// `target fn type: fn<A: wasmy_abi::Message, R: wasmy_abi::Message>(A) -> Result<R>`
/// command to check expanded code: `cargo +nightly rustc -- -Zunstable-options --pretty=expanded`
#[proc_macro_attribute]
#[cfg(not(test))] // Work around for rust-lang/rust#62127
pub fn vm_handler(args: TokenStream, item: TokenStream) -> TokenStream {
    let raw_item = proc_macro2::TokenStream::from(item.clone());
    let raw_ident = syn::parse_macro_input!(item as syn::ItemFn).sig.ident;
    let method = args.to_string().parse::<i32>().expect("expect #[vm_handler(i32)]");
    if method < 0 {
        panic!("vm_handler: method({})<0", method);
    }
    let new_ident = Ident::new(&format!("_vm_handler_{}", method), Span::call_site());
    let new_item = quote! {
        #raw_item


        #[allow(redundant_semicolons)]
        fn #new_ident(args: &Any) -> Result<Any> {
            #raw_ident(VmHandlerAPI::unpack_any(args)?).and_then(|res|VmHandlerAPI::pack_any(res))
        }
        submit_handler!{
           VmHandlerAPI::new(#method, #new_ident)
        }
    };
    #[cfg(debug_assertions)] println!("{}", new_item);
    TokenStream::from(new_item)
}


/// Entry pointer of function, take function handler as argument.
///
/// `target fn type: fn<A: wasmy_abi::Message, R: wasmy_abi::Message>(wasmy_abi::Ctx, A) -> Result<R>`
/// command to check expanded code: `cargo +nightly rustc -- -Zunstable-options --pretty=expanded`
#[proc_macro_attribute]
#[cfg(not(test))] // Work around for rust-lang/rust#62127
pub fn wasm_handler(args: TokenStream, item: TokenStream) -> TokenStream {
    let mut new_item = item.clone();
    let raw_ident = syn::parse_macro_input!(item as syn::ItemFn).sig.ident;
    let method = args.to_string().parse::<i32>().expect("expect #[wasm_handler(i32)]");
    if method < 0 {
        panic!("wasm_handler: method({})<0", method);
    }
    let inner_ident = Ident::new("_inner", Span::call_site());
    let inner_item = wasm_gen_inner(inner_ident.clone(), raw_ident);
    let outer_ident = Ident::new(&format!("_wasm_handler_{}", method), Span::call_site());
    let outer_item = quote! {
        #[allow(redundant_semicolons)]
        #[inline]
        #[no_mangle]
        pub extern "C" fn #outer_ident(ctx_id: i32, size: i32) {
            #inner_item;
            ::wasmy_abi::wasm_main(ctx_id, size, #inner_ident)
        }
    };
    new_item.extend(TokenStream::from(outer_item));

    #[cfg(debug_assertions)] println!("{}", new_item);

    new_item
}

fn wasm_gen_inner(inner_ident: Ident, raw_ident: Ident) -> proc_macro2::TokenStream {
    quote! {
        #[inline]
        fn #inner_ident(ctx: Ctx, args: InArgs) -> Result<Any> {
           pack_any(#raw_ident(ctx, args.get_args()?)?)
        }
    }
}
