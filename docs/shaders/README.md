# Catálogo de shaders retro

Este diretório contém somente catálogo de pesquisa SH-0. Nenhum shader, preset, LUT ou textura de terceiros é embarcado nesta etapa.

- [Catálogo TOML](catalog.toml)
- [ADR 0014 — filtros nativos Rust/WGSL](../adr/0014-retro-shader-pipeline-librashader-wgpu.md)
- [Arquitetura híbrida nativa](../architecture/native-video-filters-hybrid-pipeline.md)
- [Arquitetura do pipeline](../architecture/video-shader-pipeline.md)

## Estratégia

Filtros nativos devem priorizar Rust na resolução de origem e WGSL embarcado para efeitos espaciais. Catálogo Slang é trilha opcional de compatibilidade futura, não dependência da arquitetura principal.

## Revisão upstream

Catálogo verificado contra libretro/slang-shaders no commit 02bf134555c4595bd6b69be89feaff923da83754 (2026-11-20). Atualizações devem fixar novo commit e verificar novamente todos os caminhos.

A grafia real do preset de mistura de frames é motionblur/mix_frames.slangp. handheld/zfast_lcd.slang é shader source, não preset .slangp completo.

## Licenças

Não presumir licença uniforme para o repositório inteiro. Antes de distribuir preset, auditar o arquivo e todos os shaders/includes/LUTs referenciados. Manter copyright/notices. Não baixar assets automaticamente em runtime.
