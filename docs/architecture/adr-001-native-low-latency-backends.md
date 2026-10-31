> **Estado vigente (2026-10-27):** hardware SG-1000 ativo exclusivamente em Rust (`native/sg1000_core`). A aplicação Qt usa uma fachada Python fina para PyO3 e carregadores de ROM/perfil. Implementações de hardware Python, ferramentas e testes que dependiam delas foram arquivados sob `legacy/` e não são executados pelo projeto ativo. Descrições de estados anteriores neste documento são históricas, não instruções atuais.

# ADR-001 — Backends nativos com foco em acurácia, desempenho e baixa latência

- **Status:** Proposto para implementação incremental
- **Data:** 2026-10-27
- **Escopo inicial:** Windows, marco SG-1000; decisões reutilizáveis para os próximos sistemas
- **Não objetivo:** integrar todos os backends ou adicionar todas as dependências em uma única alteração

## Contexto e objetivo

O projeto pretende remover gargalos do caminho de execução, reduzindo a latência percebida de controles, áudio e vídeo sem sacrificar a reprodução determinística do hardware emulado. A arquitetura deve manter o núcleo de emulação independente do sistema operacional e dos backends de apresentação.

**Princípio central:** primeiro reduzir travessias de fronteira, cópias, bloqueios e trabalho por evento; depois medir e otimizar os pontos que o profiling provar serem caros. A escolha de uma API nativa, isoladamente, não garante menor latência.

## Decisões arquiteturais

### 1. Separação de responsabilidades

Manter três camadas com dependências unidirecionais:

1. **Núcleo de hardware Rust:** CPU, barramento/memória, I/O, VDP, PSG, scheduler, estados e sincronização temporal. Não depende de Qt, SDL, WASAPI, Vulkan ou APIs Win32.
2. **Runtime/host Rust:** recebe snapshots/eventos de entrada, avança a máquina em unidades explícitas, entrega PCM em blocos e disponibiliza frames/estado observável. Coordena os backends, sem definir regras de hardware.
3. **Adaptador da UI:** PyO3 expõe comandos de alto nível, estado e configuração. Qt/PySide6 continua responsável por menus, configuração e ciclo de vida da janela. Não há chamadas Python por instrução, acesso de memória, pixel ou amostra.

Backends de entrada, áudio e vídeo ficam em crates separados do núcleo, com features e dependências próprias. Não adicionar dependências de SO ou GPU ao crate de hardware SG-1000.

### 2. Contrato de fronteira Python–Rust

- Usar chamadas coarse-grained, por lote de execução/frame ou comando explícito; nunca uma chamada Python/Rust por instrução ou amostra.
- Não criar objetos Python no caminho quente. Converter configuração e comandos na fronteira.
- Evitar cópias de framebuffer e PCM. Preferir buffers de propriedade clara, reutilizáveis e com capacidade conhecida; medir qualquer cópia inevitável.
- Definir ownership, duração dos buffers, reset, pausa, shutdown e erros antes de compartilhar memória ou introduzir threads.
- Não liberar o GIL nem criar threads apenas por conveniência. Qualquer operação que rode sem o GIL deve ter ownership e sincronização demonstrados e testes de shutdown.
- A UI não deve ser obrigada a receber um framebuffer completo por cópia em todo frame se o renderer puder apresentar diretamente em uma superfície da janela Qt.

### 3. Entrada — SDL3 como padrão portátil; backends Windows opcionais

- **SDL3 (crate sdl3):** primeira opção portátil para gamepads/joysticks e eventos; iniciar apenas os subsistemas necessários. A integração não deve assumir que SDL é dono do loop de janela/eventos Qt.
- **XInput:** backend Windows opcional para controles Xbox compatíveis; usar bindings Win32 suportados pela versão fixada da crate windows.
- **DirectInput:** compatibilidade opcional com joysticks e dispositivos legados. Só implementar depois de confirmar a disponibilidade e a qualidade dos bindings da API clássica; não declarar suporte com base apenas na existência da crate windows.
- Capturar estado e eventos no host, convertê-los em um snapshot de entrada imutável e aplicá-lo no ponto de sincronização definido pelo scheduler.
- O instante real de chegada do evento é dado externo: registrá-lo separadamente do instante de aplicação em tempo emulado. Replays devem gravar a ordem e o ponto de aplicação, não depender do relógio do host.
- Evitar polling redundante em vários componentes. Uma única autoridade coleta dispositivos e publica snapshots/eventos.
- Tratar hot-plug, desconexão, foco da janela, controles ausentes e mapeamentos como casos testáveis.

