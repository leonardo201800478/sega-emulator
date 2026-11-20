# ADR 0012 — política de panic e crash diagnostics

- **Estado:** SUPERSEDED.
- **Data da substituição:** 2026-11-20.
- **Substituída por:** política de falha crítica de bootstrap em `docs/architecture/observability.md`.

## Decisão substituída

Esta ADR aprovava um hook global de panic instalado após o consentimento, capaz de gravar um registro privado de panic. Essa abordagem não corresponde mais à política de produto aprovada e **não deve ser implementada nem ampliada**.

## Política vigente

1. Não instalar hook global de panic para fins diagnósticos.
2. Panics de runtime pós-boot seguem o mecanismo padrão do Rust e não geram arquivos diagnósticos automáticos.
3. A única gravação automática permitida é um relatório mínimo para falha crítica, classificada, de bootstrap que impeça o emulador de iniciar.
4. O relatório de bootstrap é independente do logger e do consentimento; só é criado no caminho de falha.
5. O conteúdo limita-se a uma categoria estática e timestamp. É proibido incluir payload, backtrace, ROM/BIOS, memória, caminhos absolutos, diretórios pessoais ou conteúdo da sessão.
6. Falhas comuns de ROM, firmware, áudio, sessão e erros ocorridos após o boot não geram relatório automático.

## Estado de implementação

A remoção do hook legado e a implementação/testes de `record_bootstrap_failure` permanecem pendentes P0. DP-0 não pode ser declarado concluído antes de comprovar que sucesso de bootstrap não cria arquivos, que somente falhas críticas classificadas criam o relatório mínimo e que panics pós-boot não são capturados.
