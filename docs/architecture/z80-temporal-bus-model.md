# Contrato temporal do barramento Z80 — Fase 10

**Estado:** contrato definido; trace M-cycle-level e amostras por T-state implementados para as oito instruções ED de transferência/comparação sem prefixos DD/FD. A amostragem cobre endereço/dados e RD/WR/MREQ/IORQ, com fase de refresh inferida; waits, todos os pinos Z80 e scheduler permanecem pendentes.  
**Base auditada originalmente:** main em `05cb8259dfaaea5844c3290945301c88ec0fdc54`; consulte a atualização pós-merge ao final para o estado corrente.  
**Escopo:** modelo determinístico de T-states, ciclos de máquina, acessos ao barramento, waits e eventos de interrupção.  
**Não declarar:** TEMPORAL_PASS até que os traces sejam produzidos pelo runtime e comparados com uma referência fixada.

## 1. Diagnóstico do contrato atual

O núcleo SMS mantém Z80::total_tstates e retorna uma contagem agregada por chamada de step. O trait CpuBus agora oferece um observador opcional de eventos temporais lógicos. A primeira fatia emite eventos M-cycle-level e amostras por T-state para as oito instruções ED de transferência/comparação sem prefixos DD/FD. As amostras incluem endereço/dados e RD/WR/MREQ/IORQ; a fase de refresh é inferida e RFSH/M1/WAIT não são modelados como pinos independentes. O trace não controla o scheduler.

O teste externo raxoft/z80test e os vetores SingleStepTests usados atualmente continuam valiosos para semântica e contagem agregada. Uma duração correta não demonstra que endereço, dados e sinais foram apresentados nos instantes corretos.

## 2. Base técnica documentada

A referência principal é o manual ZiLOG **Z80 CPU User Manual, UM008011-0816 (08/2016)**:

- **pp. 7–10, seção Timing:** diferencia T-cycles de M-cycles; o fetch M1 normalmente ocupa quatro T-states; leituras/escritas de memória normalmente ocupam três; waits podem estender ciclos; I/O insere um wait state automático no comportamento descrito pelo manual.
- **p. 130, LDI:** 16 T-states com decomposição (4, 4, 3, 5).
- **p. 132, LDIR:** 21 T-states durante repetição, (4, 4, 3, 5, 5); termina em 16 T-states quando o contador chega a zero.
- **p. 138, CPI:** 16 T-states com (4, 4, 3, 5).
- **pp. 139–140, CPIR:** 21 T-states durante repetição e 16 ao terminar; o manual também descreve reconhecimento de interrupções e ciclos de refresh após a operação.

Esses valores são contratos documentados para o Z80 descrito pelo manual. Wait states, decodificação de portas, contenção e integração com dispositivos precisam ser modelados conforme o alvo; não se devem inferir comportamentos específicos do SMS apenas a partir da contagem nominal.

