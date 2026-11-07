# SMS — plano executável de integridade de boot e compatibilidade de cartuchos

**Data:** 2026-11-07  
**Baseline histórica examinada:** `main` em `3ca0f648c841bfe7c85a301fb561273e5c62f676` (não representa a `main` atual).  
**Branch histórica de origem:** `audit/sms-cartridge-boot-integrity-main-20261107`; esta revisão foi transplantada para uma branch baseada na `main` atual apenas para atualização documental.  
**Estado:** auditoria de código e especificação; nenhuma alteração de runtime nesta etapa.  
**Prioridade:** P0 — bloqueador de confiabilidade do boot e dos testes in-game.

## 1. Decisão de projeto

A inicialização correta de uma ROM SMS depende da cadeia completa:

1. preservar e validar o dump;
2. identificar a plataforma/variante e o fingerprint;
3. selecionar explicitamente o perfil elétrico do cartucho;
4. inicializar registradores e RAM conforme o hardware;
5. decodificar independentemente leituras e escritas;
6. entregar bytes corretos ao Z80;
7. executar reset, inicialização, menu e gameplay.

Uma ROM que apenas abre ou apresenta uma tela não comprova compatibilidade. `MAPPER_IDENTIFIED`, `BOOT_CONFIRMED`, `GAMEPLAY_SMOKE` e `VALIDATED` são estados distintos.

**Regra fail-closed:** uma identidade de cartucho não resolvida deve permanecer `UNKNOWN`/ `AMBIGUOUS`, com diagnóstico explícito. Não selecionar silenciosamente Sega mapper como se a identificação estivesse comprovada. Um perfil de compatibilidade provisório poderá ser escolhido explicitamente pelo usuário, mas deve ser registrado como override e não promovido ao catálogo.

## 2. Estado do código encontrado na base

Arquivos examinados:

- `native/sms_core/src/memory.rs`
- `native/sms_core/src/cartridge/image.rs`
- `docs/audits/sms-cartridge-mapper-compatibility.md`
- `docs/hardware/sms-special-mappers-research.md`

### 2.1 Implementação atual observada

Em `memory.rs`, `MapperKind` contém somente:

- `Sega`
- `SegaExtended`
- `Codemasters`

Não há implementações ativas para MSX 8 KiB/Nemesis, Korean A000, Janggun, Korea 16K V1/V2, Hi-Com, multicarts coreanos, 4-Pak ou EEPROM.

A seleção atual `detect_mapper()` reconhece Codemasters por uma assinatura/header específico, SegaExtended por tamanho maior que 1 MiB + header TMR SEGA, e encaminha os demais casos a `Sega`. Portanto, o comportamento atual não satisfaz a política UNKNOWN/fail-closed.

O caminho Sega mantém três registradores de banco e work RAM de 8 KiB. A leitura de `$8000–$BFFF` vai diretamente ao banco ROM: não há seleção de SRAM de cartucho nessa janela. O registrador `$FFFC` é armazenado em `mapper_control`, mas seus bits de habilitação/seleção de RAM não controlam uma memória SRAM separada. Também não foi observada persistência de SRAM no código examinado.

A RAM do sistema `$C000–$FFFF` é tratada na mesma estrutura de memória e os writes do mapper Sega são processados na faixa alta. A semântica de bit 4 de `$FFFC`, espelhos e estado inicial deve ser confrontada com a documentação e com o contrato da porta $3E antes de alterar esse caminho.

Codemasters já tem estado inicial de bancos `[0, 1, 0]` e writes decodificados pelas três janelas de 16 KiB. O caminho atual não expõe uma matriz de perfis/fingerprints nem seleção manual tipada. A semântica de mirrors e do caso de RAM especial deve ser testada separadamente.

### 2.2 Conclusão de auditoria

O suporte atual é parcial. Não é tecnicamente correto declarar que o SMS aceita genericamente ROMs por dump sem metadados externos. A presença de `SegaExtended` também não deve ser confundida com suporte universal a qualquer ROM maior que 1 MiB ou a mappers customizados de homebrew.

## 3. Escopo funcional e ordem de implementação

### P0-A — contrato de imagem e identificação

