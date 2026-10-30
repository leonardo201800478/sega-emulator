# SMS ROM metadata, compatibilidade de hardware e OSD

**Status:** DESIGN / NOT IMPLEMENTED  
**Base auditada:** `main` em `50fbbaf0d3504ee33785c24eba377224c75c82a3`  
**Escopo inicial:** Sega Master System; modelo extensível a Game Gear e Mega Drive  
**Runtime de hardware:** Rust  
**Interface e apresentação:** Python / Qt / PySide6  
**Fronteira:** PyO3, com transferência de metadados imutáveis e sem callbacks por instrução

## 1. Objetivo

Definir um contrato único para identificar uma ROM SMS, descrever suas características conhecidas, resolver o perfil regional/de hardware, informar compatibilidade e apresentar essas informações em um OSD legível.

O subsistema deve responder, sem confundir conceitos diferentes:

- Que sistema e jogo parecem estar carregados?
- Quais territórios/regiões estão associados à release?
- Qual perfil de console e padrão de vídeo estão efetivamente selecionados?
- O jogo utiliza ou requer FM/YM2413?
- Quais periféricos são requeridos, suportados ou não identificados?
- Existe memória de save? Qual tipo, capacidade e persistência?
- Qual mapper é conhecido?
- A BIOS é compatível, necessária ou deve ser ignorada?
- Quais informações são confirmadas, inferidas ou desconhecidas?

O OSD é uma visualização dos metadados e do perfil efetivo. Ele não identifica ROMs, não consulta DATs e não determina hardware por conta própria.

## 2. Auditoria do estado existente

### 2.1 Catálogo

Em `main`, `src/sega_emulator/rom_catalog.py` contém:

- `DatRom`, com sistema, nomes, tamanho, CRC32, MD5, SHA-1, regiões e nome do DAT;
- `CatalogIdentification`, com estado da correspondência, jogo, sistema, regiões, algoritmo de match e candidatos;
- índice carregado sob demanda;
- identificação por SHA-1, depois MD5, depois CRC32 combinado com tamanho;
- preservação de múltiplas regiões/candidatos, sem escolher arbitrariamente uma entrada;
- estado `not_catalogued` quando não há correspondência.

O catálogo atual não descreve mapper, FM, periféricos, save, BIOS requerida ou perfil elétrico do console. Não devemos fingir que os DATs No-Intro fornecem esses dados.

### 2.2 Região e firmware

O código atual `src/sega_emulator/machine/sms_region.py` oferece `auto`, `usa`, `europe` e `japan`. O modo automático usa a informação do cabeçalho para escolher tentativas; para exportação, o código atual ainda usa USA → Europe. O seletor ainda não expõe um perfil Brazil/PAL-M nem variantes coreanas.

O firmware SMS é resolvido em `sms_firmware.py`; a PR #282 está aberta e propõe mapeamento integral da BIOS de 8 KiB, integração DAT e seleção de firmware. A PR #267 documenta a investigação da seleção automática regional. Ambas devem ser reconciliadas antes de alterar a política regional ou BIOS. Não assumir que a descrição de uma PR equivale a código integrado em `main`.

### 2.3 Vídeo Qt

`src/sega_emulator/frontend/qt/video_widget.py` usa atualmente:

- `QQuickView` embutido por `QWidget.createWindowContainer()`;
- cena QML que apresenta o framebuffer;
- fallback raster baseado em `QLabel`;
- filtros e escala inteira existentes.

A camada de OSD deverá respeitar ambos os caminhos e não alterar o backend gráfico apenas para facilitar a sobreposição.

### 2.4 Trabalho relacionado

- PR #282 — BIOS SMS e identificação DAT: https://github.com/leonardo201800478/sega-emulator/pull/282
- PR #267 — seleção automática de região: https://github.com/leonardo201800478/sega-emulator/pull/267
- PR #280 — pesquisa de mappers e roadmap de hardware: https://github.com/leonardo201800478/sega-emulator/pull/280
- PR #287 — implementação da resolução regional determinística e perfil PAL-M, empilhada sobre a PR #282: https://github.com/leonardo201800478/sega-emulator/pull/287

