> **Estado vigente (2026-10-27):** o hardware SG-1000 ativo está em Rust. Implementações Python anteriores e ferramentas que dependiam delas foram arquivadas em `legacy/` e não fazem parte do runtime ou da validação ativa. Trechos que descrevem o estado anterior são históricos.

# Especificação documental do Z80 — manuais ZiLOG

**Projeto:** Sega Emulator — Emulação de Alta Precisão  
**Status:** síntese documental inicial, orientada à implementação e aos testes; não representa certificação do núcleo.  
**Data:** 23/10/2026.

## 1. Objetivo e método

Este documento consolida informações relevantes dos três manuais adicionados às fontes do projeto, convertendo-as em requisitos rastreáveis para arquitetura, execução e testes. É uma síntese em palavras próprias; os PDFs integrais não são reproduzidos no repositório.

A fonte principal é **ZiLOG, Z80 CPU User Manual, UM008011-0816 (agosto de 2016)**, arquivo **um0080.pdf**. A edição **Z80 Family CPU User Manual (2001)**, arquivo **z80cpu_um.pdf**, serve como comparação histórica. O **Z80-CPU Technical Manual**, arquivo **Zilog_Z80-CPU_Technical_Manual.pdf**, foi identificado, mas a extração automatizada de texto é incompleta/corrompida; não atribuímos a ele afirmações específicas sem conferência visual.

As páginas citadas são as páginas impressas no manual de 2016, não necessariamente a posição física no PDF. A documentação descreve o Z80 de referência; não generalizar automaticamente para clones ou revisões sem evidências.

## 2. Fontes e qualidade da extração

| Arquivo | Identificação | Extensão | Uso |
|---|---|---:|---|
| um0080.pdf | Z80 CPU User Manual, UM008011-0816 (2016) | 332 páginas | Fonte primária para arquitetura, sinais, temporização, interrupções e instruções. |
| z80cpu_um.pdf | Z80 Family CPU User Manual (2001) | 306 páginas | Comparação histórica; partes da extração apresentam codificação corrompida. |
| Zilog_Z80-CPU_Technical_Manual.pdf | Zilog Z80-CPU Technical Manual, edição histórica | 78 páginas | Fonte histórica potencial para diagramas/sinais; extração textual não confiável em várias páginas. |

## 3. Modelo arquitetural

### 3.1 Registradores

O manual de 2016 descreve 208 bits de armazenamento de registradores acessíveis ao programador (página impressa 2). A organização inclui:
- Conjunto principal: A, F, B, C, D, E, H e L.
- Conjunto alternativo: A', F', B', C', D', E', H' e L'.
- Pares BC, DE e HL, utilizáveis como pares de 16 bits ou registradores de 8 bits.
- Registradores especiais: PC, SP, IX, IY, I e R.
- IFF1/IFF2 e modo de interrupção como estado de controle da CPU.

Implicações para a implementação:
1. EX AF,AF' troca A/F com o conjunto alternativo; EXX troca BC/DE/HL com seus pares alternativos. Testar todos os valores e não modelar a troca como cópia parcial.
2. PC avança durante a busca dos bytes e pode ser substituído por saltos, chamadas, retornos e interrupções. Testar comprimento da instrução e PC final.
3. SP é um endereço de 16 bits para pilha em memória externa. PUSH/CALL/RST/interruptos escrevem; POP/RET/RETN/RETI leem. Testar ordem dos bytes, wraparound e SP final.
4. IX/IY são bases de endereçamento indexado. O deslocamento é um byte com sinal em complemento de dois. Testar 00, 7F, 80 e FF, além do wraparound de endereço.
5. I fornece o byte alto do ponteiro usado por IM 2; o dispositivo fornece o byte baixo. A tabela de vetores é lida em posições adjacentes.
6. R é o registrador de refresh. O manual afirma que sete bits são incrementados automaticamente após cada busca de instrução e que o bit 7 permanece conforme programado por LD R,A (p. 3). Preservar o bit 7 e testar R separadamente; não tratá-lo como contador simples de oito bits sem validar os eventos e a variante.
7. LD A,I e LD A,R transferem o valor para A e refletem IFF2 na flag P/V, conforme a seção de interrupções (p. 18) e as descrições individuais.