**Meta de acurácia:** mesma sequência de snapshots e mesmos pontos de aplicação devem produzir o mesmo estado emulado. Não prometer latência de entrada específica antes de medi-la no Windows.

### 4. Áudio — WASAPI como padrão Windows

- **WASAPI (crate wasapi):** backend padrão no Windows, com modo compartilhado como escolha inicial de compatibilidade e modo exclusivo/opções de menor latência somente após teste explícito de dispositivo e configuração. Usar mecanismo event-driven quando suportado.
- **XAudio2 (crate windows):** backend Windows alternativo para comparação e compatibilidade; só mantê-lo se medições e requisitos reais justificarem a segunda implementação.
- **SDL3:** alternativa portátil ou backend de desenvolvimento, não motivo para duplicar caminhos sem benefício demonstrado.
- O núcleo gera amostras de áudio com temporização determinística e as entrega em blocos PCM. O backend de host apenas consome/resampleia/converte conforme necessário; não altera a lógica do PSG.
- Usar buffer circular limitado, com produtor e consumidor claramente definidos (SPSC quando houver exatamente um produtor e um consumidor). Não usar mutexes contenciosos, alocações, logging, I/O de arquivo ou chamadas Python no callback/trecho de tempo real.
- Dimensionar o buffer por medições de underrun e latência, não por uma constante arbitrária. Expor métricas de underrun, preenchimento, tamanho efetivo e atraso estimado.
- Separar taxa de amostragem do dispositivo, relógio emulado e política de resampling. Mudanças de dispositivo e drift do host não podem mudar o estado lógico da máquina.
- Em shutdown/reset, parar e drenar os produtores/consumidores com protocolo explícito; não liberar buffers enquanto o dispositivo ainda os usa.

### 5. Vídeo — Vulkan padrão; D3D11/D3D12 como opções

- **Vulkan (crate ash):** backend padrão planejado, por oferecer controle explícito; tratar unsafe, sincronização e lifetime de recursos como riscos de correção, não apenas de desempenho.
- **D3D11/D3D12** por bindings Win32 (crate windows) como backends alternativos. Não adotar wgpu como solução única se Direct3D 11 continuar requisito, pois a cobertura de backends precisa ser verificada na versão escolhida.
- Separar geração determinística do framebuffer/estado do VDP de sua apresentação. O backend gráfico nunca decide comportamento do VDP.
- Evitar readback GPU→CPU, alocações por frame e recriação de textura/swapchain em operação normal. Reutilizar recursos e buffers de upload; atualizar apenas dados necessários.
- Integrar com a janela Qt por uma superfície nativa válida e ciclo de vida documentado. Não criar uma segunda janela/event loop SDL apenas para apresentar a imagem.
- Começar com **um frame em voo** para reduzir fila e latência. Tornar a política de apresentação configurável: FIFO como padrão seguro sem tearing; MAILBOX quando suportado para baixa latência sem tearing; IMMEDIATE como opção explícita com possível tearing. Validar o comportamento real de cada driver.
- Evitar wait_idle/esperas globais em cada frame. Usar fences/semaphores e sincronização por recurso/frame, com destruição segura.
- Redimensionamento, DPI, minimização, perda/recriação de superfície e shutdown precisam ser tratados sem vazamentos ou deadlocks.

### 6. Scheduler e determinismo

- O scheduler Rust é a autoridade do tempo emulado. Backends recebem/produzem dados externos; não avançam clocks de hardware por conta própria.
- Representar timestamps de host e ticks/T-states emulados como domínios distintos, com conversão explícita quando necessária. Não misturar Instant diretamente com ciclos emulados.
- Aplicar entrada em pontos determinísticos e manter ordenação definida para eventos concorrentes.
- Áudio e vídeo podem ter threads de host, mas a execução da máquina deve ter ownership inequívoco. Nenhuma thread adicional sem objetivo medido, modelo de sincronização e testes de repetibilidade.
- Uma execução repetida com ROM sintética, eventos de entrada gravados e configuração igual deve reproduzir os mesmos checkpoints de CPU, memória/I/O, VDP, PSG e saída digital. Diferenças de apresentação do host não devem contaminar o estado emulado.

## Dependências candidatas

As versões são referências de pesquisa, não aprovação automática para produção. Confirmar versões estáveis, MSRV, features, licenças, changelog e compatibilidade com o toolchain do repositório no commit em que cada backend for introduzido; atualizar Cargo.lock e testar em CI.

