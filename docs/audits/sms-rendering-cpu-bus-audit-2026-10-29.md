# Auditoria do subsistema de renderização SMS — CPU Z80 e barramento

**Estado:** inventário estático inicial concluído; sem alterações no runtime.  
**Base auditada:** `main` em `756666ae6216d8466f40259c11c345bbc1921848`.  
**Branch deste relatório:** `audit/sms-cpu-bus-rendering-2026-10-29`.  
**Escopo desta fatia:** CPU Z80, contrato de barramento, integração CPU/VDP e limites dos traces temporais.  
**Classificação geral:** `IMPLEMENTED` para execução funcional; `INTEGRATION_PENDING` para scheduler temporal compartilhado; `TEMPORAL_PASS` não declarado.

## 1. Resumo executivo

O core Z80 executa instruções inteiras e retorna uma contagem agregada nominal de T-states. Os acessos de memória e I/O ocorrem diretamente durante a execução sem um contrato geral que associe cada acesso ao T-state/M-cycle em que ocorre. A máquina SMS avança o VDP somente depois de a instrução terminar.

Em consequência, uma escrita de CPU em registradores, VRAM ou CRAM do VDP pode alterar o estado que será usado pelo renderer antes que o VDP tenha avançado pelo intervalo temporal da instrução. O renderer por scanline então consome o estado corrente no fechamento da linha, sem reconstruir quando, dentro daquela instrução, o acesso ocorreu.

Esta é uma lacuna arquitetural confirmada e uma explicação plausível para classes de defeitos raster/parallax. Não identifica, isoladamente, a causa de cada jogo ou imagem divergente.

## 2. Evidências de código

### 2.1 CPU: contagem agregada

Arquivo: `native/sms_core/src/cpu.rs`.

- `Z80::step()` delega para `step_inner()` e produz diagnóstico arquitetural opcional.
- `step_inner()` chama `fetch_opcode`, `fetch8`, `fetch16`, `read_memory`, `write_memory`, `read_port` e `write_port` diretamente no `CpuBus`.
- A maioria dos caminhos soma os ciclos a `total_tstates` ao final da instrução ou do caminho de execução.
- O próprio contrato documentado em `step()` diz que os T-states retornados são agregados e não representam eventos ordenados por T-state.
- HALT soma quatro T-states por passo; NMI/INT têm caminhos próprios de entrada, mas não emitem uma sequência geral de eventos temporais de barramento.

### 2.2 Barramento: contrato funcional, não temporal

Arquivos: `native/sms_core/src/bus.rs` e `native/z80_bus/src/lib.rs`.

- `CpuBus` define leitura/escrita de memória e portas.
- Os observadores `observe_timing_event` e `observe_tstate_sample` são opcionais e no-op por padrão.
- A documentação dos tipos deixa explícito que os traces são observacionais e não callbacks do scheduler.
- As amostras representam um perfil simplificado de endereço/dados e RD/WR/MREQ/IORQ; não constituem amostragem completa dos pinos físicos Z80.

### 2.3 Cobertura temporal parcial

Arquivo: `native/sms_core/tests/z80_ed_block_temporal_trace.rs`.

A instrumentação e a comparação de amostras estão implementadas somente para oito instruções ED sem prefixos DD/FD:
LDI, LDD, LDIR, LDDR, CPI, CPD, CPIR e CPDR.

O teste compara traces contra um corpus SingleStepTests fixado. Isso é evidência útil de conformidade com aquele perfil de referência, mas o documento do projeto registra simplificações e ancestralidade comum do corpus; não é prova independente de sinais físicos do Z80 nem de integração com o VDP.

Outros testes Z80 cobrem semântica, flags, interrupções, prefixos e contagem agregada, mas não estendem automaticamente a cobertura temporal a esses caminhos.

### 2.4 Integração da máquina SMS

Arquivo: `native/sms_core/src/machine.rs`.

O método `SmsMachine::step()`:
1. captura `cpu.total_tstates`;
2. executa `cpu.step(&mut bus)`;
3. captura o contador final;
4. chama `vdp.advance(cycles)`;
5. processa eventos enfileirados do PSG;
6. atualiza a linha INT da CPU a partir do estado do VDP.

Em `SmsMachineBus::write_port()`, portas VDP são encaminhadas diretamente a `vdp.write_port()`. Logo, o estado do VDP é mutado durante a execução da instrução, antes de `vdp.advance(cycles)`.