Este documento define a camada integradora. Não substitui nem duplica a pesquisa detalhada de mappers da PR #280.

## 3. Vocabulário obrigatório

Os seguintes conceitos permanecem separados:

| Conceito | Significado | Não confundir com |
|---|---|---|
| Região da release | Território associado ao lançamento/catalogação da ROM | Região elétrica do console |
| Código regional do header | Indicador TMR SEGA, em geral Japan ou Export | USA, Brazil ou Europe individualmente |
| Perfil do console | Variante de hardware selecionada | Nome/território do jogo |
| Padrão de vídeo | NTSC, PAL ou PAL-M | Família da BIOS |
| Família da BIOS | Firmware compatível com um perfil | Mapper do cartucho |
| Mapper | Lógica de seleção de bancos/RAM do cartucho | Região ou periférico |
| Capacidade FM | O console possui FM/YM2413 funcional | O jogo efetivamente usa FM |
| Uso de FM | O jogo foi catalogado/observado escrevendo no OPLL | Compatibilidade FM do console |
| Periférico | Dispositivo de entrada/saída usado pelo jogo | Mapper |
| Save | Memória/armazenamento de progresso | Qualquer RAM presente no mapa |

## 4. Política de identificação da ROM

### 4.1 Ordem das fontes

1. **Cabeçalho TMR SEGA válido:** primeira fonte estrutural para sistema e grupo regional declarado.
2. **RomCatalog/DAT:** identificação por conteúdo, confirmação ou complemento quando o header está ausente/incompleto.
3. **Catálogo técnico SMS curado:** características de hardware por identidade exata da ROM, com origem e confiança.
4. **Configuração manual:** prevalece para o perfil de console/região selecionado pelo usuário.
5. **Fallback determinístico:** usado somente quando não há identificação suficiente.

O header SMS pode ocorrer em offsets `$1FF0`, `$3FF0` ou `$7FF0`, contém a assinatura `TMR SEGA` e um código que distingue, entre outros, Japan e Export. O código Export não separa USA, Brazil e Europe, e o header não identifica mapper.

Se header e DAT divergirem, preservar ambos os resultados e registrar `IDENTITY_CONFLICT`; não sobrescrever silenciosamente uma fonte pela outra. O resolvedor deverá expor a origem da decisão e sua confiança.

### 4.1.1 Resolução automática do perfil de vídeo

A família do header SMS é somente Japan (`$3`) ou Export (`$4`); ela não distingue USA, Europe e Brazil. A seleção automática do perfil de vídeo deve seguir estas regras:

- preferência manual sempre vence;
- header Japan seleciona perfil Japan, registrando conflito se o DAT exato apontar território de exportação;
- header Export é refinado por DAT exato de território único: USA → USA/NTSC; EUR/AUS → Europe/PAL; BRA → Brazil/PAL-M;
- sem header, DAT exato de território único pode selecionar o mesmo perfil;
- releases com múltiplos territórios que implicam padrões diferentes (por exemplo, Brazil + Europe) são ambíguas: usar USA como fallback determinístico, marcar `UNKNOWN/AMBIGUOUS` e permitir seleção manual; não afirmar que USA é a região correta;
- sem header e sem DAT confiável, usar USA como padrão e iniciar diretamente pelo cartucho, sem BIOS;
- Korea/Taiwan não são convertidas automaticamente em Japan ou Export genérico. Até existir um modelo específico, usar fallback conservador, marcar a resolução como conflitante/ambígua e não selecionar BIOS automaticamente.

O fallback não tenta sequencialmente outros perfis quando o primeiro frame falha. Falha de execução não é evidência de região. A decisão deve ocorrer no carregamento da ROM e ser registrada com fonte, regiões DAT, ambiguidade e conflito.

