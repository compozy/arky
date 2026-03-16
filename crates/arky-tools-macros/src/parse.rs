use proc_macro2::Span;
use syn::{
    Error,
    FnArg,
    Ident,
    ItemFn,
    Pat,
    PatIdent,
    PathArguments,
    ReturnType,
    Type,
    TypePath,
};

#[derive(Debug)]
pub struct ParsedToolFn {
    pub item: ItemFn,
    pub function_name: String,
    pub tool_ident: Ident,
    pub input_ident: Ident,
    pub description: String,
    pub input_kind: InputKind,
    pub output_kind: OutputKind,
    pub output_type: Type,
    pub has_cancel: bool,
}

#[derive(Debug)]
pub enum InputKind {
    None,
    Direct(Box<ToolArg>),
    Named(Vec<ToolArg>),
}

#[derive(Debug)]
pub struct ToolArg {
    pub ident: Ident,
    pub ty: Type,
}

#[derive(Debug)]
pub enum OutputKind {
    ToolResult,
    Text,
    Unit,
    Json,
}

impl ParsedToolFn {
    pub fn parse(item: ItemFn) -> syn::Result<Self> {
        validate_signature(&item)?;

        let function_name = normalize_ident(&item.sig.ident);
        let tool_ident = Ident::new(
            &format!("{}Tool", to_pascal_case(&function_name)),
            item.sig.ident.span(),
        );
        let input_ident = Ident::new(
            &format!("{}ToolInput", to_pascal_case(&function_name)),
            item.sig.ident.span(),
        );
        let description = parse_description(&item)?;

        let ParsedInputs {
            input_kind,
            has_cancel,
        } = parse_inputs(&item)?;
        let ParsedOutput {
            output_kind,
            output_type,
        } = parse_output(&item)?;

        Ok(Self {
            item,
            function_name,
            tool_ident,
            input_ident,
            description,
            input_kind,
            output_kind,
            output_type,
            has_cancel,
        })
    }
}

struct ParsedInputs {
    input_kind: InputKind,
    has_cancel: bool,
}

struct ParsedOutput {
    output_kind: OutputKind,
    output_type: Type,
}

fn validate_signature(item: &ItemFn) -> syn::Result<()> {
    if item.sig.asyncness.is_none() {
        return Err(Error::new_spanned(
            item.sig.fn_token,
            "#[tool] can only be applied to async functions",
        ));
    }

    if let Some(constness) = &item.sig.constness {
        return Err(Error::new_spanned(
            constness,
            "#[tool] does not support const functions",
        ));
    }

    if let Some(unsafety) = &item.sig.unsafety {
        return Err(Error::new_spanned(
            unsafety,
            "#[tool] does not support unsafe functions",
        ));
    }

    if item.sig.abi.is_some() {
        return Err(Error::new_spanned(
            &item.sig.abi,
            "#[tool] does not support extern functions",
        ));
    }

    if !item.sig.generics.params.is_empty() || item.sig.generics.where_clause.is_some() {
        return Err(Error::new_spanned(
            &item.sig.generics,
            "#[tool] does not support generic parameters",
        ));
    }

    if item.sig.variadic.is_some() {
        return Err(Error::new_spanned(
            &item.sig.variadic,
            "#[tool] does not support variadic parameters",
        ));
    }

    Ok(())
}

fn parse_description(item: &ItemFn) -> syn::Result<String> {
    let mut lines = Vec::new();

    for attr in &item.attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }

        let syn::Meta::NameValue(meta) = &attr.meta else {
            continue;
        };
        let syn::Expr::Lit(expr_lit) = &meta.value else {
            continue;
        };
        let syn::Lit::Str(lit) = &expr_lit.lit else {
            continue;
        };
        lines.push(lit.value().trim().to_owned());
    }

    while lines.first().is_some_and(String::is_empty) {
        lines.remove(0);
    }

    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }

    if lines.is_empty() {
        return Err(Error::new_spanned(
            &item.sig.ident,
            "#[tool] functions require at least one doc comment line",
        ));
    }

    Ok(lines.join("\n"))
}