O PSG já possui uma fila de escritas com timestamp nominal para OUT imediato; a própria implementação comenta que a borda completa de barramento permanece pendente. Esse mecanismo isolado não deve ser confundido com um scheduler mestre.

### 2.5 VDP e renderização

Arquivo: `native/sms_core/src/vdp.rs`.

- `SmsVdp::advance()` consome T-states em segmentos até completar linhas de 228 T-states.
- `complete_scanline()` chama `render_scanline()` com VRAM, CRAM e registradores correntes.
- R9 possui um tratamento de latch vertical dedicado; isso demonstra que nem todos os registradores devem ser tratados como alterações instantâneas equivalentes.
- R8 é consultado pelo renderer ao calcular coordenadas de cada pixel da linha, mas não existe um mecanismo geral de escrita intralinha que altere a posição temporal do efeito.
- O próprio documento `docs/architecture/sms-vdp.md` lista ausência de efeitos de registradores no meio da scanline, contenção de VRAM e bordas de IRQ subinstrução como limites conhecidos.

## 3. Referência Zilog anexada

Fonte examinada: **Z80 CPU User Manual, UM008011-0816 (08/2016)**, arquivo fornecido nesta conversa (`um0080.pdf`).

O manual descreve:
- sinais M1, MREQ, IORQ, RD, WR, RFSH e WAIT;
- ciclos de máquina e T-states;
- fetch M1, ciclos de leitura/escrita de memória, I/O e acknowledge de interrupção;
- estados de espera e refresh.

No texto extraído do PDF, as descrições de sinais e timing aparecem na seção de arquitetura/timing, aproximadamente nas páginas impressas 7–10; a figura de WAIT adicional em M1 está na página impressa 24. O manual serve para o contrato temporal Z80. **Não** determina os latches, a contenção nem a temporização específica do VDP Sega.

## 4. Lacunas confirmadas

| ID | Lacuna | Evidência | Impacto |
|---|---|---|---|
| CPU-BUS-01 | Acesso de memória/I/O não tem timestamp geral | Chamadas diretas a `CpuBus` durante `step_inner` | Não é possível posicionar genericamente escritas no raster |
| CPU-BUS-02 | Duração agregada é aplicada após execução | `SmsMachine::step` avança VDP depois de `cpu.step` | Periféricos não observam a ordem intrainstrução |
| CPU-BUS-03 | Trace temporal cobre só oito ED | `z80_ed_block_temporal_trace.rs` | Não extrapolar conformidade para demais instruções |
| CPU-BUS-04 | Trace não dirige scheduler | callbacks são observadores no-op por padrão | Instrumentar não sincroniza dispositivos |
| CPU-BUS-05 | I/O e interrupções não possuem sequência temporal geral validada | caminhos de `read_port/write_port`, NMI/INT sem traces completos | Risco de bordas e acessos em posições incorretas |
| CPU-BUS-06 | WAIT/contenda e pinos físicos completos pendentes | contrato e documentação temporal | Não há precisão física/ciclo a ciclo |

## 5. O que ainda não está demonstrado

- Não foi demonstrado qual T-state de cada opcode deve apresentar cada acesso no barramento SMS.
- Não foi demonstrado o instante físico de latch de cada registrador do VDP.
- Não foi demonstrado se uma escrita específica em VRAM/CRAM é visível no pixel seguinte, tile seguinte, próxima linha ou outro limite.
- Não foi comparado um trace de execução completa CPU+VDP com um oráculo independente de hardware.
- Não foi feita validação de screenshots/jogos nesta etapa.

## 6. Critérios de aceitação para a próxima implementação

Antes de alterar runtime, definir e aprovar um contrato com:
1. contador de tempo mestre único, determinístico e monotônico;
2. transações de memória e I/O com instante e duração explicitamente definidos;
3. distinção entre M-cycle, T-state e transições de sinais quando a evidência exigir;
4. waits/contenda como extensão de duração, sem somar ciclos duas vezes;
5. avanço dos dispositivos na ordem dos eventos, incluindo VDP, PSG e interrupções;
6. instrumentação desativada sem mudança de estado ou custo de alocação no caminho normal;
7. testes de primeira divergência por evento/pixel contra vetores fixos e oráculos com ancestralidade avaliada.

