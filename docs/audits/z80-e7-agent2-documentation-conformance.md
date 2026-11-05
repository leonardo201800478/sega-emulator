# E7 — Agente 2: conformidade com documentação primária e fidelidade ao hardware

**Data:** 2026-11-04  
**Documento primário analisado:** ZiLOG, *Z80 CPU User Manual*, UM008011-0816 (arquivo fornecido `um0080.pdf`).  
**Documentação SMS:** SMS Power! — SMS Cartridge, Cartridge Slot, VDP Registers, Control Port e SMS Official Docs.  
**Regra:** este relatório separa o que o fabricante documenta do que é inferência arquitetural. Quando o manual não especifica a fase elétrica exata, a lacuna é declarada.

## 1. Resumo das conclusões documentais

1. WAIT é amostrado na falling edge do clock durante T2 e em cada TW automático subsequente. WAIT ativo nessa amostra provoca outro TW.
2. Um ciclo de I/O insere automaticamente um TW entre T2 e T3; durante esse estado a entrada WAIT é amostrada.
3. O acknowledge de INT é um M1 especial com IORQ ativo no lugar de MREQ e dois TW automáticos para a cadeia de prioridade.
4. INT é amostrado na rising edge do último clock da instrução; a aceitação é bloqueada se IFF1 estiver desabilitado ou BUSREQ estiver ativo.
5. NMI é negative-edge-triggered, tem prioridade sobre INT e é reconhecido ao fim da instrução, independentemente de IFF.
6. Em HALT, INT e NMI são amostrados na rising edge durante T4 de cada ciclo de retenção; o próximo rising edge encerra HALT e o ciclo seguinte reconhece a interrupção aceita.
7. BUSREQ é amostrado na rising edge do último clock do M-cycle; se ativo, os sinais de endereço/dados/controle entram em alta impedância na rising edge do clock seguinte. Durante o ciclo de BUSREQ, INT/NMI não interrompem a CPU.
8. EI habilita IFF1/IFF2, mas o INT pendente só é aceito depois da instrução seguinte.
9. RETI e RETN têm semântica de retorno semelhante a RET; RETN restaura IFF1 a partir de IFF2, enquanto RETI é a notificação reconhecida pelos periféricos Z80 para encerrar serviço na daisy-chain.
10. No SMS, o VDP é a fonte da IRQ maskable (frame/VBlank e line); Pause gera NMI. A IRQ do VDP deve ser integrada como linha externa, não implementada dentro do Z80.

## 2. Matriz de conformidade documental

