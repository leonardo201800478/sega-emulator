# Auditoria do código-base para o Sega Master System

**Data:** 2026-10-27  
**Base auditada:** main em SHA fec7a6d66e80693655d5295cb499ec3c572a02dc  
**PR de arquitetura:** [#116](https://github.com/leonardo201800478/sega-emulator/pull/116), branch feat/rust-master-system-foundation.

## Resumo executivo

O repositório tem uma base útil para SG-1000 no frontend Qt: apresentação Qt Quick/RHI com Vulkan solicitado e fallback raster, saída PCM por WASAPI através de PortAudio/sounddevice, PSG SN76489, sincronização de escritas por T-state e uma suite de testes por subsistema. Ainda não há core SMS independente nem contrato de máquina multissistema.

A decisão central é criar um crate SMS independente. Não implementar o Master System como uma ramificação de SG1000Machine e não importar sega-sg1000-core para reutilizar hardware. Componentes comuns só devem ser extraídos para crates neutros depois de auditoria e testes.

## Estado por subsistema

| Área | Evidência observada na base | Implicação para SMS |
|---|---|---|
| Rust standalone SG-1000 | native/sg1000_core tem CPU, memória, barramento e máquina mínima; machine.rs declara VDP, PSG, I/O e scheduler não integrados | Não é um runtime completo a ser clonado |
| Z80 Rust standalone | cpu.rs descreve implementação incremental; opcodes não suportados retornam erro; prefixos e interrupções não estão implementados nesse recorte | Não selecionar para SMS sem completar ou substituir com candidato validado |
| Z80 PyO3 | native/z80_core tem cobertura mais ampla, incluindo famílias CB/DD/FD/ED, interrupções e estado auxiliar; README diz experimental e sem certificação universal | Candidato a comparação, não biblioteca neutra pronta |
| Dependências Rust | Workspace inclui sg1000_core e z80_alu; z80_core está excluído e depende de sega-sg1000-core | Desacoplar adaptador PyO3 do mapa SG-1000 antes de compartilhar CPU |
| Memória SG-1000 | mapa e validações específicos do console; perfis de RAM de cartucho por hash | Criar memória/cartucho SMS separado, com mapper explícito |
| Loader ROM | rom_loader.py define MAX_ROM_SIZE = 0xC000 (48 KiB) e calcula SHA-256 | Preservar política SG-1000; criar política SMS própria sem truncamento |
| VDP Python | TMS9918A usa 256×192, VRAM 16 KiB, 228 T-states/linha e 262 linhas/quadro no perfil inicial; declara que não modela acessos VRAM por pixel nem contenção | VDP SMS próprio; não reutilizar constantes TMS9918A |
| VDP Rust PyO3 | NativeTms9918A é um VDP específico SG-1000 dentro do runtime experimental | Não usar como VDP SMS |
| PSG Python | SN76489 gera áudio digital mono, com clock/divisor e filtro; módulo não afirma modelar resposta analógica nem temporização sub-instrução | Reutilizar só após validar perfil SMS e sincronização |
| PSG Rust/PyO3 | Existe outra implementação SN76489 e sincronizador em native/z80_core/src/audio.rs | Duplicação Python/Rust requer regressões; não criar terceira implementação |
| Sincronização áudio | AudioSynchronizer agenda eventos em T-state absoluto, preserva ordem em timestamp igual e avança o dispositivo em segmentos | Conceito reaproveitável; generalizar para relógio da máquina e eventos PSG/FM |
| WASAPI | WasapiAudioOutput usa fila PCM limitada; callback consome dados e preenche silêncio, sem executar emulação | Preservar backend como consumidor PCM, não como relógio do hardware |
| Vulkan | Qt Quick/RHI solicita Vulkan; QQuickImageProvider publica cópia thread-safe do framebuffer; fallback raster disponível | Reutilizar apresentação, mas aceitar dimensões dinâmicas em vez de 256×192 fixo |
| Frontend | EmulatorMainWindow tem menus fixos SG-1000 e callbacks; qt_frontend.py orquestra máquina, carregamento, áudio e vídeo | Adicionar SystemId/capacidades no coordenador e adaptar a janela sem lógica de hardware dispersa |
| Preferências | FrontendPreferences guarda configurações globais e uma lista recent_roms, sem sistema/território/FM | Adicionar preferências e ROMs recentes por sistema com migração compatível |
| Testes/CI | Há testes para Z80, SG-1000, PSG, VDP, WASAPI, Vulkan, preferências e integração | Reutilizar regressões e adicionar testes SMS; verificar CI no SHA candidato |

## Arquivos relevantes

### Core e CPU
- native/sg1000_core/src/cpu.rs: CPU Z80 standalone parcial, erros explícitos para opcodes não implementados e T-states nominais por instrução.
- native/sg1000_core/src/bus.rs: CpuBus tem leitura/escrita de memória e defaults de I/O 0xFF/no-op; esses defaults não constituem decodificador SMS.
- native/sg1000_core/src/memory.rs: mapa de memória SG-1000.
- native/sg1000_core/src/machine.rs: conecta CPU e memória, sem VDP/PSG/I/O/scheduler integrado.
- native/z80_core/src/lib.rs: runtime Z80 PyO3 de cobertura mais ampla.
- native/z80_core/src/machine.rs: NativeSG1000Machine conecta CPU, memória, NativeTms9918A, PSG, sincronizador e I/O específico SG-1000.
- native/z80_core/src/memory.rs: envolve Sg1000Memory, confirmando acoplamento ao mapa SG-1000.
- native/z80_core/src/vdp.rs e audio.rs: dispositivos nativos específicos do runtime SG-1000.

### Frontend e dispositivos
- src/sega_emulator/frontend/qt/main_window.py: menus Arquivo, Emulação, Vídeo, Configurações, Diagnóstico e Ajuda; título e ajuda fixos em SG-1000.
- src/sega_emulator/frontend/qt_frontend.py: constrói SG1000Machine, executa run_frame, drena amostras, reamostra e envia PCM ao WASAPI, converte framebuffer em imagem Qt.
- src/sega_emulator/frontend/qt/video_widget.py: QML e Python assumem dimensão-base 256×192; a superfície Vulkan é apresentação, não VDP.
- src/sega_emulator/frontend/qt/wasapi_audio.py: saída PCM limitada, callback não executa hardware.
- src/sega_emulator/frontend/qt/preferences.py: preferências validadas sem perfil de sistema, território, padrão de vídeo ou FM.
- src/sega_emulator/rom_loader.py: limite SG-1000 de 48 KiB; não mudar globalmente para acomodar SMS.
- src/sega_emulator/audio/synchronization.py: sincronização de eventos PSG com timestamps nominais.

## Lacunas críticas

1. CPU compartilhável: nenhuma implementação foi certificada como CPU oficial SMS. A CPU standalone é parcial; a CPU PyO3 é experimental, acoplada a SG-1000 e cruza objetos PyO3.
2. Temporização: execução atual avança VDP/áudio depois de instruções com T-states nominais; ainda não existe scheduler SMS com clocks/eventos regionais explícitos.
3. Vídeo dinâmico: a apresentação assume 256×192 e precisa aceitar dimensões, stride e formato fornecidos pelo VDP.
4. ROM/mapper: o loader rejeita acima de 48 KiB. SMS precisa de imagem de cartucho e mapper próprios; tamanho grande não implica suporte automático a qualquer mapper.
5. Configuração: falta um tipo que separe sistema, território, padrão PAL/NTSC, variante de console, FM e opções de compatibilidade.
6. UI: sistema e opções específicas devem ser guiados por capacidades declaradas, não por condicionais espalhadas.
7. FM: não existe YM2413 no código auditado; será novo componente digital/temporal, separado de WASAPI.
8. Sprites: o modo sem limite deve ser específico do VDP SMS e opt-in; não modificar globalmente o TMS9918A.

## Estratégia

- Criar native/sms_core independente, primeiro com configuração, erros, imagem de cartucho, memória e testes sintéticos.
- Não adicionar sega-sg1000-core como dependência do core SMS.
- Auditar e comparar candidatos Z80 antes de decidir a extração neutra; não bloquear a primeira fatia de memória numa migração geral da CPU.
- Reutilizar apresentação Vulkan e saída WASAPI através das fronteiras framebuffer/PCM existentes.
- Manter hardware e temporização no Rust; Qt/Python fica no frontend, ferramentas e testes diferenciais.
- Regressões SG-1000 são obrigatórias quando contratos compartilhados forem alterados.

## Validação desta auditoria

A inspeção foi realizada por leitura remota dos arquivos na branch main. Nenhum código foi compilado ou executado nesta etapa.

| Check | Estado | Evidência |
|---|---|---|
| Branch/HEAD e PR | PASS | main e PR #116 consultados; SHAs registrados |
| Arquivos relevantes | PASS | Core Rust, PyO3, loader, frontend, áudio, vídeo e documentos de arquitetura inspecionados |
| cargo fmt / Clippy / cargo test | NOT RUN | Não houve checkout local nem execução de comandos |
| pytest / Ruff | NOT RUN | Não houve execução de testes Python |
| Teste manual Windows/Vulkan/WASAPI | NOT RUN | Não executado neste ambiente |
| Compatibilidade funcional SMS | BLOCKED | Core SMS ainda não existe |

## Próximo passo

Criar o esqueleto compilável de native/sms_core com SystemId, SmsMachineConfig, perfis temporais tipados, erros explícitos de tamanho/mapper e memória/cartucho SMS. Fixar regras de banking/reset somente depois de convertê-las em testes apoiados por documentação e vetores independentes. A primeira PR funcional deve conter essa fatia e testes sintéticos, sem alterar a UI nem migrar a CPU ao mesmo tempo.
