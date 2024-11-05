use proc_macro2::Span;
use quote::__private::TokenStream;
use quote::{quote, ToTokens};
use regex::Regex;
use syn::TraitItem;
use syn::{FnArg, Ident, ItemTrait, ReturnType};

pub(crate) fn build_subtrait(super_trait: &ItemTrait) -> TokenStream {
    let supertrait_ident = &super_trait.ident;
    let subtrait_vis = &super_trait.vis;
    let subtrait_ident = quote::format_ident!("Async{}", supertrait_ident);
    // println!("{:?}", subtrait_vis);
    // println!("{:?}", subtrait_unsafety);
    // println!("{:?}", subtrait_ident);
    let supertrait_items = &super_trait
        .items
        .iter()
        .filter(|fn_items| {
            // fn_items
            match fn_items {
                TraitItem::Fn(trait_item_fn) => {
                    let sig = &trait_item_fn.sig;
                    let ret = &sig.output;
                    if ret.to_token_stream().to_string().contains("Poll") {
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            }
        })
        .collect::<Vec<&TraitItem>>();
    let mut subtrait_items = Vec::new();
    for trait_item in supertrait_items {
        match trait_item {
            TraitItem::Fn(trait_item_fn) => {
                let super_sig = &trait_item_fn.sig;
                // 生成函数 ident
                let super_fn_ident = &super_sig.ident;
                let super_fn_str = &super_fn_ident.to_string();
                let re = Regex::new(r"poll_").unwrap();
                let sub_fn_ident = Ident::new(&re.replace_all(super_fn_str, ""), Span::call_site());
                // println!("{:?}", sub_fn_ident);
                // 获取输入参数信息
                let super_inputs = &super_sig.inputs;
                let sub_receiver = super_inputs.first().unwrap();
                let mut self_mut = false;
                let sub_receiver = match sub_receiver {
                    FnArg::Receiver(receiver) => {
                        let poll_ty = receiver.ty.as_ref();
                        let self_type = match poll_ty {
                            syn::Type::Path(type_path) => {
                                // 修改路径中的类型参数
                                let args = &type_path.path.segments.last().unwrap().arguments;
                                if let syn::PathArguments::AngleBracketed(args) = args {
                                    // 例如，将T更改为u32
                                    let new_args = args.clone().args; // 这里需要根据需要修改args
                                                                      // println!("{:?}", new_args.last().unwrap());
                                    let new_args_string =
                                        new_args.last().unwrap().to_token_stream().to_string();
                                    if new_args_string.contains("mut") {
                                        self_mut = true;
                                    }
                                    new_args
                                } else {
                                    panic!("return type is not supported");
                                }
                            }
                            _ => panic!("return type is not supported"),
                        };
                        quote! {
                            #self_type
                        }
                    }
                    FnArg::Typed(_) => panic!("Only support self receiver"),
                };
                let sub_inputs = super_inputs
                    .iter()
                    .enumerate()
                    .filter(|(idx, _args)| *idx > 1)
                    .map(|(_idx, args)| args.clone())
                    .collect::<Vec<FnArg>>();
                let cx_input = super_inputs.get(1).unwrap();
                let cx_ident = match cx_input {
                    FnArg::Receiver(_receiver) => panic!("Not support self receiver"),
                    FnArg::Typed(pat_type) => match pat_type.pat.as_ref() {
                        syn::Pat::Ident(pat_ident) => pat_ident.ident.clone(),
                        _ => panic!("Not support other pattern"),
                    },
                };
                // for i in sub_inputs {
                //     println!("{:?}", i.to_token_stream().to_string());
                // }
                // 获取返回值信息
                let super_ret = &super_sig.output;
                let sub_ret = match super_ret {
                    ReturnType::Type(_, poll_ty) => {
                        let poll_ty = poll_ty.as_ref();
                        match poll_ty {
                            syn::Type::Path(type_path) => {
                                // 修改路径中的类型参数
                                let args = &type_path.path.segments.last().unwrap().arguments;
                                if let syn::PathArguments::AngleBracketed(args) = args {
                                    // 例如，将T更改为u32
                                    let new_args = args.clone().args; // 这里需要根据需要修改args
                                    new_args
                                } else {
                                    panic!("return type is not supported");
                                }
                            }
                            _ => panic!("return type is not supported"),
                        }
                    }
                    ReturnType::Default => {
                        panic!("return type is not supported");
                    }
                };
                let sub_inputs_ident = sub_inputs
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

                subtrait_items.push(
                    if self_mut {
                        quote! {
                            async fn #sub_fn_ident(self: #sub_receiver, #(#sub_inputs), *) -> #sub_ret {
                                let mut pinned = Pin::new(self);
                                core::future::poll_fn(|#cx_ident| pinned.as_mut().#super_fn_ident(#cx_ident, #(#sub_inputs_ident), *)).await
                            }
                        }
                    } else {
                        quote! {
                            async fn #sub_fn_ident(self: #sub_receiver, #(#sub_inputs), *) -> #sub_ret {
                                let mut pinned = Pin::new(self);
                                core::future::poll_fn(|#cx_ident| pinned.as_ref().#super_fn_ident(#cx_ident, #(#sub_inputs_ident), *)).await
                            }
                        }
                    }
                );
            }
            _ => panic!("Only support trait item function"),
        }
    }
    quote! {
        #subtrait_vis trait #subtrait_ident: #supertrait_ident + Unpin {
            #(#subtrait_items)*
        }

        impl<T: #supertrait_ident + Unpin + ?Sized> #subtrait_ident for T {}
    }
}
