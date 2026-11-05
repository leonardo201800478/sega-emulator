# Z80 compartilhado — contrato temporal multiplataforma

**Estado:** proposta técnica para revisão; não altera o runtime.  
**Baseline:** `main` em `c8da934d051f69f0fad01d133a923c26248a7250` (2026-11-05).  
**Escopo:** Z80 único compartilhado por SG-1000, Master System, Game Gear e Mega Drive (coprocessador de áudio).  
**Autoridade:** [ADR-0016 — fidelidade observável e precisão proporcional](adr/0016-observable-fidelity-and-proportional-timing.md) e [roadmap mestre](../project-roadmap.md).

> Este documento não cria uma exigência global de simulação elétrica ou de atualização de todos os pinos a cada clock. Define um núcleo temporal comum suficientemente expressivo para reproduzir a sequência de instruções, transações e esperas que os consoles suportados tornam observável.

## 1. Decisão arquitetural

Manter **uma implementação Z80 em Rust**. O núcleo descreve semântica, estados internos, M-cycles, T-states, transações de barramento, interrupções e suspensão/retomada. Cada máquina fornece um perfil de plataforma e um adaptador de barramento próprios.

Não duplicar a CPU para SMS, SG-1000, GG e MD. Não embutir endereços, clocks, WAIT ou arbitragem específicos de console dentro do decoder de instruções.

A composição de cada console é responsável por:
- fornecer a frequência/razão de clock do Z80 em relação ao relógio mestre da máquina;
- decodificar memória e I/O e realizar efeitos dos dispositivos no instante de transferência definido pelo ciclo;
- apresentar WAIT/READY, INT, NMI e BUSREQ apenas quando ligados no hardware/perfil selecionado;
- resolver propriedade do barramento e sincronização com outros chips/CPUs;
- salvar e restaurar o estado necessário para continuar uma transação em andamento.

A granularidade normal de execução pode avançar em blocos de T-states internos quando não houver interação externa intermediária. Uma transação externa nunca pode ser deslocada para antes/depois de sua janela temporal observável.

## 2. Contrato temporal comum do Z80

### 2.1 Unidade e propriedade do tempo

- O Scheduler da máquina é a autoridade do tempo decorrido.
- O Z80 consome bordas de seu domínio de clock; não avança o relógio mestre por conta própria.
- O core informa a próxima ação: operação de barramento, T-states internos, espera por sinal externo ou instrução concluída.
- O adaptador não pode completar uma leitura/escrita antes do ponto de amostragem/commit da operação.
- O estado de uma instrução pode permanecer suspenso entre quaisquer duas fases modeladas e deve ser serializável.

### 2.2 Ciclos documentados de referência

Os valores abaixo são a base do Zilog Z80 CPU User Manual UM008011-0816. Devem ser preservados pelo sequenciador e cobertos por testes:

| Operação | Duração nominal | Observação |
|---|---:|---|
| M1 — opcode fetch | 4 T | Inclui refresh; R incrementa no M1 |
| Memory read | 3 T | Fora do M1 |
| Memory write | 3 T | Fora do M1 |
| I/O read/write | 4 T | 3 T básicos + 1 TW automático |
| INT acknowledge | 5 T | M1 especial: 3 T básicos + 2 TW automáticos |
| WAIT externo | variável | TW adicionais conforme amostragem de WAIT |

Regras de fase:
- WAIT é amostrado na borda descendente ao final de T2 e de cada TW; se permanecer ativo, outro TW é inserido.
- Em M1, o dado do opcode é amostrado ao final de T3; o ciclo inclui a fase de refresh.
- Em I/O, o Z80 insere um TW automático para permitir a decodificação do dispositivo e a amostragem de WAIT.
- BUSREQ é reconhecido no limite de um M-cycle; após a resposta, os sinais/barramentos do CPU ficam em alta impedância para permitir controle externo.
- INT é reconhecido no limite de instrução quando habilitado e não bloqueado por BUSREQ. NMI tem prioridade sobre INT, mas BUSREQ tem prioridade de arbitragem conforme a especificação do Z80.
- HALT continua produzindo ciclos de fetch/refresh até uma condição de saída válida.
- Os T-states internos entre transações precisam ocupar sua posição correta na instrução; não podem ser somados indiscriminadamente no final.

