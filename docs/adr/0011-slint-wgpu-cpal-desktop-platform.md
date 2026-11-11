# ADR 0011 — Replataforma desktop Rust com Slint, WGPU e CPAL

- **Status:** ACCEPTED FOR PROOF-OF-CONCEPT; produção bloqueada pelos gates desta ADR.
- **Data:** 2026-11-10.
- **Baseline auditada:** `main` = `392a5c67e85a11a5a980d8f7055bcc8a12fe3784`.
- **Escopo:** substituição da aplicação egui/eframe e dos adaptadores de apresentação/saída de áudio; preservação do runtime de emulação Rust.
- **Precedência:** esta ADR substitui as decisões de tecnologia de interface da ADR 0010 e a decisão de Vulkan nativo/`ash` da ADR 0001. Não substitui as decisões de CPU, VDP, PSG, Scheduler, barramentos, ownership ou determinismo.

## 1. Decisão executiva

O projeto será replatformado dentro do repositório existente. Não será criado um segundo repositório nesta etapa: a história, os testes, as matrizes Z80, os dados DAT, a documentação e o runtime atual permanecem na mesma fonte de verdade.

A nova stack de aplicação será:

| Responsabilidade | Tecnologia normativa |
|---|---|
| Interface desktop | Slint 1.18.x, Rust |
| API gráfica única | WGPU 30.x |
| Backend gráfico preferencial inicial | Vulkan, através do backend WGPU |
| Saída de áudio | CPAL 0.18.x |
| Entrada | Adaptador atual preservado durante a migração; SDL3 continua sendo a direção portátil sujeita à auditoria do código existente |
| Hardware e runtime | Rust existente, sem reimplementação |
| Comunicação UI/runtime | Comandos coarse-grained e snapshots/frames de propriedade clara |

WGPU é a única API gráfica usada pelo código do produto. Vulkan, Direct3D 12 e Metal são backends internos possíveis do WGPU; não serão implementados como APIs independentes da aplicação. No alvo inicial Windows x64, Vulkan será o backend preferido e será explicitamente identificado nos testes. Uma troca automática silenciosa de backend não poderá mascarar falhas do caminho Vulkan.

CPAL será a única API de saída de áudio da aplicação. No Windows, CPAL utiliza WASAPI; isso é um backend interno da biblioteca, não uma segunda API implementada pelo emulador.

## 2. Fundamentação e versões auditadas

Versões verificadas na documentação/repositórios oficiais em 2026-11-10:

- Slint **1.18.1**; release 1.18 publicada em 2026-09-16.
- WGPU **30.0.1**.
- CPAL **0.18.2**; release de 2026-08-16.

Referências:
- Slint releases: https://www.slint.dev/blog/slint-1.18-released
- Slint backends/renderers: https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/
- Slint WGPU 30 integration: https://docs.slint.dev/latest/docs/rust/slint/wgpu_30/
- WGPU: https://wgpu.rs/doc/wgpu/
- WGPU HDR/color spaces: https://docs.rs/wgpu/latest/wgpu/
- CPAL: https://docs.rs/cpal/0.18.2/cpal/

As versões exatas serão fixadas no `Cargo.lock`; não usar dependências gráficas/áudio sem versão reproduzível. A atualização de major/minor será uma mudança deliberada, acompanhada por revisão de changelog, MSRV, licença e testes.

## 3. Slint e a fronteira da interface

Slint será responsável por:
- janela principal, menus, toolbar, diálogos e atalhos;
- biblioteca/catalogação visual de jogos, pesquisa, ordenação e filtros;
- telas de configuração de vídeo, áudio e controles;
- telemetria, logs e estado visual da sessão;
- apresentação do viewport dentro da composição da janela.

Slint não poderá:
- executar instruções de CPU ou avançar o Scheduler;
- implementar VDP, PSG, barramentos, mappers ou timing;
- possuir diretamente a máquina emulada;
- executar callbacks por instrução, M-cycle, T-state, pixel ou amostra;
- bloquear a thread de UI esperando a conclusão de uma operação de hardware.

A UI envia somente comandos de alto nível (`load_rom`, `start`, `pause`, `stop`, `reset`, `step_frame`, configurações e comandos de debugger). O runtime publica snapshots imutáveis, telemetria e frames completos.

