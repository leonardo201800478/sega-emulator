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
