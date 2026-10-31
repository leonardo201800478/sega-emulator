# Escopo do projeto — Sega Emulator

**Status:** diretriz de escopo e arquitetura-alvo  
**Runtime de hardware:** exclusivamente Rust  
**Frontend e ferramentas:** Python / Qt / PySide6  
**Integração:** PyO3 / Maturin, fronteira fina  
**Autoridade temporal:** Master Scheduler Rust único por máquina

Este documento consolida o escopo de produto e as decisões arquiteturais transversais do Sega Emulator. Ele orienta o roadmap, os documentos específicos de hardware e a priorização das implementações. Não declara como implementado nenhum componente que ainda não tenha evidência no código e nos testes.

## 1. Objetivo e princípios

Construir um emulador Sega determinístico, modular, testável e de alta fidelidade demonstrável no Windows, preservando o trabalho validado e avançando por marcos incrementais.

Prioridades, em ordem:

1. Fidelidade demonstrável, sustentada por especificações, observações e testes independentes.
2. Determinismo e uma única autoridade temporal por máquina.
3. Integridade arquitetural, sem duplicação de runtime ou abstrações genéricas prematuras.
4. Preservação de funcionalidades e regressões já validadas.
5. Progresso incremental, com critérios de aceitação antes de cada implementação.
6. Desempenho somente após baseline, profiling e equivalência funcional.

Não sacrificar correção por velocidade. Não declarar compatibilidade universal, precisão ciclo a ciclo ou equivalência com hardware real sem evidências adequadas.

## 2. Plataformas e ordem dos marcos

O escopo de plataformas é:

1. **SG-1000** — preservar e consolidar o marco funcional já validado, incluindo CPU Z80, memória/barramento, I/O, VDP TMS9918A, PSG, entrada e integração.
2. **Master System / Game Gear** — consolidar o SMS, fechar as lacunas do Z80, corrigir o VDP SMS e integrar as diferenças de hardware e LCD do Game Gear.
3. **Mega Drive / Genesis** — integrar 68000, Z80 auxiliar, VDP, áudio, cartuchos, mappers e periféricos.
4. **Sega CD** — compor com Mega Drive, incluindo CPUs, BIOS, CD/media, Word RAM, ASICs, DMA, PCM e CDDA.
5. **32X** — compor com Mega Drive, incluindo dois SH-2, firmware, VDP/framebuffers, PWM e composição de vídeo.

A sequência é uma ordem de dependências e validação, não uma promessa de prazo. Não iniciar um subsistema de plataforma posterior enquanto isso comprometer o fechamento de lacunas prioritárias da plataforma atual, salvo dependência arquitetural justificada.

### 2.1 Extensões e hardware especializado

O projeto também contempla processadores auxiliares e coprocessadores especializados presentes em cartuchos e expansões, como o **SVP (Sega Virtua Processor)** de Virtua Racing, DSPs, aceleradores matemáticos/gráficos programáveis e ASICs híbridos com processador próprio.

Esses dispositivos são diferentes de VDPs convencionais, PSG/FM, controladores comuns e ALUs integradas às CPUs. Cada arquitetura especializada terá seu próprio núcleo Rust, memória, estado, interface externa, perfil de placa e integração temporal. Não haverá um interpretador universal de instruções.