A thread proprietária da máquina permanece responsável pelo `SmsTemporalMachine`, CPU, Scheduler, VDP, áudio e avanço do tempo. A UI nunca recebe referências mutáveis a esses objetos.


### 3.1 Política normativa de pausa do menu

A abertura de qualquer menu de sistema do emulador (incluindo menu principal, overlay, configurações e menu invocado por Guide) pausa a emulação, tanto em fullscreen quanto em modo janela. Essa regra é independente da composição visual e vale para todos os sistemas suportados.

- O evento de abertura solicita a pausa imediatamente; o runtime efetiva a suspensão na primeira fronteira segura e determinística, sem interromper uma operação de barramento ou transação temporal pela metade.
- Depois de confirmada a pausa, nenhum ciclo, instrução, evento do Scheduler, avanço de VDP/PSG, frame emulado ou produção de áudio emulado pode progredir.
- O último frame completo permanece apresentado. A saída de áudio do host deve ser silenciada/interrompida de forma controlada, sem avançar os chips nem descartar estado temporal; buffers já enfileirados devem ser drenados ou descartados segundo política explícita do adaptador.
- O estado integral da máquina e do Scheduler é preservado. Fechar o menu retoma a partir desse estado, sem reset, compensação artificial de ciclos ou avanço retroativo do relógio virtual.
- A pausa do menu utiliza o mesmo mecanismo central de pausa da sessão; a UI não implementa uma segunda pausa paralela.
- O estado de execução anterior ao menu deve ser preservado: fechar um menu não pode iniciar uma sessão que já estava pausada por outro motivo.
- O botão Guide é um comando do host e nunca é encaminhado ao controle emulado. Enquanto o menu estiver aberto, os comandos de navegação são consumidos pela UI.

A pausa da emulação não exige suspender a thread de UI, o renderer ou a apresentação do último frame. O host continua responsivo para desenhar e navegar no menu, mas o tempo virtual da máquina permanece parado.

### 3.2 Risco conhecido: integração Slint/WGPU

A integração externa WGPU 30 do Slint é atualmente exposta sob `unstable-wgpu-30`. A API pode mudar em uma atualização minor do Slint. Portanto:

- fixar Slint na série 1.18.x e WGPU em 30.0.1 durante o POC;
- usar o mesmo `wgpu::Device` e `wgpu::Queue` para o renderer Slint e para os recursos do viewport;
- avaliar `BackendSelector::require_wgpu_30`, `WGPUConfiguration::Manual`, `Window::set_rendering_notifier` e importação de `wgpu::Texture` como os caminhos de integração;
- documentar explicitamente o ownership e o ciclo de vida de textures, views, surfaces e recursos importados;
- não atualizar Slint/WGPU sem repetir os testes de integração gráfica.

**Gate obrigatório:** a prova de conceito deve demonstrar uma janela Slint com menus e uma textura produzida pelo renderer do emulador, compartilhando device/queue, sem janela filha nativa ou segunda API gráfica, sem cópias completas redundantes na CPU; um upload CPU→GPU por frame é esperado pelo contrato atual de `VideoFrame`.

Se esse gate falhar, a arquitetura não será declarada aprovada para produção. Não será permitido esconder o problema adicionando um segundo renderer ou outra API sem uma nova ADR.

## 4. API gráfica única: WGPU

O código do produto utilizará WGPU para:
- criação/seleção de adapter, device, queue e surface;
- upload de frames e gerenciamento de texturas;
- composição do viewport e apresentação;
- escalonamento 1:1, 4:3, 16:9 e integer scaling;
- shaders de pós-processamento;
- sincronização, frames-in-flight e modos de apresentação.

Não adicionar `ash`, `ash-window`, bindings diretos de D3D11/D3D12 ou outro renderer ao caminho de produção nesta fase. Chamadas nativas só poderão existir atrás de uma nova ADR caso uma necessidade comprovada não seja exposta por WGPU.

### 4.1 Política de apresentação

