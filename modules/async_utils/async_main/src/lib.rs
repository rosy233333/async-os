use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_attribute]
pub fn async_main(_args: TokenStream, item: TokenStream) -> TokenStream {
    let f = syn::parse_macro_input!(item as syn::ItemFn);
    let block = f.block;
    quote! {
        extern crate alloc;

        use core::{future::Future, pin::Pin};
        use alloc::boxed::Box;
        #[used]
        #[no_mangle]
        static ASYNC_MAIN: fn() -> BoxFut = keep_name;
        type BoxFut = Pin<Box<dyn Future<Output = isize> + 'static>>;

        fn keep_name() -> BoxFut {
            Box::pin(async { #block })
        }
    }
    .into()
}
