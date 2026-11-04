# Auditoria de arbitragem BUSREQ/BUSACK — E7.4

**Data:** 2026-11-04  
**Baseline:** `main` = `869b402e4c8f2dbd9c5f5fa4a10d10ac26506461`  
**Branch:** `docs/z80-e7-4-busreq-audit-20261104`  
**Escopo:** auditoria documental/arquitetural preparatória; não implementa BUSREQ/BUSACK e não declara E7.4 concluída.

## 1. Resultado executivo

E7.4 exige arbitragem de barramento, não extensão de WAIT. O Z80 deve terminar o M-cycle corrente, amostrar BUSREQ no limite documentado, preservar a continuação e só então ceder os barramentos. Enquanto BUSACK estiver ativo, o mestre externo usa o barramento e o Z80 não aceita INT/NMI. A retomada deve continuar a partir do estado suspenso, sem repetir commits.

**Bloqueio técnico encontrado:** o runtime atual não expõe uma notificação genérica e comprovável de conclusão de cada M-cycle. `mcycle_index` e `tstate_index` são índices de progresso usados por fases selecionadas; a documentação do próprio estado diz que `tstate_index` expõe offsets intra-M-cycle selecionados, não bordas elétricas. Portanto, não é correto amostrar BUSREQ inferindo a borda a partir desses dois campos.

A implementação não deve avançar para uma integração funcional até existir um contrato explícito de fronteira M-cycle que cubra ciclos de barramento e ciclos internos.

## 2. Evidência documental primária

Fonte: ZiLOG, *Z80 CPU User Manual*, UM008011-0816 (2016), fornecido nos materiais do projeto.

### 2.1 Sinais e prioridade — página impressa 6

- BUSREQ é uma entrada ativa em nível baixo.
- BUSREQ possui prioridade superior a NMI.
- O sinal é reconhecido no fim do M-cycle corrente.
- A aceitação força address bus, data bus e MREQ/IORQ/RD/WR para alta impedância.
- Períodos longos de BUSREQ podem impedir o refresh adequado de DRAM; se houver DMA longo com DRAM, o controlador externo também deve realizar refresh.

### 2.2 Amostragem e concessão — páginas impressas 11–12, Figura 8

- BUSREQ é amostrado na borda ascendente do último período de clock do M-cycle corrente.
- Se ativo nessa amostragem, os barramentos e controles passam para alta impedância na borda ascendente do próximo pulso de clock.
- O dispositivo externo pode controlar os barramentos durante o intervalo concedido.
- O tempo máximo de resposta à solicitação é o comprimento de um M-cycle.
- O controlador externo pode manter o barramento por quantos clocks forem necessários.
- Durante o ciclo de BUSREQ, a CPU não pode ser interrompida por NMI nem INT.

A Figura 8 mostra a amostragem de BUSREQ e o estado BUSACK durante a janela de barramento disponível. O detalhe exato da liberação deve ser implementado a partir da forma de onda completa e verificado contra uma referência independente; não se deve substituir isso por uma regra arbitrária de "liberar imediatamente".

## 3. Evidência específica do Master System

