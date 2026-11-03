# Auditoria DD/FD — Frente 2: estado arquitetural, flags, interrupções e snapshots

**Data:** 2026-11-03  
**Baseline:** `main` `e4604f3152c09e8d16c37f94f94a4b344db33b62`  
**Schema:** 25  
**Escopo:** estado do microsequenciador Z80 compartilhado e efeitos DD/FD ainda não implementados.  
**Natureza:** inspeção estática de código e testes existentes; nenhum teste foi executado localmente nesta conexão.

## 1. Resumo executivo

O estado serializável do microsequenciador já reserva campos para IX/IY, flags, WZ, Q/previous-Q, R, IFF1/IFF2, IM, EI delay, NMI, INT e HALT. Isso não significa que as respectivas rotinas de serviço estejam implementadas no microsequenciador.

A lacuna determinante para DD/FD é que `Z80MicroSequencerPhase` não contém estados de execução indexada. O campo `opcode: Option<u8>` guarda o prefixo mais recente durante fetches repetidos, mas não representa displacement, subopcode DDCB/FDCB, effective address ou micro-operação de acesso indexado. O sequenciador rejeita o opcode seguinte depois do prefixo DD/FD, com exceção de ED, que cancela o modo indexado.

## 2. Estado presente no DTO e no sequenciador

Em `native/z80_cpu/src/sequencer.rs`, `Z80MicroSequencerState` e `Z80MicroSequencer` contêm:

- Registradores primários e alternativos.
- IX e IY.
- I e refresh R.
- Q e q_previous.
- IFF1, IFF2 e interrupt_mode.
- WZ/MEMPTR.
- ei_delay.
- nmi_pending e int_pending.
- p e halted.
- handshake pendente, phase, opcode, contadores M-cycle/T-state.

O codec binário em `native/z80_cpu/src/snapshot.rs` serializa os campos do DTO e valida a consistência entre fase, request pendente e contadores. O schema permanece 25.

**Conclusão:** os campos arquiteturais já existem, mas o estado de continuação DD/FD não. Não é necessário criar uma segunda cópia de IX/IY ou flags; é necessário adicionar latches/fases específicos de execução indexada quando a implementação começar.

## 3. Flags, Q e WZ

| Item | Evidência no código | Lacuna DD/FD |
|---|---|---|
| S/Z/H/PV/N/C/X/Y | Helpers de ALU e `set_flags` no sequenciador; handlers BASE/ED já atualizam flags | Não há transições indexadas no sequenciador; os helpers não são oráculo independente |
| Q / previous-Q | Campos persistidos; `begin_instruction` captura q_previous e zera Q; `set_flags` atualiza Q | Não há validação de Q para operações IX/IY, indexed memory e DDCB/FDCB |
| WZ/MEMPTR | Campo persistido e atualizado em algumas famílias BASE/ED | Não há regra por família DD/FD nem validação independente para EA/indexed-CB |
| Flags preservadas | Testes existentes verificam preservação em algumas instruções BASE | Não há matriz indexada de flags por opcode e por variante |

Não inferir as flags X/Y de BIT indexed-CB a partir de uma regra genérica de BIT (HL). A política deve ser comprovada por manual/variante e vetor discriminante.

## 4. R, PC e prefixos

O sequenciador incrementa PC e os sete bits inferiores de R ao concluir cada `OpcodeFetch`, preservando R7. Testes existentes demonstram:

- DD/FD reconhecido como prefix fetch M1.
- Prefixos DD/FD repetidos produzem M1 adicionais.
- O último DD/FD fica no campo opcode.
- DD CB é rejeitado após o fetch M1 de CB.

Ainda falta provar, no caminho completo:

- Quantos fetches são M1 em DD/FD CB d op.
- O fetch de displacement não deve ser contado como M1.
- O subopcode DDCB/FDCB deve ser lido como ciclo não-M1.
- R deve refletir somente os M1 efetivos.
- PC deve consumir exatamente os quatro bytes, incluindo wraparound em FFFF.
- Prefixos repetidos/alternados devem manter o prefixo efetivo e o custo temporal correto.

## 5. Interrupções e HALT

A estrutura de estado possui `nmi_pending`, `int_pending`, IFF1/IFF2, IM, EI delay e halted. A inspeção do caminho de execução do microsequenciador não encontrou rotina de serviço de NMI/INT, interrupt acknowledge ou wake-up de HALT. `begin_instruction` inicia um OpcodeFetch normal; o decoder atual contém EI/DI e seleção de IM no subset ED, mas não serviço de interrupções.

O executor atômico `Z80::step()` possui rotinas `service_nmi` e `service_int`, incluindo caminhos IM0/1/2. Esse executor é comparador interno e não comprova que a implementação futura do sequenciador está correta.

**Implicação:** E8 não pode ser marcado como concluído apenas porque os campos são serializados. A sequência de interrupção precisa ser implementada no sequenciador com acknowledge e ciclos de barramento suspensíveis, e validada separadamente para INT, NMI, EI delay, HALT e os três modos.

## 6. Snapshot e replay

O codec atual preserva identidade de request, próximo request ID, WAIT acumulado, fase, opcode, M-cycle/T-state e estado arquitetural exposto. Os testes do codec cobrem safe points de famílias implementadas, inclusive request pendente e fases selecionadas de operações BASE/CB/ED.

Não existe ainda snapshot de uma instrução indexada porque não existem fases indexadas. Ao introduzir E2–E7, cada novo estado deve ser adicionado ao enum, codec, validação de invariantes e testes de round-trip. O teste precisa restaurar e comparar:

1. DTO e fase.
2. Identidade e conteúdo do request pendente.
3. Contagem WAIT.
4. Próxima operação e endereço.
5. Estado arquitetural após cada conclusão.
6. Trace e T-states totais até retirement.

O schema só deve mudar quando os novos latches existirem; qualquer bump parte de 25.

## 7. Entregáveis e status da Frente 2

| Componente | Status |
|---|---|
| Campos arquiteturais IX/IY, flags, WZ, Q/R e interrupções no DTO | IMPLEMENTED |
| Serialização dos campos atuais | IMPLEMENTED |
| Round-trip de snapshots de fases atualmente implementadas | QUICK_PASS conforme testes existentes; execução nesta auditoria NOT RUN |
| Estado de continuação DD/FD | NOT_IMPLEMENTED |
| Flags/WZ/Q para famílias DD/FD | FUNCTIONAL_UNVERIFIED |
| R/PC para DDCB/FDCB | FUNCTIONAL_UNVERIFIED no sequenciador |
| Serviço INT/NMI/HALT no microsequenciador | INTEGRATION_PENDING / NOT_IMPLEMENTED |
| Snapshot/replay de fases DD/FD | NOT_IMPLEMENTED |
| TEMPORAL_PASS | NÃO CONCEDIDO |

## 8. Próximas ações

1. Definir o estado de prefixo efetivo e os latches de displacement/subopcode sem duplicar IX/IY.
2. Especificar transições de Q/WZ/flags por família e manter flags não documentadas explicitamente classificadas.
3. Adicionar snapshot/replay para cada safe point indexado.
4. Tratar INT/NMI/HALT em uma etapa de integração temporal separada; não declarar interrupções completas com base no DTO.
5. Validar por vetores externos fixados e testes discriminantes de wraparound, R, WZ, Q e flags.