| Área | Candidata | Papel e ressalvas |
|---|---|---|
| Entrada portátil | sdl3 | Gamepad/joystick; verificar API SDL3, subsystem init e convivência com Qt |
| Entrada Xbox | windows | XInput por bindings Win32; opcional |
| Entrada legada | API DirectInput confirmada separadamente | Não presumir suporte clássico apenas pelo crate windows |
| Áudio padrão Windows | wasapi | Event-driven; testar modo compartilhado/exclusivo e dispositivos reais |
| Áudio alternativo | windows / XAudio2 | Opcional, somente se benchmarking justificar |
| Vídeo Vulkan | ash | Controle explícito; maior responsabilidade de sincronização/unsafe |
| Vídeo D3D11/D3D12 | windows | Alternativas Windows; features específicas e validação de dispositivo |
| Abstração gráfica futura | Avaliar após protótipo | Não abstrair antes de confirmar os requisitos Vulkan + D3D11 + D3D12 |

Não centralizar essas dependências em sg1000_core. Usar crates host separados e features específicas; o build do núcleo puro deve continuar possível sem inicializar dispositivo de áudio, janela ou GPU.

## Plano de execução priorizado

### P0 — contrato e medição, antes de otimizar

- [ ] Registrar arquitetura atual, fronteiras Python/Rust e caminhos de framebuffer/PCM/entrada.
- [ ] Criar baseline reproduzível em build Release com workload sintético e ROM/homebrew legalmente obtida.
- [ ] Medir separadamente tempo de execução do núcleo, tempo de chamada PyO3, cópias de buffers, atraso de entrada, fila/underruns de áudio e latência de apresentação.
- [ ] Definir carga, warm-up, número de repetições, mediana, p95/p99 e variabilidade; guardar versão, commit, hardware, SO, driver e configuração.
- [ ] Fixar vetores de regressão/diferenciais antes de alterar comportamento.

### P1 — remover custos estruturais no caminho quente

- [ ] Auditar chamadas por instrução/byte/pixel/amostra entre Rust e Python; substituí-las por API coarse-grained.
- [ ] Estabelecer propriedade e reutilização de buffers de PCM/framebuffer; contar bytes copiados por frame/bloco.
- [ ] Fazer o núcleo Rust avançar a máquina e produzir saídas por blocos com scheduler explícito.
- [ ] Eliminar alocações/logging no loop crítico comprovadas por profiling.
- [ ] Manter uma referência independente para testes diferenciais; não manter dois runtimes de produção.

### P2 — entrada e áudio nativos

- [ ] Implementar SDL3 input com snapshot/eventos, timestamp, hot-plug e replay determinístico.
- [ ] Integrar WASAPI event-driven com ring buffer limitado e métricas de underrun/latência.
- [ ] Validar controle→snapshot→aplicação, sincronização do PSG, sample rate, reset, pausa e shutdown.
- [ ] Comparar WASAPI e XAudio2 somente com workload, formato, dispositivo e configuração equivalentes; manter apenas backends que agreguem valor.

### P3 — apresentação Vulkan integrada ao Qt

- [ ] Criar protótipo Vulkan que apresente um framebuffer fornecido pelo VDP sem alterar o estado do núcleo.
- [ ] Usar janela/superfície Qt, um frame em voo inicialmente e recursos reutilizados.
- [ ] Medir tempo de CPU, submit/present, fila de frames e latência; comparar políticas FIFO/MAILBOX/IMMEDIATE.
- [ ] Validar resize, DPI, alt-tab, minimização, drivers e shutdown antes de tornar Vulkan o padrão de produção.

### P4 — backends alternativos sob evidência

- [ ] Adicionar XInput e DirectInput somente conforme requisitos de dispositivos e cobertura comprovada.
- [ ] Adicionar XAudio2 se o comparativo mostrar vantagem ou compatibilidade necessária.
- [ ] Implementar D3D11/D3D12 se forem necessários para compatibilidade, estabilidade ou latência; compartilhar contratos, não forçar uma abstração universal prematura.
- [ ] Seleção/fallback deve ser explícita e registrada; nenhuma troca silenciosa durante uma sessão que altere comportamento.

### P5 — otimização orientada a profiling

- [ ] Usar profiling Release sem instrumentação invasiva para localizar hotspots reais.
- [ ] Testar uma mudança por vez e repetir a mesma matriz de regressões.
- [ ] Considerar SIMD, cache/layout de dados, batching adicional ou threads apenas se o perfil mostrar ganho e a equivalência funcional for demonstrada.
- [ ] Não introduzir JIT, paralelismo especulativo ou complexidade de lock-free sem evidência quantitativa.

## Critérios de aceitação e evidência

