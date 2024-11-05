use proc_macro::TokenStream;
use quote::quote;
mod subtrait;
use subtrait::build_subtrait;
mod imp;
use imp::impl_wrapper;

#[proc_macro_attribute]
pub fn async_trait(_args: TokenStream, item: TokenStream) -> TokenStream {
    let trait_item = syn::parse_macro_input!(item as syn::ItemTrait);
    let sub_trait = build_subtrait(&trait_item);
    let impl_wrapper = impl_wrapper(&trait_item);

    quote! {
        #trait_item
        #sub_trait
        #impl_wrapper
    }
    .into()
}