- Consultar as capacidades reais da surface/adapter; nunca assumir que um formato ou modo é suportado.
- FIFO é o fallback seguro.
- Mailbox/Immediate só podem ser escolhidos se anunciados como suportados e após validação do comportamento.
- Iniciar com baixa quantidade de frames em voo (alvo inicial: 1); medir latência e throughput antes de alterar.
- Redimensionamento, minimização, restauração, fullscreen, troca de monitor, perda/recriação de surface e shutdown são estados explícitos.
- A apresentação não pode alterar o Master Clock, a ordem de eventos, a execução da CPU, o estado do VDP ou a sincronização lógica do áudio.
- É proibido frame skipping no runtime. A política de apresentação pode substituir frames ainda não apresentados somente quando isso não descarte execução ou estado emulado; qualquer comportamento desse tipo precisa ser documentado e testado.

### 4.2 HDR e VRR — limites conhecidos

WGPU 30 expõe espaços de cor de surface, incluindo BT.2100 PQ/HLG, e capacidades de formato/cor. O shader final é responsável por conversão de gamut, encoding PQ/HLG e tone mapping; WGPU não realiza automaticamente esse processamento.

**Limitação importante:** a documentação atual de WGPU declara que `DisplayHdrInfo` descreve a tela, mas WGPU não escreve os metadados estáticos HDR (`vkSetHdrMetadataEXT` / DXGI `SetHDRMetaData`). Portanto:
- HDR fica inicialmente desativado;
- HDR PQ só poderá ser oferecido quando formato e color space forem anunciados pela surface;
- a ausência de metadados estáticos deve ser registrada como limitação, não ocultada;
- não anunciar conformidade completa HDR10 com metadados até existir caminho validado;
- HDR10+ dinâmico não é requisito desta fase.

WGPU oferece modos de apresentação, mas isso não garante G-Sync/FreeSync/VRR. VRR depende do SO, driver, compositor, monitor e modo de apresentação. A comprovação será feita em hardware real e separada de qualquer alegação baseada apenas em enumeração de capacidades.

## 5. Saída de áudio única: CPAL

CPAL será a única API de saída de áudio do host. A versão inicial é 0.18.2.

O crate existente `native/audio_runtime` permanece responsável por DSP/mixagem e pelos dados de áudio derivados dos chips. O crate `native/audio_output` será migrado para CPAL, preservando contratos úteis de PCM e ring buffer, mas removendo WASAPI/XAudio2 como implementações concorrentes mantidas pelo produto.

No Windows, CPAL usa WASAPI; em Linux usa ALSA (com backends opcionais conforme configuração) e em macOS usa CoreAudio. Não implementar chamadas diretas WASAPI/XAudio2 paralelas à CPAL.

Regras do callback:
- nenhuma alocação dinâmica;
- nenhum mutex contencioso, I/O, logging bloqueante ou chamada de UI;
- nenhuma execução de chip ou avanço do Scheduler;
- consumir dados de um buffer limitado com ownership SPSC claro;
- contabilizar underruns/overruns, tamanho efetivo de buffer e falhas de dispositivo;
- tratar mudança/desconexão de dispositivo e reconfiguração de formato sem corromper a máquina;
- conversão/resampling do host não pode alterar o estado temporal dos chips.

A geração PCM dos chips permanece determinística e independente do sample rate físico. A thread CPAL consome blocos já produzidos; nunca determina quando o PSG/YM avança.

## 6. Runtime que será preservado

A migração não é uma reescrita do emulador. O baseline de runtime no SHA auditado contém, entre outros:

- `native/z80_cpu`, `native/z80_alu`, `native/z80_bus`;
- `native/temporal_core`, `native/machine_scheduler`;
- `native/sms_core`, `native/sg1000_core`;
- `native/sn76489`, `native/audio_runtime`;
- `native/video_core`, `native/video_runtime`;
- `native/input_runtime`, `native/debugger_core`;
- `rust/runtime/sms-runtime`.

Esses componentes não serão reescritos por causa da troca de UI, renderer ou saída de áudio. Correções de hardware continuam exigindo evidência própria e PRs separadas.

O worker SMS e seu contrato de comandos limitados/snapshots serão preservados. O SG-1000 validado não será removido nem convertido em fallback silencioso para SMS.

### 6.1 Componentes de host substituíveis