O perfil Brazil é representado como PAL-M, com temporização NTSC-like de 262 linhas e clock nominal de 3.579.545 Hz. O VDP mantém PAL-M distinto de NTSC para configuração e diagnóstico, mas a saída RGB atual não simula a codificação analógica de cor PAL-M/subportadora.

### 4.2 ROM sem identificação no header e no DAT

Regra de compatibilidade aprovada:

- classificar internamente como `UNIDENTIFIED_ROM_HACK_CANDIDATE` (categoria operacional de ROM não identificada; não é prova de que o arquivo seja realmente um hack);
- desabilitar o boot da BIOS de qualquer região;
- iniciar diretamente pelo cartucho;
- usar USA/NTSC como perfil padrão;
- se o usuário tiver selecionado manualmente outro perfil, respeitar essa seleção;
- manter a ausência de identificação explícita nos metadados e no diagnóstico.

A seleção manual altera o perfil de console/vídeo, mas não autoriza escolher uma BIOS para uma ROM sem identidade confiável.

### 4.3 Regra para avisos regionais

O OSD pode declarar incompatibilidade somente quando houver evidência suficiente de regiões/perfis suportados e o perfil selecionado estiver fora desse conjunto.

Estados:

- `COMPATIBLE`: compatibilidade sustentada por metadados confiáveis;
- `INCOMPATIBLE`: incompatibilidade sustentada por metadados confiáveis;
- `UNKNOWN`: dados ausentes, ambíguos, conflitantes ou insuficientes.

Um header que informa apenas Export não permite concluir, sozinho, que USA é compatível e Europe incompatível (ou vice-versa). Nessa situação, exibir a informação conhecida e manter compatibilidade como `UNKNOWN`; não emitir falso aviso vermelho.

Regiões múltiplas devem ser preservadas. Exemplo: uma ROM catalogada como Brazil + Europe deve aceitar ambos os perfis, desde que a fonte identifique essas regiões para aquela imagem. USA deverá gerar aviso apenas quando a matriz de compatibilidade confiável não o incluir.

### 4.4 Território e padrão de vídeo

Perfis mínimos planejados:

| Perfil | Padrão de vídeo | Observação |
|---|---|---|
| Japan | NTSC | Perfil japonês; não equivale a todas as variantes coreanas |
| USA | NTSC | Perfil de exportação NTSC |
| Brazil | PAL-M | Perfil brasileiro; não usar temporização PAL europeu |
| Europe | PAL | Perfil europeu de 50 Hz |
| Korea | Perfil específico por modelo | Não reduzir toda a Coreia a Japan |

PAL-M deve permanecer distinto de PAL europeu. O perfil brasileiro é NTSC-like em linhas/cadência (525 linhas, aproximadamente 60 campos/s), mas tem codificação de cor PAL-M. A temporização efetiva deverá seguir a documentação e o perfil de VDP/console, não apenas o território comercial.

Coreia deve ser modelada por variantes concretas de hardware (por exemplo, Mark III/OACS, Samsung Gam-Boy e Gam-Boy II/Aladdin Boy II), com BIOS, FM, slots e periféricos próprios quando comprovados. `Korea = Japan` não é uma regra arquitetural aceitável.

## 5. Modelo de metadados

### 5.1 Estrutura conceitual

A implementação deverá ter um objeto imutável, versionável e serializável. O exemplo abaixo é conceitual, não uma API já implementada:

```rust
pub struct SmsRomMetadata {
    pub identity: RomIdentity,
    pub release_regions: RegionMetadata,
    pub header: HeaderMetadata,
    pub hardware: CartridgeHardwareMetadata,
    pub audio: AudioMetadata,
    pub peripherals: Vec<PeripheralMetadata>,
    pub save_memory: Option<SaveMemoryMetadata>,
    pub bios: BiosRequirements,
    pub provenance: Vec<MetadataProvenance>,
}
```

