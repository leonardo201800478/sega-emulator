# SMS VDP — auditoria integral, precisão temporal e backends gráficos nativos

**Tipo desta PR:** Draft de arquitetura e plano de implementação incremental.  
**Base:** `main` em `0da3098893cce4d80f7a22291d00488e4e7d0f9b`.  
**Runtime alterado nesta fatia:** não.  
**Estado inicial:** DESIGN / IMPLEMENTATION PENDING.  
**Precisão ciclo a ciclo:** não declarada.

## 1. Objetivo

Conduzir uma auditoria funcional, visual, temporal e de estado completa do VDP do Master System e preparar a migração integral do pipeline gráfico para Rust nativo, com backends explícitos para:

- Vulkan;
- Direct3D 11 (D3D11);
- Direct3D 12 (D3D12).

A implementação final não poderá depender de Python, PySide/PyQt, PyO3 ou callbacks Python no caminho de vídeo. O runtime Rust será proprietário do VDP, renderer, recursos gráficos, apresentação, sincronização de GPU e integração com o controle temporal da máquina.

Esta PR não afirma que os três backends ou a auditoria integral já estejam implementados. Ela fixa contratos, fases, evidências e critérios de aceite para impedir uma migração parcial apresentada como concluída.

## 2. Inventário e fatos já confirmados

### 2.1 VDP SMS atual

A implementação ativa está em `native/sms_core/src/vdp.rs`. O modelo já inclui:

- 16 KiB VRAM e 32 bytes CRAM;
- portas de dados/controle, latch de comando e read-ahead;
- Mode 4, modos legados compatíveis com TMS;
- scroll, prioridade, máscaras, sprites, overflow e colisão;
- perfis NTSC/PAL, alturas 192/224/240, VBlank, contador de linha e IRQ;
- saída RGB e recursos de Light Phaser/SegaScope.

Limitações já documentadas em `docs/architecture/sms-vdp.md`:

- renderização por scanline, não dot/T-state exact;
- ausência de contenção de VRAM;
- efeitos de registradores no meio da scanline não modelados genericamente;
- bordas subinstrução de IRQ não modeladas;
- H/V counter, revisões de ASIC e detalhes de região ainda aproximados;
- conversão CRAM digital, sem modelo analógico de encoder/composite.

### 2.2 Lacuna temporal CPU/barramento/VDP

A auditoria `docs/audits/sms-rendering-cpu-bus-audit-2026-10-29.md` registra que:

- Z80 retorna T-states agregados por instrução;
- acessos de memória e I/O ocorrem diretamente durante a instrução;
- `SmsMachine::step()` avança o VDP depois do término da instrução;
- escritas em registradores/VRAM/CRAM podem alterar o estado antes do avanço correspondente do VDP;
- traces existentes não constituem um scheduler que dirige os dispositivos.

A auditoria `docs/audits/sms-vdp-latches-raster-audit-2026-10-30.md` registra especificamente o tratamento parcial de R9 e a ausência de timestamp/latch horizontal geral para R8, VRAM e CRAM.

Essas são lacunas arquiteturais confirmadas. Não provam, isoladamente, a causa de cada jogo divergente.

### 2.3 Apresentação gráfica existente

`docs/architecture/vulkan-presentation-qt-quick.md` descreve um protótipo de apresentação Vulkan por Qt Quick/RHI, com cópia de framebuffer e fallback raster Qt. Esse caminho não é o runtime gráfico Rust-owned pretendido aqui e não atende ao requisito final de ausência de Python no pipeline.

`docs/architecture/migrations/native-video-runtime.md` é uma proposta anterior de migração. Esta especificação a especializa para o VDP SMS, três APIs gráficas e controle temporal nativo.

## 3. Separação obrigatória: hardware emulado versus apresentação GPU

A API gráfica do host **não é o VDP** e não poderá determinar o tempo do hardware emulado.

### VDP / hardware (Rust determinístico)

- Possui registradores, VRAM, CRAM, status, latches, sprites, IRQs e estado raster.
- Avança exclusivamente sob o scheduler/controle de ciclos da máquina.
- Executa fetches, composição de pixels e efeitos de hardware de acordo com eventos e fases temporais documentados.
- Produz um frame concluído ou blocos de pixels com ownership e timestamp definidos.
- Não chama Vulkan, D3D11, D3D12, Qt, Python ou APIs de host durante a execução de instruções/ciclos.

