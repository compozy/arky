//! # 08 MCP Integration
//!
//! Demonstrates MCP import/export bridging with a local HTTP server, a remote
//! client, and a bridge registry that exposes both local and imported tools.

mod common;

use std::{
    io,
    net::{
        IpAddr,
        Ipv4Addr,
        SocketAddr,
    },
    sync::Arc,
};

use arky::{
    McpClient,
    McpHttpClientConfig,
    McpServer,
    McpServerHandle,
    McpServerTransport,
    McpToolBridge,
    ToolCall,
    ToolContent,
    ToolOrigin,
    ToolRegistry,
    prelude::*,
};
use async_trait::async_trait;
use common::ExampleError;
use serde_json::json;
use tokio_util::sync::CancellationToken;

struct PrefixTool {
    descriptor: ToolDescriptor,
    prefix: &'static str,
}

impl PrefixTool {
    fn new(
        canonical_name: &str,
        display_name: &str,
        prefix: &'static str,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            descriptor: ToolDescriptor::new(
                canonical_name,
                display_name,
                format!("{display_name} tool"),
                json!({
                    "type": "object",
                    "properties": {
                        "value": {
                            "type": "string"
                        }
                    },
                    "required": ["value"]
                }),
                ToolOrigin::Local,
            )?,
            prefix,
        })
    }
}

#[async_trait]
impl Tool for PrefixTool {
    fn descriptor(&self) -> ToolDescriptor {
        self.descriptor.clone()
    }

    async fn execute(
        &self,
        call: ToolCall,
        _cancel: CancellationToken,
    ) -> Result<ToolResult, ToolError> {
        let value = call
            .input
            .get("value")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                ToolError::invalid_args(
                    "value must be a string",
                    Some(json!({ "input": call.input })),
                )
            })?;

        Ok(ToolResult::success(
            call.id,
            call.name,
            vec![ToolContent::text(format!("{}: {value}", self.prefix))],
        ))
    }
}

fn http_transport(path: &str) -> McpServerTransport {
    McpServerTransport::StreamableHttp {
        bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        path: path.to_owned(),
    }
}

fn handle_url(handle: &McpServerHandle) -> Result<String, ExampleError> {
    handle
        .url()
        .map(ToOwned::to_owned)
        .ok_or_else(|| io::Error::other("expected an HTTP MCP handle").into())
}

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let origin_registry = Arc::new(ToolRegistry::new());
    origin_registry.register(PrefixTool::new(
        "mcp/origin/echo",
        "Origin Echo",
        "origin",
    )?)?;

    let origin_server = McpServer::from_registry("origin", origin_registry);
    let mut origin_handle = origin_server.serve(http_transport("/mcp-origin")).await?;
    let origin_url = handle_url(&origin_handle)?;

    let bridge_registry = Arc::new(ToolRegistry::new());
    bridge_registry.register(PrefixTool::new(
        "mcp/local/local_note",
        "Local Note",
        "local",
    )?)?;

    let origin_client = McpClient::http(McpHttpClientConfig::new("origin", origin_url));
    let mut bridge = McpToolBridge::builder()
        .registry(bridge_registry)
        .server_name("bridge")
        .import_client(origin_client)
        .build()?;

    let imported = bridge.import_tools().await?;
    println!(
        "imported tools: {:?}",
        imported
            .iter()
            .map(|descriptor| descriptor.canonical_name.as_str())
            .collect::<Vec<_>>()
    );

    let mut bridge_handle = bridge.serve(http_transport("/mcp-bridge")).await?;
    let bridge_url = handle_url(&bridge_handle)?;
    let external_client = McpClient::http(McpHttpClientConfig::new("bridge", bridge_url));
    external_client.connect().await?;

    let local_result = external_client
        .call_tool("local__local_note", json!({ "value": "hello bridge" }))
        .await?;
    let bridged_result = external_client
        .call_tool("origin__origin__echo", json!({ "value": "through mcp" }))
        .await?;

    println!("local result: {:?}", local_result.content);
    println!("bridged result: {:?}", bridged_result.content);

    external_client.disconnect().await?;
    bridge.disconnect_all().await?;
    bridge_handle.close().await?;
    origin_handle.close().await?;

    Ok(())
}