- `native/sega_app`: frontend egui/eframe atual; será congelado como baseline temporária e substituído por um novo app Slint. Não continuar adicionando funcionalidades ao egui.
- `native/audio_output`: adaptador de saída atual com WASAPI/XAudio2; será substituído por CPAL mantendo `audio_runtime`.
- `native/video_runtime` e mailbox de frames: preservar o contrato de frames completos; alterar somente o consumidor/apresentador quando necessário.
- `native/input_runtime`: manter sem mudanças na primeira fatia de replatform. A escolha de backend físico de entrada será auditada separadamente; não misturar uma migração de input com a troca de GUI, vídeo e áudio.

A nova aplicação será criada inicialmente em `native/sega_desktop` para coexistir com o frontend antigo durante a prova de conceito. Após paridade funcional e validação, haverá uma troca controlada de entrypoint/packaging; o frontend egui será removido em PR própria.

## 7. Organização alvo

Estrutura conceitual após a migração:

```text
native/
  sega_desktop/       # Slint: janela, menus, biblioteca e composição da aplicação
  video_output_wgpu/  # WGPU: surface, textures, shaders e apresentação
  audio_output/       # CPAL: dispositivos, stream e callback de saída
  audio_runtime/      # DSP/mixagem e contrato de áudio — preservado
  input_runtime/      # entrada e snapshots — preservado na primeira fase
  sms_core/           # máquina SMS temporal — preservada
  sg1000_core/        # máquina SG-1000 — preservada
  z80_cpu/            # CPU compartilhada — preservada
  machine_scheduler/  # Scheduler — preservado
  video_core/
  video_runtime/
rust/
  runtime/sms-runtime/ # worker e ownership da máquina — preservado
```

O objetivo final é um workspace Rust coerente, com um único lockfile e versões centralizadas. A consolidação dos workspaces standalone e do crate atualmente excluído será feita somente depois que o POC compilar, para não combinar mudança de arquitetura com reestruturação de build sem evidência.

## 8. Sequência de migração

### R0 — Congelamento e auditoria
- registrar a baseline e os consumidores do app/áudio/vídeo;
- congelar novas features egui e APIs nativas paralelas;
- manter testes, runtime, docs e SG-1000 intactos;
- reconciliar README, ADRs, roadmap, scripts e workflows com o estado real da main.

### R1 — Prova de conceito gráfica/desktop + HUD end-to-end
- Slint 1.18.x com WGPU 30.x e backend Vulkan explicitamente selecionado no Windows x64;
- executar uma ROM homebrew/sintética de SMS no SmsTemporalWorker real; não aceitar somente uma textura gerada artificialmente como prova de integração do emulador;
- criar o viewport WGPU na mesma janela Slint e compartilhar wgpu::Device/wgpu::Queue; não criar child HWND ou janela de overlay;
- consumir os VideoFrame completos e imutáveis pela mailbox já existente, desacoplando a publicação do frame do backend D3D12 atualmente acoplado ao worker;
- transferir o frame CPU para textura GPU uma vez por frame, sem cópias completas intermediárias no lado CPU; preservar execução de todos os ciclos e eventos do Scheduler;
- fullscreen borderless inicial, redimensionamento, minimização/restauração, mudança de proporção e shutdown repetido;
- imagem 4:3 centralizada sem crop/stretch em monitor 16:9; não reservar gutters nem alterar a área do jogo para HUD;
- HUD mínimo: exibir somente o FPS de emulação, obtido de `SmsPerformanceMetrics`, em texto discreto sobre o viewport; não exibir FPS de apresentação, frame time, input-to-frame, resolução, proporção ou outros indicadores;
- Não exibir informações de GPU, backend, present mode, refresh rate, HDR, VRR ou áudio na HUD da POC;
- botão SDL3 SDL_GAMEPAD_BUTTON_GUIDE abre/fecha o menu; borda de pressionamento evita alternância repetida; Guide nunca é enviado ao controle emulado;
- menu translúcido sobre o último frame apresentado; ao abrir o menu, o host solicita imediatamente a pausa da máquina e o Scheduler interrompe o avanço em uma fronteira segura e determinística. CPU, VDP, PSG, áudio emulado e demais domínios temporais permanecem congelados enquanto o menu estiver aberto; D-pad e botões de confirmação/cancelamento navegam no menu e são consumidos pelo host;
- testar aspect ratio 4:3 e 16:9, navegação integral pelo gamepad, pausa efetiva ao abrir o menu e retomada do SMS a partir do estado exato preservado ao fechá-lo;
- nenhuma alteração de semântica, clock ou Scheduler do runtime SMS.

