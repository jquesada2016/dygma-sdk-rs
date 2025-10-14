mod impl_from_enum_for_u16;
mod impl_from_u16_for_key_kind;
mod key_kind_enum;
mod key_table_enum;

use crate::{
    Ir, KeyCodeOverflowsU16Error,
    codegen::{
        impl_from_enum_for_u16::{ImplFromKeyKindEnumForU16, ImplFromKeyTableEnumForU16},
        impl_from_u16_for_key_kind::ImplFromU16ForKeyKind,
        key_kind_enum::KeyKindEnum,
        key_table_enum::KeyTableEnum,
    },
    lit_int_to_u16,
};
use itertools::Itertools;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

impl ToTokens for Ir {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let key_kind_enum = KeyKindEnum::from(self);
        let key_table_enums = self.0.iter().map(KeyTableEnum::from);
        let impl_from_u16_for_key_kind = ImplFromU16ForKeyKind::from(self);
        let impl_from_key_table_enum_for_u16s = self.0.iter().map(ImplFromKeyTableEnumForU16::from);
        let impl_from_key_kind_enum_for_u16 = ImplFromKeyKindEnumForU16::from(self);

        let token_stream = quote! {
            paste::paste! {
                #key_kind_enum

                #impl_from_key_kind_enum_for_u16

                #impl_from_u16_for_key_kind

                #( #key_table_enums )*

                #( #impl_from_key_table_enum_for_u16s )*
            }
        };

        token_stream.to_tokens(tokens);
    }
}

#[derive(Clone, Copy, Display)]
enum Modifier {
    Ctrl,
    Alt,
    AltGr,
    Shift,
    Os,
}

impl std::ops::BitOr<Modifier> for u16 {
    type Output = u16;

    fn bitor(self, rhs: Modifier) -> Self::Output {
        rhs.as_modifier_value() | self
    }
}

impl Modifier {
    const fn as_modifier_value(self) -> u16 {
        match self {
            Self::Ctrl => 0x0100,
            Self::Alt => 0x0200,
            Self::AltGr => 0x0400,
            Self::Shift => 0x0800,
            Self::Os => 0x1000,
        }
    }

    fn powerset() -> Vec<Vec<Self>> {
        [Self::Ctrl, Self::Alt, Self::AltGr, Self::Os, Self::Shift]
            .into_iter()
            .powerset()
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Display)]
enum DualFunctionModifier {
    Ctrl,
    Alt,
    AltGr,
    Os,
    Shift,
    #[display("Layer 1")]
    Layer1,
    #[display("Layer 2")]
    Layer2,
    #[display("Layer 3")]
    Layer3,
    #[display("Layer 4")]
    Layer4,
    #[display("Layer 5")]
    Layer5,
    #[display("Layer 6")]
    Layer6,
    #[display("Layer 7")]
    Layer7,
    #[display("Layer 8")]
    Layer8,
}

impl DualFunctionModifier {
    const VALUES: &[Self] = &[
        Self::Ctrl,
        Self::Alt,
        Self::AltGr,
        Self::Os,
        Self::Shift,
        Self::Layer1,
        Self::Layer2,
        Self::Layer3,
        Self::Layer4,
        Self::Layer5,
        Self::Layer6,
        Self::Layer7,
        Self::Layer8,
    ];

    const fn as_modifier_value(self) -> u16 {
        match self {
            Self::Ctrl => 49169,
            Self::Alt => 49681,
            Self::AltGr => 50705,
            Self::Os => 49937,
            Self::Shift => 49425,
            Self::Layer1 => 51218,
            Self::Layer2 => 51474,
            Self::Layer3 => 51730,
            Self::Layer4 => 51986,
            Self::Layer5 => 52242,
            Self::Layer6 => 52498,
            Self::Layer7 => 52754,
            Self::Layer8 => 53010,
        }
    }
}
