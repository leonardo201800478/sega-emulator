# Roadmap mestre — Sega Emulator

**Status:** documento canônico de direção do projeto; todas as fases abaixo são planejadas, salvo quando um estado for explicitamente comprovado por código, testes e SHA.
**Autoridade:** este documento define o escopo global, a ordem dos marcos e as composições de máquinas. Roadmaps de subsistemas são planos subordinados e não podem redefinir o escopo global.
**Runtime:** hardware e tempo emulado em Rust; Python/Qt somente aplicação e tooling; PyO3/Maturin como fronteira coarse-grained.
**Plataforma de desenvolvimento prioritária:** Windows x64.

> Este roadmap substitui o antigo cronograma geral de reorganização. Ele não substitui os planos técnicos detalhados de SMS, Z80, PSG, mappers, VDP ou Scheduler. Esses documentos permanecem válidos somente para seu domínio e devem seguir as decisões globais aqui registradas.

## 1. Regras arquiteturais não negociáveis

1. Um único runtime digital de hardware em Rust.
2. Um único Master Clock Scheduler por máquina composta; cada chip mantém seu estado e domínio de clock, mas não avança o tempo por conta própria.
3. CPUs compartilhadas por família quando a equivalência estiver demonstrada: Z80, Motorola 68000 e SH-2. Adaptadores de plataforma implementam barramento, interrupções, clocks, contenção, reset e periféricos.
4. Chips de vídeo e áudio são módulos próprios, com perfis/revisões explícitos. Não presumir equivalência entre variantes apenas por pertencerem à mesma família comercial.
5. Região, revisão de placa, modelo comercial e configuração de vídeo são dimensões independentes.
6. Hardware do host (Windows, GPU, áudio, input e armazenamento) não determina o tempo emulado.
7. Sem JIT, frame-skip, hacks temporais ou paralelismo especulativo.
8. Nenhuma funcionalidade é considerada implementada por estar documentada ou por existir em outro emulador.
9. Mudanças não triviais em branch + PR; nunca publicar diretamente na main, force-push ou fazer merge sem autorização explícita.
10. Não versionar ROMs comerciais, BIOS proprietárias, firmware protegido ou dumps de memória protegida.

## 2. Catálogo de sistemas e variantes

### 2.1 SG-1000 e Master System

- SG-1000 original e SG-1000 II.
- Master System / Mark III: Model I e Model II, revisões de placa e ASICs documentados.
- Variantes regionais, incluindo Japão, América do Norte, Europa, Brasil/TecToy e Coreia, conforme evidência de hardware.
- Perfis de VDP, PSG, memória, I/O e RF/vídeo devem refletir diferenças comprovadas; não inferir ASIC exclusivamente pelo nome comercial.

### 2.2 Game Gear

- Família Game Gear, incluindo revisões de placa, ASICs, LCD, áudio estéreo, I/O e variantes regionais documentadas.
- Não inventar uma família comercial “Game Gear II” sem evidência de um modelo oficial distinto.
- Compartilhar Z80 e componentes somente onde o comportamento for comprovadamente comum ao SMS.

### 2.3 Mega Drive / Genesis

- Genesis/Mega Drive Model 1 e Model 2 como modelos-base do escopo inicial.
- **Genesis 3 / Mega Drive 3 está excluído do escopo atual**, conforme decisão do projeto.
- Sega Nomad é uma variante portátil da família Genesis, com perfil próprio de LCD, alimentação, entrada e particularidades de placa. Não presumir suporte nativo a Sega CD ou 32X.
- Revisões de placa, ASICs, codificadores de vídeo, áudio e regiões são perfis explícitos, não novos executores de console.

### 2.4 Expansões modulares

- Sega CD / Mega CD: Model 1 e Model 2, revisões, BIOS e regiões.
- Sega 32X: revisões e perfis de hardware/firmware.
- Genesis + Sega CD, Genesis + 32X e Genesis + Sega CD + 32X são composições de uma máquina, não três implementações duplicadas.
- O Sega CD reutiliza o Genesis VDP e acrescenta seus próprios recursos de processamento/memória/vídeo; não deve ser modelado como um segundo VDP Genesis completo.
- O 32X acrescenta SH-2, framebuffer, composição de vídeo e áudio próprios, sincronizados com o Genesis.

