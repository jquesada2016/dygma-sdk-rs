mod impl_from_enum_for_u16;
mod impl_from_str_for_key_table_enums;
mod impl_from_u16_for_key_kind;
mod key_kind_enum;
mod key_table_enum;

use crate::{
    Ir,
    codegen::{
        impl_from_enum_for_u16::{ImplFromKeyKindEnumForU16, ImplFromKeyTableEnumForU16},
        impl_from_str_for_key_table_enums::ImplFromStrForKeyTableEnum,
        impl_from_u16_for_key_kind::ImplFromU16ForKeyKind,
        key_kind_enum::KeyKindEnum,
        key_table_enum::KeyTableEnum,
    },
};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

impl ToTokens for Ir {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let key_kind_enum = KeyKindEnum::from(self);
        let key_table_enums = self.0.iter().map(KeyTableEnum::from);
        let impl_from_u16_for_key_kind = ImplFromU16ForKeyKind::from(self);
        let impl_from_key_table_enum_for_u16s = self.0.iter().map(ImplFromKeyTableEnumForU16::from);
        let impl_from_key_kind_enum_for_u16 = ImplFromKeyKindEnumForU16::from(self);
        let impl_from_str_for_table_enums = self.0.iter().map(ImplFromStrForKeyTableEnum::from);

        let token_stream = quote! {
            paste::paste! {
                #key_kind_enum

                #impl_from_key_kind_enum_for_u16

                #impl_from_u16_for_key_kind

                #( #key_table_enums )*

                #( #impl_from_key_table_enum_for_u16s )*

                #( #impl_from_str_for_table_enums )*
            }
        };

        token_stream.to_tokens(tokens);
    }
}