A tabela não substitui a tabela de temporização por instrução. Cada família de opcode deve especificar a sequência de M-cycles, operações e extensões internas aplicáveis.

### 2.3 Contrato de transação

Uma transação suspensível deve preservar:
- identificador estável da requisição;
- operação (opcode fetch, memória read/write, I/O read/write, interrupt acknowledge);
- endereço e dado de escrita;
- M-cycle e posição T-state;
- duração nominal restante;
- TW acumulados e fase exata de amostragem de WAIT;
- dado de leitura somente quando atingir o ponto de amostragem;
- estado de interrupção/arbitragem que possa alterar a continuação.

Uma resposta duplicada, atrasada, com ID incorreto ou sem dado obrigatório deve falhar explicitamente. Não converter erro em NOP, leitura de `0xFF` ou execução atômica silenciosa.

### 2.4 Estado e save states

O snapshot do Z80 deve conter estado arquitetural e continuação temporal ativa: prefixos, opcode parcial, microfase, índices M/T, transação pendente, WAITs, refresh, HALT, IFF/IM, EI delay, INT/NMI pendentes e estado BUSREQ/BUSACK observado pelo core.

O snapshot da máquina contém adicionalmente fase dos clock domains, fila de eventos, estado do barramento/periféricos e propriedade do barramento. Restaurar um snapshot em voo deve produzir exatamente a mesma sequência de operações e o mesmo instante de conclusão que a execução sem interrupção.

## 3. Perfis temporais por plataforma

### 3.1 SG-1000

**Evidência conhecida**
- A SEGA documenta Z-80A a aproximadamente 3,58 MHz.
- Placas NTSC comuns usam 3,579545 MHz; variantes PAL documentadas usam 3,546893 MHz.
- O clock do Z80 é derivado do domínio de vídeo.
- Uma referência de engenharia reversa de placa SG-1000 descreve `/WAIT` do Z80 ligado ao sinal READY do PSG.

**Regra de implementação**
- Usar o mesmo sequenciador Z80 e um perfil SG-1000 próprio.
- Modelar a relação de clock CPU/VDP com razão exata do perfil da placa.
- Tratar READY do PSG como fonte de WAIT somente nos perfis de placa/PSG cuja ligação esteja confirmada por esquema ou medição. Não aplicar WAIT indiscriminadamente a todo SG-1000.
- Inserir TW no ciclo afetado, preservando a fase de amostragem do Z80; não simular a espera como atraso global depois da instrução.
- Preservar o botão HOLD/PAUSE do console como sinal/entrada do sistema, sem confundi-lo com pausa do host.

**Pendente de confirmação**
- Vincular por revisão de placa e variante de PSG a ligação READY → WAIT, polaridade, condição de busy e ciclo de escrita afetado. A referência disponível é secundária; obter esquema primário ou medição antes de fixar o comportamento como padrão de todos os SG-1000.

### 3.2 Master System / Mark III

**Evidência conhecida**
- O clock NTSC comum é 3,579545 MHz; o perfil PAL comum é 3,546893 MHz.
- O clock do Z80 é derivado do domínio do VDP.
- O conector de cartucho expõe WAIT, BUSREQ e BUSACK, além de M1, MREQ, IORQ, RD, WR e RFSH.
- A documentação do SMS Power informa que WAIT normalmente não é acionado pelo cartucho; BUSREQ pode ser usado por cartucho/dispositivo que solicite controle do barramento.