### 3.2 ALU e famílias

A ALU de 8 bits é descrita como responsável por adição, subtração, AND, OR, XOR, comparação, shifts/rotações, incremento/decremento e operações SET/RES/BIT (p. 4). Para cada instrução, separar resultado, flags, efeitos em registradores/memória, bytes consumidos, PC final, T-states nominais e sequência de acessos ao barramento. Acertar o acumulador não basta se flags, PC, memória ou temporização estiverem errados.

## 4. Temporização e barramento

### 4.1 T-states e M-cycles

A seção Timing (pp. impressas 7–16) descreve instruções como sequências de operações básicas. Os períodos de clock são T-cycles/T-states; as operações básicas são machine cycles/M-cycles. Operações típicas incluem leitura/escrita de memória, leitura/escrita de I/O e acknowledge de interrupção.

O manual descreve:
- O primeiro ciclo M1 de uma instrução é o fetch de opcode, com 4, 5 ou 6 T-states conforme a operação, salvo extensão por WAIT.
- Ciclos comuns de leitura/escrita de memória geralmente duram 3 T-states antes de wait states.
- Estados WAIT podem alongar operações para sincronizar o CPU com dispositivos externos.
- O diagrama de I/O inclui um wait state automático para permitir decodificação do porto e amostragem de WAIT (p. 10).
- Durante T3/T4 do fetch há atividade de refresh. R contribui com o endereço de refresh e RFSH sinaliza a operação. O manual ressalta que MREQ deve participar da operação e que RFSH isoladamente não garante endereço estável durante todo o intervalo (pp. 8–9).

Não inferir o timing de toda instrução a partir de um fetch genérico: cada opcode tem sua própria sequência de ciclos.

### 4.2 Sinais documentados

O manual descreve CLK, A0–A15, D0–D7, MREQ, IORQ, RD, WR, M1, RFSH, WAIT, HALT, INT, NMI, RESET, BUSRQ e BUSACK. O núcleo Rust pode abstrair pinos quando isso for sustentado pelo modelo inicial, mas a arquitetura deve permitir representar seus efeitos temporais relevantes.

- MREQ/RD/WR: seleção e controle de leitura/escrita de memória.
- IORQ: ciclo de entrada/saída e participação em sequências de acknowledge.
- M1: identifica o fetch de opcode; não é sinônimo de qualquer leitura de memória.
- RFSH: sinaliza atividade de refresh.
- WAIT: pode inserir estados de espera em ciclos aplicáveis.
- BUSRQ/BUSACK: solicitação e reconhecimento de controle do barramento, relevantes para dispositivos que assumem o barramento.
- HALT: estado de parada do fluxo normal enquanto se aguarda interrupção.
- RESET: reinicializa estados definidos pelo hardware, incluindo IFFs e modo de interrupção conforme o manual.

### 4.3 Consequência arquitetural

A auditoria existente descreve o núcleo como executor de uma instrução por chamada, retornando T-states nominais e realizando acessos de barramento durante a execução da instrução. Isso é útil como etapa funcional, mas **não é emulação ciclo a ciclo**.

Evolução recomendada:
1. Preservar API funcional e testes existentes.
2. Introduzir um traço determinístico de eventos por instrução: fetch, leitura, escrita, I/O, acknowledge e refresh quando modelado.
3. Associar eventos a M-cycles/T-states e permitir WAIT de forma explícita.
4. Comparar traços e contagens com diagramas do manual e testes de referência.
5. Integrar contenção e temporização específica do sistema depois de documentar a máquina SG-1000.

Não declarar precisão por ciclo antes de existir sequência temporal explícita e testes de traço.

## 5. Interrupções e controle

### 5.1 INT, NMI, IFF1 e IFF2