| Item | Comportamento documentado | Fonte / página impressa (PDF) | Diagrama | Consequência interna | Lacuna a validar na implementação |
|---|---|---|---|---|---|
| M-cycle / T-state | Instruções são séries de operações básicas; cada operação é M-cycle e cada clock é T-state. M1 de fetch possui 4, 5 ou 6 T-states; ciclos seguintes normalmente 3–5 T-states, alongáveis por WAIT. | UM008011-0816, seção Timing, pp. 7–8 (PDF 21–22) | Fig. 4 e Fig. 6 | CPU precisa identificar tipo de M-cycle e progresso T-state | O projeto deve provar que todos os M-cycles das famílias BASE/CB/ED/DD/FD/DDCB/FDCB têm sequência correta, não apenas total nominal |
| WAIT — memória/fetch | Durante T2 e cada TW subsequente, WAIT é amostrado na falling edge do clock. Se ativo, outro WAIT state entra no ciclo seguinte. | UM008011-0816, Timing, p. 7 (PDF 21) | Fig. 6 | Uma amostra pode estender o ciclo por um TW; a amostra é uma fase elétrica distinta da conclusão do T-state | A PR #512 recebe `wait_asserted` no avanço de T-state, mas não serializa uma falling-edge explícita; validar a integração |
| I/O automático TW | Cada I/O read/write tem um TW automaticamente inserido entre T2 e T3; a linha WAIT é amostrada durante esse TW. | UM008011-0816, Input or Output Cycles, p. 10 (PDF 24) | Fig. 7 | I/O não é apenas um ciclo genérico de 4T; contém TW de origem automática | Representar `AutomaticIo` explicitamente e permitir WAIT externo durante o TW |
| INT acknowledge | M1 especial; IORQ ativo no lugar de MREQ; dois TW automáticos para estabilizar a ripple priority chain e selecionar o vetor. | UM008011-0816, Interrupt Request/Acknowledge Cycle, p. 12 (PDF 26) | Fig. 9 | Acknowledge é ciclo de barramento próprio; vetor deve vir de dispositivo externo | O contrato atual deve preservar os dois TW e continuar observando WAIT externo nas fases aplicáveis; conferir forma de onda Fig. 9 |
| Amostragem INT | INT é amostrado na rising edge do último clock ao fim de qualquer instrução; não é aceito se IFF1=0 ou BUSREQ ativo. | UM008011-0816, p. 12 (PDF 26) | Fig. 9 | Aceitação é uma decisão no limite de instrução, não em qualquer T-state | Testar request antes/na/depois da borda; provar que BUSREQ bloqueia |
| NMI | Entrada negative-edge-triggered; maior prioridade que INT; reconhecida ao fim da instrução, independente de IFF; vetor fixo $0066. | UM008011-0816, Pin Functions, p. 6 (PDF 20); CPU Response, pp. 18–19 (PDF 32–33) | Fig. 10 | Precisa capturar borda de descida e reter pendência até safe point; IFF1 é limpo e IFF2 preserva estado anterior | Verificar se o runtime distingue captura da borda de despacho no fim da instrução |
| EI delay | EI põe IFF1/IFF2 em 1, mas INT pendente não é aceito até depois de executar a instrução seguinte. | UM008011-0816, Interrupt Request/Acknowledge, p. 18 (PDF 32) | Fig. 13 (contexto de enable) | Precisa de estado de atraso de uma instrução, não apenas IFF boolean | Testar EI;NOP;INT, EI;HALT;INT, EI;RET e interrupção coincidente |
| HALT | HALT executa ciclos M1/NOP para refresh; INT/NMI são amostrados na rising edge em T4 de cada ciclo; se aceito, HALT sai no rising edge seguinte e o próximo ciclo é acknowledge. NMI vence se ambos ocorrerem. | UM008011-0816, HALT Exit, p. 13 (PDF 27) | Fig. 11 | HALT é uma sequência repetida de ciclos de barramento, não apenas um bit de espera | A integração deve provar T4 sampling, refresh, saída e snapshot entre ciclos HALT |
| BUSREQ | BUSREQ amostrado na rising edge do clock mais recente do M-cycle; se ativo, endereço/dados/controles passam a alta impedância na rising edge do próximo clock. BUSREQ tem prioridade superior a NMI; não há interrupção durante ciclo de BUSREQ. | UM008011-0816, BUS Request/Acknowledge, pp. 11–12 (PDF 25–26) | Fig. 8 | Arbitragem precisa de fronteira M-cycle e estado de ownership externo | Não inferir fronteira por `tstate_index`; representar aceitação, latência de alta impedância e BUSACK |
| BUSACK | BUSACK indica que address/data e sinais MREQ/IORQ/RD/WR entraram em alta impedância; host externo pode dirigir o barramento. | UM008011-0816, Pin Functions, p. 6 (PDF 20); Fig. 8, pp. 11–12 | Fig. 8 | A CPU não pode emitir transações enquanto o barramento é do agente externo | Snapshot precisa conter estado de ownership, request ativo e ponto de retomada |
| RETN | Ao aceitar NMI, IFF1 é limpo e IFF2 guarda o estado anterior; RETN copia IFF2 para IFF1 e conclui o serviço NMI. | UM008011-0816, Interrupt Flip-Flops, p. 19 (PDF 33) | Tabela 1 | RETN tem efeito interno sobre IFF1/IFF2 | Testar IFF1/IFF2 em NMI com IFF previamente 0 e 1 |
| RETI | RETI é retorno (equivalente à operação de retorno de RET), e os periféricos Z80 reconhecem sua execução para finalizar serviço de prioridade aninhada. | UM008011-0816, seção RETI/RETN, p. 60 (PDF 74); instruções RETI/RETN no índice, pp. 288/290 | Sem waveform de barramento detalhada nesta seção | A notificação da daisy-chain é externa ao efeito IFF de RETN | Implementar evento/callback de RETI no contrato compartilhado; RETI não deve ser alias semântico de RETN |
| Daisy-chain | Os dois TW do acknowledge permitem estabilizar a ripple priority chain; RETI sinaliza o fim do serviço para periféricos. | UM008011-0816, pp. 12 e 60 | Fig. 9; descrição de RETI | O periférico seleciona vetor e mantém estado de serviço/IEO até RETI | O Z80 deve emitir evento RETI; dispositivo periférico mantém sua prioridade/IUS/IEO |
| IRQ VDP SMS | VDP leva INT baixo quando ocorre frame ou line interrupt; Pause gera NMI. | SMS Power!, SMS Cartridge e Cartridge Slot | Diagrama do slot; descrição de sinais | VDP é produtor da linha INT; CPU só reconhece a linha segundo IFF/timing | Conectar IRQ do VDP real ao Z80 e preservar estado de pending/enable |
| VDP status/ack | Status port $BF informa VBlank em D7 e flags de sprite; leitura limpa as flags/status latches. Line IRQ não deve ser confundida com o bit de VBlank. | SMS Power!, Control Port, VDP Registers, SMS Official Docs | Tabela de status/registradores | Leitura de status tem efeito colateral e participa do acknowledge/limpeza | Testar VBlank e line IRQ separadamente; comprovar quando a linha INT é desativada |
| R10 line counter | R10 define o contador de linha; ao expirar, gera line interrupt se habilitada e recarrega conforme a regra documentada. R0 bit4 habilita line IRQ; R1 bit5 habilita frame IRQ. | SMS Power!, VDP Registers e SMS Official Docs | Tabela R0/R1/R10 | VDP precisa de estado interno de contador, flags e enable bits | Testes em limites de linha, reload, disable e status read |
| Pause NMI SMS | Pause button aciona NMI para Z80 via VDP; liberação não gera evento inverso. | SMS Power!, SMS Cartridge e Gearsystem smstech | Diagrama do slot | NMI é borda do botão, não uma IRQ maskable do VDP | Testar pressionar, manter e liberar; não gerar repetição artificial |

