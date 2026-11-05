# E7 — Agente 1: pesquisa comparativa de emuladores e testes independentes

**Data:** 2026-11-04  
**Baseline analisada:** `main` = `249d7a3da613dec456f22aa6a7ceaccb2418ee0b`  
**Escopo:** pesquisa independente de documentação primária. Este relatório descreve estratégias observáveis em código/fontes públicas; não atribui autoridade de hardware a nenhum emulador.

## 1. Resultado executivo

A comparação confirma que o contrato desejado precisa separar três dimensões que implementações simplificadas tendem a colapsar:

1. progresso nominal do M-cycle/T-state;
2. eventos elétricos e amostragem de sinais em bordas específicas;
3. continuação serializável do processador e do barramento.

Nenhuma referência de alto nível consultada, isoladamente, comprova toda a semântica WAIT/TW, BUSREQ/BUSACK, interrupções e retomada requerida pelo projeto. MAME fornece estados de controle e integração daisy-chain; chips/z80 fornece uma interface pin/tick útil para modelagem por clock; SingleStepTests e Fuse fornecem corpora diferenciais; Gearsystem fornece contexto de hardware SMS. Esses resultados devem ser cruzados com o manual ZiLOG e testes próprios.

## 2. Fontes comparativas e evidência rastreável

### 2.1 MAME

**Fonte:** `mamedev/mame`, `src/devices/cpu/z80/z80.cpp`, classe `z80_device`.

- `execute_run()` delega a execução ao corpo de microsequenciamento incluído como `cpu/z80/z80.hxx`; a expansão/geração desse corpo não foi inspecionada nesta leitura, portanto não se atribuem aqui fases elétricas exatas a funções internas.
- `execute_set_input()` trata `Z80_INPUT_LINE_BUSREQ`, `INPUT_LINE_NMI`, `INPUT_LINE_IRQ0` e `Z80_INPUT_LINE_WAIT`.
- Ao ativar BUSREQ, o código agenda `SA_BUSREQ`; NMI é convertido em `SA_NMI_PENDING` na transição de borda; IRQ atualiza o estado e consulta `daisy_update_irq_state()`; WAIT mantém um estado de entrada.
- `device_start()` registra para save state, entre outros, `m_nmi_state`, `m_irq_state`, `m_wait_state`, `m_busreq_state`, `m_busack_state`, `m_service_attention`, `m_tmp_irq_vector`, latches temporários e registradores.
- A presença de estado salvo para BUSREQ/BUSACK/WAIT/IRQ é uma referência útil de completude do snapshot, mas não prova que o formato ou a granularidade de snapshot do MAME sejam adequados ao Scheduler deste projeto.

**Daisy-chain em dispositivos:** `src/devices/machine/z80sio.cpp`, `z80sio_device::z80daisy_irq_ack()`, seleciona a fonte de maior prioridade com INT pendente, ativa IUS/IEO e calcula o vetor. Isso demonstra que o acknowledge muda estado do periférico, e não é apenas uma leitura de byte.

**Limitação:** a lógica de microciclos efetiva não foi atribuída a uma função concreta do arquivo incluído `z80.hxx`; não usar MAME como prova de falling-edge WAIT ou de waveform sem inspeção adicional do corpo de execução e sem comparação com ZiLOG.

Links:
- https://github.com/mamedev/mame/blob/master/src/devices/cpu/z80/z80.cpp
- https://github.com/mamedev/mame/blob/master/src/devices/machine/z80sio.cpp

### 2.2 ares

**Fonte:** `ares-emulator/ares`, árvore `ares/component/processor/z80`.

A árvore pública do projeto identifica um componente Z80 compartilhado sob `ares/component/processor/z80`. A navegação direta dos arquivos dessa pasta falhou por cache/acesso nesta sessão; portanto, não atribuo nomes de funções, tratamento de WAIT, nem comportamento BUSREQ/RETI a arquivos não lidos. Uma auditoria anterior do repositório fixou a revisão `d400eee9764f` como referência comparativa; essa referência histórica não substitui inspeção da revisão atual.

