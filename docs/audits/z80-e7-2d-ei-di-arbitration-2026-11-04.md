# Z80 E7.2D — Arbitragem EI/DI e atraso de INT

**Data:** 2026-11-04  
**Estado:** cenários focados integrados à main pela PR #500  
**Base main antes da PR:** HEAD 8d13178ad79763cc23180aa8d6d334028632c29a (PR #499, documentação); baseline de código a756e51cf6c5245357831896de293cd2e9d3a859  
**Merge SHA main:** ea327025b312feb53f1e3e5326b5a5e7c40e72f1  
**SHA candidato:** c6f0930f6820e760d8b0d60ae7770da9cb22be32  
**PR:** [#500 — testes de arbitragem EI/DI](https://github.com/leonardo201800478/sega-emulator/pull/500)  
**CI da PR:** [run #41793815959](https://github.com/leonardo201800478/sega-emulator/actions/runs/41793815959) — SUCCESS  
**CI pós-merge:** [run #41794506556](https://github.com/leonardo201800478/sega-emulator/actions/runs/41794506556) — SUCCESS no merge SHA acima.  
**Referência:** ZiLOG Z80 CPU User Manual UM008011-0816, comportamento EI/DI/NMI/INT; implementação auditada em native/z80_cpu/src/sequencer.rs.

---

## 1. Auditoria da implementação existente

A lógica de produção de EI/DI já estava presente antes desta PR; esta fatia acrescenta evidência direta no microsequenciador:

- EI (FB) define IFF1/IFF2 e ei_delay = 1.
- retire() não consome o atraso na própria EI; uma instrução seguinte aposentada consome o atraso.
- DI (F3) limpa IFF1/IFF2 e zera ei_delay.
- begin_instruction() aceita INT somente quando IFF1 está ativo e ei_delay == 0.
- NMI é verificado antes de INT e não é bloqueado por ei_delay.
- retire_interrupt() não consome o atraso; a primeira instrução aposentada após o serviço NMI o consome.
- ei_delay já faz parte do estado serializado e é validado no restore. A PR verifica o round-trip binário com atraso ativo.

Nenhuma lógica de hardware ou timing foi adicionada em Python. Não houve alteração de produção nesta PR; o escopo é teste e evidência dos contratos já codificados.

## 2. Cenários cobertos

### 2.1 EI;NOP;INT

Com INT pendente antes de EI:
1. EI aposenta em 4T e deixa IFF1/IFF2 ativos com ei_delay = 1.
2. O estado é codificado, decodificado e restaurado.
3. NOP, a instrução seguinte, aposenta em 4T e consome o atraso.
4. No boundary seguinte, o acknowledge INT começa; o acknowledge tem 5T nominais.

### 2.2 EI;DI;INT

- EI define o atraso.
- DI executa como a instrução seguinte, limpa IFF1/IFF2 e zera o atraso.
- A linha INT continua pendente, mas não é aceita enquanto IFF1 estiver limpo.
- A instrução seguinte é executada normalmente.

### 2.3 EI;EI;NOP;INT

- O segundo EI é executado apesar do atraso criado pelo primeiro.
- O segundo EI reinicia ei_delay = 1.
- NOP consome o atraso.
- INT é aceito somente no boundary posterior.

### 2.4 EI;NMI;RETN;INT

- EI deixa ei_delay = 1 e IFF1/IFF2 ativos.
- Com NMI e INT pendentes, NMI é aceito primeiro.
- O serviço NMI preserva o atraso; IFF2 retém o estado anterior de IFF1.
- RETN, como primeira instrução do handler no cenário de teste, restaura IFF1 a partir de IFF2 e consome o atraso.
- INT só é aceito no boundary posterior a RETN.

## 3. Testes

- ei_defers_pending_int_until_the_following_instruction_and_survives_snapshot
- di_cancels_ei_delay_and_keeps_pending_int_masked
- a_second_ei_restarts_the_one_instruction_delay
- nmi_is_accepted_during_ei_delay_and_retn_consumes_the_delay

Os testes exercitam o microsequenciador por requests de barramento e ciclos internos, não apenas comparação de registradores finais com o executor atômico.

## 4. CI e resultado

Run #41793815959 no SHA c6f0930f6820e760d8b0d60ae7770da9cb22be32:

| Gate | Resultado |
|---|---|
| cargo fmt --all -- --check | PASS |
| cargo clippy --workspace --all-targets --locked -- -D warnings | PASS |
| Testes direcionados SMS | PASS |
| Testes direcionados SG-1000 | PASS |
| cargo test --workspace --locked | PASS |
| python -m pytest | PASS |
| python -m ruff check . | PASS |
| python -m ruff format --check . | PASS |
| Windows x64 D3D12/video compile e testes | PASS |
| Windows WASAPI/XAudio2 compile e testes | PASS |
| Scheduler deterministic stress | SKIPPED |

SKIPPED não é contabilizado como PASS. Nenhum LONG_PASS, DIFFERENTIAL_PASS ou TEMPORAL_PASS é declarado.

## 5. Limites

- EI;HALT;INT não foi declarado correto nesta fatia. A implementação atual não possui amostragem física de NMI/INT no T4 dos ciclos HALT; este é um gap temporal conhecido, separado do atraso EI validado com NOP/DI/RETN.
- BUSREQ, RETI/daisy-chain, snapshots completos de todas as fases e integração da fonte IRQ real do VDP SMS permanecem pendentes.
- Não foram executados ZEXDOC/ZEXALL nem oráculo diferencial externo.
- A lógica de produção EI/DI foi auditada e exercitada; esta PR não deve ser descrita como implementação de um novo mecanismo de hardware.

**Conclusão:** os cenários EI/DI e NMI durante atraso foram demonstrados no microsequenciador e integrados à main; os gates convencionais passaram tanto na PR quanto no CI pós-merge. E7 global permanece INTEGRATION_PENDING, especialmente pela amostragem HALT T4, BUSREQ, RETI/daisy-chain e validação independente.