O perfil efetivo selecionado deve ser outro objeto:

```rust
pub struct SmsResolvedConfiguration {
    pub selected_console_profile: SmsConsoleProfile,
    pub video_standard: VideoStandard,
    pub bios_policy: BiosBootPolicy,
    pub compatibility: RegionCompatibility,
    pub decision_source: DecisionSource,
}
```

A separação impede que a identidade do jogo seja confundida com o hardware selecionado.

### 5.2 Origem e confiança por campo

Cada atributo relevante deve indicar sua proveniência, não apenas o objeto inteiro.

Valores conceituais de confiança:

- `CONFIRMED`: comprovado por teste técnico, documentação primária ou registro validado;
- `CATALOG_MATCH`: associado a identidade exata no catálogo versionado;
- `HEADER_DERIVED`: derivado de header válido;
- `INFERRED`: inferência documentada, ainda não confirmada;
- `MANUAL`: configuração explícita do usuário;
- `UNKNOWN`: sem evidência suficiente.

A origem deve distinguir, no mínimo, `HEADER`, `NO_INTRO_DAT`, `CURATED_HARDWARE_DB`, `USER_OVERRIDE` e `RUNTIME_OBSERVATION`.

O OSD não pode colorir inferência ou desconhecido como se fosse confirmação.

### 5.3 Identificação e catálogo técnico

Manter duas responsabilidades:

1. **RomCatalog / No-Intro:** identidade, sistema, título, hashes, tamanho e regiões de release.
2. **SmsHardwareCatalog:** registros técnicos curados e versionados, associados a hashes/identidades exatas, contendo mapper, SRAM, FM, periféricos e requisitos especiais.

Não adicionar campos inventados aos DATs No-Intro nem inferir hardware apenas pelo nome do jogo. A base técnica deve incluir fonte, revisão, confiança, notas e histórico de alteração. Quando não houver entrada técnica, os campos permanecem `UNKNOWN`.

O formato de armazenamento (JSON/TOML ou outro) deverá ser escolhido na fatia de implementação após avaliar o padrão de empacotamento e validação já existente. Não manter dois bancos concorrentes com a mesma autoridade.

### 5.4 Áudio FM

Separar explicitamente:

- capacidade de FM do perfil do console (por exemplo, YM2413/OPLL presente ou ausente);
- uso de FM pelo jogo (confirmado, não observado ou desconhecido);
- requisito do jogo (obrigatório, opcional ou nenhum requisito conhecido).

Não declarar que um jogo usa FM apenas porque o console suporta YM2413. Não redirecionar PSG para FM. O OSD poderá exibir, por exemplo, `FM: DISPONÍVEL`, `FM: UTILIZADO PELO JOGO`, `FM: INDISPONÍVEL NESTE PERFIL` ou `FM: NÃO IDENTIFICADO`, cada estado com origem adequada.

### 5.5 Periféricos

Cada periférico deve ter estado independente:

- `REQUIRED`;
- `SUPPORTED`;
- `NOT_REQUIRED`;
- `UNKNOWN`;
- `NOT_APPLICABLE`.

A lista inicial inclui Joypad, Paddle, Sports Pad, Light Phaser, SegaScope 3-D e Reset. Não tratar periféricos como mappers.

Light Phaser exige integração com sensor de luz, trigger/linhas TH/TL, raster do VDP e captura temporal coerente; posição do mouse isolada não equivale ao hardware. SegaScope exige alternância de quadros e sincronização das lentes. O OSD pode indicar necessidade/compatibilidade, mas não pode anunciar suporte funcional antes da implementação.

### 5.6 Save e memória de cartucho

Separar RAM volátil de memória persistente:

- `NONE_CONFIRMED`;
- `VOLATILE_RAM`;
- `BATTERY_BACKED_SRAM`;
- `NONVOLATILE_MEMORY` (quando o tipo específico for confirmado);
- `UNKNOWN`.

