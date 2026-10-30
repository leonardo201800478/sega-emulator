# Patch de teste — SMS raster/parallax

**PR:** #277 (consolidada)  
**Base:** `main` SHA `151eae75c8851aeb9cdbc056d99cfa093013e46c`  
**Objetivo:** disponibilizar um único build de teste que combine a cobertura horizontal já integrada, a correção candidata R10 e logs temporários para distinguir hipóteses de raster/IRQ.

## Conteúdo

1. **Cobertura horizontal já existente em main:** os testes adicionados pela PR #272 foram incorporados antes desta branch. Incluem alteração de R8 entre scanlines, invariância ao dividir avanços em blocos, varredura dos 256 valores de R8 e invariância para todas as divisões de T-state de uma linha.
2. **Correção candidata R10:** escrita em R10 altera o reload futuro sem reiniciar o countdown ativo; teste `mid_irq_r10_write_changes_next_reload_not_active_countdown`.
3. **Instrumentação temporária:** `eprintln!` para escritas R8/R9/R10, IRQ de linha, entrada VBlank e leituras de status em $BF, com quadro/linha/T-state.
4. **Auditorias:** relatórios Aladdin/parallax, CPU/barramento, latches VDP, R10, revisão de CSV Z80/VDP e temporização raster.

## Como testar no Windows

```powershell
gh pr checkout 277
cargo fmt --all -- --check
cargo test --workspace --locked
```

Se o repositório exigir ambiente Python/PyO3 para executar a GUI, use o procedimento de build local já documentado. Para o teste visual, execute Aztec Adventure e Aladdin no mesmo perfil regional e resolução nativa, com stdout/stderr capturados.

```powershell
# Ajuste o caminho para o executável produzido pelo build local
.\caminho\do\emulador.exe 2> sms-raster-trace.log
```

Deixe a captura cobrir um quadro completo estável, preferencialmente o quadro em que ocorre a divisão/parallax. O trace registra:
- `R8/R9/R10`: escrita do registrador;
- `LINE-IRQ`: disparo do IRQ de linha;
- `VBLANK`: transição de VBlank;
- `STATUS`: leitura de $BF antes da limpeza, incluindo bit 7 e estado de line IRQ.

## Hipóteses discriminadas

- **R10 reescrito no handler:** frequência e valores das linhas `R10=` mostram se o jogo reprograma o reload durante o quadro.
- **HINT e VBlank coincidentes:** compare `LINE-IRQ`, `VBLANK` e a leitura seguinte de `STATUS`; bit 7 ligado na leitura do handler é evidência relevante, não prova isolada da decisão de caminho do jogo.
- **Fase do HINT:** os logs atuais registram o limite de scanline do modelo. Comparação com referência Ares/hardware continua necessária.
- **Entrega de INT:** `SmsMachine::step` executa uma instrução, avança VDP por ciclos agregados e então atualiza a solicitação INT. Não há duas filas de IRQ separadas no glue atual.

## Segurança e privacidade

Não versionar ROM comercial, BIOS protegida, dumps de memória ou traces que contenham bytes/conteúdo da ROM. Compartilhar somente o trecho de eventos raster.

## Estado de validação

- Cobertura sintética horizontal: já integrada em main pela PR #272; não equivale a TEMPORAL_PASS.
- Correção R10: candidata, ainda não validada contra hardware/Ares e gameplay.
- Instrumentação: temporária; remover antes de qualquer PR de produção/merge.
- Gameplay A/B: pendente de execução na máquina Windows do usuário.
- HINT/VBlank no mesmo instante: hipótese a confirmar com trace e fonte independente.
- `TEMPORAL_PASS`: **NOT ACHIEVED**.