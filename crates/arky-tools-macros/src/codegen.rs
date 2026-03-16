use proc_macro2::TokenStream;
use quote::quote;
use syn::LitStr;

use crate::{
    parse::{
        OutputKind,
        ParsedToolFn,
    },
    schema::build_input_support,
};

pub fn expand_tool(parsed: &ParsedToolFn) -> TokenStream {
    let original_fn = &parsed.item;
    let function_ident = &parsed.item.sig.ident;
    let tool_ident = &parsed.tool_ident;
    let function_name = LitStr::new(&parsed.function_name, parsed.item.sig.ident.span());
    let canonical_name = LitStr::new(
        &format!("mcp/local/{}", parsed.function_name),
        parsed.item.sig.ident.span(),
    );
    let description = LitStr::new(&parsed.description, parsed.item.sig.ident.span());
    let input_support = build_input_support(parsed);
    let output_mapping = build_output_mapping(parsed);

    let InputSupport {
        definitions,
        bind_input,
        schema_expr,
        call_args,
    } = input_support;
    let mut handler_args = call_args;
    if parsed.has_cancel {
        handler_args.push(quote!(cancel));
    }

    quote! {
        #original_fn

        #definitions

        #[doc = #description]
        #[derive(Debug, Clone, Copy, Default)]
        pub struct #tool_ident;

        #[::arky_tools::__private::async_trait]
        impl ::arky_tools::Tool for #tool_ident {
            fn descriptor(&self) -> ::arky_tools::ToolDescriptor {
                ::arky_tools::ToolDescriptor::new(
                    #canonical_name,
                    #function_name,
                    #description,
                    #schema_expr,
                    ::arky_tools::ToolOrigin::Local,
                )
                .expect("generated tool descriptor should always be valid")
            }

            async fn execute(
                &self,
                call: ::arky_tools::ToolCall,
                cancel: ::arky_tools::__private::tokio_util::sync::CancellationToken,
            ) -> ::core::result::Result<::arky_tools::ToolResult, ::arky_tools::ToolError> {
                let ::arky_tools::ToolCall {
                    id,
                    name: canonical_name,
                    input,
                    parent_id,
                } = call;

                #bind_input

                let output = #function_ident(#(#handler_args),*).await?;
                #output_mapping
            }
        }
    }
}

use crate::schema::InputSupport;

fn build_output_mapping(parsed: &ParsedToolFn) -> TokenStream {
    let output_type = &parsed.output_type;

    match parsed.output_kind {
        OutputKind::ToolResult => quote! {
            let mut result = output;
            result.id = id;
            result.name = canonical_name;
            result.parent_id = parent_id;
            Ok(result)
        },
        OutputKind::Text => quote! {
            let mut result = ::arky_tools::ToolResult::success(
                id,
                canonical_name,
                vec![::arky_tools::ToolContent::text(output)],
            );
            result.parent_id = parent_id;
            Ok(result)
        },
        OutputKind::Unit => quote! {
            let mut result = ::arky_tools::ToolResult::success(id, canonical_name, Vec::new());
            result.parent_id = parent_id;
            Ok(result)
        },
        OutputKind::Json => quote! {
            let value = ::arky_tools::__private::serde_json::to_value(&output).map_err(|error| {
                ::arky_tools::ToolError::execution_failed(
                    format!("failed to serialize tool output `{}`: {error}", stringify!(#output_type)),
                    Some(canonical_name.clone()),
                )
            })?;

            let mut result = ::arky_tools::ToolResult::success(
                id,
                canonical_name,
                vec![::arky_tools::ToolContent::json(value)],
            );
            result.parent_id = parent_id;
            Ok(result)
        },
    }
}