SMS Power — [Cartridge Slot](https://www.smspower.org/Development/CartridgeSlot) e [SMS Cartridge](https://www.smspower.org/Development/SMSCartridge):

- O slot exportado de 50 pinos possui BUSREQ no pino 44 e BUSACK no pino 45.
- A tabela declara aplicabilidade a unidades Master System comercializadas fora do Japão e da Coreia.
- A documentação explica que o cartucho pode conduzir linhas de endereço quando o Z80 cede o barramento.
- A documentação informa que não há cartuchos comerciais originais conhecidos usando esse recurso; existem protótipos homebrew com transferências para RAM, PSG e VDP.

**Consequência:** BUSREQ/BUSACK deve ser parametrizado pelo perfil físico da máquina. Não habilitar os pinos em Mark III/SMS japonês, SG-1000 ou perfis sem evidência de sua presença. A existência do recurso no slot exportado não significa que todos os perfis SMS o tenham.

## 4. Referência de implementação

MAME — [z80.cpp](https://github.com/mamedev/mame/blob/master/src/devices/cpu/z80/z80.cpp):

- Mantém estados separados para BUSREQ e BUSACK e registra ambos no mecanismo de save state.
- A entrada BUSREQ detecta a transição de inativo para ativo e registra `SA_BUSREQ`.
- O código de referência é comparativo; não substitui o manual nem prova por si só a temporização exata do hardware.

A implementação ares e a modelagem registrada na PR #508 também são referências comparativas. Nenhuma delas deve ser copiada sem fixar revisão, variante e semântica.

## 5. Auditoria do runtime na baseline

### 5.1 Z80 compartilhado

Arquivo: `native/z80_cpu/src/sequencer.rs`

- `Z80MicroStep` expõe BusRequest, InstructionRetired, InterruptServiced, InterruptAcknowledge e InternalCycle.
- `Z80MicroSequencerState` serializa fase, handshake, request pendente, índices M/T e estado de interrupção/HALT.
- `tstate_index` é explicitamente documentado como posição selecionada dentro do M-cycle, não como borda elétrica.
- A sequência interna avança por `complete_internal_cycle()`, que consome um T-state e pode mudar a fase ou emitir uma próxima operação.
- Não existe estado BUSREQ/BUSACK, latch de concessão ou evento de borda de barramento.

### 5.2 SMS temporal machine

Arquivo: `native/sms_core/src/temporal_machine.rs`

- A máquina é proprietária do dispatch e chama o microsequenciador após eventos do Scheduler.
- M1 já é avançado por eventos individuais de T-state.
- Requests de memória/I/O são agendados como uma conclusão de barramento após duração base + WAIT.
- Ciclos internos são expostos em passos de um T-state e cada passo é agendado separadamente.
- A máquina não possui estado de proprietário externo, BUSACK ou fila de transações DMA.

### 5.3 Barramento e Scheduler

Arquivos: `native/sms_core/src/temporal_bus.rs`, `native/sms_core/src/temporal.rs`, `native/machine_scheduler/src/lib.rs`.

- `SmsTemporalEvent` possui BusCompletion, InternalCycleComplete e M1TStateComplete.
- `SmsTemporalBus` admite uma única transação de CPU pendente e aplica leitura/escrita no evento de conclusão, com acknowledgement posterior.
- Não há evento de amostragem BUSREQ, concessão BUSACK, liberação, arbitragem de proprietário ou transação de mestre externo.
- O Scheduler já fornece linha temporal determinística, domínios de clock, timestamp e ordenação por fase/prioridade. E7.4 deve estendê-lo, não criar outro scheduler.

## 6. Contrato arquitetural requerido

A modelagem deve ser dividida em duas responsabilidades:

1. **Z80 compartilhado:** representa a entrada BUSREQ, o estado de arbitragem, a continuação suspensa e o impedimento de aceitar INT/NMI durante BUSACK. Não executa DMA e não conhece cartucho SMS.
2. **Perfil de máquina/barramento:** converte as fronteiras M-cycle em eventos temporais, aplica a disponibilidade dos pinos conforme perfil, transfere a propriedade do barramento e executa operações do mestre externo na mesma linha temporal do Scheduler.

O contrato deve expor explicitamente:

- nível BUSREQ (ativo baixo) e sua amostragem;
- identidade/fase do M-cycle concluído;
- evento de concessão na próxima borda ascendente de clock após a amostragem elegível;
- BUSACK ativo e proprietário externo;
- operações externas de leitura/escrita com commits nos limites temporais;
- amostragem de liberação e retorno da propriedade à CPU;
- estado de arbitragem e eventos pendentes no snapshot.

O ponto de amostragem deve ser produzido pelo microsequenciador como metadado explícito de transição, ou por um contrato temporal equivalente cuja validade seja testável para todas as famílias. Não inferir o ponto a partir de `mcycle_index`/ `tstate_index` isoladamente.

## 7. Semântica e invariantes

1. BUSREQ não altera a duração do M-cycle e não é WAIT.
2. A transação em andamento termina e seus efeitos são confirmados exatamente uma vez antes da cessão.
3. O request lógico seguinte pode permanecer preparado/suspenso, mas não pode iniciar no barramento enquanto o proprietário for externo.
4. BUSACK é saída ativa baixa; a máquina deve distinguir nível lógico de seu estado interno de propriedade.
5. A CPU não executa requests nem aceita INT/NMI durante a janela BUSACK.
6. O relógio da máquina continua avançando e o mestre externo usa o Scheduler existente.
7. A liberação não descarta a continuação nem reinicia a instrução.
8. Snapshot/restore preserva o estado das linhas, proprietário, request suspenso, transações DMA e eventos agendados.
9. Reset deve ter comportamento explicitamente modelado para BUSREQ/BUSACK; não presumir que a linha externa desaparece.
10. O suporte elétrico é condicionado ao perfil SMS exportado; perfis incompatíveis devem rejeitar ou ignorar a linha de acordo com contrato explícito, sem criar pinos fictícios.

## 8. Plano incremental e critérios de aceite

### E7.4.1 — fronteira M-cycle explícita no Z80

- Inventariar todas as transições de BusRequest e InternalCycle e suas durações.
- Definir evento/metadado de M-cycle concluído, inclusive ciclos internos e M1 com WAIT.
- Provar que nenhuma família pula ou duplica a notificação.
- Preservar snapshot/restore e equivalência contínua/retomada.
- Não adicionar BUSACK à SMS machine até a fronteira ser completa.

### E7.4.2 — arbitragem no Z80 compartilhado

- Adicionar BUSREQ/BUSACK e latch serializáveis.
- Implementar prioridade sobre NMI/INT.
- Impedir emissão/execução de operações de CPU enquanto BUSACK estiver ativo.
- Retomar a continuação original após liberação.

### E7.4.3 — proprietário externo e DMA no perfil SMS

- Adicionar evento(s) tipados ao Scheduler existente.
- Implementar mestre externo com requests de leitura/escrita e commits transacionais.
- Ativar somente em perfis de slot que exponham BUSREQ/BUSACK.
- Não presumir DMA integrado ao VDP/PSG enquanto esses dispositivos não estiverem conectados à fatia temporal.

### E7.4.4 — snapshots e validação

Testes obrigatórios:
- BUSREQ em cada classe de M-cycle: M1, leitura, escrita, I/O e ciclo interno.
- Solicitação antes, na e depois da borda de amostragem.
- WAIT prolongando M-cycle sem antecipar amostragem.
- BUSREQ simultâneo com INT e NMI; nenhum atendimento durante BUSACK.
- BUSACK por 1, vários e muitos T-states.
- Mestre externo lê/escreve; ordem, dados e commits sem duplicação.
- Retirada de BUSREQ e liberação do barramento.
- Snapshot antes da amostragem, após amostragem e concessão, durante DMA e durante liberação.
- Execução contínua versus blocos e snapshot/restore com trace e estado final idênticos.
- Perfis SMS exportado, Mark III/japonês e SG-1000.

### Gates

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

Executar também Scheduler deterministic stress e os jobs Windows x64 aplicáveis. Classificar cada suíte como PASS, FAIL, BLOCKED ou NOT RUN. SKIPPED não é PASS.

## 9. Estado e bloqueios

- Baseline confirmada: `869b402e4c8f2dbd9c5f5fa4a10d10ac26506461`.
- E7.3 (#509): integrada, CI Rust/Python/Windows/Scheduler 4/4 SUCCESS no workflow #41877136387.
- E7.4: não implementada; esta auditoria é preparação documental.
- O clone local não pôde ser obtido nesta sessão por indisponibilidade de resolução DNS de github.com; portanto, testes locais são NOT RUN.
- A PR #508 continua aberta em base anterior à #509 e é draft; não foi usada como se estivesse integrada.
- E8 permanece bloqueada até E7.1–E7.7 estarem implementadas, reconciliadas e auditadas.

**Conclusão:** o próximo incremento de código correto é E7.4.1: tornar explícita e verificável a fronteira de M-cycle no contrato de transição do Z80 compartilhado. Implementar BUSREQ/BUSACK antes disso produziria arbitragem baseada em índices parciais, sem garantia de precisão temporal.