O corpus [SingleStepTests/Z80](https://github.com/SingleStepTests/z80/tree/ebe1875d48f374bcfd4b505d8eb8ee751568b5f7) fixado no SHA ebe1875d48f374bcfd4b505d8eb8ee751568b5f7 fornece amostras de barramento em cycles. O README descreve endereço, dados e RD/WR/MREQ/IORQ; valores nulos devem permanecer distintos de zero. A configuração do corpus usa acessos de memória simplificados e põe valores de refresh no barramento de endereço durante M1, portanto os testes demonstram conformidade com essa configuração, não fidelidade independente a hardware. O corpus tem ancestralidade no núcleo Z80 do Ares. O manual ZiLOG UM008011-0816, figura 5, descreve transições de MREQ/RD em meia fase de clock e MREQ/RFSH durante refresh; esse detalhe temporal não é representado pelo perfil simplificado atual e precisa de um modelo de transições sub-T-state.

## 3. Modelo de dados proposto

A implementação deve manter duas vistas relacionadas e derivadas do mesmo avanço temporal.

### 3.1 Amostra por T-state

A primeira fatia implementa os campos abaixo para as oito instruções ED cobertas; a extensão para outros opcodes e pinos ainda é pendente. Cada amostra determinística deve conter, no mínimo:

- índice de T-state relativo à instrução e contador absoluto do núcleo;
- índice do M-cycle e posição dentro do M-cycle, quando aplicável;
- endereço do barramento como valor opcional;
- dados do barramento como valor opcional;
- sinais normalizados: MREQ, IORQ, RD, WR, M1, RFSH, HALT e WAIT, quando definidos para aquela amostra;
- origem/fase: fetch, leitura/escrita de memória, I/O, acknowledge de interrupção, refresh, wait ou estado interno.

Ausência de endereço/dados (None) não pode ser convertida em zero: a fonte de referência usa valores nulos para estados em que o barramento não apresenta um valor definido.

### 3.2 Transação/ciclo de máquina

Cada M-cycle deve poder ser reconstruído com:

- tipo de ciclo;
- T-state inicial e duração;
- endereço/dados aplicáveis; ciclos internos podem manter o último endereço observado, que deve ser preservado quando a referência o define;
- wait states efetivamente inseridos;
- relação com a instrução e com o evento que o iniciou.

O trace de M-cycles não substitui as amostras por T-state quando o critério exige comparação de sinais dentro do ciclo.

## 4. Invariantes arquiteturais

1. **Tempo determinístico:** nenhuma decisão temporal depende de relógio de parede, FPS do host, scheduler do sistema operacional ou velocidade de renderização.
2. **Uma única fonte de avanço:** CPU, barramento e dispositivos compartilham um contador lógico coerente; não somar a duração completa ao final e, ao mesmo tempo, avançar os mesmos T-states durante a execução.
3. **Acesso e tempo inseparáveis:** toda leitura/escrita observável tem posição temporal definida; não reconstruir a ordem de barramento apenas a partir do opcode e do total agregado.
4. **Waits explícitos:** waits aumentam a duração e aparecem no trace; não podem ser ocultados dentro de um total final.
5. **Interrupções como eventos temporais:** NMI, INT, acknowledge, fetches, refresh e acessos à pilha precisam de ordem observável e reproduzível.
6. **Repetição correta:** LDIR/LDDR/CPIR/CPDR devem produzir traces por iteração, inclusive quando BC inicial é zero e sofre wrap para FFFF.
7. **Instrumentação opcional:** a coleta de trace exige opt-in do barramento; por padrão não gera eventos nem amostras e não altera estado arquitetural, contagem de ciclos ou comportamento emulado.
8. **Primeira divergência:** comparações devem apontar o primeiro T-state/evento divergente, não somente o hash final ou a duração total.

## 5. Plano de implementação incremental

1. Implementado: tipos de trace e observador opcional em native/z80_bus; o callback padrão não altera o runtime.
2. Implementado parcialmente: eventos M-cycle-level e amostras por T-state para LDI/LDD/LDIR/LDDR e CPI/CPD/CPIR/CPDR sem prefixos DD/FD; a fase refresh é inferida, não um sinal RFSH observado.
3. Implementado parcialmente: oito traces de amostras são comparados com vetores SingleStepTests/Z80 fixados, incluindo endereço/dados e RD/WR/MREQ/IORQ em cada T-state. O README do corpus configura acessos de memória simplificados e refresh nos pinos de endereço; a comparação demonstra conformidade com esse corpus/configuração, não validação independente de hardware. Pendente: WAIT, pinos M1/RFSH explícitos, outros opcodes/prefixos, interrupções e integração com scheduler.
4. Instrumentar progressivamente CB, demais ED, DD/FD, indexed-CB, I/O e acesso à pilha, incluindo prefixos ignorados/repetidos, HALT e entrada de interrupções.
5. Integrar waits e avanço dos dispositivos ao scheduler do SMS, sem callbacks Python por instrução.
6. Executar regressões de semântica e temporização, z80full/z80ccf, comparação de traces e testes de determinismo no SHA candidato exato.
7. Após integração, repetir os checks pós-merge. Checks ausentes ou resultados agregados isolados não constituem aprovação temporal.

## 6. Gate de aceitação para TEMPORAL_PASS

TEMPORAL_PASS só poderá ser declarado quando:

- o runtime produzir traces ordenados e reproduzíveis por T-state;
- os eventos de memória, I/O, refresh, waits e interrupções relevantes tiverem contratos explícitos;
- houver comparação automatizada contra traces fixados, com a primeira divergência relatada;
- cada diferença for corrigida ou classificada com evidência, sem descartar falhas para obter um resultado verde;
- a suíte de regressão e os workflows aplicáveis passarem no SHA candidato exato;
- os testes de integração demonstrarem que a instrumentação temporal não altera o estado funcional quando desativada.

O fechamento funcional das instruções de bloco e os testes externos de instrução não substituem este gate. Até que os itens acima sejam demonstrados, o estado permanece INTEGRATION_PENDING; TEMPORAL_PASS não é reivindicado.

## 7. Referências

- ZiLOG, *Z80 CPU User Manual*, UM008011-0816, 08/2016, pp. 7–10, 130, 132, 138–140.
- SingleStepTests/Z80, corpus fixado em ebe1875d48f374bcfd4b505d8eb8ee751568b5f7, README e arquivos de traces sob v1/.
- native/z80_bus/src/lib.rs e native/sms_core/src/cpu.rs no baseline identificado no cabeçalho.


## Atualização de estado — pós-merge #227

A Fase 10 descrita acima deve ser lida como uma fatia limitada, não como modelo temporal completo. A PR #227 integrou trace por T-state para oito instruções ED de transferência/comparação sem DD/FD, com endereço/dados e RD/WR/MREQ/IORQ no perfil definido pelo teste. O trace é passivo: não avança o scheduler, não injeta wait states e não modela todos os pinos Z80 nem as transições sub-T-state do manual.



## Estado consolidado após as PRs #227–#229 — 2026-10-29

**SHA de `main` verificado após os merges:** `a662b7256ed5b92d06fd33875e4031b932e25945`.  
**CI da `main` nesse SHA:** PASS — [workflow 40792001983](https://github.com/leonardo201800478/sega-emulator/actions/runs/40792001983).  
**Limite:** o CI verde comprova apenas os checks executados pelo workflow; não certifica completude global do Z80 nem fidelidade física/ciclo a ciclo.

### Mudanças integradas

- **PR #227**, candidato `d4447db5f0dce9a2ecde9e50c97f39b10ce8da17`, merge `28da159634aa60b9a804f3bd7f781ccf82f3e1cf`: trace por T-state para LDI/LDD/LDIR/LDDR e CPI/CPD/CPIR/CPDR sem prefixos DD/FD, além de trace M-cycle associado. Validação de plataforma no candidato: [run 40782245521](https://github.com/leonardo201800478/sega-emulator/actions/runs/40782245521) — PASS.
- **PR #229**, candidato `56cf4743bf0f05660f21a798de72e19afcfab34a`, merge `7b6fef3300c753c2b089f6b0dd287f8120b9f93f`: testes determinísticos para JR, JP, CALL, RET e DJNZ condicionais, caminhos tomados/não tomados, fronteiras de deslocamento, wraparound de PC/SP, ordem de acesso à pilha e T-states nominais; correção de `RET cc` para atualizar WZ/MEMPTR com o endereço de retorno somente quando a condição é tomada. [Fast Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40791517040), [CI](https://github.com/leonardo201800478/sega-emulator/actions/runs/40791516982) e [SMS Z80 Platform Validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40791516943) — PASS no candidato exato.
- **PR #228**, candidato documental `a2c0850c20232c95a5210a39fbd53cc7aeb5101e`, merge `a662b7256ed5b92d06fd33875e4031b932e25945`: auditoria de continuidade incorporada; [CI documental](https://github.com/leonardo201800478/sega-emulator/actions/runs/40791436529) — PASS.

### Estado técnico depois dos merges

| Área | Estado | Limite/evidência |
|---|---|---|
| Executor Z80 Rust em `native/sms_core` | IMPLEMENTED | Caminho ativo consolidado; não significa que todos os estados de todas as instruções estejam provados. |
| Testes de controle de fluxo condicional | IMPLEMENTED; SHORT_PASS no candidato #229 | Cobertura adicionada às famílias JR/JP/CALL/RET/DJNZ e aos limites declarados; não é prova combinatória de todos os estados. |
| Trace ED de bloco | IMPLEMENTED; SHORT_PASS no candidato #227 | Restrito às oito instruções indicadas e ao perfil de observação documentado. |
| Classificação dos espaços de opcode | IMPLEMENTED | Enumeração/classificação não equivale à correção semântica integral. |
| Semântica global, flags não documentadas, Q/P, WZ, interrupções e aliases | INCOMPLETO / auditoria contínua | Exigir vetores discriminantes e proveniência explicitada por família. |
| Integração temporal com scheduler/dispositivos SMS | INTEGRATION_PENDING | A instrumentação atual não controla o scheduler nem representa todos os pinos/esperas. |
| Fidelidade ciclo a ciclo integral | TEMPORAL_PASS NÃO DECLARADO | Não há evidência para afirmar cobertura temporal completa ou equivalência física. |

### Proveniência e regras de interpretação

O corpus SingleStepTests/Z80 fixado em `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7` tem ancestralidade no núcleo Z80 do Ares; é útil para regressão, mas não é um oráculo independente de hardware. Separar sempre comportamento descrito pelo manual ZiLOG, comportamento documentado por referências de terceiros, observação em emuladores e hipótese ainda não comprovada. Não promover um estado de validação com base apenas no número de opcodes enumerados ou no CI geral.


**Gate temporal ainda aberto:** ampliar o modelo a outras instruções somente com critérios e vetores fixos; diferenciar amostras de barramento de T-states nominais; incluir waits, refresh/M1, I/O, acknowledge de interrupção e ordenação de eventos quando o contrato e as fontes estiverem definidos; testar que a instrumentação desativada não altera o estado funcional. Não declarar `TEMPORAL_PASS` nesta fase.

## 8. Diagnóstico arquitetural por instrução

A instrumentação opcional `Z80InstructionTrace` registra um JSONL determinístico por chamada externa de `Z80::step`: índice sequencial desde reset, bytes de opcode/prefixo (até oito, com indicador de truncamento), resultado, T-states agregados e snapshots antes/depois de AF/BC/DE/HL, bancos alternativos, IX/IY, PC/SP, I/R, IFF1/IFF2, modo de interrupção, WZ, atraso de EI, pedidos de interrupção, Q/P, HALT e contador absoluto de T-states. Erros `UnsupportedOpcode` são registrados como resultado explícito.

O coletor é opt-in via `CpuBus::instruction_trace_enabled`; sem opt-in, o núcleo não formata JSON nem chama o callback de diagnóstico. O host deve persistir cada linha em arquivo local e manter limites de tamanho/rotação; logs podem crescer rapidamente e não devem entrar no Git, especialmente se incluírem conteúdo de ROM. O registro de instrução não substitui os eventos M-cycle/T-state existentes, cuja cobertura continua restrita às oito instruções ED documentadas acima.

**Limites deliberados:** o registro não é um trace de todos os acessos de memória/I/O e não atribui um offset T-state a cada acesso. Não declarar precisão temporal global. Repetições de PC/estado e ciclos agregados poderão ser analisados offline para localizar loops; um estado repetido não prova sozinho um loop incorreto. Os ED não atribuídos documentados como NOP devem ser classificados como comportamento definido, não como opcode ausente. O callback do núcleo oferece a interface de coleta; a integração do sink de arquivo ao caminho de execução real do SMS e a campanha de testes in-game permanecem `INTEGRATION_PENDING`.
