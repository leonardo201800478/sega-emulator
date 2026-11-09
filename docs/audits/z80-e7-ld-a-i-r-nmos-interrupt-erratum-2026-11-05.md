# E7.7 — LD A,I / LD A,R: erratum P/V no aceite de INT

**Data:** 2026-11-05  
**Baseline histórica de implementação:** `1da784f556b9524f5a6f43ce8a7c525e68354f9e`  
**Main após PR #716:** `56d75cc79803494e9fd8a54e8930871c9101fc84`  
**Escopo:** ED 57 (LD A,I) e ED 5F (LD A,R), latch P e aceite de INT/NMI.  
**Referência normativa:** ZiLOG UM008011-0816, páginas impressas 94–95.

## 1. Evidência documental

O manual ZiLOG declara para LD A,I e LD A,R:

- P/V recebe o valor de IFF2;
- se uma interrupção ocorrer durante a execução da instrução, P/V contém zero.

A regra é uma exceção NMOS conhecida do comportamento normal de P/V. O caminho precisa distinguir INT mascarável de NMI: a exceção é associada ao aceite de INT; NMI não deve limpar P/V apenas por esse mecanismo, pois IFF2 preserva o estado anterior de IFF1.

Fonte: PDF UM008011-0816 fornecido no contexto do projeto, instruções LD A,I (p. 94) e LD A,R (p. 95).

## 2. Comparação entre executor atômico e microsequenciador

### Executor atômico — já correto na baseline

Arquivo: `native/z80_cpu/src/cpu.rs`.

O executor:
1. define `p = true` ao executar LD A,I/R;
2. na próxima fronteira, se INT estiver pendente e IFF1 permitir aceite, limpa P/V quando `p` ainda está ativo;
3. limpa `p` antes de executar o acknowledge;
4. limpa `p` quando a próxima instrução normal começa;
5. no caminho NMI, limpa `p` sem aplicar a exceção de INT.

### Microsequenciador — divergência encontrada

Arquivo: `native/z80_cpu/src/sequencer.rs`.

A versão integrada:
1. definia `p = true` ao completar LD A,I/R;
2. em `begin_interrupt_acknowledge()`, limpava `p` sem limpar P/V;
3. não consumia `p` ao iniciar uma instrução normal.

Consequências:
- INT imediatamente após LD A,I/R preservava P/V incorretamente;
- INT após uma instrução intermediária podia limpar P/V incorretamente porque o latch persistia além da fronteira válida.

Isso era uma divergência funcional entre os dois caminhos do Z80 compartilhado.

## 3. Correção aplicada nesta branch

