use proc_macro2::TokenStream;
use quote::{
    format_ident,
    quote,
};

use crate::parse::{
    InputKind,
    ParsedToolFn,
};

pub struct InputSupport {
    pub definitions: TokenStream,
    pub bind_input: TokenStream,
    pub schema_expr: TokenStream,
    pub call_args: Vec<TokenStream>,
}

pub fn build_input_support(parsed: &ParsedToolFn) -> InputSupport {
    match &parsed.input_kind {
        InputKind::None => build_wrapper_support(parsed, &[]),
        InputKind::Direct(arg) => {
            let ty = &arg.ty;
            let binding = format_ident!("tool_input");
            InputSupport {
                definitions: TokenStream::new(),
                bind_input: quote! {
                    let #binding = ::arky_tools::__private::serde_json::from_value::<#ty>(input.clone())
                        .map_err(|error| {
                            ::arky_tools::ToolError::invalid_args(
                                format!("failed to deserialize tool input: {error}"),
                                Some(::arky_tools::__private::serde_json::json!({
                                    "canonical_name": canonical_name,
                                    "expected_type": stringify!(#ty),
                                    "input": input,
                                })),
                            )
                        })?;
                },
                schema_expr: quote! {
                    ::arky_tools::__private::serde_json::to_value(
                        &::arky_tools::__private::schemars::schema_for!(#ty),
                    )
                    .expect("generated tool schema should serialize to JSON")
                },
                call_args: vec![quote!(#binding)],
            }
        }
        InputKind::Named(args) => build_wrapper_support(parsed, args),
    }
}

fn build_wrapper_support(
    parsed: &ParsedToolFn,
    args: &[crate::parse::ToolArg],
) -> InputSupport {
    let input_ident = &parsed.input_ident;
    let binding = format_ident!("tool_input");
    let field_idents = args.iter().map(|arg| arg.ident.clone()).collect::<Vec<_>>();
    let field_types = args.iter().map(|arg| arg.ty.clone()).collect::<Vec<_>>();

    InputSupport {
        definitions: quote! {
            #[derive(
                ::arky_tools::__private::serde::Deserialize,
                ::arky_tools::__private::schemars::JsonSchema,
            )]
            struct #input_ident {
                #( #field_idents: #field_types, )*
            }
        },
        bind_input: quote! {
            let #binding = ::arky_tools::__private::serde_json::from_value::<#input_ident>(input.clone())
                .map_err(|error| {
                    ::arky_tools::ToolError::invalid_args(
                        format!("failed to deserialize tool input: {error}"),
                        Some(::arky_tools::__private::serde_json::json!({
                            "canonical_name": canonical_name,
                            "expected_type": stringify!(#input_ident),
                            "input": input,
                        })),
                    )
                })?;
        },
        schema_expr: quote! {
            ::arky_tools::__private::serde_json::to_value(
                &::arky_tools::__private::schemars::schema_for!(#input_ident),
            )
            .expect("generated tool schema should serialize to JSON")
        },
        call_args: field_idents
            .into_iter()
            .map(|field_ident| quote!(#binding.#field_ident))
            .collect(),
    }
}
