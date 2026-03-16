# Plano: ampliar `examples/` com cobertura live real do `prd-gaps`

## Resumo

- Corrigir primeiro o provider do Claude para a CLI real instalada hoje, porque o baseline atual já falha antes mesmo de rodar qualquer exemplo novo.
- Depois adicionar exemplos extras que validem features P2 de forma realmente útil e executável contra providers reais, não demos artificiais.
- Expandir a matriz live e a documentação para que esses exemplos virem smoke/E2E checks repetíveis.

## Mudanças de implementação

### 1. Corrigir o root cause atual do Claude antes de mexer nos exemplos

- Ajustar `crates/arky-claude-code` para a superfície real do `claude 2.1.75`, validada por `claude --help`.
- Parar de emitir flags removidas ou renomeadas:
- `--streaming-input` -> usar `--input-format stream-json` somente quando houver stdin streaming real
- `--plugin` -> serializar para `--plugin-dir` com normalização do diretório raiz do plugin
- `--mcp-server` -> usar `--mcp-config`
- flags singulares antigas de settings/tools -> usar a forma suportada hoje
- Corrigir o fluxo de stdin do Claude para usar de fato `ClaudeInjectedPromptStream` quando o modo streaming JSON for necessário, em vez de sempre fechar stdin.
- Manter texto simples em `--print` para cenários que não precisam input streaming, evitando forçar um modo mais complexo sem necessidade.
- Adicionar testes de compatibilidade de argumentos no crate do Claude para impedir regressão para flags antigas.

### 2. Adicionar exemplos Claude que cubram features novas e sejam úteis de verdade

- `examples/10_claude_mcp.rs`
- Subir um MCP server HTTP temporário no próprio processo.
- Configurar o provider Claude com MCP compatível com a CLI atual.
- Pedir para Claude usar o MCP para buscar um token exato.
- Validar uso de tool/MCP e resposta final exata.
- Cobre a feature nova de MCP/custom server de forma live.
- `examples/11_claude_runtime_config.rs`
- Configurar `env` no provider e `debug_file`.
- Forçar Claude a usar ferramenta nativa real para ler a variável de ambiente ou um artefato derivado dela.
- Validar execução de tool, resposta final exata e criação do arquivo de debug.
- Cobre env passthrough e debug config com um cenário real de troubleshooting, não sintético.

### 3. Adicionar exemplo Codex para features P2 novas que ainda não estão em `examples/`

- `examples/12_codex_metadata_compaction.rs`
- Fazer um stream real e validar os custom events `stream-start` e `response-metadata`.
- Confirmar que as mensagens carregam `part_id` válido.
- Extrair o `thread_id`/session metadata do fluxo real.
- Chamar `CodexProvider::compact_thread()` contra a thread real.
- Fazer um follow-up depois da compactação e validar que a conversa continua funcional.
- Converter `Usage` em `NormalizedUsage` e imprimir/validar `compute_estimated_cost()` quando houver dados suficientes.
- Cobre thread compaction, metadata emission, part IDs e cost estimation num cenário quase-E2E.

### 4. Integrar os novos exemplos à suíte live existente

- Atualizar `examples/09_live_matrix.rs` para incluir os novos cenários nos grupos `claude`, `codex` e `all`.
- Atualizar `examples/README.md` com a nova matriz e o propósito de cada exemplo.
- Atualizar `docs/getting-started.md` para refletir os novos entrypoints live.

## APIs e interfaces afetadas

- `ClaudeCodeProviderConfig`
- Mantém a API pública existente, mas a serialização CLI passa a ser compatível com a CLI real instalada.
- `plugins` passam a ser normalizados para diretório de plugin compatível com `--plugin-dir`.
- `mcp_servers` passam a ser emitidos via `--mcp-config`.
- `ClaudeCodeProvider`
- `stream()` passa a usar stdin streaming apenas quando o cenário realmente exige isso.
- `CodexProvider`
- Nenhuma quebra de API pública; o exemplo novo passa a exercitar `compact_thread()` explicitamente.
- `examples/09_live_matrix.rs`
- Amplia a matriz pública de smoke/live scenarios.

## Testes e cenários

- Baseline fix:
- `cargo run -p arky --example 01_claude_basic` deve voltar a funcionar no ambiente atual.
- `cargo run -p arky --example 04_codex_basic` continua verde.
- Novos exemplos:
- `cargo run -p arky --example 10_claude_mcp`
- `cargo run -p arky --example 11_claude_runtime_config`
- `cargo run -p arky --example 12_codex_metadata_compaction`
- Matriz live:
- `make test-live PROVIDER=claude`
- `make test-live PROVIDER=codex`
- `make test-live`
- Verificação obrigatória antes de fechar:
- `make fmt`
- `make lint`
- `make test`

## Assumptions e defaults

- Target principal de compatibilidade do Claude: a CLI real disponível no ambiente, hoje `Claude Code 2.1.75`.
- Target principal de compatibilidade do Codex: `codex-cli 0.114.0`, que já está funcional no baseline atual.
- Não vou adicionar exemplo live de imagem/plugin “por obrigação” só para marcar cobertura:
- imagem fica fora desta rodada porque a documentação e o `--help` atuais do Claude só comprovam de forma estável o caminho de input streaming por `stream-json`, e isso já é um alvo de compatibilidade sensível;
- plugin só entra se o cenário ficar deterministicamente executável com `--plugin-dir` sem inventar workaround.
- O objetivo desta rodada é maximizar cobertura live útil das features novas, não forçar paridade de exemplo para toda feature interna que só faz sentido em teste unitário.
