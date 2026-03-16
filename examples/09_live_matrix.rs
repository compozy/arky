//! # 09 Live Matrix
//!
//! Runs the live example suite for `claude`, `codex`, or `all`.

mod common;

use std::{
    env,
    io,
    process::Command,
};

use common::ExampleError;

fn scenario_names(selection: &str) -> Result<Vec<&'static str>, ExampleError> {
    match selection {
        "claude" => Ok(vec![
            "01_claude_basic",
            "02_claude_tools",
            "03_claude_resume",
        ]),
        "codex" => Ok(vec![
            "04_codex_basic",
            "05_codex_tools",
            "06_codex_resume",
            "07_codex_mcp",
            "08_codex_control_flow",
        ]),
        "all" => Ok(vec![
            "01_claude_basic",
            "02_claude_tools",
            "03_claude_resume",
            "04_codex_basic",
            "05_codex_tools",
            "06_codex_resume",
            "07_codex_mcp",
            "08_codex_control_flow",
        ]),
        _ => Err(io::Error::other(
            "usage: cargo run -p arky --example 09_live_matrix -- [claude|codex|all]",
        )
        .into()),
    }
}

#[tokio::main]
async fn main() -> Result<(), ExampleError> {
    let selection = env::args().nth(1).unwrap_or_else(|| "all".to_owned());
    let scenarios = scenario_names(&selection)?;

    println!("Running live provider matrix: {selection}");

    for scenario in scenarios {
        println!("\n== {scenario} ==");
        let status = Command::new("cargo")
            .args(["run", "-p", "arky", "--example", scenario])
            .status()?;
        if !status.success() {
            return Err(io::Error::other(format!(
                "scenario `{scenario}` failed with status {status}"
            ))
            .into());
        }
    }

    println!("\nPASS: live provider matrix `{selection}` completed");
    Ok(())
}