**Regra de implementação**
- O perfil padrão SMS não deve inserir WAIT sem uma fonte real assertando a linha.
- WAIT e BUSREQ devem permanecer entradas/eventos explícitos do adaptador, habilitados por dispositivo/perfil compatível.
- BUSREQ deve suspender o Z80 no limite de M-cycle e transferir a propriedade do barramento; não equivale a inserir TW.
- INT do VDP e NMI/periféricos devem chegar ao core conforme as ligações reais do perfil selecionado.
- VDP e PSG recebem efeitos de I/O no ponto de commit da transação, não no início da instrução nem no retorno do host.

### 3.3 Game Gear

**Evidência conhecida**
- O Hardware Reference Manual oficial indica clock de CPU de 3,579545 MHz.
- O manual documenta interrupção sincronizada ao vídeo e uso recomendado de IM 1.
- O Game Gear tem portas de controle próprias, incluindo START/PAUSE, região, comunicação serial e distribuição L/R do áudio.

**Regra de implementação**
- Reutilizar o Z80 compartilhado, mas selecionar perfil de I/O/IRQ Game Gear; não herdar automaticamente a decodificação de portas SMS.
- A origem e o instante de INT devem acompanhar o evento de vídeo efetivo do perfil GG.
- Não importar BUSREQ/WAIT do cartucho SMS por analogia. Habilitar tais sinais apenas se documentação específica do GG ou evidência de hardware confirmar a ligação.
- A diferença de viewport/LCD não altera a semântica do Z80; só o VDP e a composição da máquina tratam essas diferenças.

**Pendente de confirmação**
- Frequência exata para variantes regionais PAL, e eventual ligação de WAIT/BUSREQ em revisões específicas, devem ser confirmadas por documentação de placa/medição antes de adicionar comportamento.

### 3.4 Mega Drive / Genesis — Z80 de áudio

**Evidência conhecida**
- O Z80 opera a partir do master clock: NTSC 53,693175 MHz / 15 = 3,579545 MHz; PAL 53,203424 MHz / 15 = 3,546895 MHz.
- O Z80 controla o subsistema de som, com 8 KiB de Sound RAM, acesso ao YM2612, PSG e janela bancária para acesso ao espaço do 68000.
- O 68000 controla a propriedade do barramento Z80 por registrador BUSREQ e consulta o estado de acknowledge; há também controle de reset do Z80.
- O manual de software Sega exige que o 68000 solicite e aguarde a concessão do barramento antes de acessar a área Z80.

**Regra de implementação**
- Reutilizar o mesmo core Z80 com perfil MD, clock derivado do master /15.
- Modelar BUSREQ/BUSACK como arbitragem de propriedade do barramento entre Z80 e 68000, independente do mecanismo WAIT.
- A solicitação do 68000 não pode tornar o barramento disponível antes do acknowledge do Z80. O Z80 termina o M-cycle corrente, entra em estado de barramento cedido e não executa novos ciclos enquanto a concessão estiver ativa.
- O Scheduler deve ordenar requisição, acknowledge, acesso do 68000, liberação e retomada do Z80; a liberação não pode retroagir ou interromper uma transação já iniciada.
- A janela de ROM bancária, Sound RAM, YM2612, registrador de banco e PSG é decodificada pelo adaptador MD. Não colocar esse mapa no core Z80.
- Reset do Z80 é uma condição de máquina com prioridade e estado próprios, não uma instrução nem uma pausa de frontend.

**Pendente de confirmação**
- O ponto exato de sincronização entre o clock do 68000, escrita de BUSREQ, acknowledge e retomada deve ser validado contra documentação de hardware/traços independentes. A regra geral de limite de M-cycle do Z80 é base, mas o arbitro do ASIC MD precisa de validação específica.

## 4. Matriz comparativa