### R2 — Prova de conceito de áudio e integração com o SMS
- CPAL 0.18.x por meio da interface AudioSession já aceita pelo SmsTemporalWorker;
- enumerar dispositivos, selecionar saída e negociar formato;
- primeiro validar PCM sintético; depois consumir o PCM real produzido pelo audio_runtime do SMS;
- medir sample rate efetivo, buffer, latência reportável, underruns/xruns, frames descartados e estabilidade em 44,1/48/96 kHz quando suportados;
- callback de áudio sem alocação, bloqueio, logging ou acesso à UI; callback consome somente o ring buffer pré-alocado;
- testar unplug/replug, pausa, reset, troca de ROM e shutdown;
- comparar os resultados com a saída atual antes de removê-la.

### R3 — Integração do runtime existente
- conectar o worker SMS atual à UI Slint;
- consumir frames completos da mailbox existente;
- conectar CPAL ao fluxo PCM do áudio runtime;
- preservar ownership do worker e comandos coarse-grained;
- validar que a UI não bloqueia e não avança hardware.

### R4 — Interface de produto
- biblioteca de jogos, filtros, busca, capas e histórico;
- menus e configurações;
- input/perfis e hot-plug;
- debugger e telemetria;
- persistência e recuperação de configurações;
- UX responsiva e acessibilidade.

### R5 — Apresentação avançada e comprovação de display
- proporções, integer scaling, shaders em passadas combináveis;
- VSync e modos Mailbox/Immediate conforme capacidades reais da surface;
- VRR/G-Sync/FreeSync: teste em monitor e driver reais, com variação controlada de FPS e confirmação externa por OSD/telemetria do driver; WGPU PresentMode suportado não será confundido com VRR efetivamente ativo;
- HDR: consultar SurfaceCapabilities::format_capabilities, SurfaceColorSpace e Surface::display_hdr_info; executar padrão de teste PQ/BT.2020 em fullscreen, confirmar sinal HDR no monitor/OS e validar tone mapping;
- registrar que WGPU 30 não configura metadados estáticos HDR10; resultado deve ser descrito como teste de saída PQ/HDR disponível, não como conformidade HDR10 completa com metadados;
- testes de cores, gamut, luminância e retorno seguro para SDR.

### R6 — Corte definitivo
- atingir paridade funcional documentada;
- remover egui/eframe, D3D12 direto e WASAPI/XAudio2 diretos do produto;
- consolidar workspace/lockfile, scripts, empacotamento e CI;
- fechar PRs antigas substituídas pela nova arquitetura;
- manter somente uma implementação de cada subsistema host.

## 9. Gates obrigatórios

Nenhuma fase avança por aparência visual ou compilação isolada.

**Gates Rust (Release):**
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- build Windows x64 Release do aplicativo;
- validação Linux Release dos componentes portáveis;
- testes específicos de GPU e áudio nos targets aplicáveis.

Não usar build Debug como evidência de desempenho. A validação de correção pode executar testes unitários Release; a validação gráfica precisa incluir Windows x64 com drivers reais.

Classificar cada suíte como PASS, FAIL, BLOCKED ou NOT RUN. SKIPPED nunca equivale a PASS.

**Gate WGPU/Slint + HUD:**
- device/queue compartilhados comprovados e backend Vulkan efetivamente identificado;
- SMS temporal real produzindo frames no viewport;
- nenhuma janela overlay/child HWND para viewport ou menu;
- menu acionado por Guide, navegação pelo controle e retorno ao jogo sem perda de input ou alteração indevida do estado da máquina;
- telemetria distinguindo FPS de emulação, FPS de apresentação e refresh do monitor;
- aspect ratio 4:3 e 16:9 sem distorção ou crop; HUD sempre discreta, sobreposta em um canto seguro, sem gutters, sem reservar espaço e sem alterar o viewport;
- sem vazamento de recursos ou erro de sincronização;
- resize/minimize/fullscreen/recreate/shutdown repetidos;
- sem cópias completas redundantes no lado CPU; um upload CPU→GPU por frame é esperado pelo contrato atual de VideoFrame.

