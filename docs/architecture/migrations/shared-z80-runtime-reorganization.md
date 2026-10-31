# Reorganização arquitetural — runtime Rust, Z80 compartilhado e sincronização temporal

**Data do inventário:** 2026-10-31  
**Baseline de `main` antes desta atualização documental:** `3cf846d688798080d35f997f6e0d34680372d7e3`  
**Natureza desta publicação:** documentação e decisões de arquitetura; nenhum código de runtime é alterado por este documento.  
**Estado global:** REORGANIZATION_PLANNED; migrações e integrações permanecem em andamento.

## 1. Objetivo

Consolidar a arquitetura do Sega Emulator para que todo hardware emulado seja executado por um runtime Rust determinístico, modular e testável. Python/PySide6 permanece como frontend e camada de ferramentas, sem implementar CPU, memória, barramento, VDP, PSG, temporização ou execução de máquina.

A reorganização cobre:

1. runtime de hardware 100% Rust;
2. consolidação de um único motor Z80 reutilizável;
3. timeline e scheduler temporal por instância de máquina;
4. separação entre CPU, barramento, dispositivos e perfis de plataforma;
5. integração host por interfaces coarse-grained PyO3/Maturin;
6. documentação, testes e workflows alinhados à arquitetura efetivamente presente.

Esta reorganização não autoriza remover código legado, alterar comportamento de gameplay ou declarar validação sem evidências. A prioridade arquitetural não elimina os gates de compilação, semântica, determinismo e regressão antes de integrar cada fatia.

## 2. Inventário do estado remoto

O HEAD de `main` observado no inventário foi `3cf846d688798080d35f997f6e0d34680372d7e3` (`update readme`). O manifesto raiz usa Cargo resolver 3 e contém:

- `native/sg1000_core`
- `native/sms_core`
- `native/z80_alu`
- `native/z80_bus`
- `native/audio_runtime`

O workspace exclui `native/z80_core`, `native/sega_emu_core` e `native/audio_output`. A extensão em `native/z80_core` é uma fronteira PyO3/Maturin, não deve ser confundida com um motor Z80 independente de plataforma.

### 2.1 Duplicação Z80 confirmada

Em `main`, existem implementações de CPU Z80 em:

- `native/sg1000_core/src/cpu.rs`
- `native/sms_core/src/cpu.rs`

Os dois arquivos expõem estado e fluxo de execução muito semelhantes, mas vivem em módulos e contratos de barramento distintos. Os cabeçalhos documentam que o código SMS consolidou trabalho anterior do SG-1000, enquanto o código SG-1000 foi portado do SMS mais maduro. Isso confirma duplicação de implementação, não a existência de uma biblioteca Z80 única compartilhada.

O executor atual contabiliza T-states nominais por instrução e mantém instrumentação temporal parcial. Isso não é um trace completo de acessos de barramento por T-state e não comprova precisão ciclo a ciclo.

### 2.2 Situação temporal

A documentação e o inventário de código registram caminhos locais distintos:

- SG-1000 avança a CPU por instrução e sincroniza VDP/PSG em fronteiras de instrução;
- SG-1000 converte T-states para clocks PSG por conversão racional e resto local;
- SMS mantém contador T-state/PSG e fila de escritas próprios;
- VDPs avançam por contadores agregados, sem precisão dot-cycle completa;
- não havia integração de um scheduler temporal compartilhado no runtime das máquinas no baseline.

A PR #315 propõe a primeira fundação compartilhada `sega-temporal-core`. A PR permanece separada desta documentação e precisa ser avaliada pelos seus próprios checks e critérios. Não assumir que seu conteúdo já está em `main`.

### 2.3 Outros subsistemas

- **Áudio:** existe runtime Rust para DSP/PCM e uma fronteira de saída nativa em evolução. O desacoplamento integral do avanço da máquina em relação ao pacing Qt permanece pendente.
- **Input:** PR #313 propõe runtime Rust/SDL3 e ponte Qt; PR #300 é uma proposta anterior relacionada. As duas não devem ser integradas em duplicidade.
- **Vídeo:** PR #301 propõe migrar renderização e apresentação para Rust; não considerar concluída a migração só porque existe uma proposta.
- **Frontend:** Python/PySide6 continua apropriado para janela, menus, configuração, carregamento de ROMs, logs e ferramentas.
- **Hardware legado Python:** código de referência em `legacy/python_reference/` não é runtime ativo e não deve voltar a ser importado pelo caminho de produção nem usado como oráculo permanente.