A solução deve preservar o executor Z80 funcional e ampliar o contrato de forma incremental. Não substituir o runtime nem reescrever a CPU por preferência.

## 7. Plano da auditoria seguinte

1. Inventariar todos os pontos de acesso do Z80: fetch, memória, I/O, pilha, CB/ED/DD/FD, HALT, NMI e INT.
2. Classificar cada caminho por ciclos M/T documentados e evidência independente disponível.
3. Desenhar contrato mínimo de evento e scheduler mestre antes de implementação.
4. Auditar os latches do VDP separadamente, com referências Sega/VDP e testes independentes.
5. Criar testes sintéticos de escritas intrainstrução e intralinha.
6. Só então abrir uma PR de runtime pequena e coesa.

## 8. Validação desta entrega

- Inventário remoto: `PASS` — GitHub tree e fontes lidas no SHA base.
- Análise estática: `PASS` — evidências descritas acima.
- Testes Rust: `NOT RUN` — nenhuma alteração de runtime; não foi executado build local nem solicitado workflow específico nesta entrega documental.
- Teste em emulador: `NOT RUN`.
- `TEMPORAL_PASS`: não declarado.


## 9. Inventário detalhado dos caminhos temporais Z80

**Revisão de código:** branch `audit/sms-cpu-bus-rendering-2026-10-29`, head auditado `2dc984e5277d8f7d132226279655f4dcebb3ecc1`.  
**Fontes de implementação:** `native/sms_core/src/cpu.rs`, `native/z80_bus/src/lib.rs`, `native/sms_core/src/machine.rs`.  
**Escopo:** classificação estrutural dos caminhos existentes; não é uma declaração de cobertura ciclo a ciclo.

### 9.1 Matriz de cobertura

| Caminho | Implementação observada | Temporização disponível | Estado da evidência |
|---|---|---|---|
| Opcode fetch / M1 | `fetch_opcode → fetch8 → CpuBus::read_memory`; incrementa R | Contagem nominal; M1/T-state samples somente nos ED block instrumentados | Sem trace geral |
| Immediate/operand fetch | `fetch8/fetch16`, leituras de memória sequenciais | Incluído na duração nominal da instrução | Sem timestamp/evento geral |
| Base register-only | Decodificação base, operações internas de registrador/ALU | Contagem nominal por caminho | Sem sequência de M-cycles |
| Base memory read/write | `read_register/write_register` e acessos diretos em instruções | Contagem nominal por caminho | Sem posição temporal do acesso |
| Stack PUSH/POP/CALL/RET/RST | `push16/pop16` e caminhos explícitos de pilha | Leituras/escritas funcionais e ciclos nominais | Ordem funcional observável; sem timestamp de barramento |
| EX (SP),HL | Duas leituras e duas escritas diretas na pilha | Duração nominal | Sem fases temporais por acesso |
| CB | `fetch_opcode` do segundo byte e `execute_cb`; memória via registrador código 6 | Duração nominal | Sem trace temporal CB |
| ED não bloco | `execute_ed`: I/R, 16-bit loads, ADC/SBC HL, IN/OUT (C), RETN/RETI, IM, RRD/RLD etc. | Duração nominal | Sem trace temporal geral ED |
| ED block transfer | LDI/LDD/LDIR/LDDR | Eventos e samples instrumentados | Trace de subconjunto |
| ED block compare | CPI/CPD/CPIR/CPDR | Eventos e samples instrumentados | Trace de subconjunto |
| ED block I/O | INI/IND/INIR/INDR/OUTI/OUTD/OTIR/OTDR | Acesso funcional a porta/memória e ciclos nominais | Sem trace temporal no teste especializado |
| DD/FD | `execute_indexed`, prefixos repetidos, IX/IY, deslocamentos e operações indexadas | Contagem nominal, incluindo custo extra de prefixos | Sem trace temporal DD/FD |
| DDCB/FDCB | Fetch displacement/opcode e `execute_indexed_cb` | Contagem nominal | Sem trace temporal indexado-CB |
| HALT | Passo retido, incrementa R e soma 4 T-states | 4 T-states agregados por passo | Sem ciclo M1/refresh observado |
| NMI | Limpa HALT, salva PC, vetor 0x0066 | 11 T-states agregados | Sem sequência de acknowledge/stack temporizada |
| INT IM 0 | Injeta byte fornecido pelo barramento e executa decoder; acrescenta 2 T-states | Contagem nominal derivada do opcode injetado | Sem acknowledge IORQ/M1 temporal |
| INT IM 1 | Salva PC e vetor 0x0038 | 13 T-states agregados | Sem sequência temporal |
| INT IM 2 | Salva PC, lê vetor em (I:data_bus) e vetor+1 | 19 T-states agregados | Leituras funcionais sem timestamp |
| I/O base `IN A,(n)/OUT (n),A` | `read_port/write_port` com porta formada por A e imediato | 11 T-states nominais | Sem evento I/O temporal |
| I/O ED `IN r,(C)/OUT (C),r` e block I/O | Acesso funcional por `CpuBus` | 12 T-states nominais para IN/OUT (C); bloco usa seus caminhos próprios | Sem trace IORQ/M-cycle |
| WAIT / contention | Não há retorno de wait states pelo contrato `CpuBus` atual | Não modelado como extensão do ciclo | Pendente |
| Refresh e pinos físicos | R é atualizado; amostras do corpus têm fase Refresh inferida | Somente no subconjunto instrumentado e perfil simplificado | Não equivale a amostragem física completa |