**Gate CPAL:**
- callback sem alocação/bloqueio;
- PCM real do SMS audível e contínuo;
- ausência de underruns/xruns sob carga sustentada dentro do buffer alvo;
- dispositivo removido/reconectado sem travar o worker;
- saída PCM de referência determinística antes do host;
- shutdown seguro.

**Gate VRR/HDR:**
- registrar GPU, driver, SO, monitor, conexão, modo de tela e refresh selecionado;
- VRR somente PASS se a faixa variável for observada/confirmada no monitor ou ferramenta do fabricante durante FPS variável; caso contrário BLOCKED/NOT RUN, nunca inferir VRR de PresentMode;
- HDR somente PASS para o escopo explicitamente testado: surface anuncia espaço HDR, shader produz PQ/BT.2020 correto, SO/monitor confirmam entrada HDR e SDR fallback funciona;
- declarar separadamente a limitação de metadados estáticos HDR10 do WGPU 30.

**Gate de determinismo:**
- execução contínua versus execução em blocos equivalente;
- UI inativa/oculta não altera o Scheduler;
- áudio e apresentação não comandam o clock emulado;
- pausa, reset e troca de ROM em fronteiras seguras.

**Gate de corte:** nenhum frontend ou backend antigo será removido antes da paridade, regressão e validação de Release no HEAD exato da PR.

## 10. Licenciamento

Slint 1.18.1 oferece GPLv3, Royalty-free e licença comercial. Para uma aplicação desktop independente, a rota proposta é a licença Royalty-free, com atribuição Slint visível em uma tela Sobre ou badge público conforme os termos. O código próprio do projeto pode permanecer MIT. A escolha e os avisos de distribuição precisam ser registrados em `THIRD_PARTY_NOTICES.md` antes do primeiro binário público.

CPAL e WGPU também terão licenças e notices conferidos e fixados na atualização do inventário de dependências.

## 11. Estado inicial e itens explicitamente não aprovados

- Main auditada: `392a5c67e85a11a5a980d8f7055bcc8a12fe3784`.
- PR #743 está integrada na main e seu worker/contrato de comandos limitados é baseline a preservar.
- PR #744 permanece Draft, aberta e não integrada; o workflow do HEAD `a357f25b23da7655b421f4e9735c42c2d253c754` falhou no Clippy da aplicação nativa Windows. Ela não será tratada como PASS nem mesclada como parte desta migração.
- O POC Slint/WGPU ainda não foi executado: estado BLOCKED/NOT RUN até haver código e CI.
- CPAL ainda não foi integrado: estado NOT RUN.
- HDR10 completo com metadados estáticos e VRR físico não estão comprovados.

**Conclusão:** preservar o runtime de emulação; substituir gradualmente a camada de aplicação e os adaptadores host por Slint + WGPU + CPAL. A primeira entrega é uma prova de conceito isolada, não uma reescrita do hardware nem uma migração em massa.

## 12. Especificação de validação por fases — HUD / fullscreen

A validação deverá usar o runtime SMS temporal real e uma ROM homebrew ou sintética legalmente distribuível. Os gates são separados por fase: R1 comprova somente o caminho gráfico/desktop e a HUD mínima; R2 comprova áudio CPAL; R5 comprova HDR/VRR. Uma fase posterior não bloqueia a aprovação independente de R1.

### 12.1 Composição visual e controle