## 3. Arquitetura-alvo do Z80 compartilhado

### 3.1 Decisão

Criar um único motor CPU Z80 independente de console, em um crate Rust próprio, proposto como:

- caminho: `native/z80_cpu`
- pacote: `sega-z80-cpu`
- biblioteca: `sega_z80_cpu`

O nome `native/z80_core` já identifica a extensão/adaptador PyO3 no projeto; portanto, não reutilizar esse caminho para o novo motor de CPU. A nomenclatura final deve ser confirmada na implementação e em `cargo metadata`.

O novo crate será a única implementação de semântica de instruções Z80 em produção. SG-1000, SMS, Game Gear e Mega Drive instanciam o mesmo motor e fornecem seus próprios adaptadores de barramento, perfil de máquina, fontes de interrupção e regras de arbitragem.

### 3.2 Um motor, duas granularidades de execução

A arquitetura pretendida é híbrida no sentido de manter **uma única semântica/decodificação**, com duas formas de conduzir a execução:

1. **Modo de instrução:** API coarse-grained que executa uma instrução até a próxima fronteira segura e retorna resultado/telemetria. Deve preservar o caminho de compatibilidade durante a extração inicial e continuar útil para testes sem exigir que o scheduler execute uma chamada por T-state.
2. **Modo temporal:** a mesma instrução é decomposta progressivamente em micro-operações e acessos ao barramento com instante/duração explícitos. O scheduler pode suspender e retomar a CPU entre fronteiras temporais, WAIT, contenção ou arbitragem.

Não criar dois decodificadores nem manter dois motores semanticamente independentes. O modo por instrução deve consumir a mesma sequência de operações do motor temporal, ou permanecer uma fachada transitória com equivalência demonstrada durante a migração.

### 3.3 Responsabilidades do motor CPU

O crate compartilhado é responsável por:

- registradores principais e alternativos, IX/IY, I/R, IFF1/IFF2, IM, HALT e estado interno documentado;
- decodificação base, CB, ED, DD/FD, DDCB/FDCB e tratamento de prefixos;
- semântica de flags, WZ/MEMPTR e latches cuja inclusão seja sustentada por evidência;
- solicitações e acknowledge de INT/NMI, EI delay e retorno de interrupção;
- estado determinístico de reset e snapshots;
- emissão de operações de memória/I/O e eventos temporais conforme o estágio implementado;
- erros explícitos para estados/opcodes sem suporte, sem fallback silencioso.

O motor não deve conhecer registradores VDP, portas específicas de SMS, slots ROM, banking, mapper, FM, estéreo Game Gear, registradores de controle do Mega Drive ou política de ownership do barramento Genesis.

### 3.4 Contratos e extensões por plataforma

A CPU acessa o ambiente externo por um contrato de barramento comum, hardware-neutral. A implementação concreta pertence à máquina:

| Plataforma | Adaptador/responsabilidade externa |
|---|---|
| SG-1000 | Mapa de memória SG-1000, portas, cartucho, I/O e IRQs dos dispositivos da plataforma |
| SMS | Slots/banking, RAM, portas, VDP, PSG/FM quando suportado e IRQs do SMS |
| Game Gear | Mapa compatível com a família SMS, particularidades de portas, VDP/áudio estéreo e comportamento próprio do handheld |
| Mega Drive/Genesis | Barramento Z80 da máquina, RAM Z80, ROM/áreas mapeadas, acesso a dispositivos de som e arbitragem/ownership compartilhado com 68000 |

Diferenças não devem ser implementadas como condicionais dispersas no decodificador. Quando uma diferença for realmente comportamento da CPU/silício, usar perfil explícito de modelo ou capability com documentação e vetores. Quando for comportamento da placa/máquina, implementar no adaptador de barramento ou no scheduler da plataforma.

O contrato deve evoluir de chamadas lógicas de leitura/escrita para eventos tipados, por exemplo fetch M1, leitura/escrita de memória, leitura/escrita de I/O, acknowledge de interrupção, ciclos internos e estados de espera. O timestamp e a duração só podem ser afirmados quando o motor emitir esses eventos de forma validada.

