use darling::{FromField, FromMeta};
use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Data, DeriveInput, Error, GenericArgument, PathArguments, Type, TypePath, parse_macro_input,
    parse_quote,
};

#[derive(FromMeta)]
enum OuDiaType {
    /// [key => Val]
    /// return Val
    SinglePairSingleEntry(String),
    /// [key => Val1, Val2, Val3, ...]
    /// return Vec<Val>
    SinglePairManyEntries(String),
    /// [Struct::key => Struct]
    /// return Struct
    SingleStruct(String),
    /// [Struct::key => Struct, Struct::key => Struct, ...]
    /// return Vec<Struct>
    ManyStructs(String),
    /// [key => [Struct::key => Struct, Struct::key => Struct, ...]]
    /// return Vec<Struct>
    SingleStructManyEntries(String),
    /// [key1 => [Struct::key => Struct, ...], key2 => [Struct::key => Struct, ...]]
    /// return Vec<Struct>
    TwinStructMultipleEntries { first: String, second: String },
}

#[derive(FromMeta)]
#[darling(derive_syn_parse)]
struct StructArgs {
    key: Option<String>,
    alias: Option<String>,
}

#[derive(FromField)]
#[darling(attributes(oudia))]
struct FieldOpts {
    #[darling(rename = "type")]
    kind: OuDiaType,
    alias: Option<String>,
    parse_fn: Option<syn::Expr>,
    silence_fn: Option<syn::Expr>,
    serialize_fn: Option<syn::Expr>,
}