### Renderer e apresentação (Rust nativo)

- Consome snapshots imutáveis produzidos pelo VDP.
- Executa conversão/upload, filtros de apresentação, composição, escala e apresentação.
- Possui device/context/queue/swapchain e recursos gráficos sob ownership Rust.
- Não escreve em registradores, VRAM, CRAM, status ou relógios da máquina.
- Não avança, pausa ou corrige o scheduler para acompanhar VSync.
- Device loss, resize, minimize/restore e backpressure são eventos de host; não alteram retroativamente o estado emulado.

**Regra:** a timeline emulada governa a produção de frames; o relógio da GPU governa somente a apresentação física. VSync/VRR e frame pacing não são fontes de tempo do Z80/VDP.

## 4. Backends gráficos exigidos

Implementar três backends selecionáveis e identificáveis em runtime:

| Backend | API nativa | Plataforma-alvo inicial | Estado |
|---|---|---|---|
| Vulkan | Vulkan | Windows; expansão multiplataforma posterior | PLANNED |
| D3D11 | Direct3D 11 | Windows | PLANNED |
| D3D12 | Direct3D 12 | Windows | PLANNED |

Requisitos comuns:

- implementação em Rust; sem chamada de API gráfica via Python;
- seleção explícita e modo `auto` apenas quando a política de fallback estiver documentada;
- log do backend solicitado, backend efetivo, adapter/GPU, versão/feature level, fallback e motivo;
- gestão de surface/swapchain, resize, DPI, fullscreen, alt-tab, minimize/restore e encerramento;
- recuperação ou falha explícita em device loss;
- sincronização CPU/GPU por fences/semaphores e ownership de recursos;
- buffers de upload e apresentação sem mutação do framebuffer de origem;
- filtros e escalas com vetores de referência;
- captura de frame e diagnóstico de primeira divergência;
- nenhuma dependência de Qt Quick/QImage/QPixmap no caminho normal;
- fallback não pode ocultar falha de backend solicitado explicitamente.

Não assumir que uma única biblioteca Rust fornece equivalência de recursos para Vulkan, D3D11 e D3D12. A fase de spike deverá verificar suporte real por backend, maturidade, licenças, suporte Windows, custo de manutenção e interoperabilidade com a janela nativa. Se for necessária uma camada comum e adaptadores nativos específicos, isso deverá ficar explícito em ADR e nos crates.

## 5. Integração com o controle temporal de ciclos

O scheduler Rust é a única autoridade temporal por máquina.

A integração do VDP deverá evoluir para:

1. receber avanço até timestamp/tick solicitado pelo scheduler;
2. processar eventos internos de raster, fetch, IRQ e latch em ordem determinística;
3. sincronizar acessos de CPU/barramento com o VDP antes de aplicar efeitos colaterais;
4. representar efeitos temporais de registradores, VRAM e CRAM com instante definido;
5. publicar snapshots/frame concluído sem bloquear o scheduler à espera da GPU;
6. manter a apresentação assíncrona, limitada por filas com política explícita de backpressure;
7. reproduzir o mesmo estado digital e framebuffer para a mesma ROM, estado inicial e sequência de entradas, independentemente do backend gráfico ou da GPU instalada.

O renderer não poderá inserir waits de GPU na timeline lógica. A CPU não poderá avançar ou atrasar por latência de apresentação. Não executar callbacks por pixel/ciclo através de FFI.

A passagem da atual granularidade por instrução/scanline para precisão por T-state/dot deve ser guiada por documentação, traces e oráculos independentes. Não declarar cycle-accurate apenas porque o scheduler possui timestamps.

## 6. Auditoria integral do VDP SMS

A auditoria deve cobrir pelo menos os seguintes domínios.

### A. Estado e registradores

- R0–R10 e bits reservados/ignorados;
- reset e valores iniciais por perfil/revisão;
- leitura/escrita, efeitos colaterais, latches e read-ahead;
- endereçamento e wraparound de VRAM/CRAM;
- status, acknowledge, VBlank, HINT e IRQ;
- H-counter/V-counter por região, modo e revisão;
- modos 192/224/240 e combinações inválidas.