### 3.5 Limite do Mega Drive

Mega Drive/Genesis não usa o Z80 como CPU principal. O sistema possui 68000 como processador principal e Z80 como processador auxiliar, tipicamente associado ao subsistema de som e sujeito às regras de barramento/ownership da máquina.

Portanto, a unificação solicitada significa reutilizar o mesmo motor Z80, não substituir ou absorver o 68000. O futuro core Mega Drive deve coordenar 68000, Z80, VDP, YM2612/PSG, DMA e arbitragem sob um scheduler da máquina. O Z80 não terá scheduler autônomo que avance fora da timeline Genesis.

## 4. Arquitetura-alvo temporal

### 4.1 Um scheduler por máquina

Cada instância de máquina possui exatamente um proprietário temporal Rust. Não criar scheduler independente por CPU, VDP, PSG ou chip.

- SG-1000: um scheduler coordena Z80, barramento, VDP, PSG, I/O e interrupções.
- SMS/Game Gear: um scheduler coordena Z80, barramento, VDP, PSG/FM aplicável, I/O e interrupções.
- Mega Drive: um scheduler coordena 68000, Z80 auxiliar, barramentos, VDP, áudio, DMA e arbitragem.

O scheduler poderá administrar múltiplos domínios de clock, cada um com razão e fase próprias sustentadas por documentação/evidência. `MachineTime` é unidade inteira definida por perfil de máquina; não presumir um clock universal nem inserir frequências sem fonte.

### 4.2 Componentes previstos

- `MachineTime`: timestamp inteiro e monotônico;
- `ClockRatio` / `ClockDomain`: conversão racional com resto/fase preservada;
- `EventPriority` / `EventQueue`: ordenação estável por timestamp, prioridade da plataforma e sequência de inserção;
- `MachineScheduler`: proprietário do relógio virtual, safe points, despacho e política delta-cycle;
- adaptadores temporais de CPU/barramento, VDP, PSG/FM, DMA, interrupções e input.

A PR #315 introduz uma fundação T0 de tempo/domínios/fila, mas declara explicitamente que ainda não implementa MachineScheduler, micro-operações, bus cycles, waits, contenção ou integração dos dispositivos. Esse limite deve permanecer explícito até que código e testes demonstrem o contrário.

### 4.3 Separação entre relógios emulados e host

O scheduler controla o tempo emulado. O relógio de áudio, refresh de vídeo, VSync/VRR, timer Qt e chegada física de input não podem avançar diretamente registradores ou ciclos da máquina.

- Input do host é normalizado na fronteira nativa e inserido com política temporal definida.
- Áudio consome dados produzidos pela máquina; worker WASAPI/XAudio2 não avança CPU ou chips.
- Vídeo apresenta frames produzidos no tempo da máquina; apresentação física não define o relógio lógico.
- Qt envia comandos coarse-grained e recebe snapshots/telemetria.

## 5. Migração do runtime e dependências Python

### Python permanece para

- Qt/PySide6: janela, menus, diálogos, configuração e acessibilidade;
- carregamento/seleção de ROMs e perfis;
- configuração de dispositivos e apresentação de diagnósticos;
- logs da aplicação, ferramentas de desenvolvimento, auditorias e automação;
- testes de integração da fronteira PyO3/Maturin e CI.

### Rust deve possuir

- CPU(s), ALU, memória, barramentos, mapeamento e I/O emulado;
- VDP, PSG/FM/PCM e demais dispositivos digitais;
- interrupções, DMA, arbitragem, relógios, scheduler e replay;
- captura/normalização de input físico e backends nativos aprovados;
- renderização/filtros/apresentação no escopo da migração de vídeo;
- mixagem/resampling e saída de áudio nativa.

### Regras de dependência

1. Nenhuma biblioteca Python de hardware pode ser chamada pelo runtime ativo.
2. Nenhum callback Python por instrução, acesso de memória, pixel ou amostra PCM.
3. PyO3 deve expor comandos e snapshots coarse-grained, com ownership/lifetime e sincronização explícitos.
4. Não transportar framebuffer integral ou PCM pelo Python no caminho normal.
5. Dependências Python de UI, empacotamento, ferramentas e testes não são “legado” e não devem ser removidas indiscriminadamente.
6. Uma dependência só pode ser removida após busca de imports/consumidores, substituição de funcionalidade, testes e verificação do pacote instalado.
7. `legacy/python_reference/` permanece arquivo de referência histórico; não é runtime alternativo nem oráculo permanente.

