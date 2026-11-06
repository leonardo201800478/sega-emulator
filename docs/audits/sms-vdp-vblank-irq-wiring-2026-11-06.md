# Auditoria e correção — ligação do IRQ VBlank ao Z80 temporal

Data: 2026-11-06

## Evidência de entrada

O relatório externo fornecido junto com `irq_wiring_repro.rs` descreve uma ROM sintética que:
- executa `DI`, configura `IM 1`, habilita VBlank IRQ no registrador 1 do VDP;
- executa `EI` e entra em `HALT`;
- instala ISR em `$0038`, lê o status do VDP para reconhecer o IRQ e incrementa RAM em `$C000`.

O relatório afirma que o VDP sinalizou IRQ em 20/20 frames, mas a ISR executou zero vezes via `step_frame()`; ligando manualmente `interrupt_pending()` a `set_int_line()`, a ISR executou 20 vezes. A ROM comercial Phantasy Star não foi usada nessa reprodução.

## Auditoria independente do código

No código de produção, `SmsTemporalMachine::dispatch_next()` avança os domínios de clock e aplica os efeitos de barramento via `SmsTemporalBus::dispatch_next()`. Porém, antes desta alteração, não havia atualização do pino INT do Z80 a partir da saída IRQ do VDP nesse caminho.

`SmsTemporalMachine::set_int_line()` já existia e o sequenciador Z80 já implementava amostragem da linha INT, inclusive na borda M1 T4 durante HALT. Faltava a interconexão entre a saída do VDP e a entrada do CPU.

## Correção

Após cada evento efetivamente despachado pelo Scheduler, a máquina agora atualiza a linha INT com base em `vdp.interrupt_pending()`:
- IRQ pendente: INT assertado com vetor de barramento `0xFF`;
- IRQ não pendente: INT desassertado.

A atualização ocorre depois que o barramento despacha o evento, portanto depois do avanço temporal do VDP e de eventuais efeitos de I/O (incluindo leitura do status/ack). Isso permite que o Z80 observe o nível atualizado na próxima amostragem válida, incluindo M1 T4 durante HALT. Não há polling em Python, callback por instrução ou avanço de tempo adicional.

## Regressão

Foi adicionado teste de integração em `temporal_machine.rs` usando a ROM sintética do relatório. O critério é que, após 20 frames executados pelo mesmo `step_frame()` usado pelo worker, a ISR tenha incrementado `RAM[$C000]` 20 vezes e o VDP não mantenha IRQ pendente após os reconhecimentos.

## Limites e validação

- A correção resolve a conexão IRQ VDP → INT Z80 no runtime temporal SMS; não prova que Phantasy Star agora inicializa corretamente.
- O relatório de profiling e suas métricas de desempenho são evidência externa fornecida, não foram reproduzidos nesta sessão.
- A reprodução com ROM comercial e a validação visual/sonora em Windows continuam pendentes.
- CI deve passar no HEAD final. Qualquer suíte SKIPPED/NOT RUN será classificada como tal, nunca como PASS.