### 2.5 Sistemas integrados de terceiros

Registrar modelos comerciais como perfis de máquina integrados quando houver diferenças reais de placa, firmware, I/O, áudio, vídeo ou sequência de boot:

- Sega CDX / Multi-Mega.
- JVC/Victor Wondermega, incluindo as famílias/revisões M1 e M2 conforme identificação correta.
- JVC X'Eye.
- Aiwa CSD-G1M.
- Pioneer LaserActive com módulos Sega PAC-S1 (Japão) e PAC-S10 (América do Norte).
- Outros modelos regionais ou licenciados poderão ser acrescentados mediante documentação e identificação de firmware.

**Correção terminológica:** a referência histórica pertinente é JVC/Victor; “JVS” não será tratado como fabricante ou família de console neste catálogo sem evidência adicional.

O Pioneer LaserActive é um host modular: o PAC Sega fornece a compatibilidade Sega; a reprodução de Mega-LD exige subsistema LaserDisc/AV próprio. Não reduzir LaserActive a um Mega CD convencional.

### 2.6 Híbridos PC + Mega Drive

TeraDrive e Amstrad Mega PC podem constar em um catálogo de hardware histórico/futuro, mas são combinações PC + Mega Drive, não consoles Mega Drive + Sega CD. Não fazem parte do núcleo de compatibilidade prioritário até haver uma demanda de emulação do subsistema PC.

## 3. Sega Neptune — definição oficial do projeto

Há duas identidades que não podem ser confundidas:

- **Neptune histórico:** projeto cancelado da Sega concebido como Genesis/Mega Drive + 32X integrado. A compatibilidade com Sega CD deve ser tratada como expansão/acessório quando aplicável, não como evidência de um drive de CD incorporado.
- **Sega Neptune Definitive (projeto hipotético):** composição máxima criada pelo Sega Emulator, reunindo Genesis + Sega CD + 32X em uma única máquina lógica sincronizada.

A Neptune Definitive não será apresentada como reconstrução comprovada de um protótipo comercial da Sega. É uma máquina hipotética do nosso projeto, construída com os componentes reais e interfaces temporais das três plataformas.

Critérios:
- Um único estado global de máquina e um único Scheduler.
- CPU principal 68000, Z80 auxiliar, CPU(s) do Sega CD e dois SH-2 do 32X, cada qual com seu domínio de clock e arbitragem.
- Genesis VDP + pipeline/framebuffer do 32X; recursos de vídeo do Sega CD modelados como extensão do caminho real.
- BIOS, CD media, memória, DMA, interrupções, PCM/CD-DA, PWM e mistura de áudio com timestamps.
- Reset, power-on, save states e restore coordenados para toda a composição.
- Perfis regionais e BIOS compatíveis; nunca selecionar firmware apenas pelo nome de arquivo.

## 4. Regiões, revisões e perfis

O resolvedor de máquina deve separar, no mínimo:

- fabricante e modelo comercial;
- revisão de placa e identificadores ASIC/chips;
- região física e variante de firmware/BIOS;
- padrão de vídeo e frequência: NTSC, PAL e PAL-M;
- clocks mestres, divisores, relações de fase e tolerâncias documentadas;
- mapa de memória, barramentos, portas, periféricos e expansões;
- mídia, firmware obrigatório/opcional/proibido e sequência de boot.

Perfis regionais candidatos incluem Japão, América do Norte, Europa, Brasil/TecToy e Coreia, além de variantes asiáticas documentadas. A lista é um catálogo de pesquisa, não uma afirmação de que todos os modelos existem em todas as regiões. PAL-M brasileiro é um perfil de vídeo distinto de PAL europeu. Não substituir região física pelo código regional da ROM.

## 5. Processadores e chips compartilhados

### Processadores

- Z80: um núcleo compartilhado; adaptadores SG-1000, SMS, Game Gear e Z80 auxiliar Genesis.
- Motorola 68000: núcleo compartilhado entre Genesis e subsistemas que realmente utilizem a mesma arquitetura, com perfis e barramentos próprios.
- SH-2: núcleo compartilhado para os dois processadores do 32X, mantendo estados, linhas e sincronização independentes.
- Processadores e DSPs especializados de cartuchos/expansões (ex.: SVP) permanecem núcleos específicos, não são forçados a um interpretador universal.