**Conclusão comparativa:** manter ares na lista de fontes a revisar no próximo ciclo de pesquisa, mas classificar evidência de código detalhada como **NOT VERIFIED** nesta execução. Não inferir que a existência de um core Z80 signifique suporte a pin-level WAIT ou BUSREQ.

Link:
- https://github.com/ares-emulator/ares/tree/master/ares/component/processor/z80

### 2.3 Gearsystem

**Fonte consultada:** `drhelius/Gearsystem`, documentação técnica mantida em `skills/gearsystem-debugging/references/smstech.md` e documentação do projeto.

A documentação descreve o comportamento SMS/GG:
- Pause do SMS ligado à entrada NMI; pressionar gera NMI e o PC segue para `$0066`.
- INT do SMS normalmente é usado em IM 1, com entrada em `$0038`; IM 0/2 dependem do byte apresentado pelo dispositivo no barramento.
- No SMS2, Game Gear e Genesis, o valor típico retornado no acknowledge é `$FF`, que corresponde a `RST 38H`; o SMS original pode apresentar valor não determinístico conforme a fonte consultada.
- O material identifica diferenças entre modelos e reforça que a linha INT e o vetor de acknowledge pertencem à interface máquina/VDP, não apenas ao Z80.

**Limitação:** a referência aberta é documentação de hardware, não a implementação interna do core Z80. Ela não comprova a temporização de borda ou snapshots do Gearsystem.

Links:
- https://github.com/drhelius/Gearsystem/blob/master/skills/gearsystem-debugging/references/smstech.md
- https://github.com/drhelius/Gearsystem

### 2.4 Fuse e testes temporais

**Fontes:** Fuse (ZX Spectrum) e seus materiais de teste Z80.

- A suíte de teste Fuse inclui resultados por instrução e timestamps de ciclos de barramento; isso permite comparar a posição dos ciclos dentro da instrução, não somente o total de T-states.
- O projeto Spectrum Workbench descreve a suíte Fuse como contendo timestamp por ciclo e a utiliza para testar temporização de barramento/contenda.
- Fuse também é relevante por seu core Z80 reutilizado em outros projetos; deve ser tratado como implementação comparativa, não como autoridade de silício.

**Limitação:** o comportamento de contenda de memória do Spectrum é específico de ULA/máquina. Não transportar suas regras para SMS; aproveitar apenas o método de trace e os casos de teste do processador.

Links:
- https://fuse-emulator.sourceforge.net/
- https://github.com/fuse-emulator/fuse
- https://github.com/poetix/spectrum-workbench

### 2.5 SingleStepTests/Z80

O repositório descreve testes JSON com:
- estado inicial de CPU e RAM;
- sequência de ciclos/pinos;
- comparação ciclo a ciclo de R/W/MREQ/IORQ/endereço/dados quando o runner dá suporte;
- estado final da CPU e RAM.

O próprio README informa que os vetores foram produzidos inicialmente traduzindo o core Z80 do ares e corrigindo erros identificados. Consequentemente, é um bom corpus diferencial para divergências de implementação, mas não deve ser rotulado como medição independente de hardware nem como oráculo físico independente.

Link:
- https://github.com/SingleStepTests/z80

### 2.6 raxoft/z80test

O README do projeto declara que os resultados esperados foram obtidos em um Spectrum 48K real com CPU Zilog Z80. Os programas incluem `z80full`, `z80doc`, `z80flags`, `z80docflags`, `z80ccf` e `z80memptr`.

Uso recomendado:
- `z80doc` primeiro para registradores e flags documentadas;
- `z80full` e `z80flags` para comportamento não documentado;
- `z80ccf` para comportamento autêntico de SCF/CCF;
- `z80memptr` como detecção parcial de problemas em WZ/MEMPTR.

O autor alerta que o corpus MEMPTR não foi projetado para cobrir todos os casos possíveis. Esses programas validam principalmente resultado arquitetural, não waveform WAIT/BUSREQ por T-state.

Links:
- https://github.com/raxoft/z80test
- https://github.com/raxoft/z80test/releases

### 2.7 floooh/chips — referência pin/tick