Quando conhecido, registrar capacidade em bytes, bancos, janela de mapeamento, persistência e evidência. Não criar save persistente para um jogo sem perfil confirmado.

A persistência em arquivo deverá ficar na fronteira da aplicação (`SaveRamStore`), fora do caminho de instruções. O runtime Rust expõe bytes tipados/estado dirty; filesystem não é acessado pelo loop de CPU. Nome do arquivo deve ser estável por identidade da ROM e não por nome editável.

### 5.7 Mapper e BIOS

O mapper deve ser identificado independentemente da região. Usar o roadmap de mappers existente como base de pesquisa e implementar variantes em fatias independentes.

A família de BIOS é resolvida separadamente do padrão de vídeo. A política de identificação DAT implementada na PR #282 usa correspondências SHA-1/MD5 inequívocas: `JPN` seleciona a família japonesa; `USA`, `EUR`, `BRA` e `AUS` selecionam a imagem compartilhada da família de exportação US/EU. Assim, uma ROM Brazil ou Europe + Brazil pode selecionar a BIOS de exportação, mas isso **não** seleciona Europe/PAL. Brazil continua exigindo perfil PAL-M, com temporização NTSC-like. Regiões coreanas e combinações mistas Japan + Export não escolhem automaticamente uma família.

O cabeçalho TMR SEGA continua sendo a primeira fonte para o grupo Japan/Export; DAT exato complementa a escolha quando o cabeçalho não resolve a família. DAT de release não substitui o perfil de console nem prova o padrão de vídeo.

A política de BIOS deve considerar o perfil real de console e a identidade do cartucho. ROM não identificada conforme §4.2 sempre inicia sem BIOS, mesmo se uma BIOS local existir. Não escolher firmware por nome de arquivo ou por região presumida.

## 6. OSD — contrato visual e funcional

### 6.1 Posição e composição

- Canto inferior esquerdo da **área efetiva da imagem emulada**, não da janela inteira.
- Margem interna segura para não colar o texto à borda.
- Fundo preto semitransparente, com contraste suficiente sobre cenas claras.
- Linhas curtas, alinhadas à esquerda, sem cobrir HUD importante quando houver alternativa de posicionamento futuro.
- OSD resumido ao carregar uma ROM; alertas relevantes permanecem enquanto a condição existir.
- Painel técnico expandido poderá apresentar todos os metadados, origem e confiança.
- O usuário poderá ocultar/exibir o OSD sem alterar o estado do hardware ou do jogo.

### 6.2 Paleta semântica aprovada

| Cor | Valor de referência | Uso |
|---|---|---|
| Verde | `#64FF75` | Estado OK confirmado, recurso disponível e compatibilidade confirmada |
| Vermelho | `#FF5555` | Atenção, falha ou incompatibilidade confirmada |
| Branco | `#FFFFFF` | Texto informativo, valor neutro, desconhecido ou não verificado |

Não introduzir amarelo como quarto estado. Informação desconhecida deve ser branca e explicitamente rotulada; ausência de dado não é falha nem sucesso.

### 6.3 Tipografia pixel art

A fonte deverá ser monoespaçada, pixel art, nítida em escalas inteiras e legível em 256×192 e em janelas maiores. A família **Silkscreen** é candidata inicial por ser uma fonte pixelada e estar publicada sob SIL Open Font License 1.1. Antes de incorporá-la:

- verificar cobertura de caracteres acentuados em português;
- verificar legibilidade em tamanhos pequenos e em escalas 1x/2x/3x;
- incluir o arquivo de licença e avisos exigidos;
- manter fallback monoespaçado se a fonte não carregar;
- desabilitar suavização quando necessário para preservar os pixels, validando o resultado visual.

A escolha definitiva depende de teste visual; não adicionar a fonte ao pacote nesta etapa documental.