- Antes de iniciar acknowledge INT, se `p` estiver ativo, limpar `FLAG_PV`; depois limpar `p`.
- NMI continua limpando `p` sem alterar P/V.
- Quando não há NMI/INT elegível, a próxima busca normal consome `p`.
- O caminho HALT sem saída pendente também consome `p) antes de iniciar o próximo M1.
- O latch permanece incluído no estado serializado, permitindo snapshot antes do aceite de INT.

Nenhuma alteração de timing nominal, número de T-states, fases de barramento ou contrato da ADR-0016 foi introduzida.

## 4. Testes adicionados

### Microsequenciador

Arquivo: `native/z80_cpu/tests/ed_ld_a_i_r_interrupt_erratum.rs`.

- INT imediatamente após ED 57/ED 5F limpa P/V;
- snapshot/restore entre LD A,I/R e acknowledge preserva o latch e aplica a exceção;
- uma instrução normal intermediária consome o latch sem limpar P/V;
- INT após essa instrução intermediária preserva P/V;
- NMI após LD A,I/R não aplica a exceção de INT.

### Executor atômico

Arquivo: `native/sms_core/tests/z80_ld_a_i_r_interrupt_erratum.rs`.

- INT após LD A,I/R limpa P/V;
- NMI não aplica a exceção;
- uma instrução normal intermediária consome o latch.

Os testes exercitam os dois opcodes, preservação de C e a fronteira temporal de uma instrução.

## 5. Critérios de aceitação

| Critério | Resultado nesta revisão |
|---|---|
| Regra oficial P/V=0 quando INT é aceito durante LD A,I/R | PASS — manual consultado |
| Executor atômico | PASS — implementação já existente e teste adicionado |
| Microsequenciador — INT imediato | Implementado nesta branch; CI pendente |
| Microsequenciador — instrução intermediária | Implementado nesta branch; CI pendente |
| NMI sem erratum de INT | Implementado nesta branch; CI pendente |
| Snapshot/restore do latch P | Teste adicionado; CI pendente |
| Hardware físico | NOT RUN |
| CMOS/clones | NOT VERIFIED |

## 6. Impacto E7

Esta é uma correção funcional localizada no Z80 compartilhado. Não reabre a timeline racional, BUSREQ/BUSACK, RETI/daisy-chain ou qualquer modelagem elétrica adiada pela ADR-0016.

A correção fecha uma divergência entre o executor atômico e o microsequenciador, mas não fecha E7.7 global. E8 permanece bloqueada até a auditoria semântica integral.

## 7. Validação

- Baseline de origem: `1da784f556b9524f5a6f43ce8a7c525e68354f9e`.
- Testes locais: NOT RUN nesta sessão (sem rustc/cargo no ambiente).
- CI remoto: aguardando execução no HEAD exato da PR.
- Nenhum resultado SKIPPED será classificado como PASS.


## 8. Auditoria complementar exaustiva e fechamento F-024 — 2026-11-09

### Cobertura de todos os quatro opcodes

PR #716 adicionou `native/z80_cpu/tests/ed_ld_i_r_exhaustive.rs` com 4.096 vetores: ED 47 (LD I,A), ED 4F (LD R,A), ED 57 (LD A,I) e ED 5F (LD A,R), para todos os 256 valores de byte, IFF2=0/1 e Carry=0/1. O oracle calcula flags S/Z/PV/H/N/C; o comportamento X/Y derivado do valor carregado é testado conforme a concordância MAME/ares, sem alegar que o UM0080 o especifica.

O teste considera os dois M1 de fetch antes da leitura de R em LD A,R e antes da sobrescrita de R em LD R,A. O microsequenciador recebe WAIT em cada fetch M1 e passa por snapshot encode/decode + restore após cada fronteira. Os tempos são 9T nominais e 11T com dois WAITs.

### Erratum P/V — matriz ampliada

- Microsequenciador: ED 57/5F × IFF2=0/1 × Carry=0/1; INT é afirmada durante a instrução e aceita na fronteira seguinte. PV é comparado com IFF2 antes do aceite e deve ser 0 após o acknowledge; C é preservado.
- Executor atômico SMS: mesma matriz de opcode/IFF2/Carry.
- NMI sem aplicação do erratum e consumo do latch por uma instrução normal intermediária continuam cobertos pelos testes dedicados anteriores.

### CI e integração

PR #716 MERGED por squash em `56d75cc79803494e9fd8a54e8930871c9101fc84`; HEAD validado `9338f8dfee679fee6270c01f9670d6b678d219a0`. CI #6062 / run `42653692191` PASS no HEAD da PR; CI pós-merge #6063 / run `42653949243` PASS no HEAD da main. fmt, Clippy, testes Z80/SMS/SG-1000, SingleStepTests DD/FD e agregador Rust quality/unit PASS. Workspace e subsistemas não relacionados ficaram NOT RUN.

### Classificação e limites

**F-024 CLOSED somente para o perfil de compatibilidade Zilog NMOS clássico.** UM0080, pp. 94–97, documenta os quatro opcodes, 9T (4,5), flags normativas e erratum P/V=0 quando INT ocorre durante LD A,I/R. X/Y não é explicitamente enumerado na descrição de flags do UM0080; a implementação segue a referência comparativa MAME/ares. Não há prova de silício para X/Y, validação física no SMS ou compatibilidade CMOS/clones. Hardware SMS NOT RUN; CMOS/clones NOT_VERIFIED. E7 IN_PROGRESS; E8 BLOCKED.
