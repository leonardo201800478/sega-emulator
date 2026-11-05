# Consolidação arquitetural — Scheduler, SMS1 VDP e D3D12

**Data de referência:** 2026-11-02  
**Repositório:** `leonardo201800478/sega-emulator`  
**Base desta consolidação:** `58448485453825d6c4c7808c5694a3aec4fcf65d` (após merge da PR #393)  
**Natureza:** registro do estado implementado e dos contratos de integração; não é declaração de precisão temporal integral.

## 1. Resumo executivo

As três frentes possuem entregas reais no repositório, mas ainda são componentes desacoplados:

1. **Machine Scheduler:** implementa uma linha temporal determinística, domínios de clock racionais, fila de eventos, avanço até eventos, snapshots/restores e proteções contra eventos delta excessivos.
2. **SMS1 VDP 315-5124:** existe um módulo novo com identidade ASIC explícita, estado de memória/registradores, protocolo de portas, VRAM read-ahead, CRAM, composição funcional Mode 4 por scanline, sprites/status e armazenamento do valor fornecido ao latch H Counter.
3. **Video Runtime D3D12:** define frames imutáveis, mailbox de apresentação e backend Windows/DXGI/D3D12 capaz de receber um frame, converter RGB565 para RGBA8888 e apresentá-lo.

**Não existe ainda uma cadeia temporal/visual integrada Scheduler → Z80 → SMS bus → SMS1 VDP → VideoFrame → D3D12.** O SMS runtime ativo continua usando o VDP legado `SmsVdp`; o Scheduler não despacha callbacks de hardware e o novo `Sms1Vdp` ainda não é o dispositivo ativo da máquina. O renderer recebe frames, mas não os produz a partir do VDP SMS1.

Portanto:
- Scheduler: **IMPLEMENTED / QUICK_PASS / SHORT_PASS / LONG_PASS** conforme suítes de software já registradas; integração de máquina **INTEGRATION_PENDING**.
- SMS1 VDP: implementação funcional parcial; raster/fetch/IRQ/bus temporal **INTEGRATION_PENDING**.
- D3D12: backend de apresentação implementado e validado por CI Windows x64; integração com VDP e validação em GPU física pendentes.
- **TEMPORAL_PASS: NÃO DECLARADO.**

## 2. Autoridade temporal e responsabilidades

### 2.1 Master Scheduler

O crate `native/machine_scheduler` mantém uma única linha temporal de máquina. Os domínios registrados são identificados por `ClockDomainId` e usam razões racionais com fase própria. O avanço é calculado para todos os domínios em ordem canônica de ID.

A fila ordena eventos por:
1. timestamp;
2. estrato `HardwarePhase`;
3. prioridade de plataforma;
4. sequência de inserção estável.

A fase/priority é uma chave de ordenação determinística. **Não representa, por si só, uma fase elétrica ou prioridade real do hardware SMS1.** O perfil da máquina só deve atribuir precedência após evidência específica.

APIs e invariantes centrais:
- `register_clock_domain[_with_phase]`: registra domínio e fase sem duplicação;
- `schedule_at[_with_phase]`: agenda evento absoluto, rejeitando timestamp anterior ao presente;
- `dispatch_next`: calcula previamente os avanços, avança domínios e retorna evento + ciclos decorridos; não chama dispositivos;
- `advance_idle_to`: avança apenas se não pular evento pendente;
- `export_state / restore_state`: exporta/restaura estado completo com validação de versão, ordem canônica, timestamps, watermark e histórico de dispatch;
- `DeltaEventPolicy`: limita eventos no mesmo timestamp para impedir loop delta ilimitado. É política de segurança do scheduler, não limite físico.

A operação de dispatch não executa callbacks de hardware. A máquina proprietária deve consumir o evento e os avanços retornados, invocar os dispositivos na ordem definida e agendar os próximos eventos. A integração deve ser transacional: falha aritmética não pode deixar relógios parcialmente avançados ou evento removido.

### 2.2 Z80 e barramento SMS

O Z80 atual ainda executa instruções completas em `SmsMachine::step()`. A duração nominal agregada é aplicada ao VDP legado após o retorno da instrução. O código documenta explicitamente que essa contagem não é temporização de máquina cycle-accurate.

Há instrumentação temporal parcial do Z80 para oito instruções ED de transferência/comparação sem prefixos DD/FD, com amostras T-state/M-cycle no perfil documentado. Essa instrumentação é passiva e não dirige o Scheduler; não representa cobertura geral de opcodes, todos os pinos, WAIT, refresh físico, acknowledge e transições sub-T-state.

**Contrato obrigatório para integração:** a CPU deverá expor progresso interrompível por ciclos/fases de barramento ou micro-operações, permitindo que cada transação seja sincronizada com o timestamp correto. Não conectar o Scheduler diretamente ao `SmsMachine::step()` atual nem somar T-states apenas ao final da instrução.

WAIT, arbitragem CPU/VDP, contenção e prioridade de acessos simultâneos permanecem bloqueados até evidência adequada.

### 2.3 Separação de tempo emulado e tempo de host

- Scheduler: determina exclusivamente o tempo virtual e a ordem de eventos.
- Pacer/engine: quando integrado, relaciona tempo virtual ao relógio real; não decide quais operações de hardware são executadas.
- Video runtime: consome frames completos e os apresenta; fences e espera de GPU não podem bloquear avanço da máquina.
- Python/Qt: controle e apresentação de interface; não pode assumir timing, contagem de ciclos, barramento ou estado interno de chips.

## 3. SMS1 VDP — identidade e escopo

O primeiro alvo do VDP novo é exclusivamente o ASIC **SMS1 / 315-5124**. A revisão SMS2 / 315-5246 é uma etapa comparativa posterior. NTSC, PAL-M, PAL e perfil desconhecido são dimensões regionais/configuracionais distintas da identidade ASIC; não inferir a revisão pelo nome comercial do console.

O módulo `native/sms_core/src/vdp_sms1.rs` contém:
- `VdpRevision::Sms1_315_5124` e `VdpProfile` com região separada;
- 16 KiB de VRAM, 32 bytes de CRAM e 16 posições de registrador bruto;
- protocolo de portas de dados/controle, palavra de controle em dois bytes e buffer de read-ahead;
- endereço VDP de 14 bits e wrap;
- escrita CRAM com valor digital limitado a seis bits;
- leitura de status com clear e cancelamento de palavra de controle parcial;
- composição funcional Mode 4 de scanline para background e sprites, incluindo flags de status;
- estado `Option<u8>` para valor H Counter capturado por chamada explícita `capture_h_counter_on_th_transition`.

O latch H Counter armazena apenas o valor recebido. Ele não calcula H Counter, fase de dot, transição TH ou instante de captura. A leitura permanece indisponível até uma amostra ser fornecida; reset limpa o latch. A PR #393 integrou essa capacidade aditiva e seus testes.

### 3.1 Limites do modelo funcional atual

O novo VDP ainda não implementa:
- gerador temporal de raster/counters com fase SMS1 comprovada;
- pipeline de fetch por dot e arbitragem VRAM;
- avaliação/fetch de sprites com slots temporais;
- timestamps de status, HINT/VINT e linha IRQ;
- geração automática de WAIT;
- integração de transações com o Scheduler/Z80/barramento;
- publicação de frames reais do novo VDP para o Video Runtime.

A API `render_mode4_scanline` é uma composição funcional por linha. Não prova fetch cycle-accurate, latches intralinha ou pixel pipeline temporal. O módulo não deve ser descrito como VDP já conectado ao `SmsMachine`.

### 3.2 Evidência e incertezas SMS1

Fontes documentais e comunitárias são registradas com sua classe e limites. Referências iniciais:
- SMS Power — revisões SMS1/SMS2: https://www.smspower.org/Development/SMS1SMS2VDPs
- SMS Power — registradores e máscaras: https://www.smspower.org/Development/VDPRegisters
- SMS Power — sprites: https://www.smspower.org/Development/Sprites
- SMS Power — scanline counter: https://www.smspower.org/Development/ScanlineCounter
- SMS Power — manual de serviço: https://www.smspower.org/Development/SegaMasterSystemServiceManual
- SMS Power — documentação oficial Mark III/SMS: https://www.smspower.org/Development/SMSOfficialDocs

O manual de serviço publica frequências nominais de referência; isso não determina fase de reset, duty cycle, alinhamento de bordas ou ordem CPU/VDP. As regras de cadência de acesso de software não demonstram WAIT automático. Fontes comunitárias, código MAME/Ares/MEKA e notas com geometria proveniente de TMS9918/Genesis/Game Gear são pistas ou referências comparativas, não medições físicas do 315-5124.

Permanecem **UNKNOWN / BLOCKED** até evidência adequada:
- offset, escala, wrap e fase do H Counter;
- instante/polaridade de captura TH no SMS1;
- timestamp de transição do V Counter e fronteiras HBlank/VBlank;
- fase de latch dos registradores e timestamp de HINT/VINT;
- fetch slots, disputa de VRAM/CRAM e ordem de eventos simultâneos;
- ponto exato de atualização de overflow/collision;
- WAIT e arbitragem do barramento.

Não importar para SMS1 as contagens e timings do Game Gear, nem os modos extra-height conhecidos do SMS2. Não assumir 228 T-states por linha como fato físico apenas porque o VDP legado usa esse valor.

## 4. Video Runtime — contrato de frame e D3D12

O crate `native/video_runtime` separa contrato de frame e backend de apresentação.

### 4.1 Contrato independente de backend

`VideoFrame` é um frame completo, imutável e compartilhável por referência contada:
- formatos declarados: `Rgb565Le` e `Rgba8888`;
- largura/altura não nulas;
- stride explícito, que pode incluir padding;
- validação de overflow, stride mínimo e comprimento exato do armazenamento;
- leitura de linha ativa sem padding ou linha de armazenamento completa;
- número de sequência fornecido pelo produtor.

`FrameMailbox` é um mailbox limitado de uma posição. Aceita somente sequências estritamente crescentes. Se houver frame pendente, um frame mais novo o substitui e incrementa o contador de substituições. O consumidor retira o frame mais recente. O mailbox não altera tempo de máquina nem descarta eventos de emulação: coalesce apenas trabalho de apresentação que ainda não foi consumido.

O mailbox usa sincronização interna; não é correto qualificá-lo como lock-free. A garantia é que o produtor não espera pela conclusão da GPU. O caminho de apresentação pode ter sincronização própria.

### 4.2 Backend Windows D3D12

O módulo `d3d12` é condicionado a Windows e possui responsabilidade de host:
- janela Win32 e infraestrutura DXGI/swap chain;
- device e command queue D3D12;
- command allocator/list, RTVs e recursos de apresentação;
- upload de frames imutáveis do mailbox;
- conversão RGB565 para RGBA8888 quando necessária;
- cópia para back buffer e apresentação via DXGI;
- fences e sincronização de recursos no thread de apresentação.

A apresentação não é autoridade temporal. Qualquer espera de fence/GPU deve permanecer no thread de apresentação, nunca no Scheduler ou no stepping da máquina.

O exemplo `d3d12_frame_smoke` cria uma janela e apresenta um frame sintético 256×192 RGB565. Ele valida o caminho de host/transferência, não o VDP, pixels reais de jogo ou desempenho da GPU.

### 4.3 Limitações atuais do renderer

Fora do marco atual:
- integração com `Sms1Vdp` e extração de framebuffer real;
- resize/recriação de recursos;
- integer scaling, letterbox e política de aspect ratio;
- recuperação completa de device loss;
- lifecycle completo de thread/render host;
- validação visual em GPU física e medição de latência real.

O CI Windows compila o exemplo e executa testes unitários do crate em runner headless; isso não equivale a abrir janela ou validar apresentação em GPU física.

## 5. Contrato de integração entre as três frentes

O fluxo-alvo é:

```text
Master Scheduler (machine time)
        |
        +--> Z80 clock domain / CPU bus transactions
        |           |
        |           +--> SMS bus / mapper / I/O
        |                       |
        |                       +--> SMS1 VDP events and memory transactions
        |                       +--> PSG / other machine devices
        |
        +--> dispatch ordered machine events
                    |
                    +--> VDP emits completed pixels/frame
                                  |
                                  +--> VideoFrame (immutable)
                                            |
                                            +--> FrameMailbox
                                                      |
                                                      +--> D3D12 presentation thread
```

O diagrama representa a arquitetura-alvo, não o estado atual integrado.

### Regras de fronteira

1. Só a máquina/Scheduler define timestamps e resolve a ordem temporal.
2. Z80/bus deve expor transações em granularidade suficiente; instrução completa não é unidade suficiente para WAIT ou contenção.
3. VDP consome tempo/eventos fornecidos pela máquina; não cria clock independente.
4. Renderer consome frames concluídos; não lê registradores/VRAM em tempo real nem determina raster.
5. Mailbox pode substituir frames pendentes para apresentação, nunca operações, eventos ou frames usados como estado interno da emulação.
6. GPU fence, VSync e pacing do host não alteram timestamp virtual.
7. Estado de Scheduler, CPU, bus, VDP, PSG e pipeline em voo deve ser salvo/restaurado de forma coordenada antes de alegar replay temporal determinístico.

## 6. Matriz de status consolidada

| Componente/capacidade | Estado | Evidência / limite |
|---|---|---|
| Scheduler: clocks racionais e fase | IMPLEMENTED | Unidade de domínio e avanço determinístico; ainda sem perfil SMS completo |
| Scheduler: fila e ordenação | IMPLEMENTED | timestamp/phase/priority/sequence; prioridade física SMS1 não definida |
| Scheduler: snapshot/restore/codec | IMPLEMENTED | validações estruturais e replay/stress de software |
| Scheduler: dispatch integrado a dispositivos | INTEGRATION_PENDING | dispatch retorna eventos/avanços; não invoca callbacks |
| Z80: instrução interrompível por transação | INTEGRATION_PENDING | `SmsMachine::step()` ainda executa instrução inteira |
| SMS bus: WAIT/arbitragem temporal | INTEGRATION_PENDING | sem transações sincronizadas ao Scheduler |
| SMS1 VDP: portas/memória/read-ahead | IMPLEMENTED | testes funcionais unitários |
| SMS1 VDP: composição Mode 4 scanline | IMPLEMENTED parcial | não é pipeline dot-cycle |
| SMS1 VDP: H Counter latch storage | IMPLEMENTED | #393; caller/TH timestamp ausentes |
| SMS1 VDP: counters/raster/fetch/IRQ temporal | INTEGRATION_PENDING | offsets/fases/slots não comprovados |
| SMS1 VDP: ativo em SmsMachine | NOT INTEGRATED | runtime ainda consome VDP legado |
| VideoFrame/mailbox | IMPLEMENTED | testes de validação, sequência e substituição |
| D3D12 host | IMPLEMENTED | Windows x64 CI compila e testa; GPU física NOT RUN |
| D3D12 recebe frame real SMS1 | NOT INTEGRATED | smoke test usa frame sintético |
| Cadeia completa Scheduler→VDP→D3D12 | INTEGRATION_PENDING | não declarar integração end-to-end |
| TEMPORAL_PASS | NOT ACHIEVED | requer traces CPU/bus/VDP e oráculo apropriado |

## 7. Gates de validação

Cada resultado deve estar vinculado ao SHA candidato exato. Check ausente ou executado em SHA anterior é `NOT RUN`, não PASS.

### Gates de software

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```

### Gates específicos

- Scheduler: overflow/preflight, monotonicidade, chunk invariance, causal ordering, delta-event budget, snapshot/restore, replay e stress determinístico.
- Z80/bus: traces por T-state/M-cycle, transações de memória/I/O, WAIT, refresh, acknowledge e primeira divergência.
- SMS1 VDP: casos funcionais independentes; traces por região/ASIC; save/restore em meio à linha/fetch; primeiro pixel/evento divergente.
- D3D12: Windows x64 build/tests, smoke visual em GPU física, resize/device-loss e métricas separadas de renderização/apresentação.
- Integração: teste headless Scheduler+Z80+bus+VDP com replay determinístico e oráculo independente; depois validação visual com frame real.

### Critério de TEMPORAL_PASS

Somente declarar quando Scheduler, Z80 interrompível, barramento SMS, SMS1 VDP e eventos de IRQ estiverem integrados em uma linha temporal única; traces reproduzíveis por T-state/evento forem comparados com oráculo adequado; divergências forem resolvidas ou justificadas com evidência; save/restore e chunk invariance forem aprovados; e todos os gates aplicáveis passarem no SHA exato. CI verde isolado, framebuffer determinístico ou concordância entre emuladores não satisfaz esse critério.

## 8. Próximos marcos na ordem de dependência

1. **Auditoria arquitetural remota:** revisar o código consolidado Scheduler, Z80, bus, SMS1 VDP e D3D12; dependências, ownership, estados, workflows e regressões. Não fazer pull/build local antes da conclusão e aprovação desta auditoria.
2. **Contrato de transação Z80/bus:** definir API interrompível e timestamps sem callbacks Python por instrução; preservar comportamento funcional atual.
3. **Integração do Scheduler à máquina SMS:** registrar domínios e eventos com perfil explícito, sem inventar prioridades físicas.
4. **Ativar SMS1 VDP na máquina:** substituir o VDP legado somente após portar equivalentes funcionais e regressões; não remover o legado antes da prova de equivalência e do gate.
5. **Raster/fetch/IRQ:** implementar por fatias, exclusivamente quando cada comportamento tiver evidência suficiente.
6. **Publicação de frame real:** converter saída concluída do VDP em `VideoFrame` e entregar ao mailbox sem bloquear a máquina.
7. **Validação física:** traces SMS1 identificado e apresentação em GPU física; separar precisão do hardware de desempenho do host.

## 9. Proveniência documental consolidada

Conteúdo absorvido/reconciliado das propostas documentais das PRs #365, #366, #367 e #378:
- caracterização T0 do ASIC 315-5124;
- matriz T0.1 de testes funcionais, properties, transições, traces e casos bloqueados;
- protocolo de três frentes (evidência, implementação, verificação independente);
- registro de evidências temporais e limites de fontes secundárias;
- distinção entre documentação primária, medição física, teste independente, documentação comunitária, código de emulador, inferência e UNKNOWN.

As PRs documentais empilhadas não são incorporadas automaticamente: suas branches partem de bases históricas diferentes. Este documento é a consolidação atual sobre a main pós-#393. Antes de encerrar aquelas PRs, comparar novamente seus diffs com este registro e preservar qualquer evidência única que não esteja aqui.

Fontes de implementação:
- `native/machine_scheduler/src/lib.rs`
- `native/sms_core/src/vdp_sms1.rs`
- `native/sms_core/src/machine.rs` (runtime SMS atual e VDP legado)
- `native/video_runtime/src/lib.rs`
- `native/video_runtime/src/d3d12.rs`
- `native/video_runtime/README.md`

**Conclusão:** Scheduler, SMS1 VDP funcional parcial e host D3D12 estão estabelecidos como entregas separadas. O trabalho restante não é documentação cosmética: é integração temporal e visual real, sustentada por evidência. Até lá, manter `INTEGRATION_PENDING` e não reivindicar `TEMPORAL_PASS`.


## Atualização de implementação — 2026-11-05

Desde a base descrita no início deste documento, o lado de apresentação D3D12 foi ampliado e publicado na `main` pelas PRs #546/#547/#550/#553/#555/#558. Há agora uma FIFO lossless com backpressure na fronteira de frame completo, uma `D3d12Session` Rust-owned em thread dedicada, retenção FIFO durante minimize/restore, handling de DPI por janela, waits finitos com `GetDeviceRemovedReason` e recuperação one-shot para removed/reset. A PR #560 adicionou testes funcionais independentes para portas, VRAM read-ahead/wrap, CRAM, R0–R10, mirroring e status do novo `Sms1Vdp`.

Essas entregas **não alteram a conclusão de integração temporal** desta consolidação:

- O Scheduler não executa callbacks de hardware; a máquina consome seus eventos e coordena os dispositivos.
- `SmsMachine` ainda utiliza o VDP legado no caminho ativo.
- `SmsTemporalMachine` continua sem VDP/PSG conectados ao Scheduler.
- `Sms1Vdp` permanece funcional por scanline, sem raster advance/fetch arbitration/IRQ temporal/WAIT.
- A fila e a sessão D3D12 recebem frames completos, mas a cadeia `SmsMachine → MachineScheduler → Sms1Vdp → VideoFrame → D3d12Session` não está ligada.
- O gate T0/T0.1 315-5124 continua PARTIAL/BLOCKED para fase VDP/Z80, H-counter, fronteiras de linha/frame, fetch slots, contenção e WAIT real.

O CI dos sete HEADs integrados passou. Isso comprova os gates automatizados de compilação/testes; não substitui execução de janela em GPU física, injeção real de device loss, validação mixed-DPI, replay headless-vs-GPU ou hardware oracle. Esses itens permanecem `NOT RUN`.

Para os SHAs, merge commits, runs de CI e sequência de trabalho, consulte [Auditoria de integração D3D12 e testes SMS1 — 2026-11-05](../audits/video-runtime-d3d12-session-merge-audit-2026-11-05.md).