## 3. Confronto de divergências e decisões

### 3.1 WAIT de I/O: rótulo do estado versus fase elétrica

O manual descreve um TW automaticamente inserido em I/O entre T2 e T3 e diz que WAIT é amostrado durante esse estado. A regra geral de Timing descreve amostragem na falling edge durante T2 e cada TW subsequente. O contrato não deve codificar a regra como “amostra quando um T-state terminou”; deve registrar a falling edge e o TW que a originou.

**Decisão:** o modelo terá evento de amostra separado do evento de conclusão de T-state. A classificação `AutomaticIo` será explícita.

### 3.2 INT acknowledge: waits obrigatórios e WAIT externo

O manual estabelece dois TW automáticos para o acknowledge. A regra geral de WAIT se aplica aos TW subsequentes. A implementação candidata da PR #512 consome waits mandatórios antes de consultar WAIT externo, o que não demonstra amostragem externa durante os TW obrigatórios.

**Decisão:** manter contadores/origens distintas e consultar WAIT em cada falling edge de T2/TW. A ordem exata de inserção de TW extra em relação aos dois TW mandatórios deve ser alinhada à Figura 9 e confirmada por trace independente; não assumir que o contador de waits mandatórios elimina a necessidade de amostra externa.

### 3.3 RETI versus RETN

RETN copia IFF2 para IFF1; RETI notifica periféricos Z80 sobre fim do serviço de interrupção. O manual não descreve RETI como mecanismo de restauração de IFF.

**Decisão:** duas operações distintas no decoder e no evento externo. O RETI event deve ser emitido no ponto semântico de execução da instrução, e a cadeia periférica deve alterar seu estado de serviço somente ao receber esse evento.

### 3.4 VDP SMS: status VBlank versus line IRQ

O VDP pode levar INT baixo por VBlank ou line interrupt. O status bit D7 identifica a fonte VBlank; o estado de line IRQ não deve ser inferido desse bit. A leitura de status $BF limpa flags/status latches, com efeitos que devem ser testados no VDP real.

**Decisão:** manter pending state de VBlank e line IRQ no VDP, compor a saída INT como linha ativa baixa conforme os enables e pending states, e testar separadamente a leitura de status.

### 3.5 BUSREQ e interrupções

O manual declara que BUSREQ tem prioridade maior que NMI e que a CPU não pode ser interrompida por NMI ou INT durante o ciclo de BUSREQ.

**Decisão:** enquanto o estado de barramento for cedido/acknowledged, não despachar INT/NMI. Preservar pending edge NMI conforme a semântica do chip e reconciliar o momento de serviço na retomada por teste; o manual não especifica em detalhe todos os casos de borda durante DMA prolongado.

## 4. Situação da implementação atual na baseline main

Baseline remota consultada: `249d7a3da613dec456f22aa6a7ceaccb2418ee0b`.