### B. Raster e temporização

- total de linhas e clocks por região/perfil;
- início/fim de display ativo, HBlank e VBlank;
- sequência de fetches de tile, atributos, padrões e sprites;
- scroll horizontal/vertical e locks;
- writes de R0–R10 durante linha e frame;
- writes de VRAM/CRAM durante fetch/display;
- latência de IRQ e bordas subinstrução;
- WAIT/contenda quando sustentados por evidência.

### C. Tile/background e paleta

- planos, name table, pattern generator e pattern attributes;
- 4bpp, planos de bits, flips e alinhamento;
- paletas, prioridade, backdrop, left-column mask;
- scroll fino/grosso e wrap;
- legacy modes e diferenças de comportamento.

### D. Sprites

- tabela de atributos e tabela de padrões;
- terminador, coordenadas, deslocamento e wrap;
- 8x8/8x16, zoom e limites;
- limite por scanline e política de overflow;
- prioridade, colisão e status;
- sprites ocultos, fora da tela e sobrepostos;
- comportamento de overflow/collision em cada perfil/revisão;
- testes de primeira divergência por sprite, linha e pixel.

### E. Saída visual e analogia

Separar claramente:
- saída digital RGB/CRAM;
- transformação de apresentação;
- filtros nearest/bilinear/integer scale;
- eventual simulação composite/encoder, que exige calibração e não poderá ser confundida com fidelidade digital do VDP.

### F. Diagnóstico de compatibilidade

O harness deverá registrar ROM por SHA-256, mapper/perfil, BIOS/revisão, região, configuração, sequência de input, checkpoints e SHA do código. Não versionar ROM comercial nem BIOS proprietária.

Estados de resultado separados:
- execução/boot;
- progresso visual;
- resposta a input;
- áudio;
- integridade de VDP/sprites;
- falha explícita.

Tela estática não é falha automática; atividade de CPU ou mudança de cor não é boot confirmado. Incluir input scriptado (botões 1/2 alternados), análise PCM e captura em múltiplos checkpoints. Reconhecer telas de erro conhecidas sem usar apenas heurísticas de número de cores.

## 7. Arquitetura de crates proposta

Reutilizar o workspace e crates existentes; não criar duplicatas:

- `native/sms_core`: estado e comportamento do hardware SMS, sem dependência de API gráfica.
- `native/temporal_core`: primitives de tempo/eventos existentes; evolução do scheduler de máquina sem criar relógio paralelo.
- `native/video_core` (nome a confirmar após inventário): contratos de frame, snapshots, formato e filtros determinísticos compartilhados.
- `native/video_vulkan`: backend Vulkan Rust.
- `native/video_d3d11`: backend Direct3D 11 Rust.
- `native/video_d3d12`: backend Direct3D 12 Rust.
- `native/video_host` (ou crate host existente): janela/surface nativa, seleção de backend, lifecycle e telemetria.
- `native/testkit` / ferramentas Rust: captura, replay, comparação e diagnóstico.

Os nomes são propostas, não autorização para duplicar crates existentes. Antes de criar qualquer crate, mapear a árvore e reutilizar ou renomear apenas com ADR e migração testada.

## 8. Fases e entregáveis

### F0 — Inventário e baseline

- [ ] Auditar árvore atual, crates, dependências, APIs e protótipos gráficos.
- [ ] Reconciliar PRs/branches de vídeo, scheduler e VDP ainda abertas.
- [ ] Fixar baseline de ROMs legais/homebrew, screenshots e traces.
- [ ] Reproduzir cada captura duas vezes e verificar determinismo.
- [ ] Registrar limitações e proveniência dos oráculos.

### F1 — Contrato temporal VDP

- [ ] Especificar interface de avanço por timestamp/evento.
- [ ] Mapear todos os acessos CPU/barramento relevantes.
- [ ] Definir prioridades, eventos simultâneos e efeitos colaterais.
- [ ] Definir latches e fetch schedule com evidência técnica.
- [ ] Criar testes sintéticos de raster e primeira divergência.

### F2 — Auditoria funcional/visual/temporal

