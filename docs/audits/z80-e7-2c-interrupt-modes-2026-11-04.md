# Z80 E7.2C — Dispatch retomável de INT nos modos IM 0/1/2

**Data:** 2026-11-04  
**Estado:** IMPLEMENTED na branch da PR #498; ainda não integrado à main  
**Base main:** `793a1c72ef9797e045903ae2be32f8a17e2acd23`  
**SHA do candidato de código validado:** `9cb576e957fdb37529d3c8dfb5d54fb22c2c907f`  
**PR:** [#498 — IM0/IM1/IM2 retomáveis](https://github.com/leonardo201800478/sega-emulator/pull/498)  
**CI:** [run #41792143207](https://github.com/leonardo201800478/sega-emulator/actions/runs/41792143207)  
**Referência normativa:** ZiLOG, *Z80 CPU User Manual*, UM008011-0816, seções de INT acknowledge e CPU response. A referência foi fornecida no contexto do projeto.

---

## 1. Escopo

E7.2C fecha o dispatch microsequenciado do acknowledge INT para IM0, IM1 e IM2. Não fecha arbitragem EI/DI, amostragem física T4, BUSREQ, RETI/daisy-chain, auditoria integral de snapshots ou validação diferencial/temporal independente.

A implementação permanece no Z80 compartilhado em `native/z80_cpu`; a continuação temporal SMS foi atualizada em `native/sms_core`. Não há implementação de hardware em Python nem duplicação de CPU.

## 2. Contrato implementado

### 2.1 Acknowledge e latch do modo

- A operação continua sendo `InterruptAcknowledge`, com base de 3 T-states e dois WAITs automáticos, totalizando 5 T-states nominais.
- Ao concluir o acknowledge, o microsequenciador preserva o vetor e captura o modo IM na fase `InterruptAcknowledgeComplete`.
- O acknowledge incrementa R uma única vez, preservando o bit 7, não incrementa PC e limpa IFF1/IFF2 e HALT conforme o contrato já implementado para a aceitação de INT.
- O boundary após acknowledge é serializável. Uma chamada subsequente a `begin_instruction()` inicia o dispatch específico do modo.
- Os T-states internos posteriores permanecem no M1 do acknowledge (`mcycle_index = 0`); os ciclos de stack começam em M2.

### 2.2 IM0

- O byte recebido no acknowledge é encaminhado ao decoder de opcode existente por `decode_opcode_fetch`; não existe segundo decoder para IM0.
- O byte injetado não incrementa PC nem R novamente. Os bytes seguintes, incluindo opcode terminal após DD/FD/CB/ED e operandos, usam as operações normais de leitura da memória.
- Um T-state interno após o acknowledge compensa a diferença entre o M1 de acknowledge e o fetch normal, mantendo o custo total da instrução injetada em **instrução normal + 2 T-states**.
- Exemplos cobertos: NOP (6T), JP nn (12T) e DD 21 nn (16T). O decoder continua podendo rejeitar explicitamente instruções que ainda não estejam implementadas; isso não representa fechamento semântico integral do Z80.

### 2.3 IM1

- Dois T-states internos após o acknowledge, seguidos de duas escritas de stack de 3T: primeiro byte alto do PC de retorno, depois byte baixo.
- SP é decrementado antes de cada escrita; PC de retorno permanece inalterado até o serviço completar.
- Após a segunda escrita, PC recebe `0038h`; o resultado é `InterruptServiced(Int)`, com **13 T-states** nominais.
- WZ permanece inalterado. Não foi introduzida uma atualização MEMPTR não documentada; isso também preserva o comportamento do executor atômico de referência existente.

### 2.4 IM2

- Usa o mesmo empilhamento high/low do PC de retorno.
- Forma o endereço da tabela como `(I << 8) | vector`, lê low e high em transações separadas de 3T e monta o destino little-endian.
- PC e WZ recebem o destino após a leitura do byte alto; resultado `InterruptServiced(Int)), com **19 T-states** nominais.
- O byte de vetor é preservado sem mascarar bit 0. O manual exige vetor par fornecido pelo dispositivo; a CPU não corrige silenciosamente um vetor inválido. Há teste que verifica que um byte ímpar não é mascarado.

### 2.5 WAIT, suspensão e snapshot

- WAIT externo continua aditivo ao tempo base das transações. O teste de IM1 adiciona um WAIT ao acknowledge e outro à escrita high do stack; o total passa de 13T para 15T.
- Fases de dispatch interno, stack push high/low e leitura low/high da tabela IM2 têm estado explícito e validação cruzada com modo, vetor/endereço, PC, SP, IFF, request pendente, M-cycle e T-state.
- O modo é serializado também na fase de acknowledge concluído, para que a retomada não dependa de uma leitura posterior do registrador IM.
- O codec adiciona tags 66–69 para as novas fases e inclui o modo no tag 65. O schema passa de 30 para **31**.

### 2.6 Integração temporal SMS

`SmsTemporalMachine::resume_instruction()` agora retoma `begin_instruction()` quando restaura/recebe a fase `InterruptAcknowledgeComplete`; não devolve novamente o acknowledge como se fosse o fim do serviço.

Testes no Scheduler SMS percorrem IM1 e IM2 até o resultado final, verificando PC, SP, WZ quando aplicável e duração da máquina. O teste existente de IM0 agora retoma a instrução RST fornecida pelo acknowledge e verifica o stack em RAM.

## 3. Evidência de testes

### Z80 microsequencer

- `acknowledge_snapshot_latches_the_interrupt_mode`
- `im0_executes_acknowledge_opcode_without_pc_or_second_refresh_increment`
- `im0_multibyte_instruction_reads_following_bytes_from_memory`
- `im0_index_prefix_fetches_the_remaining_instruction_from_memory`
- `im1_stack_write_snapshots_preserve_both_push_stages`
- `im1_explicit_ack_and_stack_waits_extend_the_interrupt_duration`
- `im1_pushes_return_pc_and_vectors_to_0038_in_thirteen_tstates`
- `im2_reads_vector_low_high_and_vectors_in_nineteen_tstates`
- `im2_does_not_silently_mask_an_odd_vector_byte`

### SMS temporal machine

- `int_acknowledge_is_a_distinct_temporal_transaction_and_restores_waits`
- `im1_interrupt_dispatch_resumes_through_sms_scheduler`
- `im2_interrupt_dispatch_reads_vector_table_through_sms_scheduler`

### CI no SHA de código `9cb576e957fdb37529d3c8dfb5d54fb22c2c907f`

| Suíte | Resultado |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| Testes direcionados SMS | PASS |
| Testes direcionados SG-1000 | PASS |
| `cargo test --workspace --locked` | PASS |
| `python -m pytest` | PASS |
| `python -m ruff check .` | PASS |
| `python -m ruff format --check .` | PASS |
| Windows x64 D3D12/video compile e testes | PASS |
| Windows WASAPI/XAudio2 compile e testes | PASS |
| Scheduler deterministic stress | SKIPPED |

O workflow #41792143207 terminou SUCCESS. SKIPPED não é contabilizado como PASS.

## 4. Limites e pendências

- **E7.2D:** arbitragem EI/DI, atraso de EI e interações EI;HALT;INT / EI;DI / NMI durante o atraso.
- **E7.2E:** auditoria completa de snapshot/restore de todas as fases e combinações de WAIT; esta PR cobre as novas fases, não toda a matriz de estados do Z80.
- **Ainda pendente:** amostragem de INT/NMI no T4 físico de HALT, BUSREQ, RETI/daisy-chain e integração da fonte IRQ real do VDP SMS.
- O workflow não executou stress determinístico do Scheduler; nenhum LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS é declarado.
- Não foram executados ZEXDOC/ZEXALL nem oráculo diferencial externo nesta fatia.
- IM0 reutiliza o decoder existente; instruções que ainda retornam `UnsupportedOpcode` continuam fora do conjunto implementado.
- O teste de vetor ímpar verifica ausência de mascaramento silencioso; não afirma que vetor ímpar seja um valor válido para um dispositivo conforme ao manual.

**Conclusão:** E7.2C está implementado e passou os gates convencionais e os testes focados de integração SMS no SHA indicado. A PR ainda precisa ser integrada à main; E7 e o Z80 completo permanecem INTEGRATION_PENDING até os incrementos restantes e validação independente.