| Área | Situação observada | Classificação |
|---|---|---|
| Tempo do Scheduler | `MachineScheduler.now` e `ClockDomain` permanecem em `MachineTime` inteiro na main | Divergente para sub-tick |
| Fila de eventos | main usa timestamp inteiro; PR #526 candidata migra timestamp da fila para `MachineInstant` | Candidato parcial |
| ClockDomain | PR #526 mantém snapshot/time do domínio em MachineTime | Não migrado |
| WAIT falling edge | runtime não expõe evento elétrico falling-edge separado | Não implementado |
| TW automático de I/O | contratos candidatos não representam a origem automática como estado de fase explícito e independente | Não comprovado |
| INT ack waits | Z80 tem waits mandatórios; amostragem WAIT durante esses TW não está demonstrada | Divergente/incompleto |
| BUSREQ/BUSACK | PR #511 documenta a lacuna; runtime ainda sem arbitragem real | Não implementado |
| RETI/daisy-chain | ainda não há fechamento de evento genérico RETI + periférico no runtime compartilhado | Não implementado |
| IRQ VDP | SMS temporal bus atual não integra VDP; descrição do arquivo indica VDP/PSG fora da fatia temporal | Não implementado |
| HALT T4 | E7.3 integrada segundo scorecard; não confundir CI com trace externo por borda | Implementação integrada; temporal externo não comprovado |

## 5. Critérios de aceitação documentais

- Cada amostra WAIT tem timestamp racional, polaridade da borda, M-cycle, T-state/TW, origem do TW, valor lido e resultado (estender/não estender).
- I/O inclui TW automático e WAIT externo pode estender esse estado.
- INT acknowledge usa M1+IORQ, dois TW automáticos, vetor externo e WAIT amostrado em cada TW aplicável.
- BUSREQ só é aceito no limite documentado do M-cycle; BUSACK só é afirmado após os sinais saírem para alta impedância.
- INT/NMI em HALT são amostrados em T4 na rising edge; NMI tem prioridade.
- EI delay é de uma instrução completa.
- RETN restaura IFF1 a partir de IFF2; RETI emite notificação de serviço para a daisy-chain.
- SMS VDP gera IRQ de VBlank e line IRQ com os respectivos enables e flags; Pause gera NMI.
- Snapshot/restore em cada falling/rising edge e durante BUSACK reproduz o mesmo trace após retomada.
- Nenhum comportamento não especificado pelo manual deve ser promovido a regra de hardware sem evidência adicional.

## 6. Referências primárias

### ZiLOG
- ZiLOG, *Z80 CPU User Manual*, UM008011-0816, 2016, arquivo fornecido `um0080.pdf`.
- PDF oficial: https://www.zilog.com/docs/z80/um0080.pdf
- Timing/M-cycle/WAIT: pp. 7–8.
- I/O automático TW: p. 10.
- BUSREQ/BUSACK: pp. 6 e 11–12.
- INT acknowledge e dois TW: p. 12.
- HALT T4 sampling: p. 13.
- EI delay/IFF: pp. 18–19.
- RETI/RETN: p. 60; índice de instruções nas pp. 288 e 290.

### SMS Power
- SMS Cartridge (INT/NMI, VDP e Pause): https://www.smspower.org/Development/SMSCartridge
- Cartridge Slot (pinos BUSREQ #44 e BUSACK #45; INT #42; WAIT #41): https://www.smspower.org/Development/CartridgeSlot
- Control Port (status $BF e limpeza de flags): https://www.smspower.org/Development/ControlPort
- VDP Registers (IE1/IE0/R10): https://www.smspower.org/Development/VDPRegisters
- SMS Official Docs (line interrupt, VBlank/status): https://www.smspower.org/Development/SMSOfficialDocs%26num%3D5

### Referência comparativa independente de borda
- floooh, *Z80 instruction timing*, netlist simulation: https://floooh.github.io/2021/12/06/z80-instruction-timing.html
- Esta simulação ajuda a alinhar half-cycles, mas o autor alerta sobre possível derivação Z8400; não prevalece sobre o manual para o Z80 clássico.

## 7. Conclusão

A evidência primária confirma que o runtime precisa de uma timeline capaz de representar meia-fase sem arredondamento antes de qualquer implementação confiável de WAIT/TW ou BUSREQ. Portanto, C1 (instante racional compartilhado) e C2 (falling/rising edges) são dependências reais de C3. BUSREQ/BUSACK, RETI/daisy-chain e IRQ VDP permanecem bloqueados até que o contrato temporal seja completo e serializável.