### 9.2 Interpretação da matriz

- A CPU possui cobertura funcional ampla e vários testes semânticos, mas o retorno de `step()` continua sendo uma duração nominal agregada.
- Os acessos diretos ao `CpuBus` preservam a ordem programática das leituras/escritas, mas não informam quando cada acesso ocorre dentro dos T-states da instrução.
- Os eventos `Z80BusEvent` e `Z80TStateSample` são observacionais. A própria documentação de `native/z80_bus/src/lib.rs` afirma que não são callback do scheduler nem amostragem física completa.
- A instrumentação temporal especializada é acionada no caminho ED e cobre LDI/LDD/LDIR/LDDR/CPI/CPD/CPIR/CPDR. Não se deve inferir cobertura para ED block I/O, CB, DD/FD, interrupções ou instruções base.
- A implementação de IM 0 executa o byte de acknowledge como opcode injetado; isso não demonstra que o ciclo de acknowledge físico esteja temporizado ou que seus sinais estejam modelados.
- A matriz classifica o que o código oferece. Não certifica a exatidão de cada valor nominal de ciclos para todas as combinações de prefixo, interrupção ou estado.

### 9.3 Manual Zilog: aplicação e limite

O manual UM008011-0816 fundamenta a distinção entre M-cycle, T-state, fetch M1, ciclos de memória/I/O, acknowledge, refresh e WAIT. Para qualquer ampliação, os diagramas de timing específicos do ciclo devem ser transcritos para vetores com referência de página/figura e revisão do manual.

O manual Zilog descreve o Z80, não a interface elétrica/temporização particular entre CPU e VDP do Master System. Os tempos de latch, contenção e visibilidade de escrita no VDP precisam de documentação Sega específica e/ou evidência independente de hardware. Não se atribuem aqui offsets de barramento a instruções não instrumentadas.

## 10. Contrato mínimo proposto para a próxima etapa

A auditoria recomenda evoluir o contrato em etapas, sem reescrever o decoder Z80:

1. **Relógio mestre:** um único contador monotônico por máquina. A CPU reporta consumo temporal; dispositivos avançam até o próximo evento antes de observar a transação seguinte.
2. **Transação explícita:** tipo (M1/fetch, memória read/write, I/O read/write, acknowledge, refresh ou interno), endereço, dados, instante de início e duração. Os campos opcionais devem representar sinais realmente não dirigidos, não valores inventados.
3. **Sem dupla contagem:** escolher um único proprietário da duração. Preferência: CPU emite fases/micro-operações temporizadas e o scheduler contabiliza sua duração; `total_tstates` passa a ser derivado do relógio mestre, não somado novamente por fora.
4. **WAIT e contenção:** o dispositivo/barramento pode estender a transação com wait states explícitos. A extensão incrementa o relógio uma vez e desloca eventos posteriores.
5. **Ordenação determinística:** em empate de timestamp, definir prioridade estável e documentada para conclusão de transação, mutação de dispositivo, IRQ/NMI e amostragem. Não assumir prioridade sem fonte para o hardware.
6. **Dispositivos:** VDP, PSG, IO e linha de interrupção avançam até o instante do evento antes de aplicar a operação; IRQ é recalculada nas bordas documentadas.
7. **Modo de diagnóstico:** traces detalhados devem ser opcionais e sem alocação/custo relevante quando desativados; a semântica funcional não pode depender de observadores.
8. **Compatibilidade incremental:** manter `CpuBus` funcional ou introduzir uma camada adaptadora temporária, com testes equivalentes de estado final e contagem agregada antes de migrar famílias de instruções.
9. **Evidência:** cada família migrada recebe vetores fixos de sequência, offsets, duração, estado final e primeira divergência; oráculos e ancestralidade são documentados.