### Correção/acurácia — obrigatórios

- [ ] Testes de limites, erros, reset, interrupções, wraparound, ordem de eventos e repetibilidade nos componentes afetados.
- [ ] Diferencial com referência independente e relatório da primeira divergência.
- [ ] Comparar estado da CPU, flags/PC/SP/ciclos, memória/I/O, VDP/framebuffer e PCM/amostras nos mesmos checkpoints.
- [ ] Documentar o nível temporal validado; não declarar precisão ciclo a ciclo sem modelo temporal de barramento e testes específicos.

### Desempenho/latência — reportar, não presumir

- [ ] Baseline e candidato no mesmo hardware, build, workload, driver e configuração.
- [ ] Informar mediana e p95/p99 quando a amostra permitir, variabilidade, FPS/frame time, custo CPU, cópias, underruns e configuração do buffer.
- [ ] Medir latência de entrada→aplicação, geração→entrega de áudio e submit→present com método descrito; distinguir estimativa de medição de ponta a ponta.
- [ ] Rejeitar regressões de acurácia mesmo que melhorem FPS ou latência.
- [ ] Reportar separadamente **PASS**, **FAIL**, **BLOCKED** e **NOT RUN**. CI verde não substitui benchmark; benchmark rápido não prova correção.

## Riscos e decisões ainda abertas

- O loop de eventos do Qt e o polling de SDL precisam de ownership claro; dois loops de janela concorrentes não são aceitáveis.
- WASAPI em modo exclusivo pode reduzir buffering, mas prejudicar compatibilidade, mixagem e troca de dispositivo; não deve ser o padrão sem validação.
- Vulkan oferece controle, mas sincronização incorreta pode introduzir stutter, corrupção ou deadlocks. D3D11/D3D12 não devem ser implementados antes de haver testes de compatibilidade que os justifiquem.
- Um backend “mais nativo” não garante menor latência: filas de áudio, compositor, driver, vsync e buffers em voo podem dominar o resultado.
- A fronteira atual de SG-1000 ainda está incompleta. Esta ADR não declara que o runtime standalone, VDP, scheduler ou apresentação nativa já existam.
- **Estado atualizado:** os cores Python de hardware foram movidos para `legacy/python_reference/`; não são importados pelo runtime, testes ou workflows ativos. Não reativar esse caminho.

## Próximo passo

Concluir a validação da fatia atual de I/O Rust sem misturá-la a esta decisão. Em seguida, aprovar esta ADR e executar P0: mapear fronteiras e estabelecer baseline. Só depois implementar SDL3 input e WASAPI em PRs pequenos, seguidos do protótipo Vulkan/Qt.


## Decisão complementar — runtime de áudio Sega (2026-10-30)

**Status da decisão de áudio: Aceita e parcialmente implementada na PR #299.** WASAPI Shared/event-driven permanece padrão; quando disponível, IAudioClient3 consulta e negocia o período do Audio Engine. Se não suportado, o backend recua para Shared convencional. XAudio2 permanece como alternativa. Exclusive Mode continua investigação opt-in. A decisão detalhada e a matriz SG-1000 → SMS/GG → Mega Drive → Sega CD → 32X estão em [Runtime de áudio nativo](native-audio-runtime.md).

A seção de áudio anterior que descreve entrega de PCM em blocos ao host é apenas uma etapa intermediária. O contrato final proíbe drenagem, resampling, mixagem, cópia e escrita PCM em Python/PyO3 no caminho de gameplay. Rust deve possuir a sessão completa e os backends. Esta decisão não declara a migração implementada nem valida latência física.


### Atualização de implementação — IAudioClient3 e DSP (PR #299)

- A crate existente wasapi = 0.26 já expõe get_shared_mode_engine_period e initialize_shared_audio_stream; não foi adicionada segunda biblioteca COM.
- O worker WASAPI tenta IAudioClient3 em Shared/event-driven, limita o período alvo aos valores min/max e alinha ao período fundamental. Falha na consulta ou inicialização leva ao Shared/event-driven convencional com autoconversão.
- sega-audio-runtime oferece mixer DSP multi-fonte com ganho individual/mestre e scratch buffer reutilizável, além de passa-baixa opcional desativada por padrão.
- O mixer é independente do Windows e compartilhado por WASAPI/XAudio2. A sessão SG-1000/SMS o utiliza com a fonte PSG atual; fontes FM/PCM futuras não são declaradas implementadas.
- Filtros analógicos por chip permanecem desativados até calibração baseada em documentação/medições. Não há alegação de redução de latência sem testes físicos.