enum OuterType<'a> {
    Option(&'a Type),
    Vec(&'a Type),
    Plain(&'a Type),
}

impl<'a> OuterType<'a> {
    fn into_inner(&self) -> &Type {
        match *self {
            Self::Option(ty) => ty,
            Self::Vec(ty) => ty,
            Self::Plain(ty) => ty,
        }
    }
}

fn inspect_type(ty: &Type) -> OuterType<'_> {
    if let Type::Path(TypePath {
        qself: None, path, ..
    }) = ty
        && let Some(segment) = path.segments.last()
        && let PathArguments::AngleBracketed(args) = &segment.arguments
        && let Some(GenericArgument::Type(inner_ty)) = args.args.first()
    {
        if segment.ident == "Option" {
            return OuterType::Option(inner_ty);
        } else if segment.ident == "Vec" {
            return OuterType::Vec(inner_ty);
        }
    }

    OuterType::Plain(ty)
}

fn doc_aliases(names: &[String]) -> Vec<syn::Attribute> {
    if names.is_empty() {
        return Vec::new();
    }

    let first = &names[0];
    let rest = &names[1..];
    let doc_desc = quote! {
        concat!{
            "\n\nAlso known as `",
            #first,
            #( "`, `", #rest, )*
            "`."
        }
    };

    let mut attrs = vec![parse_quote!(#[doc = #doc_desc])];
    for name in names {
        attrs.push(parse_quote!(#[doc(alias = #name)]));
    }
    attrs
}

#[proc_macro_attribute]
pub fn oudia(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as StructArgs);
    let mut input = parse_macro_input!(input as DeriveInput);

    let struct_ident = input.ident.clone();
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let struct_oudia_key = args.key.clone().unwrap_or_else(|| struct_ident.to_string());

    let mut struct_names = Vec::new();
    if let Some(key) = &args.key {
        if !key.is_empty() {
            struct_names.push(key.clone());
        }
    }
    if let Some(alias) = &args.alias {
        struct_names.push(alias.clone());
    }
    input.attrs.extend(doc_aliases(&struct_names));

    let mut initializers = proc_macro2::TokenStream::new();
    let mut matchers = proc_macro2::TokenStream::new();
    let mut silencers = proc_macro2::TokenStream::new();
    let mut assembler = proc_macro2::TokenStream::new();
    let mut fixups = proc_macro2::TokenStream::new();
    let mut serializers = proc_macro2::TokenStream::new();

    {
        let Data::Struct(data_struct) = &mut input.data else {
            return Error::new_spanned(&input.ident, "You should apply `oudia` on a struct")
                .to_compile_error()
                .into();
        };

        for field in &mut data_struct.fields {
            let opts = match FieldOpts::from_field(&*field) {
                Ok(opts) => opts,
                Err(e) => {
                    return Error::new_spanned(
                        &*field,
                        format!("failed to parse oudia attributes on field: {e}"),
                    )
                    .to_compile_error()
                    .into();
                }
            };

            let field_ident = field.ident.as_ref();
            let outer = inspect_type(&field.ty);
            let inner_ty = outer.into_inner();

            if let Some(parse_fn) = &opts.parse_fn {
                initializers.extend(quote! {
                    let #field_ident = (#parse_fn)(input)?;
                });
            } else {
                match (&outer, &opts.kind) {
                    (
                        OuterType::Option(_),
                        OuDiaType::SingleStruct(..) | OuDiaType::SinglePairSingleEntry(..),
                    ) => {
                        initializers.extend(quote! {
                            let mut #field_ident: Option<#inner_ty> = None;
                        });
                    }
                    (
                        OuterType::Vec(_),
                        OuDiaType::ManyStructs(..)
                        | OuDiaType::SinglePairManyEntries(..)
                        | OuDiaType::SingleStructManyEntries(..)
                        | OuDiaType::TwinStructMultipleEntries { .. },
                    ) => {
                        initializers.extend(quote! {
                            let mut #field_ident: Vec<#inner_ty> = Vec::new();
                        });
                    }
                    (
                        OuterType::Plain(_),
                        OuDiaType::SingleStruct(..) | OuDiaType::SinglePairSingleEntry(..),
                    ) => {
                        initializers.extend(quote! {
                            let mut #field_ident: Option<#inner_ty> = None;
                        });

                        let missing = match &opts.kind {
                            OuDiaType::SingleStruct(key) => quote! { #key },
                            OuDiaType::SinglePairSingleEntry(key) => quote! { #key },
                            _ => unreachable!(),
                        };

                        fixups.extend(quote! {
                            let Some(#field_ident) = #field_ident else {
                                return Err(crate::IrConversionError::MissingField {
                                    processing: std::any::type_name::<Self>(),
                                    missing: #missing,
                                });
                            };
                        });
                    }
                    _ => {
                        return Error::new_spanned(
                            &field.ty,
                            "OuDia type don't match with field type.",
                        )
                        .to_compile_error()
                        .into();
                    }
                }

                matchers.extend(match &opts.kind {
                    OuDiaType::SingleStruct(key) => quote! {
                        crate::ast::Structure::Struct(k, v) if k == #key => {
                            #field_ident = Some(<#inner_ty as crate::OuDiaIo>::from_structure(v)?);
                        }
                    },
                    OuDiaType::SinglePairSingleEntry(key) => quote! {
                        crate::ast::Structure::Pair(k, v) if k == #key && let Some(first) = v.first() => {
                            #field_ident = Some(first.parse::<#inner_ty>()?);
                        }
                    },
                    OuDiaType::ManyStructs(key) => quote! {
                        crate::ast::Structure::Struct(k, v) if k == #key => {
                            #field_ident.push(<#inner_ty as crate::OuDiaIo>::from_structure(v)?);
                        }
                    },
                    OuDiaType::SinglePairManyEntries(key) => quote! {
                        crate::ast::Structure::Pair(k, v) if k == #key => for val in v {
                            #field_ident.push(val.parse::<#inner_ty>()?);
                        }
                    },
                    OuDiaType::SingleStructManyEntries(key) => quote! {
                        crate::ast::Structure::Struct(k, v) if k == #key => for node in v {
                            match node {
                                crate::ast::Structure::Struct(k2, v2) if k2 == <#inner_ty as crate::OuDiaIo>::OUDIA_KEY => {
                                    #field_ident.push(<#inner_ty as crate::OuDiaIo>::from_structure(v2)?);
                                }
                                // Unknown field. Give error in log.
                                crate::ast::Structure::Pair(k2, _) | crate::ast::Structure::Struct(k2, _) => {
                                    log::warn!("Encountered unknown field `{k2}` when parsing `{}`", std::any::type_name::<Self>());
                                }
                            }
                        }
                    },
                    OuDiaType::TwinStructMultipleEntries { first, second } => quote! {
                        crate::ast::Structure::Struct(k, v) if k == #first || k == #second => for node in v {
                            match node {
                                crate::ast::Structure::Struct(k2, v2) if k2 == <#inner_ty as crate::OuDiaIo>::OUDIA_KEY => {
                                    #field_ident.push(<#inner_ty as crate::OuDiaIo>::from_structure(v2)?);
                                }
                                // Unknown field. Give error in log.
                                crate::ast::Structure::Pair(k2, _) | crate::ast::Structure::Struct(k2, _) => {
                                    log::warn!("Encountered unknown field `{k2}` when parsing `{}`", std::any::type_name::<Self>());
                                }
                            }
                        }
                    },
                });
            }

            if let Some(silence_fn) = &opts.silence_fn {
                silencers.extend(quote! {
                    crate::ast::Structure::Pair(k, _) | crate::ast::Structure::Struct(k, _) if (#silence_fn)(k)=> {}
                })
            }

            if let Some(serialize_fn) = &opts.serialize_fn {
                serializers.extend(quote! {
                    (#serialize_fn)(&mut __oudia_items);
                });
            } else {
                serializers.extend(match (&outer, &opts.kind) {
                    (OuterType::Option(_), OuDiaType::SinglePairSingleEntry(key)) => quote! {
                        if let Some(value) = &self.#field_ident {
                            __oudia_items.push(crate::pair!(#key => value.to_string()));
                        }
                    },
                    (OuterType::Plain(_), OuDiaType::SinglePairSingleEntry(key)) => quote! {
                        __oudia_items.push(crate::pair!(#key => self.#field_ident.to_string()));
                    },
                    (OuterType::Vec(_), OuDiaType::SinglePairManyEntries(key)) => quote! {
                        __oudia_items.push(crate::pair!(#key => .. self.#field_ident.iter().map(|value| value.to_string())));
                    },
                    (OuterType::Option(_), OuDiaType::SingleStruct(..)) => quote! {
                        if let Some(value) = &self.#field_ident {
                            __oudia_items.push(value.to_structure());
                        }
                    },
                    (OuterType::Plain(_), OuDiaType::SingleStruct(..)) => quote! {
                        __oudia_items.push(self.#field_ident.to_structure());
                    },
                    (OuterType::Vec(_), OuDiaType::ManyStructs(..)) => quote! {
                        __oudia_items.extend(self.#field_ident.iter().map(|value| value.to_structure()));
                    },
                    (OuterType::Vec(_), OuDiaType::SingleStructManyEntries(key)) => quote! {
                        __oudia_items.push(crate::structure!(#key => .. self.#field_ident.iter().map(|value| value.to_structure())));
                    },
                    // We don't have a generic way to split the values back into the two
                    // keys, so serialize everything under the first key for now.
                    (OuterType::Vec(_), OuDiaType::TwinStructMultipleEntries { first, .. }) => quote! {
                        __oudia_items.push(crate::structure!(#first => .. self.#field_ident.iter().map(|value| value.to_structure())));
                    },
                    _ => unreachable!(),
                });
            }

            assembler.extend(quote! {
                #field_ident,
            });

            field.attrs.retain(|attr| !attr.path().is_ident("oudia"));

            let mut field_names = Vec::new();
            if let Some(key) = match &opts.kind {
                OuDiaType::SinglePairSingleEntry(key)
                | OuDiaType::SinglePairManyEntries(key)
                | OuDiaType::SingleStruct(key)
                | OuDiaType::ManyStructs(key)
                | OuDiaType::SingleStructManyEntries(key) => Some(key.clone()),
                OuDiaType::TwinStructMultipleEntries { .. } => None,
            } {
                field_names.push(key);
            }
            if let Some(alias) = &opts.alias {
                field_names.push(alias.clone());
            }
            field.attrs.extend(doc_aliases(&field_names));
        }
    }

    let expanded = quote! {
        #input

        impl #impl_generics crate::OuDiaIo for #struct_ident #ty_generics #where_clause {
            const OUDIA_KEY: &'static str = #struct_oudia_key;

            fn from_structure(input: &[crate::ast::Structure<'_>]) -> Result<Self, crate::IrConversionError> {
                #initializers
                for node in input {
                    match node {
                        #matchers
                        #silencers
                        // Unknown field. Give error in log.
                        crate::ast::Structure::Pair(k, _) | crate::ast::Structure::Struct(k, _) => {
                            log::warn!("Encountered unknown field `{k}` when parsing `{}`", std::any::type_name::<Self>());
                        }
                    }
                }
                #fixups
                Ok(Self { #assembler })
            }

            fn to_structure(&self) -> crate::ast::Structure<'static> {
                let mut __oudia_items: Vec<crate::ast::Structure<'static>> = Vec::new();
                #serializers
                crate::ast::Structure::Struct(Self::OUDIA_KEY.into(), __oudia_items)
            }
        }
    };

    TokenStream::from(expanded)
}