## 6. Plano de execução por fases

### Z0 — congelar contratos e baseline

- [ ] Registrar SHA de `main`, estado da árvore e manifests/workflows.
- [ ] Comparar CPU SG-1000 e SMS em nível de função/semântica.
- [ ] Inventariar bus traits, traces, snapshots, interrupções e APIs públicas.
- [ ] Fixar vetores de regressão para ambas as plataformas.
- [ ] Preservar evidências e proveniência dos corpora existentes.

### Z1 — extrair o motor canônico sem alterar semântica

- [ ] Criar `native/z80_cpu` / `sega-z80-cpu`.
- [ ] Escolher uma implementação canônica após comparação funcional; não copiar divergências sem revisão.
- [ ] Mover estado, decoder, semântica e ALU para o crate compartilhado.
- [ ] Consolidar o contrato de barramento sem embutir mapa de plataforma.
- [ ] Transformar os núcleos SG-1000/SMS em adaptadores finos.
- [ ] Executar testes anteriores, testes diferenciais e workflows de ambos os cores no mesmo SHA.
- [ ] Remover duplicação apenas depois de comprovar que ambos consomem o mesmo crate.

### Z2 — integrar perfis SMS e Game Gear

- [ ] Separar diferenças de máquina/VDP/áudio/input do motor Z80.
- [ ] Criar perfil de hardware Game Gear somente com evidência de mapa, portas e diferenças de dispositivo.
- [ ] Reutilizar o mesmo motor sem ramificações de plataforma no decoder.
- [ ] Não afirmar compatibilidade Game Gear até testes específicos.

### Z3 — micro-operações e barramento temporal

- [ ] Definir enumeração de micro-operações e eventos de barramento.
- [ ] Converter progressivamente famílias de instruções para execução retomável.
- [ ] Preservar uma única semântica; modo de instrução drena a mesma sequência.
- [ ] Modelar fetch M1, refresh, ciclos internos, I/O, acknowledge, WAIT e interrupções.
- [ ] Integrar com MachineScheduler e comparar traces com vetores/oráculos.

### Z4 — integração Genesis

- [ ] Implementar o core de máquina Mega Drive separadamente do motor Z80.
- [ ] Integrar 68000 e Z80 sob o mesmo scheduler da máquina.
- [ ] Modelar request/grant/ownership do barramento Z80 com evidência técnica.
- [ ] Validar RAM Z80, ROM/áreas mapeadas, comunicação com áudio e contenção/arbitragem.
- [ ] Não tratar a presença do Z80 como prova de que o Genesis está implementado.

### Z5 — consolidação do runtime Rust e limpeza de dependências

- [ ] Completar inventário de dependências/imports Python de produção.
- [ ] Concluir/reconciliar migrações de input e vídeo sem duplicar PRs.
- [ ] Desacoplar scheduler do pacing Qt e da saída de áudio/vídeo.
- [ ] Renomear a fronteira `native/z80_core` somente se isso reduzir ambiguidade e após atualizar todos os consumidores.
- [ ] Remover dependências Python obsoletas somente com prova de ausência de imports e testes do pacote Release.
- [ ] Atualizar README, arquitetura, ADRs, matriz de cobertura e workflows.

## 7. PRs abertas observadas em 2026-10-31

| PR | Tema | Estado observado | Relação com a reorganização |
|---|---|---|---|
| #315 | Rust Temporal Core | Open / Draft; base `main` no SHA `3cf846d...` | Fundação T0 temporal; não é MachineScheduler nem integração de máquinas |
| #314 | Vetor de fronteira Z80 `LD (nn),HL` | Open / Draft; base `main` no SHA `3cf846d...` | Teste sem mudança do runtime; preservar como evidência Z80 |
| #313 | Input Rust + SDL3 | Open / Draft; base `main` no SHA `3cf846d...` | Migração de input mais recente; reconciliar com #300 |
| #301 | Vídeo Rust | Open / Draft; base antiga `2318fb3...` | Branch desatualizada em relação ao baseline observado; rebase/revalidação necessária |
| #300 | Input Rust | Open / Draft; base antiga `2318fb3...` | Proposta anterior relacionada à #313; evitar integração duplicada |
| #298 | Modelo funcional Qt | Open / Draft; base antiga `2318fb3...` | Documentação/interface; revalidar base |
| #297 | Auditoria em lote SMS | Open / Draft; base antiga `41c3200...` | Ferramenta de diagnóstico; revalidar base |