O SVP é o primeiro estudo de caso planejado para essa categoria, condicionado ao marco Mega Drive. A proposta detalhada está sendo tratada separadamente na [PR #320 — Specialized Coprocessor Architecture](https://github.com/leonardo201800478/sega-emulator/pull/320); sua existência não significa que o SVP esteja implementado ou que essa proposta já esteja integrada à main.

## 3. Linguagens e fronteiras

### 3.1 Rust — único runtime de hardware

Rust é responsável por todo o estado e execução da máquina:

- CPUs, ALUs e processadores auxiliares;
- memória, barramentos, mappers e decodificação de endereços;
- VDPs, raster, sprites, DMA, composição e pipeline de vídeo/renderização;
- PSG, FM, PCM, CDDA, PWM e pipeline digital de áudio;
- I/O, interrupções, periféricos e protocolos de hardware;
- clocks, T-states, acessos, waits, arbitragem e scheduler;
- CD/media, firmware, save states e estado de execução.

### 3.2 Python — aplicação, não hardware

Python fica restrito a:

- frontend Qt/PySide6, incluindo apresentação da superfície de vídeo e controles de configuração do jogador;
- carregamento e seleção de ROMs, discos, BIOS e perfis;
- configuração, logs e diagnósticos da aplicação;
- backends de host e integração de dispositivos;
- ferramentas, auditorias, automação, testes e benchmarks;
- fachada fina para a extensão Rust.

Não manter um segundo runtime Python de hardware. Implementações em `legacy/` são referência histórica somente e não podem ser ativadas como fallback ou oráculo permanente.

### 3.3 PyO3/Maturin

A fronteira Python/Rust deve ser coarse-grained. Não usar callbacks Python por instrução, por pixel ou por evento de hardware. Evitar cópias grandes no caminho crítico. Ownership, sincronização, threads e liberação do GIL exigem modelo explícito e testes.

## 4. Identificação da mídia, perfil da máquina e sequência de boot

A configuração efetiva do hardware deve ser resolvida **antes de iniciar a execução da mídia**. O carregador não pode deixar que o VDP, a CPU ou o frontend deduzam região e clocks durante a renderização.

### 4.1 Responsabilidades separadas

A arquitetura separa três responsabilidades:

1. **Identificação da mídia:** identifica tipo, formato, identidade, região declarada, revisão/variante conhecida, requisitos de firmware e possíveis extensões, usando metadados, checksums e fingerprints verificáveis.
2. **Mapper / controlador de mídia:** implementa o mapeamento real de endereços, bancos, SRAM e registradores do cartucho. Não é autoridade para escolher o clock físico da máquina.
3. **Resolvedor do perfil da máquina:** combina modelo/revisão do console, configuração regional, padrão de vídeo, mídia, firmware e política explícita de compatibilidade para produzir o perfil efetivo.

Não inferir o hardware apenas pelo nome do jogo ou por um único byte de cabeçalho. Identificações ambíguas devem ser representadas como `UNKNOWN` ou `AMBIGUOUS`; nunca selecionar silenciosamente um perfil incompatível. Para CD, a identificação utiliza TOC, trilhas, metadados e fingerprints apropriados; não forçar a abstração de mapper de cartucho sobre discos.

A região da mídia é evidência e restrição de compatibilidade, não necessariamente a região física do console. Um jogo PAL pode ser executado em console NTSC, sujeito ao comportamento real do software e do hardware. A configuração física/regional da máquina permanece explícita; divergências devem ser sinalizadas e tratadas por política documentada, não corrigidas automaticamente alterando o clock.

### 4.2 Catálogo de mídia, DATs, fingerprints e perfis de cartucho

A identificação de mídia e a resolução do hardware do cartucho são funções centrais do emulador, não uma heurística secundária do carregador. O projeto adotará DATs reconhecidos de preservação como fontes primárias de identidade:

- **No-Intro:** referência de identificação e auditoria de dumps de cartuchos, usando os conjuntos e sistemas pertinentes.
- **Redump:** referência de identificação e auditoria de mídia óptica, incluindo a estrutura de discos/trilhas quando aplicável. Não aplicar o modelo de DAT de cartucho a CD nem reduzir discos a um único hash de arquivo.

Os DATs devem ser importáveis/atualizáveis e versionados como dependências de dados externas ao runtime. Registrar nome do conjunto, sistema, versão/data, origem, licença/termos de uso e hash do próprio DAT. A atualização de catálogo não pode alterar silenciosamente o comportamento de um jogo já carregado: a identidade e o perfil resolvidos devem ser reproduzíveis e registráveis em diagnóstico.

**Identidade e auditoria de dumps:**
- Calcular e comparar os checksums disponíveis no DAT e no formato de mídia, incluindo CRC32, MD5 e SHA-1 quando fornecidos; manter os valores e o algoritmo identificados, sem tratar hashes diferentes como intercambiáveis.
- Registrar tamanho exato, cabeçalhos/headers, organização/interleaving, overdumps, padding, trilhas e demais características relevantes antes de normalizar ou transformar a imagem.
- Distinguir hash do arquivo recebido, hash do conteúdo canônico usado para identificação e hash de cada trilha/área quando o formato exigir. Toda transformação deve ser explícita, reversível quando possível e diagnosticável.
- Classificar correspondências como exata, variante conhecida, derivada/modificada, ambígua ou desconhecida. Não aprovar uma ROM apenas por nome, tamanho ou correspondência parcial.
- Auditar região declarada, variantes/revisões, diferenças de revisão e peculiaridades conhecidas. A região identificada é metadado da mídia e não substitui a seleção explícita do perfil físico da máquina.
- Exibir divergências entre o DAT e os dados observados, preservando evidências; nunca corrigir ou reescrever a ROM original automaticamente.

**DAT não é especificação completa de hardware ou gameplay.** Número de jogadores, acessórios necessários/suportados, expansão de RAM, SRAM, peculiaridades de boot e comportamento de mapper devem vir de uma camada complementar de metadados técnicos, sustentada por documentação, pesquisas e testes. Cada campo deve indicar procedência, nível de confiança e estado (documentado, observado, inferido ou desconhecido). Não inventar esses atributos quando o DAT não os fornecer.

**CartProfile / BoardProfile:** após identificar a mídia, resolver um perfil explícito e versionado da placa/cartucho, separado do MachineProfile e do mapper executável. O perfil deve descrever, conforme evidência disponível:
- capacidade e organização da ROM, bancos, janelas, registradores, linhas de endereço e regras de seleção; suportar ROMs maiores que a janela diretamente endereçável e mapeamentos não padronizados;
- tipo, tamanho, faixa de endereço, espelhamento, retenção, write-enable/proteção e persistência de RAM, incluindo SRAM/NVRAM e RAM de expansão embutida no cartucho;
- mapper padrão, mapper proprietário, lógica discreta/ASIC, variantes de placa e requisitos de inicialização/reset;
- expansões e dispositivos integrados, interrupções, registradores auxiliares e quaisquer efeitos sobre o mapa de memória/I/O;
- compatibilidade de boot, BIOS/firmware exigido, opcional ou não utilizado, respeitando o modelo e a revisão do console;
- metadados de uso: número de jogadores, modo simultâneo/alternado quando conhecido, acessórios obrigatórios/opcionais/suportados e limitações de compatibilidade.

O mapper Rust implementa o comportamento elétrico/lógico do perfil de placa: decodificação de endereços, bancos, leitura/escrita, RAM, registradores, conflitos de barramento quando comprovados, reset e efeitos temporais observáveis. O catálogo identifica e seleciona o perfil; não executa o mapper nem substitui sua lógica. Perfis compartilhados só podem ser reutilizados quando a equivalência da placa e do comportamento estiver demonstrada.

A arquitetura deve contemplar, sem presumir que já estejam implementados:
- cartuchos com ROM banking e janelas múltiplas;
- cartuchos com ROM expandida e mapeamento diferenciado;
- cartuchos com SRAM/NVRAM ou RAM de expansão embutida;
- cartuchos com ASICs, controladores ou expansões especiais;
- cartuchos multijogos/multicarts, com seleção de título/banco e estado de seleção reproduzível;
- hacks, traduções, overdumps, revisões e ROMs modificadas, identificadas como derivadas e nunca confundidas com a revisão original;
- títulos que dependem de acessórios ou de configurações específicas de entrada;
- jogos com caminhos de boot que dependem de BIOS, firmware, região, modelo ou estado inicial específico.

Multicarts e hacks não devem receber um mapper genérico por conveniência. Multicarts precisam de perfil de placa e lógica de seleção próprios; hacks/patches devem preservar a identidade do original, registrar patch/proveniência e permitir perfil explícito quando alterarem mapeamento, boot ou requisitos. Se a identidade não for suficiente para escolher um perfil com segurança, bloquear a seleção automática ou solicitar escolha explícita, sem fallback silencioso para um mapper incorreto.

**Política de BIOS por título/perfil:** a decisão de exigir, permitir, ignorar ou proibir BIOS deve ser dado explícito do perfil resolvido e da plataforma/modelo, com justificativa e fonte. A ausência de BIOS opcional não pode bloquear o boot; uma BIOS obrigatória ausente/inválida deve produzir erro claro; não usar BIOS quando o hardware/jogo não a utiliza. Não generalizar a política de uma plataforma para outra.

**Fluxo obrigatório de carregamento:**
1. preservar a imagem original e calcular fingerprints sem modificá-la;
2. identificar formato e plataforma; para cartuchos consultar No-Intro e para mídia óptica consultar Redump;
3. resolver correspondência, revisão, região e estado de integridade;
4. consultar metadados técnicos complementares e resolver CartProfile/BoardProfile;
5. validar mapper, ROM banking, RAM/SRAM, expansões, acessórios, jogadores e requisitos de BIOS;
6. resolver separadamente MachineProfile, região física e domínios de clock;
7. validar incompatibilidades e ambiguidades; solicitar decisão explícita quando necessário;
8. instanciar o mapper e os dispositivos Rust, aplicar reset/boot e registrar a identidade e os perfis efetivamente selecionados.

O diagnóstico deve informar identidade/hash, DAT e versão usados, correspondência, região/revisão, perfil de placa, mapper, mapa/tamanho de ROM, RAM/SRAM, acessórios/jogadores quando conhecidos, política/estado de BIOS e alertas de incompatibilidade. Não registrar conteúdo da ROM nem despejar dados protegidos em logs.

### 4.2.1 Conjuntos compostos, cartuchos acoplados e mídias auxiliares

O Media Manager também deve representar mídias que não são um único arquivo ou uma única imagem de cartucho. A unidade de carregamento pode ser um **MediaSet** versionado, composto por mídia principal, cartuchos adicionais, patches e recursos auxiliares. Cada membro mantém sua identidade, hash, procedência e papel; o conjunto não deve ser achatado em uma ROM sintética sem necessidade e sem preservar a relação entre os componentes.

**Lock-on e cartuchos acoplados (Mega Drive/Genesis):**
- Modelar o cartucho-base com pass-through/slot secundário e o cartucho conectado como dois dispositivos de mídia/placa distintos.
- Representar detecção de presença, conexão do barramento e decodificação/seleção de ROMs de acordo com a lógica do cartucho-base; não concatenar as ROMs nem presumir que formem uma imagem linear.
- Resolver perfis de compatibilidade e combinações suportadas. O comportamento pode depender da identidade do cartucho secundário e de dados específicos do jogo; combinação não reconhecida não deve receber resultado inventado.
- Preservar os hashes e metadados individuais de ambos os cartuchos, a ordem/posição da conexão e a configuração de lock-on no estado de sessão/save state.
- O comportamento Lock-On é responsabilidade do perfil de placa e da lógica de mapper/barramento Rust; Media Manager identifica e prepara o conjunto físico lógico.

**ROMs com patches e recursos auxiliares (MSU-MD e MD+):**

MSU-MD e MD+ devem ser identificados como perfis distintos de mídia aprimorada para Mega Drive, não como sinônimos e nem como novos mappers de cartucho. Ambos podem associar uma ROM de cartucho modificada a recursos de áudio em formato de CD, mas os contratos de integração são diferentes:

- **MSU-MD:** patches e driver que utilizam o caminho Sega CD em Mode 1, com o jogo inicializado pelo cartucho e funções de áudio/streaming atendidas pelo subsistema CD. A configuração pode exigir BIOS Sega CD compatível e deve resolver essa exigência conforme o perfil e a implementação do modo. O conjunto pode incluir ROM modificada, imagem de dados e trilhas de áudio descritas por CUE/CHD; não presumir que seja apenas uma playlist de CDDA.
- **MD+:** formato/overlay associado ao ecossistema MegaSD e seus comandos de CD overlay, com ROM modificada e CUE/recursos de áudio correspondentes. Não equivale a executar o hardware completo do Sega CD em Mode 1 e não deve herdar automaticamente a política de BIOS do MSU-MD. A compatibilidade depende do protocolo/recursos MD+ efetivamente implementados; comandos ou recursos não suportados devem ser reportados.
- Não inferir o formato apenas pela extensão, nome do arquivo ou presença de CUE. Identificar marcadores e comandos documentados, metadados de patch, relação entre ROM e sidecars e compatibilidade do perfil. Quando a detecção for ambígua, solicitar seleção explícita.
- Preservar a ROM original sem alteração. Registrar patch aplicado, versão, algoritmo/formato do patch, hash da ROM-base, hash da ROM resultante e hashes dos recursos auxiliares. Se o patch for pré-aplicado e não houver patch reversível disponível, classificar a imagem como derivada/modificada e não como dump original validado pelo DAT.
- Resolver nomes e relações entre ROM, CUE, ISO/BIN/CHD e faixas de áudio de maneira explícita. Validar arquivos ausentes, trilhas referenciadas, índices, loops e metadados específicos do formato; não presumir que um CUE de MD+ seja equivalente a um CUE de MSU-MD.
- O runtime Rust deve separar o protocolo de extensão (MD+ ou MSU-MD), o leitor de mídia óptica/sidecar e o hardware Sega CD real. O adaptador de protocolo não pode falsificar registradores, BIOS, CPUs ou dispositivos do Sega CD quando o modo não os instancia.

**Modelo de conjunto e validação:**
- Um MediaSet deve declarar papel de cada membro (cartucho primário, cartucho lock-on, patch, imagem de dados, trilha de áudio, firmware, sidecar), identidade, hash, formato, versão do perfil e dependências.
- O carregamento deve falhar com diagnóstico específico se faltar membro obrigatório, se hashes/revisões não corresponderem, se o CUE apontar para arquivos inexistentes ou se o protocolo de extensão for incompatível.
- Testar separadamente: cartucho isolado; combinações lock-on conhecidas e desconhecidas; ROM original versus ROM modificada; MD+; MSU-MD com Mode 1; arquivos auxiliares ausentes/incompatíveis; e save/load state com a mesma composição de mídia.
- MediaSet, seleção de perfil e estado de leitura devem ser reproduzíveis. O leitor do host não controla o tempo emulado; latências e eventos percebidos pelo jogo são determinados pelo controlador de hardware e pelo Master Scheduler Rust.

### 4.3 MachineProfile

Antes do reset/boot, resolver um perfil explícito e versionado com, no mínimo:

- família e modelo da plataforma;
- revisão de placa/silício, quando conhecida;
- território e região da máquina;
- padrão de vídeo: NTSC, PAL ou PAL-M, quando aplicável;
- oscilador/fonte de clock e domínios derivados;
- CPUs e seus domínios de clock;
- modelo/revisão do VDP e perfil de raster;
- configuração de áudio e seus domínios;
- mapa de memória, mapper e expansões;
- firmware obrigatório/opcional, variantes e estado de validação;
- identidade da mídia e eventuais incompatibilidades;
- versão do perfil e procedência das evidências.

O perfil é imutável durante a sessão normal. Alterar território, padrão de vídeo, revisão ou clock exige reset/reconfiguração explícita; não se deve mudar o clock silenciosamente no meio do quadro.

### 4.4 NTSC, PAL e PAL-M

NTSC, PAL e PAL-M não são simples opções de resolução nem aliases de uma taxa de FPS.

- **NTSC:** perfil de temporização e saída de vídeo correspondente à revisão/modelo selecionado.
- **PAL:** perfil próprio de clocks, raster, linhas totais e blanking, conforme o hardware específico.
- **PAL-M:** perfil regional independente. Não o reduzir a PAL europeu nem tratá-lo como simples filtro RGB. A temporização de determinadas implementações é da família 60 Hz, mas clocks e encoder dependem da placa/revisão; não generalizar um único valor para todas as máquinas brasileiras.

Geometria lógica, clocks, raster e codificação de cor são dimensões separadas. A apresentação RGB do emulador não deve fingir que reproduz a codificação analógica PAL/NTSC quando isso não estiver modelado.

Nenhum valor de clock, linhas por quadro, pixels por linha ou ciclo será fixado sem fonte técnica rastreável e validação do modelo/revisão correspondente.

### 4.5 Firmware e BIOS

A política é específica por plataforma e perfil:

| Plataforma | Política de firmware |
|---|---|
| SG-1000 | O perfil-base deve permitir boot sem BIOS externa. Não introduzir requisito genérico de BIOS. |
| Master System | BIOS pode ser opcional conforme modelo/configuração. Se selecionada, reproduzir o mapeamento, prioridade e sequência reais de boot. |
| Game Gear | BIOS pode ser opcional; ausência não deve bloquear universalmente o boot de cartucho. Modelos e variantes precisam de evidência. |
| Mega Drive | Boot normal de cartucho não depende de BIOS externa. |
| Sega CD | BIOS do modelo/região é requisito para inicializar corretamente o subsistema e os fluxos de boot do Sega CD. Sem BIOS válida, reportar bloqueio explícito; não simular boot funcional. |
| 32X | Tratar separadamente os blocos de firmware/Boot ROM do lado 68K, SH-2 Master e SH-2 Slave, respeitando seus mapas e sequência de reset/liberação. Não assumir que são três arquivos intercambiáveis nem substituir por HLE sem decisão e evidência próprias. |

O inventário deve validar presença, integridade, variante regional e compatibilidade. BIOS protegidas não devem ser versionadas. HLE, quando considerado, precisa ser explicitamente identificado como modo alternativo e não como firmware original.

### 4.6 Sequência de inicialização

A sequência conceitual obrigatória é:

1. carregar e identificar a mídia;
2. identificar plataforma, revisão e perfil de cartucho/disco;
3. resolver região, padrão de vídeo e domínios de clock;
4. validar firmware obrigatório/opcional e escolher o caminho real de boot;
5. instanciar CPUs, memória, barramentos, VDP(s), áudio, I/O e expansões;
6. aplicar reset e sequências de inicialização na ordem documentada;
7. liberar CPUs/dispositivos nos instantes e condições previstos pelo hardware;
8. iniciar a execução da mídia e agendar os eventos na timeline Rust.

A ordem concreta varia por plataforma. Este fluxo não autoriza reset simultâneo ou liberação imediata de todos os processadores quando o hardware exige handshakes, transferência de firmware ou controle de reset.

## 5. Estratégia global para os VDPs

O vídeo é uma área crítica de fidelidade. Cada VDP deve reproduzir seu hardware, limites e temporização; uma imagem visualmente plausível não é critério suficiente.

### 5.1 Arquitetura comum sem VDP genérico

Adotar uma arquitetura comum de integração e validação, mas preservar núcleos específicos por família/revisão. O contrato comum deve ser mínimo e representar apenas necessidades realmente compartilhadas, como reset, estado, interface de barramento, agendamento de eventos, interrupções e publicação de segmentos/framebuffer.

Não criar um renderizador universal que esconda diferenças de registradores, VRAM, CRAM, pipeline, sprite evaluation, fetches, limites ou composição. Compartilhar código somente quando equivalência funcional e temporal for demonstrada.

Cada VDP será organizado, conforme o hardware exigir, em responsabilidades coesas:

- registradores, latches e portas;
- VRAM/CRAM/VSRAM e buffers;
- raster, contadores H/V, blanking e eventos;
- pipeline de fetch/decode e produção de pixels;
- avaliação de sprites e prioridade;
- interrupções e status;
- DMA, arbitragem e contenção;
- estado serializável e diagnóstico.

### 5.2 Inventário por plataforma

| Plataforma | Hardware de vídeo | Estratégia |
|---|---|---|
| SG-1000 | Família TMS9918A e variantes pertinentes ao modelo | Preservar o núcleo Rust existente como baseline; validar revisões e temporização regional sem regressão. |
| Master System | Sega VDP, família 315-5124/315-5246 conforme revisão | Corrigir o núcleo SMS atual; Mode 4, VRAM/CRAM, scroll, sprites, status, H/V counters, IRQ e efeitos raster. |
| Game Gear | Variante do VDP SMS com saída para LCD portátil | Reutilizar apenas comportamento comprovadamente comum; modelar janela LCD, paleta, recorte e temporização específica. |
| Mega Drive / Genesis | VDP 315-5313 e variantes | Núcleo próprio com H32/H40, planos, sprites, DMA, interrupções, interlace, raster e regras de acesso. |
| Sega CD | VDP do Mega Drive, integrado aos recursos gráficos do Sega CD | Não duplicar o VDP base; integrar Word RAM, Cell Arranger, ASICs, DMA e caminhos de transferência para VRAM/CRAM/VSRAM. |
| 32X | VDP/framebuffers próprios do 32X mais VDP do Mega Drive | Dois produtores de imagem sincronizados e compositor de hardware que respeite prioridade, transparência, overwrite e troca de framebuffer. |

O inventário é escopo-alvo; não declara que os núcleos posteriores já existem ou estão completos.

### 5.3 Raster, resolução e viewport

Manter três conceitos independentes:

1. **Raster físico:** posição horizontal/vertical, dots ou unidades temporais do chip, linhas totais, blanking e eventos.
2. **Área ativa:** pixels/linhas que o hardware disponibiliza como imagem visível em determinado modo.
3. **Viewport de apresentação:** região que o monitor/LCD hospedeiro mostra, podendo recortar ou escalar a imagem.

Não deduzir o raster a partir da resolução do framebuffer. O VDP deve respeitar onde começa e termina cada região visível e não visível, conforme perfil, modo e estado dinâmico do hardware.

A geometria pode mudar durante a execução. Modos como H32/H40 do Mega Drive, alterações de registradores raster e outras mudanças devem produzir efeitos a partir do instante efetivo, sem reconstruir o quadro inteiro retrospectivamente.

### 5.4 Game Gear e LCD

A resolução de apresentação do LCD Game Gear é 160×144, mas isso não significa que o raster interno do VDP seja simplesmente 160×144. A implementação deve distinguir a região de origem, a janela física do LCD, o recorte, a paleta e as restrições de acesso/temporização do VDP. Não implementar o GG como um SMS reduzido por escala.

### 5.5 Sprites, paletas e composição

Cada núcleo deve preservar as regras específicas do hardware:

- quantidade total e por linha;
- limite de pixels/tiles por linha, quando aplicável;
- ordem de avaliação e prioridade;
- colisão, overflow e índice reportado;
- comportamento fora da área visível;
- máscaras, transparência e valores de cor zero;
- limites de paleta e número de cores simultâneas;
- regras de sobreposição de planos, sprites e camadas;
- efeitos de alterações de registradores durante o raster.

**Política obrigatória de limite de sprites por linha:**

- O comportamento padrão de todo VDP deve aplicar o limite real de sprites por linha do console, modelo e revisão emulados. O limite deve estar habilitado desde a inicialização, sem depender de uma preferência do jogador.
- A Qt deve disponibilizar uma opção explícita para desativar esse limite em um modo de renderização aprimorada. A opção deve ser identificada como não fiel ao hardware, persistida na configuração do emulador e aplicada de forma controlada, sem alterar silenciosamente o perfil físico da máquina.
- A implementação deve separar a avaliação de sprites e os efeitos observáveis pelo software (status, overflow, colisões e índices) da política de apresentação aprimorada. Desativar a limitação visual não pode apagar, falsificar ou reescrever flags e efeitos de hardware que o jogo consegue observar, salvo quando um modo alternativo documentado reproduzir deliberadamente outra semântica.
- O padrão fiel continua sendo a referência para testes, comparações com hardware/oráculos e validação. Testes devem cobrir limite habilitado e modo aprimorado, garantindo que a opção não corrompa memória, estado do VDP, scheduler ou determinismo.
- Os limites são específicos de cada VDP; não criar um valor universal nem presumir que todos os consoles compartilham a mesma regra.

Não desabilitar limites físicos por padrão nem para corrigir defeitos visuais. Modos de compatibilidade/aprimoramento devem ser explicitamente separados do modo fiel.

Manter representação digital nativa de cores/paletas até a etapa apropriada de composição. Converter para RGB de apresentação somente na fronteira de saída, preservando precisão suficiente para composição e diagnósticos. Efeitos analógicos de monitor/encoder são outra camada e não devem ser confundidos com lógica digital do VDP.

### 5.6 Sega CD e 32X

O Sega CD usa o VDP do Mega Drive; seus recursos de processamento de imagem e transferência devem ser modelados como caminhos auxiliares reais, não como um segundo VDP convencional. Word RAM, Cell Arranger, ASICs e DMA devem respeitar ownership, latência, contenção e o destino de memória real.

No 32X, a imagem do VDP do Mega Drive e os framebuffers do 32X precisam ser compostos segundo as regras de hardware. Não compor dois quadros RGB completos no frontend como substituto do compositor. Troca de framebuffer, prioridade, transparência, overwrite, formato de pixel e sincronização devem ser eventos do runtime Rust.

### 5.7 Pipeline de vídeo Rust e backends de rasterização

Todo o pipeline de vídeo do emulador — lógica dos VDPs, raster, avaliação de sprites, geração de pixels, composição entre camadas/sistemas, conversão de formato, shaders e preparação da superfície de apresentação — pertence ao runtime Rust. Não haverá implementação paralela de vídeo em Python, nem controle Python de frames, buffers, rasterização, shaders, sincronização ou seleção de backend.

A arquitetura oferecerá quatro caminhos de rasterização/apresentação:

1. **Raster (software):** backend de referência e fallback de compatibilidade, executado em Rust e sem exigir GPU. Deve preservar determinismo e servir como caminho de diagnóstico e comparação.
2. **Direct3D 11:** backend gráfico para hardware e sistemas compatíveis.
3. **Direct3D 12:** backend gráfico moderno, sujeito à compatibilidade do sistema, driver e dispositivo.
4. **Vulkan:** backend gráfico multiplataforma, sujeito ao suporte efetivo do sistema, driver e dispositivo.

A seleção automática será o padrão: detectar capacidades reais, compatibilidade, estabilidade conhecida e recursos necessários; escolher o backend mais adequado ao conjunto sistema operacional/GPU/driver; e manter fallback seguro para outro backend suportado ou Raster. Não selecionar apenas pelo nome ou geração da GPU. A Qt deve oferecer seleção manual, opção de voltar ao automático e diagnóstico do backend efetivamente ativo, incluindo motivo de fallback. A troca de backend deve ocorrer em fronteira segura, preservando o estado do emulador e sem avançar ou reiniciar arbitrariamente o tempo emulado.

**Precisão terminológica e contrato dos quatro caminhos:** Raster é o backend de rasterização por software, não uma API gráfica; Direct3D 11, Direct3D 12 e Vulkan são APIs/backends gráficos. São exatamente esses quatro caminhos que compõem a base de renderização prevista. Qt QRhi não será contado como um quinto backend nem substituirá a escolha explícita desses caminhos; poderá ser usado apenas se uma decisão arquitetural futura demonstrar que não esconde requisitos de controle, sincronização ou recursos específicos, e deverá ser documentado como camada de abstração, não como API adicional.

O resolvedor automático de backend deve consultar capacidades reais em tempo de execução (sistema operacional, adaptador ativo, driver, extensões/recursos, formatos, limites e estabilidade), e não aplicar uma ordem fixa universal. A preferência automática deve considerar correção e compatibilidade antes de desempenho nominal. Se a inicialização falhar, registrar a causa, liberar corretamente os recursos parcialmente criados e tentar o próximo caminho elegível; o fallback final é Raster. A escolha efetiva e a cadeia de fallback devem aparecer nos diagnósticos da Qt. A seleção manual não pode forçar um backend incompatível nem deixar a aplicação em estado parcialmente inicializado.

A interface Qt deve expor, no mínimo: seleção Automático/Raster/D3D11/D3D12/Vulkan; estado do backend ativo; recursos avançados realmente disponíveis; opção de limite de sprites; e aviso claro quando uma opção altera a fidelidade ao hardware. Preferências de apresentação não podem modificar MachineProfile, clocks, raster físico ou estado canônico do VDP. Alterações que exigem recriação de dispositivo/superfície devem ser aplicadas em fronteira segura, preservando o estado emulado.

O contrato de apresentação deve separar o framebuffer/segmentos produzidos pelo runtime, o recurso nativo de GPU e a superfície Qt/host. Ownership, sincronização CPU/GPU, fences, filas, descarte de recursos e recuperação de device-lost precisam ser explícitos e permanecer fora do controle Python. O backend Raster deve ser utilizável em máquinas sem GPU compatível e em cenários de diagnóstico.

A separação de responsabilidades é obrigatória:

- **Rust:** dono dos buffers e recursos de vídeo, renderização, shaders, composição, sincronização com a timeline emulada e coordenação de apresentação.
- **Qt:** interface, preferências do jogador, janela/superfície hospedeira e apresentação do resultado produzido por Rust. A Qt não implementa regras de VDP, não rasteriza conteúdo do console e não se torna autoridade temporal.
- **Python:** não participa do caminho de vídeo em tempo de execução e não pode controlar saída, backend, buffers, renderização, shaders, VSync/VRR ou apresentação. Não implementar controles de vídeo em Python nem usar callbacks Python por frame/pixel/evento. A interface Qt apresenta as opções e envia comandos tipados diretamente à camada nativa Rust/Qt definida pela arquitetura; a aplicação Python não intermedeia o caminho de vídeo.

Recursos avançados devem ser habilitados somente quando o backend, o sistema operacional, o driver e o monitor realmente os suportarem. Shaders e filtros programáveis devem ter disponibilidade condicionada aos recursos gráficos; VRR depende também do caminho de apresentação e da cadeia de display/driver, não apenas da API gráfica. A interface não deve prometer nem expor como funcional um recurso indisponível. O backend Raster deve continuar oferecendo uma saída correta mesmo quando recursos avançados não estiverem disponíveis.

**CUDA/NVIDIA:** CUDA via Rust é uma capacidade auxiliar opcional, não um quinto backend de apresentação nem requisito do emulador. Só poderá ser adotada para workloads identificados por profiling — por exemplo, processamento paralelo pesado ou tarefas de IA auxiliares — quando houver ganho demonstrável e caminho de interoperabilidade seguro. Não delegar a CUDA a semântica do VDP, eventos temporais, flags de hardware ou estado canônico. O resultado precisa manter contratos de determinismo e ter comparação com uma implementação Rust de referência; qualquer variação numérica, não determinismo ou dependência exclusiva de GPU deve ser explicitamente delimitada. A ausência de CUDA nunca pode impedir o boot ou a renderização correta.

## 6. Autoridade temporal e sincronização A/V

### 6.1 Master Scheduler Rust

Cada máquina terá um único scheduler mestre Rust. Ele é a única autoridade para avançar o tempo emulado e ordenar eventos de:

- CPU(s), DSPs e coprocessadores;
- ciclos T-state, M-cycle e fases de barramento conforme o nível modelado;
- acessos a memória/I/O, waits, contenção e arbitragem;
- VDP dots/fetches/pixels, HBlank, VBlank e interrupções;
- DMA e transferências entre dispositivos;
- clocks de PSG/FM/PCM/CDDA/PWM e produção de amostras;
- input/periféricos normalizados e eventos de hardware;
- reset, handshakes, save states e transições de subsistemas.

Dispositivos não possuem schedulers independentes nem avançam por relógio de parede. Domínios de clock distintos são representados explicitamente por relações determinísticas, razões inteiras/acumuladores de fase quando apropriado e regras documentadas de desempate.

O scheduler pode avançar até o próximo evento observável, mas nunca atravessar um evento que possa alterar o resultado. CPUs devem poder parar em fronteiras seguras para respeitar contenção e waits. A campanha completa de ciclo a ciclo pode ser incremental, mas a arquitetura de eventos e a autoridade temporal não podem ser delegadas à GUI.

### 6.2 Python e relógios do host

Qt/Python não controla T-states, ciclos, scanlines, pixels, áudio ou frame progression. QTimer, sleep, loops FPS ou callbacks Python não avançam a máquina. A GUI pode enviar comandos de ciclo de vida em alto nível (load, pause, resume, reset, stop, step request) e exibir telemetria.

VSync, VRR e clock do dispositivo de áudio regulam a apresentação física. Eles não alteram a timeline lógica do console. A apresentação é coordenada pelo pipeline Rust e pela superfície hospedeira Qt; a Qt não avança frames nem controla temporização de hardware. VRR só deve ser oferecido quando o backend/caminho de apresentação, o sistema, o driver e o display o suportarem. Eventos de input do host são normalizados na fronteira, ordenados e aplicados pelo runtime Rust.

### 6.3 Eficiência

Precisão não exige chamada de função por pixel ou por T-state. O runtime pode avançar em blocos até o próximo evento e renderizar segmentos entre eventos, com buffers pré-alocados, caches corretamente invalidados e operações inteiras determinísticas.

Não remover eventos observáveis para melhorar desempenho. JIT, paralelismo de hardware e otimizações especulativas exigem profiling, baseline e equivalência de resultados.

## 7. Áudio, input e periféricos

O áudio permanece no runtime Rust: PSG, FM, PCM/DAC, CDDA, PWM, mixagem digital, clocks e pipeline de amostras. Modelos analógicos e saída física do host são camadas distintas. O Sega CD inclui PCM/CDDA; o 32X inclui PWM. Não confundir funções integradas a ASICs com chips físicos independentes.

A entrada é adquirida por backends de host, normalizada e entregue ao runtime. A lógica de periféricos e seu tempo emulado pertencem a Rust. O escopo contempla gamepads de 2/3/6 botões, Pause físico SMS, controles analógicos, paddle, Sports Pad/trackball, XE-1AP, perfis de volante, mouse, pointer, lightguns, touchpad/sensores quando disponíveis, adaptadores multiplayer em fase posterior e microfone via endpoint de captura do host. Rapid Fire, Activator e Inovation Smash Controller não fazem parte do escopo inicial aprovado. Força de feedback fica para fase futura separada.

## 8. Validação e critérios de aceitação

Cada componente deve seguir inventário → critérios de aceitação → implementação em fatias → testes rápidos → validação funcional curta → validação longa → integração → validação temporal. Preservar regressões do SG-1000 em todas as mudanças subsequentes.

### 8.1 Matriz mínima para VDP

| Domínio | Evidência exigida |
|---|---|
| Registradores/portas | Vetores de leitura/escrita, máscaras, aliases, latches e efeitos colaterais |
| Memórias | Endereçamento, wrap, buffers, acessos concorrentes e restrições |
| Identidade da mídia | DAT No-Intro/Redump versionado, fingerprints, tamanho/estrutura, revisão/região, correspondência exata/derivada/ambígua e diagnóstico reproduzível |
| CartProfile/BoardProfile | Perfil de placa separado do MachineProfile e do mapper executável; ROM banking/expandida, SRAM/RAM embutida, registradores, expansões e reset |
| Mappers | Vetores por mapper/placa, bancos/janelas, limites, leituras/escritas, reset, RAM persistente e efeitos de barramento comprovados |
| Multicarts e hacks | Seleção de título/banco reproduzível; hacks/patches identificados como derivados, com procedência e perfil explícito quando alterarem hardware |
| Requisitos do jogo | Jogadores, modo de jogo, acessórios, BIOS/firmware e peculiaridades de boot com fonte/confiança ou estado UNKNOWN |
| MediaSet e mídia composta | Identidade individual e composição de cartuchos lock-on, patches, sidecars, imagens de dados e trilhas; hashes/procedência e dependências reproduzíveis |
| Lock-on | Dois cartuchos/dispositivos distintos, pass-through, seleção/decodificação de barramento, combinações suportadas e save state |
| MSU-MD / MD+ | Perfis distintos; patch/base ROM/sidecars/CUE e trilhas validados; Mode 1/BIOS separado de MD+ overlay; incompatibilidades e arquivos ausentes diagnosticados |
| Raster | H/V counters, linhas, dots, blanking, fronteiras e eventos |
| Sprites | Limites, avaliação, prioridade, colisão, overflow e casos fora da tela |
| Paleta/composição | Valores digitais, transparência, prioridade e mistura entre camadas |
| Interrupções | Instante de geração, acknowledge, cancelamento e estado |
| Raster effects | Escritas no meio da linha/quadro e mudança de modo |
| DMA/barramento | Duração, arbitragem, waits, contenção e efeitos na CPU |
| Limite de sprites | Limite específico do hardware ativo por padrão em cada VDP; modo aprimorado opt-in; flags/status observáveis permanecem coerentes; ambos os modos têm vetores de teste |
| Backends de vídeo | Raster, D3D11, D3D12 e Vulkan testados individualmente quando disponíveis; seleção automática, seleção manual, fallback e falha de inicialização cobertos |
| Equivalência de renderização | Comparar saída Raster e backends acelerados para casos determinísticos; divergências de precisão/formato devem ser justificadas e documentadas, não aceitas por tolerância arbitrária |
| Fronteira Qt/Rust | Qt configura e apresenta; Rust possui pipeline e recursos de vídeo; nenhum controle de saída/vídeo em Python; nenhuma chamada por frame/pixel/evento |
| Recursos avançados | Shaders e VRR só aparecem ativos quando a cadeia completa de hardware/software os suporta; fallback não pode afirmar suporte inexistente |
| Determinismo | Mesma configuração/estado/eventos produzem estado, traces e pixels idênticos |
| Integração | CPU(s), VDP(s), áudio, memória e scheduler compartilham a mesma timeline |

Para cada divergência, procurar a primeira diferença de estado ou evento, não apenas comparar o framebuffer final. Capturas por linha/quadro, registradores, posição raster, endereços de memória, traces de barramento, sprites e interrupções devem ser correlacionáveis. Hashes iguais não provam correção absoluta.

### 8.2 Oráculos e evidências

Priorizar manuais de fabricante, especificações, testes independentes e evidência de hardware. Ares, MAME, Gearsystem, MEKA, Genesis Plus GX e outros emuladores são referências de comportamento e estratégia, não prova infalível nem licença para copiar código. Registrar versão, licença, procedência e ancestralidade do oráculo.

Usar vetores fixos, ROMs sintéticas/homebrew legais e testes diferenciais independentes. Nunca versionar ROMs comerciais, BIOS protegidas ou logs/snapshots que revelem conteúdo de ROM.

### 8.3 Estados separados

Registrar explicitamente:

- `IMPLEMENTED`
- `QUICK_PASS`
- `SHORT_PASS`
- `LONG_PASS`
- `INTEGRATION_PENDING`
- `TEMPORAL_PASS`

Implementação funcional não implica validação longa; renderização correta em testes estáticos não implica raster correto; contagem nominal de ciclos não implica precisão ciclo a ciclo. `TEMPORAL_PASS` exige traces temporais e evidência independente.

### 8.4 Critérios globais de qualidade

- Mesma configuração, estado inicial e eventos produzem o mesmo resultado.
- Nenhum fallback de runtime de hardware em Python.
- Alterações em sistemas posteriores preservam regressões dos sistemas anteriores.
- Checks Rust/Python, build Release/Maturin e workflows aplicáveis são executados no SHA candidato exato.
- Testes in-game são exigidos para mudanças de gameplay; quando dependem da máquina do mantenedor, registrar BLOCKED/PENDING e solicitar evidência.
- Checks ausentes, ignorados ou bloqueados não são aprovação.
- Benchmarks com baseline/candidato equivalentes, workload, build, aquecimento, repetições, mediana, variação e limitações.

## 9. Ordem de trabalho específica para vídeo

1. **SG-1000:** congelar baseline de vetores, registradores, VRAM, sprites, colisão/overflow e hashes por cena. Preservar o núcleo TMS9918A validado. Ampliar temporização e variantes somente com evidência e regressão.
2. **Master System:** prioridade imediata de auditoria. Reproduzir e catalogar bugs atuais; criar testes que falhem antes da correção; revisar Mode 4, registradores, VRAM/CRAM, scroll, sprites, limites, status, contadores H/V, IRQ, lightgun e raster effects. Corrigir primeiro semântica funcional; depois avançar para eventos intralinha/fetches e contenção.
3. **Game Gear:** especializar a base compatível SMS, com LCD 160×144 como viewport físico, janela/recorte de origem, paleta e diferenças de temporização comprovadas.
4. **Mega Drive:** iniciar VDP próprio após fundação da plataforma/68000/barramento, com H32/H40, planos, sprites, DMA, interlace, raster, status, IRQ e acessos.
5. **Sega CD:** integrar Word RAM, Cell Arranger, ASICs e transferências ao VDP Mega Drive, sem duplicar o núcleo.
6. **32X:** modelar VDP/framebuffers e compositor com o VDP Mega Drive, respeitando troca de superfície, prioridade e sincronização.

Não implementar os seis núcleos em paralelo. O SMS é a próxima trilha de correção de vídeo; SG-1000 é baseline de regressão.

## 10. Fora do escopo ou condicionado

- Não modelar transistor-level ou todos os circuitos analógicos de cada IC sem necessidade observável e evidência.
- Não transformar todo chip convencional de vídeo/áudio/I/O em coprocessador programável.
- Não criar um VDP genérico que apague diferenças reais entre consoles.
- Não tratar resolução de apresentação como raster físico.
- Não permitir que mídia escolha silenciosamente o clock físico da máquina.
- Não identificar cartuchos apenas por nome, extensão, tamanho ou um único byte de cabeçalho; usar DATs e fingerprints com proveniência e estado de confiança.
- Não tratar No-Intro/Redump como catálogo completo de comportamento de hardware, jogadores ou acessórios; esses dados exigem metadados técnicos complementares e evidência rastreável.
- Não selecionar mapper, perfil de placa, RAM/SRAM ou política de BIOS por heurística silenciosa quando a identidade for ambígua.
- Não confundir CartProfile/BoardProfile, implementação executável do mapper e MachineProfile; são responsabilidades distintas.
- Não achatar cartuchos lock-on em uma ROM concatenada nem tratar cartuchos acoplados como um único dump sem preservar seus dispositivos e relações.
- Não tratar MSU-MD e MD+ como sinônimos: seus protocolos, caminhos de áudio/CD e requisitos de hardware/BIOS são distintos.
- Não validar ROM modificada como dump original No-Intro; preservar hash da base, patch/procedência, hash resultante e recursos auxiliares.

- Não exigir BIOS de SMS/GG universalmente; respeitar modelo e configuração.
- Não simular Sega CD funcional sem firmware exigido e válido.
- Não ativar HLE como se fosse firmware original.
- Não criar scheduler Python, timers de frame ou clocks independentes por dispositivo.
- Não usar o frontend para compor imagens de hardware que deveriam ser compostas no runtime.
- Não implementar ou controlar o pipeline de vídeo em Python; a Qt limita-se à interface, configuração e apresentação da saída Rust.
- Não desabilitar por padrão os limites reais de sprites por linha; o modo aprimorado deve ser opt-in e preservar os efeitos de hardware observáveis.
- Não tratar CUDA como backend obrigatório de vídeo, nem como substituto da semântica determinística dos dispositivos.
- Não contar Raster como API gráfica nem Qt QRhi como quinto backend desta decisão.
- Não permitir que Python controle backend, buffers, renderização, sincronização ou saída de vídeo; essas responsabilidades ficam em Rust e na camada nativa Qt de apresentação/configuração.
- Não iniciar implementação do SVP antes da trilha Mega Drive e dos critérios específicos de aceitação, salvo decisão técnica expressa.
- Não introduzir JIT, paralelismo especulativo ou otimizações não medidas.

## 11. Documentos especializados relacionados

Este documento é o índice normativo de escopo. Os documentos específicos detalham contratos e evidências e devem permanecer consistentes com ele:

- [Arquitetura multi-sistema](architecture.md)
- [Política permanente de desenvolvimento, fidelidade e entrega](development-and-validation-policy.md)
- [Arquitetura e modelo TMS9918A](hardware/tms9918a-model.md)
- [Auditoria técnica SMS — fotografia de 2026-10-29](audits/sms-current-technical-status-2026-10-29.md)
- [Arquitetura do Master Scheduler nativo](architecture/native-master-scheduler.md)
- [Plano de migração do Master Scheduler](architecture/migrations/native-master-scheduler.md)
- [Runtime de áudio nativo multi-sistema](architecture/native-audio-runtime.md)
- [Escopo de periféricos e mapeamentos de host — PR #317](https://github.com/leonardo201800478/sega-emulator/pull/317)
- [Diretriz de autoridade temporal Rust — PR #318](https://github.com/leonardo201800478/sega-emulator/pull/318)
- [Arquitetura de coprocessadores especializados — PR #320](https://github.com/leonardo201800478/sega-emulator/pull/320)

Links para PRs identificam propostas relacionadas e não significam que estejam merged ou que façam parte da main. Sempre conferir seu estado remoto antes de depender de conteúdo ainda não integrado.

## 12. Regra de manutenção deste escopo

Toda nova plataforma, chip, periférico, padrão regional, firmware obrigatório, VDP ou alteração de autoridade temporal deve atualizar este documento ou apontar para uma decisão especializada vinculada a ele. A alteração deve ser feita por PR, com motivação, impacto nos marcos, dependências, critérios de aceitação, evidências e pendências.

Nenhuma decisão nova pode contradizer silenciosamente este escopo. Se surgir evidência de hardware que exija mudança, registrar a divergência, sua fonte e a decisão revisada. O escopo é vivo, versionado e auditável; não substitui a validação do código real.

---

**Decisão consolidada:** o Sega Emulator é um conjunto de máquinas Sega compostas por dispositivos Rust específicos, inicializadas a partir de perfis explícitos de hardware/mídia/firmware e sincronizadas por uma timeline mestre Rust. Os VDPs mantêm identidade e regras próprias, com SG-1000 preservado, SMS como prioridade de correção e Game Gear, Mega Drive, Sega CD e 32X tratados em sequência. Coprocessadores especializados como o SVP fazem parte do escopo futuro, com núcleos próprios e integração baseada em evidências.
