# Patch de teste — SMS raster/parallax

**PR:** #277 (consolidada)  
**Base:** `main` SHA `151eae75c8851aeb9cdbc056d99cfa093013e46c`  
**Objetivo:** disponibilizar um único build de teste que combine a cobertura horizontal já integrada e a correção candidata R10, preservando os relatórios de auditoria.

## Conteúdo

1. **Cobertura horizontal já existente em main:** os testes adicionados pela PR #272 foram incorporados antes desta branch. Incluem alteração de R8 entre scanlines, invariância ao dividir avanços em blocos, varredura dos 256 valores de R8 e invariância para todas as divisões de T-state de uma linha.
2. **Correção candidata R10:** escrita em R10 altera o reload futuro sem reiniciar o countdown ativo; teste `mid_irq_r10_write_changes_next_reload_not_active_countdown`.
3. **Desempenho:** a instrumentação síncrona `eprintln!` do VDP foi removida nesta revisão porque emitia I/O no caminho crítico e degradava severamente a velocidade do emulador. Esta branch não gera trace raster por padrão.
4. **Auditorias:** relatórios Aladdin/parallax, CPU/barramento, latches VDP, R10, revisão de CSV Z80/VDP e temporização raster.

## Como testar no Windows

```powershell
gh pr checkout 277
cargo fmt --all -- --check
cargo test --workspace --locked
```

Para o teste visual, execute Aztec Adventure e Aladdin no mesmo perfil regional e resolução nativa. Não é necessário redirecionar `stderr`; o VDP não contém logging síncrono nesta revisão.

Uma coleta futura de traces deverá usar mecanismo opt-in, limitado e sem I/O síncrono no caminho crítico. Isso não faz parte desta alteração.

## Hipóteses e limites

- **R10 reescrito no handler:** a alteração candidata preserva o countdown ativo e aplica o valor escrito na próxima recarga. A semântica ainda precisa de confirmação independente.
- **HINT e VBlank coincidentes:** hipótese não resolvida; requer evidência temporal independente.
- **Fase do HINT:** o modelo avança o VDP por T-states agregados após instruções completas. Não equivale a precisão ciclo a ciclo.
- **Entrega de INT:** `SmsMachine::step` executa uma instrução, avança VDP por ciclos agregados e então atualiza a solicitação INT.

## Segurança e privacidade

Não versionar ROM comercial, BIOS protegida, dumps de memória ou traces que contenham bytes/conteúdo da ROM.

## Estado de validação

- Cobertura sintética horizontal: já integrada em main pela PR #272; não equivale a TEMPORAL_PASS.
- Correção R10: candidata, ainda não validada contra hardware/Ares e gameplay.
- Logging síncrono do VDP: removido.
- Gameplay A/B: pendente de execução na máquina Windows do usuário.
- HINT/VBlank no mesmo instante: hipótese não confirmada.
- `TEMPORAL_PASS`: **NOT ACHIEVED**.