A consulta de status combinado aos SHAs de PR não retornou status checks nesta captura. Isso é **NOT VERIFIED**, não PASS. Os relatos de validação nos corpos das PRs referem-se a SHAs e execuções específicos e não devem ser transferidos automaticamente para o head atual.

### Issues de controle Z80

- [#158 — matriz de opcodes e semântica](https://github.com/leonardo201800478/sega-emulator/issues/158): auditoria da matriz base/CB/ED/DD/FD/DDCB/FDCB, flags, WZ/Q/R, aliases e lacunas. A cobertura amostral não equivale a completude.
- [#159 — interrupções, estado e temporalidade](https://github.com/leonardo201800478/sega-emulator/issues/159): reset, HALT, EI delay, INT/NMI, IM0/1/2, acknowledge e separação de T-states nominais e bus trace.
- [#122 — controle da migração Rust-first](https://github.com/leonardo201800478/sega-emulator/issues/122): referência histórica para arquitetura limpa e fronteira PyO3.
- [#115 — início do SMS](https://github.com/leonardo201800478/sega-emulator/issues/115): escopo original SMS, agora ampliado por esta decisão arquitetural para Game Gear e futuro Genesis.

## 8. Matriz de estados e validação

Usar os estados do projeto separadamente:

- `IMPLEMENTED`: existe implementação no código.
- `QUICK_PASS`: validação rápida direcionada aprovada.
- `SHORT_PASS`: bateria curta definida aprovada.
- `LONG_PASS`: validação longa/estresse aprovada.
- `INTEGRATION_PENDING`: falta integração entre componentes/máquinas.
- `TEMPORAL_PASS`: evidência temporal suficiente para o escopo declarado; não atribuir com base em ciclos agregados.

Para cada SHA candidato, classificar checks como PASS, FAIL, BLOCKED ou NOT RUN. Checks ausentes, filtrados, cancelados ou sem resultado são NOT RUN/NOT VERIFIED, nunca aprovação.

### Critérios de aceitação do motor Z80 compartilhado

- [ ] Um único crate de CPU utilizado pelos quatro adaptadores planejados.
- [ ] Nenhum decoder/ALU de produção duplicado nos cores de máquina.
- [ ] Vetores semânticos SG-1000 e SMS continuam aprovados após extração.
- [ ] Testes específicos Game Gear e Genesis validam seus adaptadores.
- [ ] Barramento registra tipos e ordem de acesso corretos.
- [ ] Reexecução idêntica produz snapshots/traces idênticos.
- [ ] Clippy, fmt, testes do workspace e testes de cada integração passam no SHA exato.
- [ ] Build Release/Maturin importa a extensão recém-construída.
- [ ] Testes Python não importam `legacy/python_reference` como runtime.
- [ ] TEMPORAL_PASS só é declarado quando a sequência temporal de acessos/eventos for efetivamente validada.

## 9. Regras de preservação e publicação

- Esta reorganização começa pela arquitetura e documentação; não autoriza alterações comportamentais em `main`.
- A publicação documental em `main` não equivale a aprovação ou merge das PRs abertas.
- Mudanças de código do Z80, scheduler, barramento ou dispositivos devem ocorrer em branch/PR, com baseline e critérios de aceitação.
- Não fechar issues #158/#159 apenas por extrair o crate; suas matrizes e evidências continuam válidas.
- Não apagar `legacy/python_reference` até inventário de consumidores, proveniência e evidências serem preservados.
- Não incluir ROMs comerciais, BIOS protegidas ou logs/snapshots com conteúdo de ROM no Git.
- Não presumir clocks, prioridades, ciclos ou diferenças de silício sem fonte técnica.

## 10. Próximo passo

Executar Z0: comparar as duas implementações atuais do Z80, os contratos de barramento e os testes já existentes; definir a implementação canônica e a API pública do novo crate. Em paralelo, avaliar a PR #315 no seu SHA atual. Não começar a extração nem alterar os runtimes até que o inventário e os vetores de aceitação estejam registrados.