### Vídeo

- TMS9918/TMS9918A para SG-1000 e variantes aplicáveis.
- SMS VDP por revisão/ASIC.
- Game Gear VDP/perfil portátil, compartilhando apenas o que a equivalência permitir.
- Genesis VDP por revisão.
- 32X framebuffer e composição.
- LaserActive/Mega-LD requer subsistema LaserDisc/AV separado.
- Sega CD não ganha um VDP Genesis duplicado.

### Áudio

- SN76489 e perfis Sega/TI específicos.
- YM2413 quando presente e selecionado pelo perfil de hardware.
- YM2612 e YM3438 não devem ser abstraídos como eletricamente idênticos; manter diferenças de DAC/saída e revisão.
- RF5C164, CD-DA, PWM e demais recursos por plataforma.
- Mistura de áudio coordenada por timestamps, preservando níveis e filtros específicos da máquina.

## 6. Firmware Registry e BIOS externas

O projeto terá um registro de firmware compartilhado. BIOS e firmware são arquivos externos fornecidos pelo usuário; não serão armazenados no Git nem distribuídos pelo projeto.

Metadados mínimos:
- firmware_id estável;
- plataforma/família, fabricante, modelo e revisão;
- região e versão;
- tamanho esperado e SHA-256;
- papel (obrigatório, opcional, proibido ou não utilizado);
- perfis compatíveis e incompatibilidades conhecidas;
- fonte/proveniência e notas de validação.

O carregador preserva o arquivo original, calcula hashes e valida compatibilidade. A resolução deve considerar modelo, região, revisão e composição. Nome do arquivo isolado nunca é suficiente. BIOS ausente/inválida quando obrigatória bloqueia o boot com erro claro; BIOS opcional ausente não bloqueia. Não criar fallback silencioso entre BIOS de Genesis, Mega CD, CDX, Wondermega, X'Eye, Aiwa ou PAC.

A coleção pessoal de BIOS do mantenedor poderá ser usada localmente como corpus de teste, sem ser enviada, anexada ao repositório ou exposta em logs.

## 7. Arquitetura lógica do workspace

A organização final deve refletir os crates e módulos existentes após inventário, evitando mover tudo ou duplicar implementações apenas para combinar com uma árvore idealizada.

Domínios lógicos desejados:
- processors: z80, m68000, sh2 e processadores especializados;
- chips/video, chips/audio e chips/io;
- platforms: sg1000, master_system, game_gear e genesis;
- expansions: sega_cd e sega_32x;
- integrated_systems: cdx, wondermega, jvc_xeye, aiwa e pioneer_laseractive;
- temporal: clocks, eventos, transações e Master Scheduler;
- engine: composição, configuração de perfis, firmware, mídia, reset, save states e replay;
- hosts/windows: apresentação de vídeo, saída de áudio, input, armazenamento e lifecycle de dispositivos;
- frontend_qt: UI e tooling sem lógica de hardware.

Esses nomes são agrupamentos conceituais. A migração real será incremental e guiada pelo grafo de dependências do Cargo Workspace e pelas APIs existentes.

## 8. Ordem de execução e gates

### Fase 0 — Revalidar o repositório
- Confirmar SHA de main, PRs, branches, workflows, workspace e arquivos.
- Reconciliar documentos e contratos já publicados.
- Identificar implementações existentes antes de criar crates ou mover arquivos.
- Criar matriz de hardware, perfil, chip, firmware, testes e evidência.

### Fase 1 — SG-1000 + SMS
- Preservar regressões SG-1000.
- Consolidar Z80 compartilhado sem alteração semântica não comprovada.
- Fechar lacunas Z80 e contratos temporais.
- Integrar CPU, Bus, VDP SMS1, PSG, I/O e IRQ ao Scheduler único.
- Validar mappers e perfil regional; manter SMS2/GG como comparativos distintos.
- O plano detalhado de consolidação SG-1000/SMS é subordinado a este roadmap.

### Fase 2 — Game Gear
- Resolver perfil de hardware, revisões, LCD, I/O, estéreo e região.
- Reutilizar Z80 e chips apenas com equivalência demonstrada.
- Criar testes específicos de vídeo portátil e áudio estéreo.