### 6.4 Conteúdo resumido

O OSD de carregamento deverá mostrar, quando conhecidos:

1. nome do jogo e sistema;
2. região(ões) de release e origem da identificação;
3. perfil selecionado e padrão de vídeo;
4. PSG/FM e estado de utilização de FM;
5. periféricos requeridos/compatíveis;
6. save, tipo e capacidade;
7. mapper;
8. avisos de compatibilidade.

Exemplo conceitual:

```text
JOGO: EXEMPLO SMS
REGIÃO: BRAZIL / EUROPE
VÍDEO: PAL-M (PERFIL SELECIONADO: BRAZIL)
ÁUDIO: PSG / FM DISPONÍVEL
PERIFÉRICO: JOYPAD
SAVE: SRAM 8 KiB (BATERIA)
MAPPER: SEGA STANDARD
```

As linhas devem ser omitidas ou marcadas como `NÃO IDENTIFICADO` quando não houver evidência. Não preencher campos com valores presumidos.

### 6.5 Aviso regional

Quando o perfil manual selecionado for incompatível com regiões identificadas de forma confiável, mostrar em vermelho:

```text
! REGIÃO NÃO COMPATÍVEL
ROM: EUROPE
SELECIONADO: USA
```

O aviso é informativo e não bloqueia a execução. O usuário continua podendo testar outro perfil. Para `UNKNOWN`, não exibir aviso de incompatibilidade; apresentar a informação em branco.

## 7. Integração e limites de desempenho

- Identificação, hashing, parsing de DAT e resolução de metadados ocorrem no carregamento/troca de ROM, nunca por frame ou por instrução.
- A interface recebe um snapshot imutável de metadados e estado de compatibilidade.
- Mudanças de perfil atualizam o estado do OSD sem exigir recálculo de hashes.
- A decisão efetiva de hardware é aplicada ao construir/resetar a máquina Rust; a UI não altera diretamente registradores ou estado interno do VDP/PSG.
- PyO3 transfere configuração/metadados de forma explícita; sem callbacks Python por instrução e sem cópias grandes repetidas no caminho crítico.
- Não introduzir um segundo runtime Python de hardware.
- O OSD deve atualizar apenas quando os metadados/estado mudarem, não fazer trabalho de identificação no ciclo de apresentação.

### 7.1 Estratégia de renderização

A cena Qt Quick existente poderá receber propriedades/estado do OSD e desenhar a camada sobre o framebuffer. O fallback raster deverá oferecer a mesma composição e semântica, usando uma implementação de pintura compatível com o caminho raster.

Não adicionar um widget transparente sobre o `QQuickView` embutido sem validar empilhamento nativo: a documentação Qt alerta para restrições de stacking em `QWidget.createWindowContainer()`. Também não migrar automaticamente para `QQuickWidget`: ele facilita sobreposições, mas adiciona render pass offscreen e desabilita o threaded render loop. Qualquer mudança de backend exige benchmark e teste de latência/estabilidade antes da decisão.

## 8. Coordenação em três frentes

A modelagem será dividida em três frentes com entregáveis e limites claros. O coordenador integrador consolida decisões e evita alterações sobrepostas.

### Coordenador 1 — Região, header e BIOS

**Responsabilidade:** resolver a política de identidade regional e boot.

Entregáveis:
- auditoria ponta a ponta de `sms_region.py`, `sms_firmware.py`, `RomCatalog`, PyO3 e perfil Rust;
- reconciliação da PR #282 com a investigação #267;
- parsing e precedência de header/DAT, preservando conflitos;
- política de ROM não identificada: sem BIOS, boot direto, USA padrão, seleção manual respeitada;
- perfis USA, Europe, Japan, Brazil/PAL-M e variantes coreanas documentadas;
- tabela de compatibilidade e testes de resolução.

