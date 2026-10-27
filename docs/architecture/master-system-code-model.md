# Modelo arquitetural do core Sega Master System

**Estado:** arquitetura proposta; não representa código SMS já implementado.  
**Base:** [auditoria do código-base](../audits/master-system-codebase-audit.md) e [roteiro SMS](master-system-roadmap.md).

## 1. Regras de dependência

1. sms_core não depende de sg1000_core.
2. A máquina SMS tem mapa de memória, I/O, VDP, cartucho/mapper e perfis temporais próprios.
3. Componentes compartilhados vivem em crates neutros somente depois de contratos e testes aprovados. A extração não faz parte da primeira fatia.
4. O núcleo não depende de PyO3, Qt, Vulkan ou WASAPI.
5. PyO3 é uma fronteira grossa entre Rust e Python; execução CPU/dispositivos fica em Rust, sem callback Python por instrução/byte.
6. Vulkan apresenta framebuffer finalizado; WASAPI consome PCM finalizado. Nenhum determina clocks ou estado de hardware.
7. Configuração, ROM, entradas e checkpoints iguais devem produzir estado determinístico.

## 2. Diagrama de componentes

~~~mermaid
flowchart TB
    UI[Qt / PySide6] -->|SystemId + preferences| APP[Application coordinator]
    APP -->|load cartridge / run block / snapshots| BIND[PyO3 adapter]
    BIND --> SMS[sms_core]
    SMS --> CPU[Z80 core candidate]
    SMS --> BUS[SMS memory + I/O bus]
    BUS --> CART[Cartridge + mapper]
    SMS --> VDP[SMS VDP / Mode 4]
    SMS --> PSG[SN76489 profile]
    SMS --> FM[YM2413 optional]
    SMS --> TIME[Machine timing + event scheduler]
    VDP --> FB[Framebuffer + dimensions]
    PSG --> MIX[Deterministic audio mixer]
    FM --> MIX
    MIX --> PCM[PCM blocks]
    FB --> VULKAN[Existing Qt Quick / Vulkan presentation]
    PCM --> WASAPI[Existing WASAPI PCM output]
~~~

Este diagrama é o alvo arquitetural. CPU candidata, scheduler e YM2413 precisam de validação antes de serem considerados concluídos.

## 3. Estrutura proposta

~~~text
native/
  z80_alu/                 # atual; avaliar contrato e cobertura
  z80_core/                # hoje adaptador PyO3 + runtime experimental SG-1000
  sg1000_core/             # existente; preservar sem regressão
  sms_core/
    Cargo.toml
    src/
      lib.rs
      config.rs             # SystemId, território, vídeo, capacidades
      machine.rs            # agregador SMS
      error.rs
      cartridge/
        mod.rs
        image.rs            # bytes completos, metadados e hash
        mapper.rs           # contrato e implementações suportadas
      memory.rs             # mapa SMS validado
      bus.rs                # decodificação SMS de memória e portas
      io.rs                 # controles, território e FM control/detect
      vdp/
        mod.rs
        registers.rs
        memory.rs           # VRAM/CRAM
        mode4.rs
        sprites.rs          # limite físico; opção de compatibilidade
        timing.rs
      audio/
        mod.rs
        psg.rs
        ym2413.rs
        mixer.rs
      timing/
        mod.rs
        profile.rs          # clocks, divisores, linhas e eventos
        scheduler.rs
      tests/
        fixtures.rs         # ROMs sintéticas e vetores fixos
~~~

A árvore é proposta, não uma exigência para criar todos os arquivos de uma vez. Começar por lib.rs, config.rs, error.rs, cartridge/image.rs, memory.rs e testes; adicionar módulos quando uma fatia tiver critérios de aceitação.

## 4. Esboço de tipos de domínio

Os tipos abaixo são uma API proposta, não código validado. Não fixam clocks, valores de reset, mapa completo ou comportamento ainda não confirmado.

~~~rust
pub enum SystemId {
    Sg1000,
    MasterSystem,
}

pub enum Territory {
    Japan,
    Export,
    Unknown,
}

pub enum VideoStandard {
    Ntsc,
    Pal,
    OtherDocumented,
}

pub struct SmsMachineConfig {
    pub territory: Territory,
    pub video_standard: VideoStandard,
    pub fm: FmHardware,
    pub sprite_limit: SpriteLimitMode,
}

pub enum FmHardware {
    Absent,
    Ym2413Present,
}

pub enum SpriteLimitMode {
    HardwareLimit,
    UnlockedCompatibility,
}

pub struct CartridgeImage {
    pub bytes: Vec<u8>,
    pub sha256: [u8; 32],
    pub mapper: MapperSelection,
}

pub enum MapperSelection {
    Auto,
    Explicit(MapperKind),
}

pub enum MapperKind {
    SegaStandard,
}

pub trait CartridgeMapper {
    fn read(&mut self, address: u16) -> u8;
    fn write(&mut self, address: u16, value: u8);
    fn reset(&mut self);
}

