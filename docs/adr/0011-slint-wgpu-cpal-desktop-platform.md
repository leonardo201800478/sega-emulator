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

### R1 — Prova de conceito gráfica/desktop
- Slint 1.18.x com WGPU 30.x e backend WGPU;
- janela principal, menu mínimo e viewport;
- mesma device/queue para UI e textura de vídeo;
- Vulkan backend identificado e validado no Windows x64 Release;
- resize, fullscreen, minimize/restore e shutdown repetidos;
- shader simples de textura e nearest-neighbor;
- nenhuma alteração ao runtime SMS.

### R2 — Prova de conceito de áudio
- CPAL 0.18.x;
- enumerar dispositivos, selecionar saída e negociar formato;
- reproduzir PCM sintético através do ring buffer;
- medir underruns, latência e estabilidade em 44,1/48/96 kHz;
- testar unplug/replug, pausa, reset e shutdown;
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

### R5 — Apresentação avançada
- proporções, integer scaling, shaders em passadas combináveis;
- VSync e modos Mailbox/Immediate conforme capacidade;
- VRR/G-Sync/FreeSync sob testes físicos;
- HDR PQ opcional, com a limitação de metadados claramente documentada;
- testes de cores, gamut e tone mapping.

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

**Gate WGPU/Slint:**
- device/queue compartilhados comprovados;
- nenhuma janela overlay/child HWND para o viewport;
- sem vazamento de recursos ou erro de sincronização;
- resize/minimize/fullscreen/recreate/shutdown repetidos;
- renderização 2D sem cópia CPU por frame como caminho permanente.

**Gate CPAL:**
- callback sem alocação/bloqueio;
- ausência de underruns sob carga sustentada;
- dispositivo removido/reconectado sem travar o worker;
- saída PCM de referência determinística antes do host;
- shutdown seguro.

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
