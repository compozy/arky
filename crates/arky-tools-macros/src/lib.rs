//! Proc macros for the Arky AI agent SDK tool system.

use proc_macro::TokenStream;

/// Derive macro placeholder for tool definitions.
#[proc_macro_attribute]
pub fn tool(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