pub struct FramebufferView<'a> {
    pub width: usize,
    pub height: usize,
    pub stride_bytes: usize,
    pub format: PixelFormat,
    pub pixels: &'a [u8],
}

pub enum PixelFormat {
    Rgb888,
}
~~~

### Regras de configuração

- SystemId seleciona a família de máquina; não deve ser inferido apenas pelo tamanho da ROM.
- Território, padrão de vídeo e presença de FM são campos separados. Unknown deve gerar diagnóstico ou escolha explícita, não palpite silencioso.
- Mapper Auto só é válido quando existe detecção baseada em metadados, regra documentada ou catálogo confiável. Caso contrário, retornar erro/aviso explícito e permitir seleção manual de mapper suportado.
- CartridgeImage preserva todos os bytes. O limite efetivo depende de memória disponível, representação e mapper suportado, não do teto SG-1000 de 48 KiB.
- FramebufferView expõe dimensões, stride e formato para remover a suposição 256×192 da apresentação. Manter cópia QImage enquanto a fronteira existente exigir, até uma mudança justificada.
- O limite padrão de sprites é HardwareLimit. O modo desbloqueado não deve alterar registradores/status sem contrato específico.

## 5. Fronteiras e fluxo de dados

### Inicialização

1. A UI escolhe sistema e configuração de hardware.
2. O loader produz imagem de cartucho imutável, tamanho e SHA-256.
3. SMS valida mapper e constrói memória/cartucho.
4. A máquina inicializa CPU, I/O, VDP, PSG, FM opcional e scheduler a partir da mesma configuração.
5. A UI consulta capacidades e atualiza menus e diagnósticos.

### Execução

1. Scheduler avança CPU e dispositivos conforme modelo temporal documentado.
2. Escritas de I/O geram eventos ordenados no tempo do núcleo.
3. VDP publica quadro com dimensões do modo ativo.
4. PSG e YM2413, quando instalado, geram/misturam amostras no mesmo domínio temporal.
5. PyO3 entrega blocos de framebuffer/PCM e estado de diagnóstico por chamadas grossas.
6. Qt Quick/Vulkan apresenta o quadro; WASAPI consome PCM sem executar hardware.

A API pública pode oferecer run_frame, mas deve derivar de uma primitiva run_until(event/deadline) se a sincronização exigir parar em fronteiras de linha ou eventos. Não usar frame como única unidade interna de tempo.

## 6. Contrato de capacidades do frontend

~~~text
MachineCapabilities
  system_id
  supports_vdp_snapshot
  supports_sprite_limit_override
  supports_fm_audio
  supported_video_profiles
  supported_mapper_kinds
  supported_input_devices
~~~

O menu Sistema seleciona a máquina ativa. Menus comuns permanecem disponíveis; ações exclusivas são habilitadas/ocultadas conforme capacidades. Preferências persistentes devem ser namespaced por sistema e, quando necessário, perfil de território/jogo. A troca de sistema pausa e desmonta a máquina ativa, limpa filas temporais de áudio/vídeo, cria a máquina destino e não inicia automaticamente a ROM antiga.

## 7. Sequência de implementação compilável

1. **SMS-A — contrato e crate:** crate, config/error e testes de configuração; adicionar ao workspace sem alterar runtime SG-1000.
2. **SMS-B — cartucho/memória:** bytes completos, mapper inicial validado, mapa de memória e testes de fronteira/espelhamento/banking.
3. **SMS-C — CPU e portas:** integrar candidato Z80 por barramento SMS; testar instruções, flags, PC/SP, interrupções e I/O.
4. **SMS-D — VDP:** registradores/VRAM/CRAM, Mode 4, framebuffer dinâmico, sprites, status e interrupções.
5. **SMS-E — tempo regional:** perfis PAL/NTSC com valores apoiados por fontes e testes de fronteira/deriva.
6. **SMS-F — áudio:** PSG com perfil SMS, YM2413 opcional, mixagem e integração PCM/WASAPI.
7. **SMS-G — UI:** menu Sistema, capacidades, preferências por máquina e integração Vulkan/WASAPI.
8. **SMS-H — diferencial:** oráculos independentes, ROMs sintéticas/homebrew, determinismo e regressões SG-1000.

Não combinar extração geral de CPU, nova máquina, UI e alteração de fidelidade em uma única PR.

## 8. Critérios de aceitação

- Cada comportamento tem fonte e nível de evidência documentado.
- Testes cobrem limites, reset, erros, repetibilidade e primeira divergência observável.
- PAL/NTSC altera perfil temporal, não velocidade de apresentação do host.
- Dimensões do framebuffer não alteram clocks ou estado do VDP.
- Alterar taxa PCM do host não altera estado final para a mesma duração de hardware.
- Desbloqueio de sprites é opt-in; o padrão respeita hardware.
- FM existe apenas quando a configuração declara o chip e os registros/portas estão suportados.
- ROM acima de 8 MiB não é truncada; só é aceita se mapper e endereçamento suportarem o cartucho.
- Testes SG-1000 continuam passando.