- Preservar os bytes originais e calcular tamanho, CRC32 e SHA-256.
- Detectar e reportar cabeçalho de copiadora de 512 bytes e footer conhecido, sem removê-los silenciosamente; a transformação deve ser explícita e rastreável.
- Ler TMR SEGA e SDSC apenas como metadados, nunca como prova isolada de mapper.
- Validar offsets/formatos de header e variantes SMS/GG.
- Criar registry versionado de fingerprints: identidade preferencial SHA-256; quando a fonte só fornece CRC32, exigir CRC32 + tamanho e marcar nível de confiança/proveniência.
- Preservar variantes, região, revisão, dump e integridade.
- Resultado de detecção: `Known(profile, evidence)`, `Unknown` ou `Ambiguous(candidates)`.
- Não inferir mapper por nome, publisher, região ou tamanho isoladamente.

### P0-B — barramento e Sega mapper

- Separar read decode de write decode.
- Encaminhar writes às faixas do cartucho ao mapper selecionado mesmo quando reads na mesma faixa retornam ROM.
- Completar Sega mapper: slots 0/1/2, primeiro 1 KiB fixo somente nesse perfil, registradores $FFFC–$FFFF, mirrors de RAM, bancos, limites físicos e variantes de chip/reset.
- Implementar SRAM como armazenamento separado da work RAM; cobrir 8 KiB espelhados e capacidades maiores somente com evidência.
- Modelar habilitação/seleção de SRAM conforme bits de $FFFC, incluindo a possibilidade documentada de RAM em $C000–$FFFF; não sobrescrever a RAM de trabalho sem modelar a prioridade elétrica.
- Persistência de save deve ser uma responsabilidade host/media separada da lógica de mapper; salvar/carregar não pode alterar timing do hardware.
- Save state deve preservar registradores, SRAM e estado de persistência pendente.

### P0-C — Codemasters

- Writes em qualquer endereço/mirror dentro das janelas $0000–$3FFF, $4000–$7FFF e $8000–$BFFF.
- Três bancos iniciais 0/1/0.
- Sem proteção fixa de $0000–$03FF.
- Testar mirrors, wrap, bancos fora da faixa e ROM não potência de dois.
- Caso especial Ernie Els Golf/RAM só deve ser implementado no perfil e plataforma confirmados; não generalizar automaticamente a jogos SMS por associação de publisher.

### P0-D — primeiros mappers não Sega

Implementar como perfis independentes e baseados em evidência:

1. MSX 8 KiB;
2. variante Nemesis (janela inicial especial);
3. Korean A000;
4. Janggun.

Não agrupar esses circuitos em um único KoreanMapper genérico.

### P1 — mappers e placas adicionais

- Korea 16K V1/V2 (especificação elétrica ainda incompleta);
- multicart Multi 16K e Multi 4×8K;
- Hi-Com;
- famílias coreanas 18–27, 39–40 e variantes SMS-MD, cada uma com protocolo próprio;
- 4-Pak All Action (registradores ainda precisam confirmação independente);
- EEPROM 93C46 somente para variantes/plataformas em que sua presença seja comprovada;
- mapper de flash/homebrew apenas com especificação verificável.

### P1 — estado de boot e hardware associado

- Estado da RAM no power-on: não presumir RAM zerada universalmente.
- Valor inicial de $C000 e latch da porta $3E: modelar perfil de boot com BIOS versus boot direto; testar dependências de R-Type, Phantasy Star e After Burner.
- Estado inicial de SP: especificar por modelo/BIOS; não inventar garantia para boot sem BIOS.
- FM/YM2413: detecção/seleção independente do mapper, com política por hardware/região/fingerprint.
- Espelhamento de portas VDP $80–$BF para os títulos documentados.
- SegaScope 3-D, Light Phaser, Sports Pad e Paddle são periféricos/decodificações de I/O, não mappers; trilhas separadas.
- Modos VDP 224 linhas e particularidades de sprites/CRAM pertencem à auditoria VDP, não devem ser corrigidos no mapper.

## 4. Perfis e evidências mínimas

A matriz de pesquisa `docs/audits/sms-cartridge-mapper-compatibility.md` e o inventário `docs/hardware/sms-special-mappers-research.md` permanecem as referências existentes. O arquivo recebido em 2026-11-07, `auditoria_sms_hardware_cartuchos.md`, é fonte de candidatos e deve ser reconciliado item a item; nomes de jogos e alegações não verificadas permanecem PENDENTES.

Seed inicial recebido para investigação. **Nenhum valor abaixo está aprovado como fingerprint**: o inventário mistura algoritmos e não contém evidência de rehash dos dumps exatos. Até validação local, a identidade por hash permanece PENDENTE.