Este é um contrato arquitetural proposto, não uma especificação final de sinais nem uma implementação aprovada. A ordem entre CPU/VDP deve ser fechada somente após auditoria dos latches do VDP.

## 11. Critérios para encerrar esta auditoria documental

| Critério | Resultado |
|---|---|
| Verificar branch, base e SHA remoto | PASS — base `main` `756666ae6216d8466f40259c11c345bbc1921848`; head da auditoria `2dc984e5277d8f7d132226279655f4dcebb3ecc1` |
| Inventariar fetch, memória, pilha, CB/ED/DD/FD, HALT, NMI/INT e I/O | PASS — matriz da seção 9 |
| Distinguir ciclos agregados, eventos lógicos e samples T-state | PASS — seções 2 e 9 |
| Classificar WAIT, refresh e limitações dos pinos | PASS — sem alegar implementação inexistente |
| Propor contrato mínimo de scheduler sem dupla contagem | PASS — seção 10 |
| Confirmar latches e raster VDP com fontes Sega | NOT RUN — fora do escopo desta fatia; etapa seguinte |
| Executar novos testes locais nesta auditoria | NOT RUN |
| Validar PR head por GitHub Actions | PASS — workflow CI #2382, SHA `2dc984e5277d8f7d132226279655f4dcebb3ecc1` |
| Teste visual em hardware/emulador e oráculo independente CPU+VDP | NOT RUN |
| Declarar `TEMPORAL_PASS` | NÃO — não alcançado |

### 11.1 Detalhe verificável do CI da PR

Workflow: [CI run #2382](https://github.com/leonardo201800478/sega-emulator/actions/runs/40979085538)  
Job: `quality`, concluído em `2026-10-30T00:21:12Z`, conclusão `success`.  
SHA executado: `2dc984e5277d8f7d132226279655f4dcebb3ecc1`.

Passos reportados como concluídos com sucesso:
- Checkout;
- configuração Rust e Python;
- instalação do projeto e ferramentas;
- check de formatação;
- lint;
- build e instalação da extensão Rust nativa;
- execução da etapa de testes.

O workflow não substitui evidência visual de execução do SMS, nem demonstra precisão temporal ciclo a ciclo. A etapa de testes do CI passou no commit auditado, mas esta auditoria não afirma que testes adicionais foram executados localmente.

## 12. Conclusão e próximo marco

**Auditoria documental CPU/barramento: CONCLUÍDA.** O inventário confirmou a separação entre execução funcional, contagem nominal agregada e instrumentação temporal parcial. A principal lacuna não é a ausência de operações de memória/I/O, mas a falta de um contrato geral que posicione essas operações no tempo e permita que os dispositivos SMS avancem em ordem determinística.

**Runtime: inalterado nesta auditoria.** Não se deve implementar scheduler ou inserir offsets presumidos com base apenas nos totais nominais.

**Próxima etapa recomendada:** auditoria independente dos registradores/latches e do raster do VDP SMS, usando fontes Sega específicas e testes sintéticos. Só após esse estudo deve ser consolidada a ordem temporal CPU↔VDP e aberta uma PR de runtime pequena.

Estados finais desta auditoria:
- Execução funcional Z80: `IMPLEMENTED` (não equivale a certificação semântica total).
- Cobertura de trace temporal ED selecionada: `IMPLEMENTED` no escopo explicitado.
- Validação CI do SHA da auditoria: `SHORT_PASS` para o job executado.
- Scheduler mestre integrado: `INTEGRATION_PENDING`.
- Precisão temporal ciclo a ciclo: `TEMPORAL_PASS` não declarado.
