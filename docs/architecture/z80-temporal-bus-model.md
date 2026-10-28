# Contrato temporal do barramento Z80 — Fase 10

**Estado:** contrato arquitetural proposto; instrumentação de runtime pendente.  
**Base auditada:** main em 05cb8259dfaaea5844c3290945301c88ec0fdc54.  
**Escopo:** modelo determinístico de T-states, ciclos de máquina, acessos ao barramento, waits e eventos de interrupção.  
**Não declarar:** TEMPORAL_PASS até que os traces sejam produzidos pelo runtime e comparados com uma referência fixada.

## 1. Diagnóstico do contrato atual

O núcleo SMS mantém Z80::total_tstates e retorna uma contagem agregada por chamada de step. A implementação executa leituras e escritas através de CpuBus, mas o trait em native/z80_bus/src/lib.rs só define leitura/escrita de memória e portas; ele não descreve M-cycles, amostras de barramento por T-state, waits ou timestamps. Portanto, a sequência de acessos em código não equivale a um trace temporal validado.

O teste externo raxoft/z80test e os vetores SingleStepTests usados atualmente continuam valiosos para semântica e contagem agregada. Uma duração correta não demonstra que endereço, dados e sinais foram apresentados nos instantes corretos.

## 2. Base técnica documentada

A referência principal é o manual ZiLOG **Z80 CPU User Manual, UM008011-0816 (08/2016)**:

- **pp. 7–10, seção Timing:** diferencia T-cycles de M-cycles; o fetch M1 normalmente ocupa quatro T-states; leituras/escritas de memória normalmente ocupam três; waits podem estender ciclos; I/O insere um wait state automático no comportamento descrito pelo manual.
- **p. 130, LDI:** 16 T-states com decomposição (4, 4, 3, 5).
- **p. 132, LDIR:** 21 T-states durante repetição, (4, 4, 3, 5, 5); termina em 16 T-states quando o contador chega a zero.
- **p. 138, CPI:** 16 T-states com (4, 4, 3, 5).
- **pp. 139–140, CPIR:** 21 T-states durante repetição e 16 ao terminar; o manual também descreve reconhecimento de interrupções e ciclos de refresh após a operação.

Esses valores são contratos documentados para o Z80 descrito pelo manual. Wait states, decodificação de portas, contenção e integração com dispositivos precisam ser modelados conforme o alvo; não se devem inferir comportamentos específicos do SMS apenas a partir da contagem nominal.

O corpus [SingleStepTests/Z80](https://github.com/SingleStepTests/z80/tree/ebe1875d48f374bcfd4b505d8eb8ee751568b5f7) fixado no SHA ebe1875d48f374bcfd4b505d8eb8ee751568b5f7 fornece amostras de barramento em cycles. O README descreve endereço, dados e sinais por amostra; valores nulos devem permanecer distintos de zero. O corpus tem ancestralidade no núcleo Z80 do Ares, portanto não é evidência independente de hardware.

## 3. Modelo de dados proposto

A implementação deve manter duas vistas relacionadas e derivadas do mesmo avanço temporal.

### 3.1 Amostra por T-state

Cada amostra determinística deve conter, no mínimo:

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
- endereço/dados aplicáveis;
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
7. **Instrumentação opcional:** a coleta de trace pode ser desativada no caminho normal, sem alterar estado arquitetural, contagem de ciclos ou comportamento emulado.
8. **Primeira divergência:** comparações devem apontar o primeiro T-state/evento divergente, não somente o hash final ou a duração total.

## 5. Plano de implementação incremental

1. Adicionar tipos de trace e contrato de observação em native/z80_bus, sem afirmar que todos os caminhos já emitem eventos.
2. Instrumentar fetch M1 e acessos de memória com uma única linha temporal, mantendo um modo de trace reproduzível em testes.
3. Instrumentar as oito instruções ED de transferência/comparação como primeira fatia: LDI/LDD/LDIR/LDDR e CPI/CPD/CPIR/CPDR. Usar as decomposições documentadas acima e comparar amostras externas disponíveis; registrar limitações quando a referência não cobre um caso.
4. Instrumentar progressivamente CB, ED, DD/FD, indexed-CB, I/O e acesso à pilha, incluindo prefixos ignorados/repetidos, HALT e entrada de interrupções.
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
