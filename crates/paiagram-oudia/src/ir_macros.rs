// SPDX-License-Identifier: MPL-2.0
//! Cursed territory. Don't edit it unless you know the super cow power.

macro_rules! make_ir_type {
    {
        $(#[$struct_attr:meta])*
        struct $ir_name:ident $(as [$first_ir_alias:expr $(, $rest_ir_alias:expr)*])?;
        $(
            $(#[$field_attr:meta])*
            $field_vis:vis $field_name:ident
                $(as [$first_field_alias:expr $(, $rest_field_alias:expr)*])?: $field_type:ty,
        )*
    } => {
        $(#[$struct_attr])*
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[derive(Clone, Debug, PartialEq)]
        #[doc = make_ir_type!(@doc_desc $($first_ir_alias, $($rest_ir_alias,)*)?)]
        $(
            #[doc(alias = $first_ir_alias)]
            $(#[doc(alias = $rest_ir_alias)])*
        )?
        pub struct $ir_name { $(
            $(#[$field_attr])*
            #[doc = make_ir_type!(@doc_desc $($first_field_alias, $($rest_field_alias,)*)?)]
            $(
                #[doc(alias = $first_field_alias)]
                $(#[doc(alias = $rest_field_alias)])*
            )?
            $field_vis $field_name: $field_type,
        )* }
        impl $ir_name { paste::paste! {
            make_ir_type!(@oud_name [<OUD_NAME>], $ir_name, $($first_ir_alias)?);
            $( make_ir_type!(@oud_name [<$field_name:upper _OUD_NAME>], $field_name, $($first_field_alias)?); )*
        } }
    };
    (@oud_name $const_name:ident, $struct_name:ident, $first_name:expr) => {
        #[allow(dead_code)]
        const $const_name: &'static str = $first_name;
    };
    (@oud_name $const_name:ident, $struct_name:ident,) => {
        #[allow(dead_code)]
        const $const_name: &'static str = stringify!($struct_name);
    };
    (@doc_desc) => {
        ""
    };
    (@doc_desc $first:expr, $($rest:expr,)*) => { concat!{
        "\n\nAlso known as `",
        $first,
        $(
            "`, `",
            $rest,
        )*
        "`."
    }};
}

macro_rules! make_ir_enum {
    {
        $(#[$enum_attr:meta])*
        enum $ir_name:ident $(as [$first_ir_alias:expr $(, $rest_ir_alias:expr)*])?;
        $(
            $(#[$variant_attr:meta])*
            $variant_name:ident
                $(as [$first_variant_alias:expr $(, $rest_variant_alias:expr)*])?,
        )*
    } => {
        $(#[$enum_attr])*
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[derive(Clone, Debug, PartialEq)]
        #[doc = make_ir_enum!(@doc_desc $($first_ir_alias, $($rest_ir_alias,)*)?)]
        $(
            #[doc(alias = $first_ir_alias)]
            $(#[doc(alias = $rest_ir_alias)])*
        )?
        pub enum $ir_name { $(
            $(#[$variant_attr])*
            #[doc = make_ir_enum!(@doc_desc $($first_variant_alias, $($rest_variant_alias,)*)?)]
            $(
                #[doc(alias = $first_variant_alias)]
                $(#[doc(alias = $rest_variant_alias)])*
            )?
            $variant_name,
        )* }
        impl $ir_name {
            #[allow(dead_code)]
            const fn oud_name(&self) -> &'static str{
                match self { $(
                    Self::$variant_name => make_ir_enum!(@oud_name $variant_name, $($first_variant_alias)?),
                )* }
            }
        }
    };
    (@oud_name $variant_name:ident, $first_name:expr) => {
        $first_name
    };
    (@oud_name $variant_name:ident,) => {
        stringify!($variant_name)
    };
    (@doc_desc) => {
        ""
    };
    (@doc_desc $first:expr, $($rest:expr,)*) => { concat!{
        "\n\nAlso known as `",
        $first,
        $(
            "`, `",
            $rest,
        )*
        "`."
    }};
}

pub(super) use make_ir_enum;
pub(super) use make_ir_type;