A seção Interrupt Response (pp. 17–20) descreve INT mascarável e NMI não mascarável.
- Reset força IFF1=0 e IFF2=0.
- DI limpa ambos os flip-flops.
- EI habilita ambos, mas uma solicitação INT pendente só pode ser aceita depois da execução da instrução seguinte a EI.
- Ao aceitar INT mascarável, IFF1 e IFF2 são limpos.
- Ao aceitar NMI, IFF1 é limpo e o estado anterior de habilitação fica preservado em IFF2.
- RETN restaura IFF1 a partir de IFF2.
- LD A,I e LD A,R copiam o estado de IFF2 para P/V.

Testes exigidos: reset; DI/EI; INT pendente durante EI e instrução seguinte; NMI com IFF previamente ligado/desligado; RETN; leitura de I/R após NMI; HALT interrompido; sequência de interrupções aninhadas.

### 5.2 NMI e modos de INT

O manual afirma que NMI é aceito independentemente da habilitação de INT e leva o fluxo ao endereço 0066h (p. 19). A implementação deve preservar o endereço de retorno e empilhá-lo na ordem correta.

- **IM 0:** o dispositivo coloca no barramento de dados a instrução a executar. RST é comum, mas outras instruções são possíveis. O manual descreve dois wait states adicionais na resposta de interrupção para permitir prioridade externa por daisy chain (p. 19).
- **IM 1:** a resposta efetua restart no endereço 0038h. O manual também indica dois wait states adicionais.
- **IM 2:** I fornece o byte alto do ponteiro e o dispositivo fornece o byte baixo; o ponteiro acessa uma tabela de endereços de serviço. O byte baixo do ponteiro deve produzir endereço par, pois o endereço de serviço é composto por dois bytes adjacentes (pp. 19–20).

Contar apenas T-states agregados não valida acknowledge, origem do opcode em IM 0, incremento de R, estados de espera nem sequência do barramento. A auditoria existente já aponta IM 0 e R durante interrupções como áreas que precisam de validação.

### 5.3 HALT, RETI e prioridade externa

HALT suspende o fluxo normal até um evento de interrupção; o manual inclui diagramas de saída de HALT (pp. 14–15). Não tratar HALT como retorno de zero ciclos sem modelar o comportamento documentado.

RETI e RETN não devem ser confundidos: RETN restaura IFF1 de IFF2; RETI tem papel de sinalização de fim de serviço para sistemas com prioridade por daisy chain. O núcleo deve expor um evento/gancho para RETI quando o sistema precisar observar a instrução. Confirmar o comportamento elétrico específico na fonte aplicável antes de modelar pinos detalhados.

## 6. Flags: regras específicas por instrução

A seção Z80 Status Indicator Flags (pp. 65–69) mostra que F não deve ser calculado por uma única regra genérica.

- **S (Sign):** em operações aplicáveis, reflete o bit mais significativo do resultado; em valores de 8 bits, bit 7 representa o sinal em complemento de dois.
- **Z (Zero):** depende da instrução. Aritmética/lógica, comparação, BIT e I/O de bloco possuem regras próprias.
- **H (Half Carry):** carry/borrow entre bits 3 e 4; participa de DAA.
- **P/V (Parity/Overflow):** em aritmética indica overflow assinado; em certas operações lógicas/rotações indica paridade par; em instruções de bloco acompanha condição do contador; em LD A,I/LD A,R reflete IFF2.
- **N (Add/Subtract):** distingue adição de subtração para DAA; N=0 em adições e N=1 em subtrações, conforme o manual.
- **C (Carry):** representa carry/borrow conforme a operação; várias instruções têm efeitos específicos.

Para cada instrução, manter máscara explícita de flags alteradas/preservadas e testar flags de entrada, não apenas a saída.

**Bits X/Y (bits 3 e 5):** comportamentos pouco documentados ou dependentes de valores internos não devem ser inferidos apenas deste resumo. O repositório já possui pesquisa específica de comportamentos não documentados; mantê-la separada e rastrear evidência por variante. Não afirmar conformidade X/Y completa com base apenas no manual.

