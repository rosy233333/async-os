use quote::__private::TokenStream;
use quote::{quote, ToTokens};
use syn::{FnArg, Ident, ItemTrait, Signature, TraitItem};

pub(crate) fn impl_wrapper(self_trait: &ItemTrait) -> TokenStream {
    let trait_ident = &self_trait.ident;
    let supertrait_items = &self_trait.items;

    let mut impl_pin_items = Vec::new();
    let mut impl_ref_items = Vec::new();

    let mut mutable = false;
    for trait_item in supertrait_items {
        match trait_item {
            TraitItem::Fn(trait_item_fn) => {
                let sig = &trait_item_fn.sig;
                let ident = &sig.ident;
                let inputs = &sig.inputs;
                let receiver = inputs.first().unwrap().to_token_stream().to_string();
                mutable = receiver.contains("mut");
                let get_as = if mutable {
                    quote! {get_mut().as_mut()}
                } else {
                    quote! {get_ref().as_ref()}
                };
                let actual_inputs = inputs
                    .iter()
                    .enumerate()
                    .filter(|(idx, _args)| *idx > 1)
                    .map(|(_idx, args)| args.clone())
                    .collect::<Vec<FnArg>>();
                let actual_inputs_ident = actual_inputs
                    .iter()
                    .map(|arg| {
                        let arg_ident = match arg {
                            FnArg::Receiver(_receiver) => panic!("Not support self receiver"),
                            FnArg::Typed(pat_type) => match pat_type.pat.as_ref() {
                                syn::Pat::Ident(pat_ident) => pat_ident.ident.clone(),
                                _ => panic!("Not support other pattern"),
                            },
                        };
                        arg_ident
                    })
                    .collect::<Vec<Ident>>();
                impl_pin_items.push(quote! {
                    #sig {
                        self.#get_as.#ident(cx, #(#actual_inputs_ident),*)
                    }
                });
                if mutable {
                    let mut new_inputs = sig.inputs.clone();
                    let self_arg = new_inputs.first_mut().unwrap();
                    match self_arg {
                        FnArg::Receiver(receiver) => {
                            receiver.mutability = Some(Default::default());
                        }
                        _ => panic!("First argument must be self receiver"),
                    }
                    let mut_sig = Signature {
                        constness: sig.constness.clone(),
                        asyncness: sig.asyncness.clone(),
                        unsafety: sig.unsafety.clone(),
                        abi: sig.abi.clone(),
                        fn_token: sig.fn_token.clone(),
                        ident: sig.ident.clone(),
                        generics: sig.generics.clone(),
                        paren_token: sig.paren_token.clone(),
                        inputs: new_inputs,
                        variadic: sig.variadic.clone(),
                        output: sig.output.clone(),
                    };
                    impl_ref_items.push(quote! {
                        #mut_sig {
                            Pin::new(&mut **self).#ident(cx, #(#actual_inputs_ident),*)
                        }
                    });
                } else {
                    impl_ref_items.push(quote! {
                        #sig {
                            Pin::new(&**self).#ident(cx, #(#actual_inputs_ident),*)
                        }
                    });
                }
            }
            _ => panic!("Only support trait item function"),
        }
    }

    let deref_ident = if mutable {
        quote! {DerefMut}
    } else {
        quote! {Deref}
    };

    let deref_import = if mutable {
        quote! {use core::ops::DerefMut;}
    } else {
        quote! {use core::ops::Deref;}
    };

    let impl_refs = if mutable {
        quote! {
            extern crate alloc;
            use alloc::boxed::Box;
            impl<T: ?Sized + #trait_ident + Unpin> #trait_ident for Box<T> {
                #(#impl_ref_items)*
            }

            impl<T: ?Sized + #trait_ident + Unpin> #trait_ident for &mut T {
                #(#impl_ref_items)*
            }
        }
    } else {
        quote! {
            extern crate alloc;
            use alloc::sync::Arc;
            impl<T: ?Sized + #trait_ident + Unpin> #trait_ident for Arc<T> {
                #(#impl_ref_items)*
            }

            impl<T: ?Sized + #trait_ident + Unpin> #trait_ident for &T {
                #(#impl_ref_items)*
            }
        }
    };

    quote! {
        #deref_import
        impl<P> #trait_ident for Pin<P>
        where
            P: #deref_ident + Unpin,
            P::Target: #trait_ident,
        {
            #(#impl_pin_items)*
        }

        #impl_refs

    }
    .into()
}