| Recurso | SG-1000 | SMS | Game Gear | Mega Drive |
|---|---|---|---|---|
| Core Z80 | Compartilhado | Compartilhado | Compartilhado | Compartilhado |
| Clock NTSC de referência | 3,579545 MHz | 3,579545 MHz | 3,579545 MHz | 3,579545 MHz |
| Clock PAL de referência | 3,546893 MHz | 3,546893 MHz | A confirmar por variante | 3,546895 MHz |
| WAIT | READY/PSG em placas identificadas; confirmar revisão | Linha exposta; normalmente não dirigida pelo cartucho | Não assumir sem evidência | Não confundir com BUSREQ |
| BUSREQ/BUSACK | Somente se dispositivo/perfil comprovar | Cartucho/dispositivo compatível | Não herdar do SMS | Arbitragem 68000 ↔ Z80 |
| INT | Perfil VDP/periférico SG | Perfil VDP SMS | Sincronizada ao vídeo | Fontes do subsistema de som/hardware MD |
| Barramento | Memória/I/O SG | Memória/I/O SMS | Memória/I/O GG | Sound RAM, YM2612, PSG, banco 68000 |
| Principal consumidor temporal | READY/PSG e VDP | VDP, I/O, cartucho | VDP/LCD, I/O, serial | Arbiter 68000, YM2612/PSG, banco |

Frequências iguais não significam máquinas temporalmente equivalentes: clock derivation, fase relativa, WAIT, decodificação de I/O e arbitragem são propriedades do sistema.

## 5. Estado real da implementação na baseline

A inspeção da baseline `c8da934d051f69f0fad01d133a923c26248a7250` encontrou:

1. O core atômico em `native/z80_cpu/src/cpu.rs` continua sendo usado pelo caminho normal de `SmsMachine::step()` e pelo runtime SG-1000.
2. Existe `Z80MicroSequencer` em `native/z80_cpu/src/sequencer.rs`, com estado retomável, bus requests, fases internas, WAIT acumulado e snapshots.
3. Existe `Z80M1Cycle` em `native/z80_cpu/src/m1.rs`, que modela M1, refresh e pontos de amostragem WAIT.
4. `Z80BusHandshake` preserva identidade da requisição e WAITs, mas não é, isoladamente, prova de que todos os ciclos de todas as instruções estejam modelados.
5. O sequenciador retorna `UnsupportedOpcode` em caminhos não implementados. O teste de inventário de BASE e a matriz de dispatch são controles de cobertura, não provas de semântica/timing completo.
6. `SmsTemporalMachine`/ `SmsTemporalBus` são uma fatia temporal isolada; não equivalem à máquina SMS completa integrada com VDP/PSG. Não devem ser expostos como backend de jogos.
7. A integração temporal completa de SG-1000, Game Gear e Mega Drive precisa ser auditada no momento em que cada composição for trabalhada.

Consequência: não substituir o executor Atomic pelo sequenciador experimental e não declarar conclusão temporal nesta baseline.

## 6. Plano de validação e gates

Cada evidência deve ser vinculada ao SHA e classificada PASS, FAIL, BLOCKED ou NOT RUN.

### 6.1 CPU independente da plataforma
- Vetores semânticos fixos para todos os espaços de opcode adotados: BASE, CB, ED, DD, FD, DDCB e FDCB.
- Comparação independente de registradores, flags, WZ/MEMPTR, Q, R, memória, I/O e interrupções.
- Matriz por instrução com sequência de M-cycles, operações, duração, T-states internos e ponto de amostragem/commit.
- Testes para prefixos repetidos, H/L versus IXH/IXL/IYH/IYL, deslocamentos, wraparound, instruções não atribuídas e estados especiais.
- Não confundir cobertura de dispatch com semântica correta.

### 6.2 Ciclos e sinais
- M1/refresh, memória read/write, I/O read/write, interrupt acknowledge, NMI, INT, EI delay, HALT e BUSREQ.
- WAIT amostrado em T2/TW, incluindo liberação em cada TW e snapshots no meio da espera.
- Verificar que ciclos internos não são deslocados para o final da instrução.
- Verificar que dados de leitura e efeitos de escrita ocorrem no ponto de transferência documentado.
- Snapshot/restore em cada fase de ciclo e em cada estado de arbitragem.