| Perfil candidato | Exemplo | Estado de identidade |
|---|---|---|
| Codemasters | Fantastic Dizzy (Europe) | PENDENTE — recalcular CRC32 e SHA-256 do dump exato |
| Codemasters | Micro Machines (Europe) | PENDENTE — recalcular CRC32 e SHA-256 do dump exato |
| MSX 8 KiB | Cyborg Z (Korea) | PENDENTE — confirmar revisão, tamanho e dump |
| Korean A000 | Dallyeora Pigu-Wang | PENDENTE — confirmar revisão, tamanho e dump |
| Janggun | Janggun-ui Adeul | PENDENTE — confirmar dump e placa |
| Multicart | 128 Hap | PENDENTE — identificar família elétrica e dump |
| Multicart desconhecido | 20 em 1 Brasil | UNKNOWN até evidência suficiente |

O registry deve armazenar algoritmo, valor, tamanho, região, revisão, fonte e confiança. Nunca copiar valores de checksum de campos rotulados incorretamente nem promover hashes de inventário a identidade autoritativa.
## 5. Testes obrigatórios

### Unidade por perfil

- padrão de bytes distinto por banco para cada janela;
- estado após power-on/reset;
- leituras nos limites de todas as janelas;
- writes nos endereços canônicos e mirrors;
- write em faixa de leitura ROM;
- primeiro 1 KiB do slot 0, quando aplicável;
- banco inválido, wrap e ROM parcial/não potência de dois;
- RAM/SRAM/EEPROM isoladas da work RAM;
- clone/restore de estado;
- nenhum panic, indexação fora da ROM ou alias inventado.

### Integração de barramento

- write decode ocorre antes de qualquer descarte de write em ROM;
- read decode independente;
- latch $3E habilita/desabilita os slots corretos;
- efeitos de mapper seguem a ordem temporal do barramento;
- snapshot no meio de uma troca de banco preserva o estado.

### In-game

Para cada ROM de teste legal fornecida localmente, registrar SHA-256, região, perfil, evidência, configuração e cenário:

1. reset frio;
2. primeiro fetch e sequência inicial;
3. tela de boot;
4. menu;
5. transição de nível/sub-jogo que exige bankswitch;
6. gameplay smoke;
7. execução prolongada;
8. save/restore e persistência quando aplicável.

Jogos prioritários: Super Monaco GP e Alien 3 permanecem casos de regressão de CPU/runtime; não presumir mapper especial sem fingerprint/trace. Acrescentar Codemasters, MSX/Nemesis, Korean A000, Janggun e multicarts assim que ROMs de teste estiverem disponíveis.

## 6. Critérios de aceite e classificação

- `PASS`: comportamento coberto por teste reproduzível e evidência compatível.
- `FAIL`: divergência reproduzida.
- `BLOCKED`: dependência externa ausente (dump, documentação, hardware).
- `NOT RUN`: teste não executado.
- `SKIPPED` nunca é `PASS`.

Nenhum mapper recebe status VALIDATED somente por existir um enum ou por identificar um hash. Exigir unidade + barramento + boot/gameplay de pelo menos uma ROM representativa, quando o dump for legalmente disponível.

## 7. Sequência de trabalho e gates

1. Reconciliar o código ativo e todos os testes existentes; identificar duplicações com PRs/branches históricas, especialmente PR #424 e #543.
2. Fechar P0-A: modelo de identidade/fingerprint e estados UNKNOWN/AMBIGUOUS.
3. Fechar P0-B: write decode e Sega SRAM/mapper.
4. Fechar P0-C: Codemasters com testes completos.
5. Fechar P0-D: MSX/Nemesis e Korean A000/Janggun.
6. Adicionar catálogo versionado e perfis validados.
7. Executar boot/gameplay smoke e documentar limites.
8. Avançar para P1 somente após P0 aprovado.

Gates para cada PR/HEAD:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- Gates adicionais específicos do app Windows, quando o escopo tocar `native/sega_app`: `cargo check --manifest-path native/sega_app/Cargo.toml --locked`, Clippy e testes equivalentes no target Windows suportado.

Registrar cada gate como PASS, FAIL, BLOCKED ou NOT RUN. Não declarar validação in-game sem execução real. Não integrar com warnings/falhas. Não fazer merge sem autorização explícita.

