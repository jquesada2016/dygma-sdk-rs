#[macro_use]
extern crate derive_more;
#[macro_use]
extern crate proc_macro_error2;

mod ast;
mod codegen;
mod parse;

use quote::ToTokens;

use crate::ast::Ast;

#[proc_macro_error]
#[proc_macro]
pub fn generate_keycode_tables(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let model = syn::parse_macro_input!(input as Ast);

    model.to_token_stream().into()
}