Limites:
- não alterar mapper, OPLL, periféricos ou UI visual;
- não inferir USA/Brazil/Europe a partir de um header Export genérico;
- não declarar BIOS compatível apenas pelo nome do arquivo.

### Coordenador 2 — Catálogo e hardware do cartucho

**Responsabilidade:** definir a estrutura dos metadados técnicos e suas fontes.

Entregáveis:
- contrato `SmsRomMetadata` e proveniência/confiança por campo;
- separação RomCatalog/No-Intro e SmsHardwareCatalog curado;
- modelo de mapper, SRAM/save, FM e periféricos;
- regras para valores desconhecidos e conflitos;
- fixtures sintéticas e matriz de regressão sem ROM/BIOS comercial.

Limites:
- não implementar runtime de hardware nesta fase de modelagem;
- não inferir recursos por região ou nome de jogo;
- não copiar código de emuladores externos sem auditoria de licença/proveniência.

### Coordenador 3 — OSD e integração Qt

**Responsabilidade:** definir apresentação, atualização e comportamento do OSD.

Entregáveis:
- wire contract do snapshot enviado à UI;
- layout inferior esquerdo e área segura da imagem;
- paleta verde/vermelho/branco e tipografia pixel art;
- renderização equivalente em Qt Quick e raster fallback;
- testes de escala, DPI, resize, fullscreen, fallback e legibilidade;
- preferências de exibição/ocultação e painel técnico.

Limites:
- não consultar DAT/hash no render loop;
- não modificar hardware Rust;
- não trocar QQuickView/QQuickWidget sem evidência de desempenho.

### Coordenador integrador

- mantém o contrato compartilhado e a matriz de dependências;
- resolve conflitos entre as três frentes;
- revisa interfaces antes de qualquer implementação;
- consolida evidências e registra decisões;
- mantém no máximo três agentes IA simultâneos; workflows CI não são agentes.

## 9. Plano de implementação incremental

1. **Consolidar a região/BIOS:** auditar e reconciliar #282 e #267; cobrir BRA+EUR e o fallback de ROM desconhecida sem ampliar o escopo.
2. **Fechar o contrato de metadados:** tipos, proveniência, confiança e estados desconhecidos; manter o DAT No-Intro sem campos especulativos.
3. **Criar o catálogo técnico curado:** começar com poucos registros de alta confiança e testes sintéticos.
4. **Resolver o perfil de hardware:** separar território, padrão de vídeo, família BIOS, FM, slots e periféricos.
5. **Expor snapshot pelo limite nativo:** transferência única por carregamento/troca de perfil.
6. **Implementar OSD:** primeiro layout resumido e aviso regional, depois painel técnico.
7. **Integrar save/mapper/FM/periféricos:** somente quando os respectivos componentes de runtime forem implementados e validados.
8. **Expandir para outros sistemas:** reutilizar conceitos comuns sem impor o modelo SMS a hardware diferente.

Cada etapa deverá ter branch/PR própria quando não trivial, critérios de aceitação antes da implementação e validação no SHA candidato.

## 10. Critérios de aceitação

### Identidade e região
- [ ] Header válido é reconhecido nos offsets suportados.
- [ ] Header Export não é confundido com USA, Brazil ou Europe.
- [ ] DAT preserva todas as regiões de release encontradas.
- [ ] Header/DAT conflitantes produzem estado de conflito e diagnóstico.
- [ ] Sem header e sem DAT: classificação operacional não identificada, boot direto sem BIOS, USA padrão.
- [ ] Seleção manual diferente de USA é respeitada para ROM não identificada, mantendo BIOS desabilitada.
- [ ] Brazil/PAL-M não é confundido com Europe/PAL.
- [ ] Perfis coreanos não são reduzidos genericamente ao Japão.
- [ ] Compatibilidade regional desconhecida não gera aviso vermelho.
- [ ] Incompatibilidade comprovada gera aviso vermelho sem bloquear o jogo.

