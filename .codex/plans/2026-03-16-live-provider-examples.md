# Reestruturar `examples/` como suíte live dos providers

## Summary

- Substituir o conteúdo atual de `examples/` por exemplos executáveis contra providers reais, com assertions explícitas e `exit code != 0` quando o comportamento esperado não acontecer.
- Tirar de `examples/` os demos locais do core com providers fake; eles deixam de ser “fonte de verdade” para validação de provider e passam a viver em testes/docs.
- Cobrir a matriz real de capabilities dos providers, com foco em `generate`, `streaming`, `tool calls`, `session resume`, `MCP`, e, no caso do Codex, `steering` e `follow_up`.

## Implementation Changes

- Reaproveitar `examples/common.rs` como harness live compartilhado:
  - preflight de binário e diretório temporário;
  - helpers para coletar stream, assertions de eventos e mensagem final;
  - saída padronizada `PASS` / `FAIL` com erro acionável;
  - zero sleeps arbitrários: esperar eventos/condições reais.
- Substituir os targets atuais de `crates/arky/Cargo.toml` por uma suíte capability-based, com nomes numerados e sem exemplos “fake”:
  - `01_claude_basic`
  - `02_claude_tools`
  - `03_claude_resume`
  - `04_codex_basic`
  - `05_codex_tools`
  - `06_codex_resume`
  - `07_codex_mcp`
  - `08_codex_control_flow`
  - `09_live_matrix`
- Usar `Agent` nos exemplos em que a capability é exposta no nível alto; usar `ProviderRequest` direto apenas onde a feature existir só na camada do provider.
- Mover os demos atuais de system prompt, hooks, server, registry e MCP local puro para testes/docs, preservando a cobertura funcional sem mantê-los como “examples de provider”.
- Atualizar `examples/README.md` e `docs/getting-started.md` para refletir que `examples/` agora é uma suíte live de verificação e não mais um tutorial progressivo.
- Adicionar um comando de workflow explícito para execução local da suíte live, sem alterar o CI padrão para exigir credenciais.

## Public APIs / Interfaces

- Nenhuma API pública dos crates deve mudar.
- Mudam as interfaces públicas de uso do repositório:
  - os nomes dos example targets passam a refletir cenários live dos providers;
  - o README dos examples passa a documentar pré-requisitos, comandos e critérios de PASS/FAIL;
  - o runner agregado vira a forma recomendada de validar manualmente os providers.
- Os exemplos aceitam apenas overrides mínimos e previsíveis:
  - modelo por provider via env/arg documentado;
  - modo verboso opcional;
  - seleção de provider apenas no runner agregado.

## Test Plan

- Cada example deve compilar com `cargo build --examples`.
- Cada example live deve:
  - falhar com mensagem clara quando faltar binário, auth ou capability esperada;
  - falhar quando não observar os eventos/resultados contratados;
  - retornar `0` apenas quando a feature realmente tiver sido exercitada.
- Os testes reais opt-in já existentes em `arky-claude-code` e `arky-codex` devem ser fortalecidos para reutilizar as mesmas assertions centrais dos examples live, evitando duas definições diferentes de “provider funcionando”.
- Validação final obrigatória:
  - `make fmt`
  - `make lint`
  - `make test`

## Assumptions

- `examples/` deixa de ser tutorial do core e passa a ser suíte live dos providers; os demos antigos não permanecem ali.
- Falta de autenticação/binário é erro de execução do example, não “skip silencioso”; o único skip aceitável continua sendo nos testes reais opt-in já protegidos por env flag.
- O CI continua compilando examples, mas a execução live permanece como workflow local/opt-in até existir infraestrutura com credenciais reais.