**Fonte:** `floooh/chips/chips/z80.h`.

O core expõe interface por pinos e avança por `z80_tick()`. A integração do sistema:
- examina `MREQ/RD/WR` e `IORQ`;
- injeta byte de vetor/opcode durante `M1 + IORQ`;
- injeta WAIT/INT/NMI pelas linhas de entrada;
- exige que o host mantenha o estado dos pinos entre ticks;
- orienta a execução dos periféricos Z80 em ordem daisy-chain, propagando `IEIO`;
- mantém estado de continuação como `step`, `addr`, `dlatch`, `opcode`, `hlx_idx`, `prefix_active` e `pins`.

A abordagem é útil para separar CPU e transações de barramento e para preservar estados internos. O conjunto público de pinos listado não fornece uma implementação BUSREQ/BUSACK completa, portanto não serve como prova para E7.4.

Link:
- https://github.com/floooh/chips/blob/master/chips/z80.h

## 3. Matriz comparativa das funcionalidades

| Funcionalidade | MAME | ares | Gearsystem | Fuse / SingleStep / chips | Resultado da pesquisa |
|---|---|---|---|---|---|
| M-cycles/T-states | Execução dedicada e estados de controle; corpo incluído não inspecionado | Core Z80 listado; detalhe de ciclo não verificado | Hardware SMS documentado; não core | Fuse e SingleStep registram ciclos; chips tick/pin | Exigir trace próprio por borda |
| WAIT/TW | Estado WAIT salvo; amostragem interna não comprovada nesta leitura | Não verificado | Não especifica waveform WAIT | chips expõe WAIT; doc de outro Z80 Rust descreve half-cycle, mas não é alvo SMS | Manual ZiLOG prevalece |
| Falling/rising edge | Não comprovado pelo arquivo aberto | Não verificado | Não documentado | chips avança por tick; SingleStep pode expor pinos por ciclo | C2 precisa de eventos de meia-fase |
| I/O TW automático | Não comprovado | Não verificado | Não documentado | Testes podem comparar ciclos, mas não identificam regra física por si | Especificar a partir do manual |
| INT acknowledge | MAME integra IRQ e vetor/daisy | Não verificado | SMS usa normalmente IM1 e vetor $FF nos modelos descritos | chips fornece vetor em M1+IORQ; SingleStep cobre ciclo | Modelar como M1 especial e operação de barramento |
| NMI | Entrada convertida em pendência de borda | Não verificado | Pause SMS gera NMI | chips expõe NMI edge input | Captura de borda + despacho em boundary |
| EI delay/HALT | MAME tem service attention; detalhamento temporal exige corpo de execução | Não verificado | Documenta vetores SMS | suites funcionais cobrem instruções; trace Fuse ajuda | Testar EI;HALT;INT e HALT T4 |
| BUSREQ/BUSACK | Estado de entrada/saída salvo; SA_BUSREQ; waveform não atribuída | Não verificado | SMS cartridge tem pinos BUSREQ/BUSACK | fontes consultadas não demonstram ciclo completo | Implementação deve esperar fronteira M-cycle |
| RETI/RETN | Interface daisy chain MAME; dispositivos mudam IUS/IEO no acknowledge | Não verificado | Não relevante à configuração SMS base | chips expõe RETI/IEIO | RETI sinaliza periférico; RETN restaura IFF |
| Daisy-chain | MAME implementa interface e prioridades em periféricos Z80 | Não verificado | Não descrito para VDP SMS | chips fornece ordenação IEIO entre periféricos | Abstração genérica no Z80; periféricos controlam prioridade |
| Snapshot | MAME registra estado de controle e latches | Não verificado | Gearsystem oferece rewind/debug, sem contrato interno analisado | chips guarda estado de tick; SingleStep testa estados iniciais/finais | Serializar cada latch e fase |
| IRQ VDP SMS | Não aplicável como fonte de verdade do SMS | Não verificado | SMS hardware docs | Gearsystem docs + SMS Power | VDP é fonte real de INT, CPU apenas amostra |

## 4. Sequência e implicações arquiteturais