fn parse_inputs(item: &ItemFn) -> syn::Result<ParsedInputs> {
    let mut args = Vec::new();
    let mut has_cancel = false;

    for input in &item.sig.inputs {
        match input {
            FnArg::Receiver(receiver) => {
                return Err(Error::new_spanned(
                    receiver,
                    "#[tool] does not support methods or self receivers",
                ));
            }
            FnArg::Typed(pat_type) => {
                if contains_lifetime(&pat_type.ty) {
                    return Err(Error::new_spanned(
                        &pat_type.ty,
                        "#[tool] tool parameters must be owned values; references are not supported",
                    ));
                }

                let Pat::Ident(PatIdent { ident, .. }) = pat_type.pat.as_ref() else {
                    return Err(Error::new_spanned(
                        &pat_type.pat,
                        "#[tool] parameter patterns must be simple identifiers",
                    ));
                };

                if is_cancellation_token(&pat_type.ty) {
                    if has_cancel {
                        return Err(Error::new_spanned(
                            &pat_type.ty,
                            "#[tool] only one CancellationToken parameter is allowed",
                        ));
                    }
                    has_cancel = true;
                    continue;
                }

                args.push(ToolArg {
                    ident: ident.clone(),
                    ty: (*pat_type.ty).clone(),
                });
            }
        }
    }

    let input_kind = match args.len() {
        0 => InputKind::None,
        1 => {
            InputKind::Direct(Box::new(args.pop().expect("exactly one arg should exist")))
        }
        _ => InputKind::Named(args),
    };

    Ok(ParsedInputs {
        input_kind,
        has_cancel,
    })
}

fn parse_output(item: &ItemFn) -> syn::Result<ParsedOutput> {
    let ReturnType::Type(_, return_type) = &item.sig.output else {
        return Err(Error::new_spanned(
            &item.sig.ident,
            "#[tool] functions must declare an explicit return type",
        ));
    };

    let Type::Path(TypePath { path, .. }) = return_type.as_ref() else {
        return Err(Error::new_spanned(
            return_type,
            "#[tool] functions must return Result<Output, ToolError>",
        ));
    };

    let Some(result_segment) = path.segments.last() else {
        return Err(Error::new_spanned(
            return_type,
            "#[tool] functions must return Result<Output, ToolError>",
        ));
    };

    if result_segment.ident != "Result" {
        return Err(Error::new_spanned(
            return_type,
            "#[tool] functions must return Result<Output, ToolError>",
        ));
    }

    let PathArguments::AngleBracketed(arguments) = &result_segment.arguments else {
        return Err(Error::new_spanned(
            &result_segment.arguments,
            "#[tool] Result must include success and error type arguments",
        ));
    };

    if arguments.args.len() != 2 {
        return Err(Error::new_spanned(
            arguments,
            "#[tool] Result must have the shape Result<Output, ToolError>",
        ));
    }

    let success_type = extract_type_argument(
        arguments
            .args
            .first()
            .expect("Result success arg should exist"),
    )?;
    let error_type = extract_type_argument(
        arguments
            .args
            .iter()
            .nth(1)
            .expect("Result error arg should exist"),
    )?;

    if !is_tool_error_type(&error_type) {
        return Err(Error::new_spanned(
            error_type,
            "#[tool] functions must return Result<Output, ToolError>",
        ));
    }

    let output_kind = classify_output_kind(&success_type);

    Ok(ParsedOutput {
        output_kind,
        output_type: success_type,
    })
}

fn extract_type_argument(argument: &syn::GenericArgument) -> syn::Result<Type> {
    match argument {
        syn::GenericArgument::Type(ty) => Ok(ty.clone()),
        _ => Err(Error::new(
            Span::call_site(),
            "#[tool] Result type arguments must be concrete types",
        )),
    }
}

fn contains_lifetime(ty: &Type) -> bool {
    match ty {
        Type::Reference(_) => true,
        Type::Group(group) => contains_lifetime(&group.elem),
        Type::Paren(paren) => contains_lifetime(&paren.elem),
        _ => false,
    }
}

fn is_cancellation_token(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };

    type_path
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "CancellationToken")
}

fn is_tool_error_type(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };

    type_path
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "ToolError")
}

fn classify_output_kind(ty: &Type) -> OutputKind {
    if is_tool_result_type(ty) {
        return OutputKind::ToolResult;
    }

    if is_string_type(ty) {
        return OutputKind::Text;
    }

    if matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty()) {
        return OutputKind::Unit;
    }

    OutputKind::Json
}

