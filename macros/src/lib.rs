#[macro_use]
extern crate derive_more;
#[macro_use]
extern crate proc_macro_error2;

mod ast;
mod codegen;
mod ir;

use crate::{ast::Ast, ir::Ir};
use quote::ToTokens;

struct KeyCodeOverflowsU16Error;

#[proc_macro_error]
#[proc_macro]
pub fn generate_keycode_tables(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = syn::parse_macro_input!(input as Ast);

    Ir::from(ast).to_token_stream().into()
}

fn lit_int_to_u16(lit: &syn::LitInt) -> u16 {
    if let Ok(value) = lit.base10_parse() {
        value
    } else {
        abort!(lit.span(), "code must be a valid u16");
    }
}
