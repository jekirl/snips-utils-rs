extern crate proc_macro;

use proc_macro::TokenStream;

use syn;
use syn::Type;

use quote::quote;

#[proc_macro_derive(CReprOf, attributes(converted, nullable))]
pub fn creprof_derive(token_stream: TokenStream) -> TokenStream {
    let ast = syn::parse(token_stream).unwrap();
    impl_creprof_macro(&ast)
}

fn impl_creprof_macro(input: &syn::DeriveInput) -> TokenStream {
    let data = match &input.data {
        syn::Data::Struct(data) => data,
        _ => panic!("CReprOf can only be derived for structs"),
    };

    let struct_name = &input.ident;

    let target_type_attribute: &syn::Attribute = input
        .attrs
        .iter()
        .find(|attribute| {
            attribute.path.get_ident().map(|it| it.to_string()) == Some("target_type".into())
        })
        .expect("Can't derive CReprOf without target_type helper attribute.");

    let target_type: syn::Path = target_type_attribute.parse_args().unwrap();

    let fields: Vec<_> = data.fields.iter()
        .map(|field|
            (field.ident.as_ref().expect("field should have an ident"),
             match &field.ty {
                 Type::Ptr(ptr_t) => {
                     match &*ptr_t.elem {
                         Type::Path(path_t) => {
                             if let Some(_it) = path_t.path.segments.iter().find(|it| {
                                 it.ident.to_string().contains("c_char")
                             }) {
                                 // it's a pointer to str, return tuple of (composed_type, is_str_flag)
                                 (quote!(ffi_utils::RawPointerTo::< #path_t >), true)
                             } else {
                                 (quote!(ffi_utils::RawPointerTo::< #path_t >), false)
                             }
                         }
                         _ => panic!("")
                     }
                 }
                 Type::Path(path_t) => { (generic_path_to_concrete_type_path(&path_t.path), false) }
                 _ => { panic!("") }
             },
             &field.attrs))
        .map(|(field_name, (field_type, is_str), field_attrs)| {
            let nullable = field_attrs.iter().find(|attr| {
                attr.path.get_ident().map(|it| it.to_string()) == Some("nullable".into())
            });

            if let Some(_it) = nullable {
                if is_str {
                    quote!(
                        #field_name: if let Some(it) = input.#field_name {
                            convert_to_c_string_result!(it)?
                        } else {
                            std::ptr::null() as _
                        }
                    )
                } else {
                    quote!(
                        #field_name: if let Some(it) = input.#field_name {
                            #field_type::c_repr_of(it)?
                        } else {
                            std::ptr::null() as _
                        }
                    )
                }
            } else {
                if is_str {
                    quote!(#field_name: convert_to_c_string_result!(input.#field_name)?)
                } else {
                    quote!(#field_name: #field_type ::c_repr_of(input.#field_name)?)
                }
            }
        })
        .collect::<Vec<_>>();

    quote!(
        impl CReprOf<# target_type> for # struct_name {
            fn c_repr_of(input: # target_type) -> Result<Self, ffi_utils::Error> {
                Ok(Self {
                    # ( # fields, )*
                })
            }
        }
    ).into()
}

fn generic_path_to_concrete_type_path(path: &syn::Path) -> proc_macro2::TokenStream {
    let mut path = path.clone();
    let last_segment = path.segments.pop().unwrap();
    let segments = &path.segments;
    let ident = &last_segment.value().ident;
    let turbofished_type = if let syn::PathArguments::AngleBracketed(bracketed_args) =
    &last_segment.value().arguments
    {
        quote!(#ident::#bracketed_args)
    } else {
        quote!(#ident)
    };
    if segments.is_empty() {
        turbofished_type
    } else {
        quote!(#segments::#turbofished_type)
    }
}

#[proc_macro_derive(AsRust, attributes(converted, nullable))]
pub fn asrust_derive(token_stream: TokenStream) -> TokenStream {
    let ast = syn::parse(token_stream).unwrap();
    impl_asrust_macro(&ast)
}

fn impl_asrust_macro(input: &syn::DeriveInput) -> TokenStream {
    let struct_name = &input.ident;
    let converted_attribute: &syn::Attribute = input
        .attrs
        .iter()
        .find(|attribute| {
            attribute.path.get_ident().map(|it| it.to_string()) == Some("target_type".into())
        })
        .expect("Can't derive CReprOf without target_type helper attribute.");

    let target_type: syn::Path = converted_attribute.parse_args().unwrap();

    let data = match &input.data {
        syn::Data::Struct(data) => data,
        _ => panic!("CReprOf can only be derived for structs"),
    };

    let fields: Vec<_> = data
        .fields
        .iter()
        .map(|field|
            (
                field.ident.as_ref().expect("field should have an ident"),
                match &field.ty {
                    Type::Ptr(ptr_t) => {
                        match &*ptr_t.elem {
                            Type::Path(path_t) => {
                                if let Some(_it) = path_t.path.segments.iter().find(|it| {
                                    it.ident.to_string().contains("c_char")
                                }) {
                                    true
                                } else {
                                    false
                                }
                            }
                            _ => panic!("")
                        }
                    }
                    Type::Path(_path_t) => false,
                    _ => { panic!("") }
                },
                &field.attrs
            )
        )
        .map(|(field_name, is_str, field_attrs)| {
            let nullable = field_attrs.iter().find(|attr| {
                attr.path.get_ident().map(|it| it.to_string()) == Some("nullable".into())
            });

            if let Some(_it) = nullable {
                if is_str {
                    quote!(
                        #field_name: if self.#field_name != std::ptr::null() {
                            Some(create_rust_string_from!(self.#field_name)?)
                        } else {
                            None
                        }
                    )
                } else {
                    quote!(
                        #field_name: if self.#field_name != std::ptr::null() {
                            Some(self.#field_name.as_rust()?)
                        } else {
                            None
                        }
                    )
                }
            } else {
                if is_str {
                    quote!(#field_name : create_rust_string_from!(self.#field_name))
                } else {
                    quote!(#field_name : self.#field_name.as_rust()?)
                }
            }
        })
        .collect::<Vec<_>>();

    quote!(
        impl AsRust<#target_type> for #struct_name {
            fn as_rust(&self) -> Result<#target_type, ffi_utils::Error> {
                Ok(#target_type {
                    #(#fields, )*
                })
            }
        }
    ).into()
}