fn is_tool_result_type(ty: &Type) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };

    type_path
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "ToolResult")
}

fn is_string_type(ty: &Type) -> bool {
    match ty {
        Type::Path(type_path) => type_path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "String"),
        Type::Reference(reference) => {
            matches!(reference.elem.as_ref(), Type::Path(type_path) if type_path.path.is_ident("str"))
        }
        _ => false,
    }
}

fn normalize_ident(ident: &Ident) -> String {
    let ident_string = ident.to_string();
    ident_string
        .strip_prefix("r#")
        .unwrap_or(&ident_string)
        .to_owned()
}

fn to_pascal_case(value: &str) -> String {
    let mut pascal = String::with_capacity(value.len());

    for segment in value.split('_').filter(|segment| !segment.is_empty()) {
        let mut characters = segment.chars();
        if let Some(first) = characters.next() {
            pascal.extend(first.to_uppercase());
            pascal.push_str(characters.as_str());
        }
    }

    if pascal.is_empty() {
        "Generated".to_owned()
    } else {
        pascal
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use syn::parse_quote;

    use crate::parse::{
        InputKind,
        OutputKind,
        ParsedToolFn,
    };

    #[test]
    fn parse_should_extract_name_description_inputs_and_return_type() {
        let parsed = ParsedToolFn::parse(parse_quote! {
            /// Read a file from disk.
            async fn read_file(
                path: String,
                max_lines: Option<u32>,
                cancel: tokio_util::sync::CancellationToken,
            ) -> Result<String, arky_tools::ToolError> {
                let _ = cancel;
                Ok(path)
            }
        })
        .expect("function should parse");

        assert_eq!(parsed.function_name, "read_file");
        assert_eq!(parsed.description, "Read a file from disk.");
        assert!(parsed.has_cancel);
        assert!(matches!(parsed.output_kind, OutputKind::Text));
        match parsed.input_kind {
            InputKind::Named(args) => {
                let actual = args
                    .into_iter()
                    .map(|arg| arg.ident.to_string())
                    .collect::<Vec<_>>();
                assert_eq!(actual, vec!["path", "max_lines"]);
            }
            _ => panic!("expected named arguments"),
        }
    }

    #[test]
    fn parse_should_treat_single_argument_as_direct_input() {
        let parsed = ParsedToolFn::parse(parse_quote! {
            /// Look up a city weather report.
            async fn weather(args: WeatherArgs) -> Result<serde_json::Value, ToolError> {
                Ok(serde_json::json!({ "city": args.city }))
            }
        })
        .expect("single-argument function should parse");

        match parsed.input_kind {
            InputKind::Direct(arg) => assert_eq!(arg.ident.to_string(), "args"),
            _ => panic!("expected direct argument"),
        }
    }

    #[test]
    fn parse_should_reject_missing_doc_comments() {
        let error = ParsedToolFn::parse(parse_quote! {
            async fn undocumented() -> Result<(), ToolError> {
                Ok(())
            }
        })
        .expect_err("undocumented function should fail");

        assert_eq!(
            error.to_string(),
            "#[tool] functions require at least one doc comment line"
        );
    }

    #[test]
    fn parse_should_reject_reference_arguments() {
        let error = ParsedToolFn::parse(parse_quote! {
            /// Unsupported borrowed input.
            async fn borrowed(value: &str) -> Result<(), ToolError> {
                Ok(())
            }
        })
        .expect_err("borrowed input should fail");

        assert_eq!(
            error.to_string(),
            "#[tool] tool parameters must be owned values; references are not supported"
        );
    }

    #[test]
    fn parse_should_classify_unit_and_tool_result_outputs() {
        let unit = ParsedToolFn::parse(parse_quote! {
            /// No-op tool.
            async fn noop() -> Result<(), ToolError> {
                Ok(())
            }
        })
        .expect("unit output should parse");
        let tool_result = ParsedToolFn::parse(parse_quote! {
            /// Full control tool.
            async fn raw() -> Result<ToolResult, ToolError> {
                Ok(ToolResult::success("call", "mcp/local/raw", Vec::new()))
            }
        })
        .expect("tool result output should parse");

        assert!(matches!(unit.output_kind, OutputKind::Unit));
        assert!(matches!(tool_result.output_kind, OutputKind::ToolResult));
    }
}