- O viewport do jogo é a camada de fundo da mesma janela fullscreen Slint/WGPU.
- O menu abre sobre o último frame do jogo com fundo translúcido/escurecimento. A abertura solicita imediatamente a pausa da sessão; o Scheduler conclui apenas a operação em andamento até uma fronteira segura e então deixa de avançar o tempo virtual. Não há execução de CPU, avanço de VDP/PSG, frame emulado ou produção de áudio emulado enquanto o menu estiver aberto.
- O botão Guide do gamepad alterna o menu por transição de botão (pressed edge), sem ser encaminhado ao controle do Master System.
- Com o menu aberto, D-pad, confirmação e cancelamento são consumidos pela UI; ao fechar, o mapeamento de jogo volta a receber os controles.
- Para conteúdo 4:3 em monitor 16:9, centralizar o viewport sem deformação. A única informação visual será o FPS de emulação, discreto e sobreposto no mesmo canto seguro; não reservar gutters, não redimensionar e não cortar o jogo. Aplicar a mesma HUD mínima em 4:3 e 16:9.
- Nunca cortar a imagem para abrir espaço ao HUD. Proporção e área ativa do jogo têm prioridade.

### 12.2 Telemetria e proveniência

A única informação visual da HUD nesta prova de conceito será o **FPS de emulação**, lido de `SmsPerformanceMetrics` do worker SMS e atualizado em baixa frequência. Não exibir FPS de apresentação, refresh rate, frame time, input-to-frame, resolução, proporção, GPU/backend, present mode, HDR, VRR, sample rate, underruns ou qualquer outra métrica. Métricas adicionais eventualmente necessárias à validação permanecem em logs/testes internos, nunca na HUD. A HUD não deve reservar espaço, redimensionar ou cortar o viewport.

### 12.3 Limite de transferência de vídeo

O VideoFrame atual possui pixels imutáveis em memória CPU e é publicado através de FrameMailbox. A integração WGPU deve consumir esse contrato e fazer o upload necessário para uma textura GPU. O teste não exige zero-copy físico CPU→GPU; exige evitar cópias completas redundantes na CPU e não fazer conversão de formato desnecessária a cada frame.

### 12.4 Critério de aceite — R1 (Slint/WGPU + HUD mínima)

R1 será considerada aprovada somente se:
1. O SMS temporal real inicia e executa uma ROM homebrew/sintética de teste, produzindo frames completos no `FrameMailbox`.
2. A janela Slint/WGPU apresenta os frames do SMS na mesma janela, com device/queue compartilhados e backend Vulkan explicitamente identificado no Windows x64.
3. O conteúdo 4:3 fica centralizado em monitor 16:9 sem deformação, crop ou gutters reservados; o conteúdo 16:9 também não é deformado ou cortado.
4. A única informação visual da HUD é o FPS de emulação originado em `SmsPerformanceMetrics`, em um canto seguro, sem alterar o viewport.
5. Guide abre/fecha o menu por pressed edge; Guide e comandos de navegação são consumidos pela UI e não chegam ao SMS.
6. Abrir o menu solicita pausa e o Scheduler congela a máquina na primeira fronteira segura; o último frame permanece visível. Fechar retoma do estado integral preservado, sem reset ou avanço artificial.
7. Com o menu aberto, CPU, VDP, PSG, Scheduler, frames e produção de áudio emulado não avançam; a UI e a apresentação do último frame permanecem responsivas.
8. Resize, minimize/restore, fullscreen, mudança de proporção e shutdown repetido não causam vazamento, deadlock ou erro de sincronização.
9. Não há alteração de semântica, clock ou Scheduler do runtime SMS.

Aprovação R1 não exige CPAL, HDR ou VRR. Esses itens pertencem às fases R2 e R5 e devem permanecer classificados como NOT RUN/BLOCKED até seus próprios testes.

### 12.5 Critério de aceite — R2 (áudio CPAL)

R2 será validada separadamente, conforme os gates de áudio da seção 9: PCM real do SMS, callback sem alocação/bloqueio, pausa/resume, unplug/replug, shutdown e estabilidade em dispositivos/taxas suportados. A pausa do menu não pode avançar os chips nem o Scheduler. Resultado físico não executado permanece NOT RUN/BLOCKED e não invalida retroativamente R1.

### 12.6 Critério de aceite — R5 (HDR/VRR)

HDR e VRR serão comprovados separadamente em hardware compatível, com os critérios e limitações da seção 4.2 e dos gates da seção 9. Enumeração de `PresentMode` não comprova VRR; PQ/BT.2020 sem metadados estáticos não será anunciado como conformidade HDR10 completa. Esses testes não fazem parte do gate R1.
