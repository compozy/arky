//! Proc macros for the Arky AI agent SDK tool system.
//!
//! The `#[tool]` macro lives here and generates schema-aware tool adapters that
//! integrate plain async Rust functions with the `arky-tools` registry.

mod codegen;
mod parse;
mod schema;

use proc_macro::TokenStream;
use syn::{
    Error,
    ItemFn,
    parse_macro_input,
};

use crate::{
    codegen::expand_tool,
    parse::ParsedToolFn,
};

/// Expands an async function into a zero-sized tool type implementing `arky_tools::Tool`.
#[proc_macro_attribute]
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_tokens = proc_macro2::TokenStream::from(attr);
    let original_item = proc_macro2::TokenStream::from(item.clone());
    if !attr_tokens.is_empty() {
        return Error::new_spanned(
            attr_tokens,
            "#[tool] does not accept attribute arguments",
        )
        .to_compile_error()
        .into();
    }

    let item_fn = parse_macro_input!(item as ItemFn);
    match ParsedToolFn::parse(item_fn) {
        Ok(parsed) => expand_tool(&parsed).into(),
        Err(error) => {
            let mut tokens = proc_macro2::TokenStream::new();
            tokens.extend(error.to_compile_error());
            tokens.extend(original_item);
            tokens.into()
        }
    }
}
