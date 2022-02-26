use proc_macro::TokenStream;

use quote::quote;

/// Entry pointer of function, take function handler as argument.
///
/// `target fn type: fn(wasmy_abi::Ctx, wasmy_abi::InArgs) -> wasmy_abi::Result<wasmy_abi::Any>`
/// command to check expanded code: `cargo +nightly rustc -- -Zunstable-options --pretty=expanded`
#[proc_macro_attribute]
#[cfg(not(test))] // Work around for rust-lang/rust#62127
pub fn wasm_entry(_args: TokenStream, item: TokenStream) -> TokenStream {
    let mut handler_block = item.clone();
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let handler_ident = input.sig.ident;
    let expanded = quote! {
        #[no_mangle]
        pub extern "C" fn _wasm_main(ctx_id: i32, size: i32) {
            ::wasmy_abi::wasm_main(ctx_id, size, #handler_ident)
        }
    };
    handler_block.extend(TokenStream::from(expanded));

    #[cfg(debug_assertions)]
    println!("{}", handler_block);

    handler_block
}


/// Entry pointer of function, take function handler as argument.
///
/// `target fn type: fn<A: Message, R: Message>(A) -> Result<R>`
/// command to check expanded code: `cargo +nightly rustc -- -Zunstable-options --pretty=expanded`
#[proc_macro_attribute]
#[cfg(not(test))] // Work around for rust-lang/rust#62127
pub fn vm_handler(args: TokenStream, item: TokenStream) -> TokenStream {
    let inner = proc_macro2::TokenStream::from(item.clone());
    let handler_ident = syn::parse_macro_input!(item as syn::ItemFn).sig.ident;
    let method = args.to_string().parse::<i32>().expect("expect #[vm_handler(i32)]");
    if method < 0 {
        panic!("vm_handler: method({})<0", method);
    }
    let new_item = quote! {
        #[allow(redundant_semicolons)]
        fn #handler_ident(args: &Any) -> Result<Any> {
            #inner;
            #handler_ident(HandlerAPI::unpack_any(args)?).and_then(|res|HandlerAPI::pack_any(res))
        }
        submit_handler!{
           HandlerAPI::new(#method, #handler_ident)
        }
    };
    #[cfg(debug_assertions)] println!("{}", new_item);
    TokenStream::from(new_item)
}