### Fase 3 — Mega Drive / Genesis
- Integrar M68000, Z80 auxiliar, Genesis VDP, áudio, cartuchos, mappers, I/O e perfis Model 1/2.
- Implementar regiões e revisões por evidência.
- Adicionar Nomad como perfil portátil específico, não como executor separado.

### Fase 4 — Sega CD
- Implementar subsistema CD, CPU, BIOS, mídia, Word RAM, ASICs, DMA, RF5C164 e CD-DA.
- Validar Model 1/2 e perfis regionais.
- Integrar como expansão do Genesis com arbitragem e estado coordenados.

### Fase 5 — Sega 32X
- Integrar dois SH-2, memória, framebuffer, PWM, barramento, BIOS/firmware e composição de vídeo.
- Validar Genesis + 32X e compatibilidade com cartuchos.
- Integrar ao Genesis Scheduler sem avanço temporal independente.

### Fase 6 — Neptune Definitive e composições combinadas
- Integrar Genesis + Sega CD + 32X.
- Validar jogos e inicialização Sega CD 32X.
- Definir o Neptune histórico (Genesis + 32X) como perfil de composição separado da Neptune Definitive.
- Validar reset, IRQ, DMA, bus arbitration, vídeo, áudio, save states e replay globais.

### Fase 7 — Sistemas integrados e de terceiros
- CDX/Multi-Mega, Wondermega, X'Eye, Aiwa e Pioneer LaserActive/PAC.
- BIOS e diferenças de placa específicas.
- LaserDisc/Mega-LD somente com subsistema e corpus de validação próprios.
- TeraDrive/Amstrad Mega PC permanecem em catálogo futuro, fora do caminho crítico de consoles.

### Fase 8 — Robustez e validação transversal
- Testes unitários, integração, diferencial independente e hardware-oracle quando disponível.
- Replays determinísticos, snapshots globais e testes longos.
- Validação Windows x64, PyO3/Maturin, D3D12, WASAPI e input físico.
- Perfilamento somente depois da baseline de correção.

## 9. Estados e política de validação

Cada componente e plataforma usa estados explícitos:
- IMPLEMENTED
- QUICK_PASS
- SHORT_PASS
- LONG_PASS
- INTEGRATION_PENDING
- TEMPORAL_PASS

Cada suíte deve ser registrada como PASS, FAIL, BLOCKED, NOT RUN ou SKIPPED, sempre vinculada ao SHA candidato. Teste ignorado não é PASS. CI em SHA anterior não aprova SHA atual. TEMPORAL_PASS exige traces, chunk invariance, causalidade, replay e oráculos independentes apropriados; não é sinônimo de executar jogos.

Gates obrigatórios:
```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python -m pytest
python -m ruff check .
python -m ruff format --check .
```
Build Release/Maturin, Windows x64, D3D12, áudio/input físicos e testes de mídia/BIOS devem ser acrescentados conforme a fatia.

## 10. Hierarquia documental e prevenção de conflitos

1. **Este arquivo — `docs/project-roadmap.md`:** fonte única para escopo global, ordem de plataformas, modelos e composições.
2. **`docs/project-scope.md`:** princípios, limites de runtime, mídia, perfis e contratos transversais; não redefine a ordem global.
3. **ADRs:** decisões arquiteturais aprovadas/propostas com estado explícito.
4. **Roadmaps de subsistema:** planos de execução restritos (SMS, Z80, PSG, mappers, vídeo, input etc.); não podem contradizer este documento. Se houver conflito, registrar ADR/alteração aqui antes de implementar.
5. **Auditorias e evidence registers:** fontes de fatos, incertezas e evidências; não são roteiros globais.

Roadmaps históricos que contenham cronogramas, agentes, SHAs ou PRs antigos não são autoridade operacional. A antiga carta de reorganização e cronograma de agentes foi removida por estar desatualizada e conter baseline/planos de PR históricos. Seus documentos técnicos e ADRs úteis permanecem preservados.

## 11. Estado desta consolidação

Esta é uma consolidação documental e uma decisão de direção do projeto. Não declara implementados Game Gear, Genesis, Sega CD, 32X, Neptune, CDX, Wondermega, X'Eye, Aiwa ou LaserActive. Não altera o estado de validação da PR #396 nem autoriza merge. A execução começa pela revalidação do SHA remoto e pela continuidade do trabalho temporal SMS1 já aberto.