- [ ] Fechar registradores, modos, tile/background, scroll, paleta e sprites.
- [ ] Adicionar testes de IRQ/HV counters e bordas temporais.
- [ ] Comparar com oráculos independentes (ex.: ARES/MAME, com versões e proveniência registradas) e material técnico primário.
- [ ] Documentar divergências por perfil/região/revisão.
- [ ] Não atualizar golden master sem justificativa e oráculo.

### F3 — Renderer Rust independente de API

- [ ] Definir frame snapshot imutável e ownership.
- [ ] Migrar filtros e transformações para Rust.
- [ ] Criar host/headless determinístico, sem janela/GPU.
- [ ] Comparar bytes/hash de framebuffer antes e depois.
- [ ] Demonstrar que o renderer não modifica a timeline nem o estado VDP.

### F4 — Vulkan nativo

- [ ] Implementar surface/device/swapchain/queues e sincronização.
- [ ] Integrar janela nativa sem Qt/Python no caminho de apresentação.
- [ ] Validar resize, fullscreen, alt-tab, device loss e shutdown.
- [ ] Comparar frame apresentado com snapshot de origem.

### F5 — D3D11 nativo

- [ ] Implementar adapter D3D11 e lifecycle.
- [ ] Validar feature levels, formatos, sincronização e recuperação.
- [ ] Rodar mesma suíte visual e de host do Vulkan.

### F6 — D3D12 nativo

- [ ] Implementar adapter D3D12, command queues/lists, descriptors e fences.
- [ ] Validar resource barriers, swapchain, resize e device removal.
- [ ] Rodar mesma suíte visual e de host dos demais backends.

### F7 — Integração temporal e host completa

- [ ] Integrar VDP, CPU/barramento e scheduler sem avanço por QTimer/Python.
- [ ] GPU apresenta somente frames produzidos pelo runtime.
- [ ] Filas limitadas com backpressure que nunca altera o tempo lógico.
- [ ] Replay idêntico com Vulkan/D3D11/D3D12 e headless.
- [ ] Validar SG-1000 como regressão obrigatória e SMS no Windows.

### F8 — Aceitação e retirada do legado ativo

- [ ] Testes automatizados, headless e integração Windows passam no mesmo SHA.
- [ ] Evidência em GPU dedicada e integrada quando disponível.
- [ ] Métricas distintas: tempo de emulação, renderização, submissão, apresentação, FPS lógico, FPS apresentado e latência.
- [ ] Remover caminhos Python/Qt Quick de vídeo somente após equivalência e validação.
- [ ] Manter fallback headless para CI e diagnóstico.
- [ ] Publicar matriz de maturidade por backend.

## 9. Critérios de aceitação obrigatórios

### Fidelidade e determinismo

- Mesmo input/configuração/estado inicial gera os mesmos eventos e framebuffer digital em headless, Vulkan, D3D11 e D3D12.
- Backend gráfico não altera CPU, VRAM, CRAM, IRQ, PCM ou timestamp emulado.
- Primeira divergência visual identifica frame/linha/pixel e estado relevante.
- Testes cobrem reset, BIOS handoff, mapper, input, raster, sprites, overflow/collision, VBlank/HINT e modos.
- TEMPORAL_PASS somente com trace e oráculo independentes adequados.

### Isolamento

- Nenhuma dependência de Python, PySide/PyQt ou PyO3 nos crates VDP, renderer, backends e scheduler.
- Nenhum callback Python por frame, scanline, pixel ou ciclo.
- Qt/Python não transporta framebuffer/PCM no caminho normal.
- O host gráfico não possui autoridade sobre o tempo emulado.

### Qualidade

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `python -m pytest`, Ruff check e Ruff format permanecem gates do frontend existente, sem adicionar integração Python ao vídeo.
- Testes headless sem GPU e testes Windows reais por backend.
- Nenhum warning ativo; nenhuma aprovação baseada em workflow ignorado ou SHA anterior.

## 10. Regras de implementação

- Branch + PR; nunca commit direto em `main`.
- Fatias pequenas, compiláveis e com testes próprios.
- Não alterar workflows CI existentes sem autorização específica.
- Não iniciar build Release/publicação sem autorização explícita.
- Não introduzir JIT, frame skip, paralelismo especulativo ou compute shader para emular hardware.
- Não sacrificar determinismo para desempenho.
- Não remover o renderer atual até o backend substituto estar validado.
- Documentar cada função pública e cada contrato temporal novo.
- Não declarar implementação, QUICK_PASS, SHORT_PASS, LONG_PASS ou TEMPORAL_PASS sem evidência no SHA candidato.