### Metadados técnicos
- [ ] FM de console, uso de FM pelo jogo e requisito FM são campos distintos.
- [ ] Periférico requerido/suportado/não requerido/desconhecido é representado sem inferência.
- [ ] Save volátil e persistente são diferenciados.
- [ ] Capacidade/bancos de SRAM só aparecem quando catalogados.
- [ ] Mapper é independente da região.
- [ ] Cada campo relevante possui origem e confiança.

### OSD
- [ ] Canto inferior esquerdo da imagem efetiva em todos os tamanhos/aspect ratios.
- [ ] Verde = OK confirmado; vermelho = atenção confirmada; branco = informação/unknown.
- [ ] Fonte pixel art legível em resolução base e escala inteira.
- [ ] Texto não sofre blur em escalas inteiras.
- [ ] Qt Quick e raster fallback apresentam os mesmos dados e cores.
- [ ] OSD pode ser ocultado sem afetar o jogo.
- [ ] Alterar perfil atualiza o aviso sem reidentificar a ROM.
- [ ] Não há hashing, parsing de DAT ou filesystem no render loop.

### Validação
- [ ] Testes unitários de parsing/resolução/compatibilidade.
- [ ] Testes de contrato Python/PyO3/Rust.
- [ ] Testes visuais automatizados para geometria, cores e conteúdo.
- [ ] Testes em Windows para Vulkan/D3D/raster conforme os backends suportados.
- [ ] Benchmarks antes/depois para confirmar ausência de regressão relevante na apresentação.
- [ ] Teste manual com ROMs legais/sintéticas e evidência visual no emulador.
- [ ] Nenhuma ROM comercial, BIOS protegida ou dump é versionado.

## 11. Estados e limites de conclusão

Este documento é **DESIGN / NOT IMPLEMENTED**. A auditoria do repositório confirma a existência do catálogo de identidade, seletor regional limitado e superfície de vídeo Qt Quick/raster. Não confirma a implementação de metadados técnicos completos nem do OSD.

Os estados futuros devem ser registrados separadamente:
- `IMPLEMENTED`: código presente;
- `QUICK_PASS`: verificações rápidas direcionadas;
- `SHORT_PASS`: regressões funcionais curtas;
- `LONG_PASS`: regressões/estresse abrangentes;
- `INTEGRATION_PENDING`: dependência de integração ou teste na máquina do usuário;
- `TEMPORAL_PASS`: evidência real de precisão temporal, não atribuída a este trabalho documental.

Nenhum estado de implementação ou validação é concedido por este documento.

## 12. Referências

### Repositório
- [Catálogo de ROMs](rom-catalog.md)
- [Carregamento de BIOS SMS](sms-bios-firmware-loading.md)
- [Temporização regional e sincronização A/V](regional-timing-and-av-sync.md)
- [Roadmap de mappers SMS — PR #280](https://github.com/leonardo201800478/sega-emulator/pull/280)

### Fontes técnicas e de implementação
- SMS Power — [ROM Header](https://www.smspower.org/Development/ROMHeader)
- SMS Power — [BIOSes](https://www.smspower.org/Development/BIOSes)
- SMS Power — [Region Detection](https://www.smspower.org/Development/RegionDetection)
- SMS Power — [Mappers](https://www.smspower.org/Development/Mappers)
- Qt for Python — [QQuickWidget and performance/stacking considerations](https://doc.qt.io/qtforpython-6/PySide6/QtQuickWidgets/QQuickWidget.html)
- Qt — [QQuickWidget versus QQuickView comparison](https://doc.qt.io/qt-6/qtquick-quickwidgets-qquickwidgetversuswindow-opengl-example.html)
- Google Fonts — [Silkscreen and SIL Open Font License 1.1](https://github.com/googlefonts/silkscreen)

As fontes externas devem ser revalidadas na fase de implementação; documentação de emuladores terceiros é referência comportamental, não autorização para copiar código.