## 8. Fontes técnicas de partida

- SMS Power — Mappers: https://www.smspower.org/Development/Mappers
- SMS Power — ROM Header: https://www.smspower.org/Development/ROMHeader
- SMS Power — BIOSes: https://www.smspower.org/Development/BIOSes
- SMS Power — Memory Map: https://www.smspower.org/Development/MemoryMap
- ares — SMS media database: https://github.com/ares-emulator/ares
- Genesis Plus GX — SMS cartridge hardware: https://github.com/ekeeke/Genesis-Plus-GX/blob/master/core/cart_hw/sms_cart.c
- MAME — SMS software list: https://github.com/mamedev/mame/blob/master/hash/sms.xml
- Gearsystem — documentação: https://docs.libretro.com/library/gearsystem/

## 9. Limitações atuais desta etapa

- Nenhum dump de jogo foi usado para validar boot nesta etapa.
- Nenhuma placa física foi medida.
- Nenhum mapper novo foi implementado.
- Nenhuma alteração de runtime foi feita.
- Os hashes de seed precisam ser recalculados em dumps locais exatos.
- A especificação detalhada de Korea 16K V1/V2, 4-Pak e algumas variantes de multicart continua pendente.

**Resultado desta etapa:** a lacuna de compatibilidade de cartuchos foi explicitamente registrada como P0, com estado atual do código, riscos concretos, arquitetura-alvo, prioridades, critérios de teste e gates. A implementação deve seguir em incrementos pequenos na branch dedicada, sem alterar `main`.


## 10. Reconciliação obrigatória das propostas SRAM existentes — baseline main 0fb09e2

A inspeção do runtime em `main` confirma que `SmsMemory` ainda não contém SRAM de cartucho: `mapper_control` é armazenado, mas não controla seleção de SRAM; a janela `$8000-$BFFF` continua lendo ROM e não há estado SRAM no tipo.

Foram localizadas duas propostas históricas que impedem uma implementação paralela sem reconciliação:

- **PR #424 — Sega mapper cartridge SRAM support:** proposta mais abrangente. Descreve capacidades de 8/16/32 KiB, seleção de slot 2 por `$FFFC` bit 3 e banco por bit 2, opção de slot 3 por bit 4, arquivo `.srm` versionado com CRC32, APIs Rust/PyO3 e checkpoints de host. A própria PR registra que a capacidade/caminho ainda precisam ser configurados explicitamente e que não houve validação in-game. Não está comprovadamente integrada na baseline auditada.
- **PR #543 — add Sega mapper cartridge SRAM:** proposta mais estreita, com 8 KiB, espelhamento no slot 2, janela alta quando work RAM está desabilitada e API de export/restore Rust/PyO3. Registra ausência de detecção por ROM/capacidade e persistência automática no frontend, além de ausência de teste real de jogo. Também não está comprovadamente integrada na baseline auditada.

**Decisão de arquitetura:** não copiar nem implementar uma terceira versão. Primeiro comparar os diffs/HEADs e estados atuais das PRs, identificar dependências empilhadas, CI e commits exclusivos. Reaproveitar a proposta tecnicamente mais completa somente após rebase limpo sobre o SHA atual e revisão do contrato de hardware. A detecção de SRAM por imagem é uma tarefa separada do mapeamento elétrico; não inferir tamanho/capacidade por nome de jogo ou checksum não verificado.

**Gate MAP-SRAM-RECONCILE:** PRs #424/#543 examinadas no HEAD atual; nenhum código duplicado; capacidade e banking derivados de evidência de cartucho; RAM de cartucho separada de work RAM; snapshot/restore cobre o conteúdo SRAM; persistência em host não ocorre dentro de transação de barramento; testes direcionados e gates completos executados no HEAD final.

## 11. Estado da continuidade em 2026-11-07

- Baseline confirmada por commit: `main = 3ca0f648c841bfe7c85a301fb561273e5c62f676`.
- A branch documental anterior foi criada a partir de baseline obsoleta e não deve receber implementação de runtime.
- A presente branch foi criada diretamente de `main` para corrigir a base documental.
- Nenhum dump de ROM comercial foi obtido ou analisado; nenhum fingerprint foi validado.
- Nenhum teste local foi executado nesta sessão; CI não pode ser inferido a partir de PRs históricas.
- Próximo passo permitido: reconciliar os diffs completos de #424/#543 e a implementação atual do ciclo de vida de mídia/save states. Não iniciar mapper MSX/Korean/Janggun antes dessa auditoria, porque o caminho comum de identidade/SRAM ainda está em aberto.


## 12. Reconciliação efetiva das PRs históricas #424 e #543 (2026-11-07)

A busca por PR retornou os corpos completos das duas propostas e os SHAs de código declarados em suas descrições. Os commits candidatos foram comparados com a baseline atual `main` = `3ca0f648c841bfe7c85a301fb561273e5c62f676`.

### 12.1 PR #424 — candidata funcional mais completa, mas branch obsoleta

- PR: https://github.com/leonardo201800478/sega-emulator/pull/424
- SHA de código declarado como validado: `a69356f0797518fc9036d01555f3b79aa32cf453`.
- Compare com main: `diverged`; candidato possui 50 commits exclusivos e está 849 commits atrás (merge-base `9c9b6dcad052193b81de33845b6bf94bd5f927c0`).
- Arquivos no delta reportado: 13, incluindo `native/sms_core/src/cartridge_ram_file.rs` (437 linhas), `memory.rs`, máquinas convencional/temporal, adaptador PyO3, adaptador Python, frontend Qt, testes e CI.
- Contrato proposto: SRAM explícita de 8/16/32 KiB; bit 3 de `$FFFC` seleciona slot 2; bit 2 seleciona bancos; bit 4 habilita slot 3; capacidade não é inventada para ROM sem configuração.
- Persistência: formato `.srm` próprio, versionado, CRC32, capacidade explícita, detecção de corrupção/incompatibilidade e recuperação via temporário/backup; I/O apenas em checkpoints do host.
- Limitações declaradas: tamanho e caminho ainda precisam ser configurados pelo host; alocação automática por DAT/metadata pendente; SRAM Codemasters não suportada; sem garantia contra encerramento forçado; sem teste in-game; Scheduler deterministic stress foi SKIPPED.
- O commit final `a69356f` é apenas formatação de teste; o código funcional validado indicado na PR é `a69356f`? **Não**: a descrição distingue SHA de código `a69356f0797518fc9036d01555f3b79aa32cf453` como SHA validado, enquanto o commit mostrado tem apenas alteração de formatação. Essa cadeia de evidência precisa ser conferida nos checks históricos antes de reutilizar o código.

### 12.2 PR #543 — fatia mínima, empilhada e igualmente obsoleta

- PR: https://github.com/leonardo201800478/sega-emulator/pull/543
- SHA declarado no texto de validação: `3bb07dc981d22d4242e86bd89090ed212d3e4f71`.
- Compare com main: `diverged`; candidato possui 26 commits exclusivos e está 321 commits atrás (merge-base `1da784f556b9524f5a6f43ce8a7c525e68354f9e`).
- O delta inclui 9 arquivos, mas não é isolado: contém alterações em VDP, Z80 CPU/bus, máquina temporal, PyO3 e adaptador Python. A descrição confirma que foi empilhada sobre #542.
- Contrato: SRAM fixa de 8 KiB; bit 3 de `$FFFC` seleciona slot 2 e espelha 8 KiB em 16 KiB; bit 4 seleciona janela alta quando RAM interna está desabilitada por `$3E`; export/restore via API; host fica responsável pelo arquivo `.sav`.
- Não cobre 16/32 KiB, seleção bit 2, descoberta de capacidade, persistência automática ou SRAM Codemasters.
- O próprio texto declara jogos reais/save-load NOT RUN; Qt smoke, Scheduler stress e PowerShell tooling SKIPPED/NOT RUN.
- O commit `3bb07dc` apenas move testes de módulo; a validação declarada refere-se ao HEAD `3bb07dc...`, mas não demonstra integração sobre a arquitetura atual.

### 12.3 Decisão comparativa

| Critério | #424 | #543 |
|---|---|---|
| Capacidades | 8/16/32 KiB | 8 KiB |
| Banking bit 2 | Sim, proposto | Não |
| Slot alto bit 4 | Sim | Sim |
| Persistência host | `.srm` versionado/CRC/backup | API; arquivo pelo host |
| PyO3/Python | Sim | Sim |
| Integração frontend | Ciclo de vida Qt | Não automática |
| SRAM Codemasters | Não | Não |
| Teste in-game | NOT RUN | NOT RUN |
| Base atual | 849 commits atrás | 321 commits atrás |
| Dependência empilhada | Não identificada no resumo | #542 |

**Escolha:** adotar #424 como referência de requisitos e possível fonte seletiva de algoritmos/contratos; não reutilizar sua branch ou aplicar seu delta de 50 commits. A #543 é útil como referência mais estreita para os bits de mapeamento e casos de teste, mas seu acoplamento a #542 impede cherry-pick isolado sem análise do diff completo.

**Importante:** os deltas do endpoint compare não fornecem patch por arquivo. Os resumos acima vêm das descrições das PRs e da leitura pontual de arquivos nos SHAs históricos. A auditoria integral linha a linha de todos os arquivos e commits continua PENDENTE; não declarar que as implementações foram formalmente equivalidas.

### 12.4 Próxima implementação limpa

1. Manter o contrato de hardware de #424 como alvo inicial, sujeito à conferência de SMS Power e revisão de cada bit.
2. Separar três camadas: (a) RAM emulada no `SmsMemory`, (b) serialização do estado interno/save state, (c) persistência de arquivo pelo host em checkpoint.
3. Integrar primeiro 8 KiB do Sega Mapper e seus bits de habilitação no runtime atual, com estado temporal e snapshot completo.
4. Não habilitar 16/32 KiB nem bit 2 até vetores independentes confirmarem o comportamento de placas suportadas.
5. Adicionar formato de arquivo versionado somente após estabilizar a API de bytes e os erros de corrupção/capacidade.
6. Manter descoberta automática de capacidade fora do mapper: requer registry de mídia separado e fail-closed.
7. Fazer regressão do caminho convencional e temporal, PyO3, SG-1000 e frontend; não reivindicar compatibilidade de jogos sem teste real.

### 12.5 Gate técnico para começar o port

- [ ] Inspecionar integralmente os diffs por arquivo de #424 e #543 (incluindo commits empilhados).
- [ ] Auditar código atual de Save State/serde e ciclo de vida de `SmsTemporalBusState` / `SmsTemporalMachineState`.
- [ ] Confirmar se o estado de SRAM já é capturado automaticamente por clone/serde ou precisa ser adicionado explicitamente.
- [ ] Confirmar semântica de `$FFFC` bits 2/3/4 e prioridade entre SRAM, ROM, work RAM e port `$3E`.
- [ ] Definir compatibilidade dos construtores sem quebrar consumidores existentes.
- [ ] Definir testes de unidade e integração com timestamps/WAIT e snapshot em troca de mapeamento.
- [ ] Só depois abrir branch de runtime baseada no SHA atual da `main`.

**Estado desta reconciliação:** comparação estrutural PASS; decisão de não cherry-pick PASS; auditoria integral dos patches NOT RUN; port de runtime NOT STARTED; teste in-game NOT RUN.


## 13. Revalidação documental sobre a arquitetura atual (2026-11-07)

Esta seção foi adicionada em uma branch criada diretamente de `main` no SHA
`1506f93bfa06905085da9a74b64353ff086b96d9`. O conteúdo das seções anteriores
registra a auditoria histórica feita sobre `3ca0f648c841bfe7c85a301fb561273e5c62f676`;
não deve ser interpretado como uma inspeção executada sobre o HEAD atual.

- Produto atual: Rust-only, com frontend egui/eframe. Python, PyO3, Qt,
  pytest e Ruff não fazem parte dos gates do produto.
- Os gates normativos são `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets --locked -- -D warnings` e
  `cargo test --workspace --locked`, complementados por validações específicas
  do app Windows e de hardware quando aplicáveis.
- As referências a PyO3/Python/Qt nas comparações de #424 e #543 descrevem
  somente a implementação histórica dessas PRs; não são requisitos para o
  port atual.
- A semântica do overlay `$FFFC-$FFFF` na SRAM mapeada em `$C000-$FFFF`
  continua BLOCKED até existir um vetor de teste e uma decisão documentada
  compatível com SMS Power.
- A identificação de cartucho permanece fail-closed: fingerprints sem dump
  exato revalidado não podem ser promovidos a evidência autoritativa.
- Nenhuma implementação de SRAM, mapper ou mudança de runtime foi realizada
  por esta atualização documental.

**Status na baseline atual:** auditoria de código do mapper/SRAM NOT RUN;
revalidação documental do plano PASS; semântica do overlay BLOCKED; port de
runtime NOT STARTED; boot/gameplay NOT RUN.