## 11. Estado desta PR

Esta primeira revisão registra o escopo e a arquitetura. **Nenhum backend Vulkan/D3D11/D3D12 foi implementado nesta fatia; nenhuma auditoria runtime foi executada por esta documentação.**

- Inventário documental das lacunas VDP/temporal: PASS (fontes existentes citadas neste documento).
- Alteração de runtime: NOT RUN.
- Testes Rust/Python: NOT RUN.
- Testes em GPU real: NOT RUN.
- TEMPORAL_PASS: NOT CLAIMED.

A implementação começa por F0/F1 e só então avança para renderer e backends, sem misturar correções semânticas do VDP com mudanças de apresentação.


## 12. Auditoria específica de sprites e scroll — refinamento

### Evidência de hardware

A documentação SMS Power estabelece que o SAT possui até 64 entradas; o hardware seleciona no máximo oito sprites para uma linha. O Y efetivo começa em Y+1; em 192 linhas, $D0 encerra a tabela; o bit 3 de R0 desloca sprites oito pixels para a esquerda. O modo 8x16 força o índice do tile para par. O zoom possui diferenças entre SMS1, SMS2 e Game Gear.

A referência MAME descreve parsing da SAT na linha anterior à linha exibida, buffer de oito índices e leitura em tempo real dos dados de sprite. O comportamento de zoom do SMS1 difere do SMS2/GG. Essas regras precisam ser codificadas por perfil/revisão quando a evidência permitir, e não achatadas em uma regra genérica.

### Regra de coordenadas

- R8/R9 controlam o plano de fundo; não devem ser subtraídos das coordenadas X/Y dos sprites.
- O jogo atualiza as coordenadas SAT dos objetos de mundo durante o scroll da cena; o VDP deve apresentar exatamente a coordenada de tela resultante da SAT, sem aplicar o scroll de background uma segunda vez.
- A avaliação N/N+1, o latch de oito índices e as leituras de atributos/padrões devem ser eventos do VDP, sincronizados ao scheduler.
- Writes na SAT durante uma linha não podem ser tratados como se todos os atributos tivessem sido amostrados simultaneamente no fechamento da scanline.

### Opção de limite de sprites

O SMS Rust já possuía `SpriteLimitMode::{HardwareLimit, UnlockedCompatibility}`, mas o caminho de configuração não estava exposto pelo adaptador nativo SMS. Nesta revisão, a opção foi propagada por:
- `SmsMachine::set_sprite_limit_mode()` e `sprite_limit_mode()`;
- `NativeSMSMachine.set_sprite_limit_enabled(bool)` e getter;
- `SMSMachine.set_sprite_limit_enabled(bool)` e getter.

O padrão permanece **limite de hardware ativo**. Ao desativar a opção, o renderer pode desenhar mais de oito sprites em Mode 4 (e mais de quatro nos modos legados), mas deve preservar o status OVERFLOW do hardware. A opção é uma extensão de compatibilidade, não uma representação do VDP físico. A integração automática com a preferência persistida do diálogo Qt ainda precisa ser ligada e testada ponta a ponta.

### Vetores adicionados nesta revisão

- Sprite Mode 4 em X=100 com R8 variando entre 0, 1, 3, 7, 8, 15, 127 e 255: a coordenada do sprite deve permanecer fixa, demonstrando que scroll de background não desloca sprite.
- Nove sprites ativos na mesma linha: modo limitado não desenha o nono; modo desbloqueado desenha o nono e ambos mantêm OVERFLOW.

**Limite da evidência:** esses vetores são testes sintéticos de regressão do renderer atual. Eles não provam o pipeline N/N+1, a temporização de fetches, a correção de sprites em ROMs comerciais ou a precisão de cada revisão ASIC. Os testes precisam ser executados no SHA candidato e comparados com ARES/MAME e hardware quando disponível.

### Referências de pesquisa

- SMS Power, Sprites: https://www.smspower.org/Development/Sprites
- SMS Power, VDP Registers: https://www.smspower.org/Development/VDPRegisters
- MAME, 315_5124.cpp: https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp
