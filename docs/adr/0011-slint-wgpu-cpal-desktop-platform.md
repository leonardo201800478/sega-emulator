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

### 3.1 Risco conhecido: integração Slint/WGPU

A integração externa WGPU 30 do Slint é atualmente exposta sob `unstable-wgpu-30`. A API pode mudar em uma atualização minor do Slint. Portanto:

- fixar Slint na série 1.18.x e WGPU em 30.0.1 durante o POC;
- usar o mesmo `wgpu::Device` e `wgpu::Queue` para o renderer Slint e para os recursos do viewport;
- avaliar `BackendSelector::require_wgpu_30`, `WGPUConfiguration::Manual`, `Window::set_rendering_notifier` e importação de `wgpu::Texture` como os caminhos de integração;
- documentar explicitamente o ownership e o ciclo de vida de textures, views, surfaces e recursos importados;
- não atualizar Slint/WGPU sem repetir os testes de integração gráfica.

**Gate obrigatório:** a prova de conceito deve demonstrar uma janela Slint com menus e uma textura produzida pelo renderer do emulador, compartilhando device/queue, sem janela filha nativa, sem segunda API gráfica e sem cópia CPU por frame como caminho permanente.

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
- imagem 4:3 centralizada sem crop/stretch em monitor 16:9, com telemetria nos gutters laterais; quando o conteúdo preencher 16:9 ou não houver gutters suficientes, telemetria compacta em overlays nos cantos;
- HUD esquerdo: FPS de emulação, FPS de apresentação, frame time, input-to-frame, resolução interna e proporção;
- HUD direito: adapter/backend efetivo, present mode, refresh rate quando obtido de fonte confiável, estado HDR/capacidades, estado VRR como medido ou não verificável, sample rate e métricas de underrun/drop do áudio;
- botão SDL3 SDL_GAMEPAD_BUTTON_GUIDE abre/fecha o menu; borda de pressionamento evita alternância repetida; Guide nunca é enviado ao controle emulado;
- menu translúcido sobre o jogo, sem pausar a máquina; D-pad e botões de confirmação/cancelamento navegam no menu e são consumidos pelo host enquanto ele estiver aberto;
- testar aspect ratio 4:3 e 16:9, navegação integral pelo gamepad, retorno ao jogo e continuidade da execução do SMS;
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
- aspect ratio 4:3 e 16:9 sem distorção ou crop, com HUD nos gutters quando disponíveis e nos cantos quando não;
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

## 12. Especificação do teste integrado HUD / fullscreen

A prova de conceito R1/R2 deverá ser executada com o runtime SMS temporal real e uma ROM homebrew ou sintética legalmente distribuível. O objetivo é provar a plataforma host completa antes de expandir a interface.

### 12.1 Composição visual e controle

- O viewport do jogo é a camada de fundo da mesma janela fullscreen Slint/WGPU.
- O menu abre sobre o jogo com fundo translúcido/escurecimento; a sessão continua rodando e o Scheduler continua avançando normalmente.
- O botão Guide do gamepad alterna o menu por transição de botão (pressed edge), sem ser encaminhado ao controle do Master System.
- Com o menu aberto, D-pad, confirmação e cancelamento são consumidos pela UI; ao fechar, o mapeamento de jogo volta a receber os controles.
- Para conteúdo 4:3 em monitor 16:9, centralizar o viewport sem deformação e utilizar os espaços laterais para telemetria compacta esquerda/direita. Para conteúdo 16:9 ou quando não houver área lateral suficiente, posicionar telemetria nos cantos sobre o vídeo, com fundo translúcido e margens seguras.
- Nunca cortar a imagem para abrir espaço ao HUD. Proporção e área ativa do jogo têm prioridade.

### 12.2 Telemetria e proveniência

A telemetria deve separar explicitamente:
- **Emulation FPS:** medido no worker SMS (SmsPerformanceMetrics);
- **Presentation FPS:** frames efetivamente submetidos/apresentados pelo host WGPU;
- **Frame time:** duração entre frames concluídos pelo worker e, separadamente, intervalos de apresentação;
- **Input-to-frame:** medição existente no worker;
- **Audio:** sample rate efetivo e contadores de underrun/drop da sessão CPAL;
- **GPU/backend:** adapter e backend efetivamente selecionados, formato, color space e present mode;
- **Display:** refresh rate apenas quando obtido de API confiável; caso contrário mostrar “não disponível”;
- **VRR:** distinguir capacidade, configuração e confirmação observada; não afirmar “ativo” apenas porque um modo WGPU está disponível;
- **HDR:** distinguir capacidade de surface, HDR do display reportado e modo de saída configurado.

A UI consulta snapshots em baixa frequência e nunca solicita debug_snapshot() bloqueante em cada frame. Contadores de host são coletados fora do callback de áudio e fora do avanço do Scheduler.

### 12.3 Limite de transferência de vídeo

O VideoFrame atual possui pixels imutáveis em memória CPU e é publicado através de FrameMailbox. A integração WGPU deve consumir esse contrato e fazer o upload necessário para uma textura GPU. O teste não exige zero-copy físico CPU→GPU; exige evitar cópias completas redundantes na CPU e não fazer conversão de formato desnecessária a cada frame.

### 12.4 Critério de aceite end-to-end

A prova será considerada aprovada somente se:
1. O SMS temporal real inicia e executa uma ROM de teste, produzindo imagem e áudio.
2. A janela Slint permanece fullscreen e o menu sobrepõe o jogo na mesma composição.
3. Guide abre/fecha o menu e o controle navega nele sem enviar os mesmos comandos ao jogo.
4. O jogo continua executando com o menu aberto e retoma o recebimento de input ao fechar.
5. HUD mostra valores de origem identificada, sem misturar FPS do emulador com FPS do monitor.
6. Áudio CPAL permanece estável durante gameplay, abertura do menu, fechamento, pausa, reset e shutdown.
7. VRR e HDR são testados em equipamento compatível e seus resultados são classificados separadamente, incluindo BLOCKED/NOT RUN quando o hardware não permite a comprovação.
8. Execução do SMS, Scheduler, CPU, VDP e PSG permanece inalterada pelo HUD, menu, áudio host e apresentação.

Nenhuma aprovação de R1/R2 será inferida apenas de uma captura de tela ou de uma compilação bem-sucedida.