### WAIT e TW
O código candidato da PR #512 mantém nominal T-states, waits obrigatórios e waits externos em contadores separados, o que é um avanço sobre um único deadline agregado. Entretanto, a interface `advance_tstate(wait_asserted)` só retorna a borda de subida concluída e não representa a falling edge de amostragem como evento persistível. Além disso, waits obrigatórios são consumidos antes da consulta WAIT externa no trecho revisado; isso não comprova a amostragem durante TWs automáticos obrigatórios de I/O/INT.

### Interrupções
O acknowledge precisa ser operação real de barramento, com `M1` e `IORQ` simultâneos, vetor lido no barramento e dois TW automáticos. O dispositivo periférico pode mudar seu estado de serviço ao reconhecer o acknowledge. O estado de IFF do Z80 e o estado de serviço do periférico são estados distintos.

### BUSREQ/BUSACK
O contrato não deve inferir a borda de aceitação por um índice parcial de T-state. O ponto relevante é a conclusão do M-cycle: request é aceito ao final desse ciclo e as linhas do barramento entram em alta impedância no clock subsequente. O estado de CPU precisa permanecer suspenso enquanto o host externo controla o barramento.

### Snapshots
Salvar apenas registradores arquiteturais não basta. São necessários: estado de decoder/prefixo, M-cycle, T-state, subfase elétrica, latches de endereço/dados, request de barramento, contador de waits mandatórios e externos, pending interrupt, estado BUSREQ/BUSACK, prioridade/daisy-chain e cursor/eventos do Scheduler.

## 5. Recomendações de teste diferencial

1. SingleStepTests: usar os ciclos e pinos para validar sequências de instrução e barramento, registrando a origem derivada dos vetores.
2. Fuse tests: importar casos temporais com timestamp por ciclo, sem importar a semântica de contenda do Spectrum.
3. raxoft/z80test: executar em máquina SMS virtual/runner, separando `doc` de `full`, flags e MEMPTR.
4. MAME: comparar traces de instruções, estado de interrupção e daisy-chain em casos controlados; fixar revisão antes de qualquer baseline.
5. Testes próprios de WAIT: variar WAIT imediatamente antes/depois da falling edge de T2 e em cada TW, incluindo transições durante TW obrigatório de I/O e INT acknowledge.
6. Testes próprios de BUSREQ: ativar BUSREQ em cada T-state de um M-cycle, comprovar aceitação somente no limite definido e manter BUSREQ ativo por múltiplos clocks.
7. Snapshots: serializar em cada fase e comparar o trace após restore com execução contínua.

## 6. Limitações desta pesquisa

- A implementação interna do ares não foi lida nesta execução; permanece **NOT VERIFIED**.
- A implementação do core Z80 do Gearsystem não foi inspecionada; a fonte utilizada é documentação de hardware SMS publicada no repositório.
- O corpo de microsequenciamento MAME incluído por `execute_run()` não foi recuperado nesta leitura; estados de controle e integração daisy-chain foram verificados.
- Nenhum dos projetos consultados fornece, nesta leitura, prova independente completa de WAIT/TW, BUSREQ/BUSACK e snapshots por meia-fase para o Z80 exato alvo.
- A pesquisa de código não substitui o manual ZiLOG nem medições de silício.

## 7. Fontes

- MAME Z80: https://github.com/mamedev/mame/blob/master/src/devices/cpu/z80/z80.cpp
- MAME Z80 SIO: https://github.com/mamedev/mame/blob/master/src/devices/machine/z80sio.cpp
- ares Z80: https://github.com/ares-emulator/ares/tree/master/ares/component/processor/z80
- Gearsystem SMS hardware: https://github.com/drhelius/Gearsystem/blob/master/skills/gearsystem-debugging/references/smstech.md
- Fuse: https://github.com/fuse-emulator/fuse
- SingleStepTests: https://github.com/SingleStepTests/z80
- raxoft/z80test: https://github.com/raxoft/z80test
- floooh/chips Z80: https://github.com/floooh/chips/blob/master/chips/z80.h
- SMS Power: https://www.smspower.org/