Casos críticos:
1. ADD/ADC e SUB/SBC/CP: testar carry, half-carry e overflow assinado com fronteiras distintas.
2. INC/DEC: verificar preservação de C e regras de H/PV.
3. AND/OR/XOR: não reutilizar fórmulas aritméticas.
4. DAA: testar combinações de N/H/C e valores de A, incluindo estados intermediários.
5. BIT: testar Z/H/N/C e X/Y por registrador, memória e endereçamento indexado.
6. LDI/LDD/LDIR/LDDR e CPI/CPD/CPIR/CPDR: flags e repetição dependem de BC, comparação e condição de término.
7. IN r,(C), INI/IND/INIR/INDR e OUTI/OUTD/OTIR/OTDR: regras de flags distintas da ALU comum.

## 7. Endereçamento, opcodes e famílias

O manual enumera modos de endereçamento (pp. 34–39): imediato, imediato estendido, página zero modificada (RST), relativo, estendido, indexado, registrador, implícito, indireto por registrador, bit e combinações de modos.

A tabela de opcodes começa na p. 40 e organiza load/exchange, transferência/busca de bloco, aritmética/lógica, rotações/shifts, manipulação de bits, jump/call/return, I/O e controle da CPU. As descrições individuais começam na p. 69 e apresentam operandos, efeitos e flags.

A tabela de opcodes é inventário, não prova de correção semântica. Cada combinação opcode/prefixo deve ser classificada como:
1. documentada, implementada e testada;
2. implementada mas insuficientemente validada;
3. documentada e ausente;
4. alias/NOP/forma redundante documentada;
5. comportamento não documentado que exige fonte experimental independente;
6. comportamento específico de variante;
7. não suportada deliberadamente, com exceção explícita e teste.

Prefixos CB, ED, DD, FD e DD/FD-CB merecem matriz própria, incluindo prefixos repetidos e bytes consumidos durante acknowledge de INT.

## 8. Operações de bloco e I/O

O índice do manual separa transferências e buscas de bloco (pp. 47–49; descrições individuais a partir de p. 130) e I/O (capítulo iniciado na p. 61; instruções de I/O de bloco nas pp. 298 e seguintes).

Famílias a auditar:
- Transferência: LDI, LDIR, LDD, LDDR.
- Busca/comparação: CPI, CPIR, CPD, CPDR.
- Entrada de bloco: INI, INIR, IND, INDR.
- Saída de bloco: OUTI, OTIR, OUTD, OTDR.

Por iteração, verificar ordem de acessos, atualização de HL/DE/BC ou B, condição de repetição, PC quando a repetição continua, flags e ciclos. As formas repetitivas não devem ocultar efeitos intermediários se a arquitetura precisar respeitar interrupções e temporização por iteração.

O manual descreve P/V como indicador do contador de bytes para instruções de transferência/busca de bloco e dá regras específicas de Z para instruções de I/O de bloco. Consultar a descrição individual do opcode para a regra exata.

## 9. Plano de testes derivado

Estes são critérios propostos; não declaram que os testes já existam ou tenham sido executados.

### P0 — correção funcional determinística
- Estado inicial/reset: PC, SP, I, R, IFF1/IFF2 e IM.
- Registradores principais/alternativos e instruções de troca.
- PC: bytes de opcode, operandos, saltos relativos e wraparound.
- SP: PUSH/POP/CALL/RET/RST e ordem de bytes.
- Aritmética de fronteira para S/Z/H/PV/N/C.
- Endereçamento indexado com deslocamentos positivos e negativos.
- Todas as famílias de opcode classificadas, com exceções explícitas.