### 6.3 Integração por plataforma
- **SG-1000:** READY/PSG por perfil de placa, VDP TMS9918A/TMS9929, IRQ e regressões dos jogos SG já validados.
- **SMS:** VDP SMS, portas, IRQ, cartucho e BUSREQ/WAIT somente com consumidor comprovado.
- **GG:** portas específicas, interrupção de vídeo, panning e variantes de clock/território.
- **MD:** reset Z80, BUSREQ/BUSACK, 68000 acessando Sound RAM/YM2612/banco, acesso do Z80 à janela bancária, PSG e sincronização de áudio.

### 6.4 Determinismo e desempenho
- Equivalência de execução contínua e execução em blocos para os mesmos eventos.
- Replay determinístico e save-state/restore em transações em voo.
- Nenhuma operação de host/Python por instrução ou T-state.
- Medir overhead do sequenciador no build Release contra o executor Atomic; otimizar apenas depois da equivalência funcional e temporal.

### 6.5 Gates de repositório
```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

Builds PyO3/Windows e testes manuais de hardware/ROM são adicionados conforme o impacto. Jobs SKIPPED são NOT RUN, nunca PASS.

## 7. Sequência de engenharia

1. Fechar auditoria semântica funcional do Z80 compartilhado antes de atribuir precisão temporal às instruções.
2. Consolidar um catálogo executável de timing por opcode/família e provar as transações comuns M1, memória, I/O e interrupções.
3. Completar o sequenciador compartilhado sem duplicar decoder/ALU ou criar fallback silencioso.
4. Integrar primeiro a composição SMS completa, conectando CPU, memória/bus, VDP, PSG e IRQ.
5. Integrar o perfil SG-1000 e provar READY/PSG somente nas placas aplicáveis, sem regressão SG.
6. Integrar o perfil GG e suas diferenças de I/O/IRQ/clock.
7. Integrar o perfil MD com arbitragem real do 68000 e dispositivos de áudio.
8. Executar regressões cruzadas e medir determinismo/desempenho em cada plataforma.

A ordem respeita o roadmap mestre. Nenhuma etapa autoriza pular os gates da plataforma anterior.

## 8. Referências

### Primárias / documentação de fabricante
- ZiLOG, *Z80 CPU User Manual*, UM008011-0816: https://www.zilog.com/docs/z80/um0080.pdf
- SEGA, *Genesis Software Manual*: https://nemesis.hacking-cult.org/MegaDrive/Documentation/GenesisSoftwareManual.pdf
- SEGA, *Game Gear Hardware Reference Manual*: https://segaretro.org/images/1/16/Sega_Game_Gear_Hardware_Reference_Manual.pdf
- SEGA, SG-1000 hardware overview: https://www.sega.jp/history/hard/sg1000/

### Referências técnicas / engenharia reversa
- SMS Power, SMS cartridge bus signals: https://www.smspower.org/Development/SMSCartridge
- SMS Power, cartridge slot and signal descriptions: https://www.smspower.org/Development/CartridgeSlot
- SG-1000 board analysis (WAIT from PSG): https://retrotechwiki.com/Sega_SG-1000
- MAME Z80 device: https://github.com/mamedev/mame/blob/master/src/devices/cpu/z80/z80.cpp
- MAME CPU bus/wait scheduling concepts: https://docs.mamedev.org/techspecs/cpu_device.html
- Mega Drive clock-domain reference: https://github.com/user-none/emmd/blob/main/docs/genesis_system_reference.md

As fontes secundárias e os repositórios de emuladores servem como comparação e pista de investigação. Quando houver conflito, a documentação primária e a medição reproduzível da placa suportada prevalecem.