### P1 — interrupções e efeitos arquiteturais
- EI delay com INT pendente.
- DI, NMI e RETN; IFF1/IFF2.
- IM 0 com instruções de 1, 2 e 3 bytes e prefixos; origem de bytes e PC.
- IM 1 em 0038h; IM 2 tabela de ponteiros e leitura adjacente.
- HALT com e sem interrupção.
- R em fetch normal, prefixos, HALT e respostas a interrupções.
- RETI observável por controlador externo quando necessário.

### P1 — temporização e barramento
- Traço de M1/fetch, leitura/escrita de memória e I/O.
- WAIT e wait states automáticos aplicáveis.
- Acknowledge de INT/NMI e estados adicionais documentados.
- Refresh/RFSH e endereços relacionados a R/I quando o modelo temporal for introduzido.
- Conferência tanto dos T-states totais quanto da ordem dos eventos.

### P2 — validação diferencial e longa
- Comparar estado completo por instrução contra suíte independente, registrando divergência e variante.
- Executar testes longos no Google Colab; manter testes curtos determinísticos localmente.
- Salvar seed, estado inicial, bytes de programa, traço de eventos e primeiro ponto de divergência.
- Não aceitar somente sucesso de ROM/homebrew como evidência de conformidade abrangente.

## 10. Relação com o SG-1000

O SG-1000 é o primeiro alvo, mas a CPU não pode ser considerada validada apenas por iniciar ou executar uma demonstração. O núcleo deve manter contratos de memória e I/O claros e temporização determinística. Mapa de portas, VDP, PSG e integração da máquina pertencem aos componentes do sistema, não à CPU isoladamente.

Prioridade:
1. estabilizar semântica funcional e flags das instruções usadas;
2. validar ciclos nominais e interrupções;
3. desenhar eventos de barramento que permitam integrar dispositivos no tempo lógico;
4. adicionar homebrew e testes sintéticos legalmente obtidos;
5. avaliar compatibilidade integrada somente depois.

Não assumir que timings de Master System/Game Gear se aplicam automaticamente ao SG-1000.

## 11. Diferenças e incertezas entre fontes

- O manual de 2016 tem índice extenso e extração utilizável; é a primeira referência desta matriz.
- O manual de 2001 é útil como comparação, mas a extração contém caracteres corrompidos em partes. Conferir no PDF original antes de decidir diferenças.
- O manual histórico de 78 páginas apresentou erros na extração automatizada. Diagramas de timing podem ser úteis, mas exigem inspeção visual dirigida.
- Comportamentos não documentados, X/Y em casos específicos, Q/WZ/MEMPTR e particularidades de clones/revisões exigem fontes independentes e testes. Este documento não afirma que os três manuais definam completamente esses comportamentos.
- Em divergências, registrar texto/página de cada fonte, variante, hipótese e teste discriminante. Não escolher silenciosamente uma interpretação.

## 12. Referências

1. ZiLOG, **Z80 CPU User Manual**, UM008011-0816, agosto de 2016, 332 páginas. Arquivo de fonte: um0080.pdf. Seções: Architectural Overview (pp. 1–4), Pin Functions (pp. 5–6), Timing (pp. 7–16), Interrupt Response (pp. 17–20), Addressing Modes (pp. 34–39), tabela de opcodes (a partir da p. 40), flags (pp. 65–69) e descrições individuais (a partir da p. 69).
2. ZiLOG, **Z80 Family CPU User Manual**, 2001, 306 páginas. Arquivo: z80cpu_um.pdf. Comparação histórica; validar diretamente no PDF quando a extração estiver corrompida.
3. ZiLOG, **Z80-CPU Technical Manual**, edição histórica, 78 páginas. Arquivo: Zilog_Z80-CPU_Technical_Manual.pdf. Extração automática insuficiente; consultar visualmente antes de citar detalhes.

## 13. Próxima ação

Usar este documento para atualizar, em etapas pequenas, docs/z80-coverage.md, docs/z80-code-audit-2026-10-22.md e os testes do núcleo. Não copiar tabelas inteiras dos manuais: manter matriz de conformidade própria com opcode, requisito, página-fonte, estado de implementação, teste, resultado e variante.
