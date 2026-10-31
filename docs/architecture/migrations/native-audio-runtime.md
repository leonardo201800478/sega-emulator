## Objetivo
Migrar integralmente o caminho de áudio do emulador para Rust nativo, mantendo Python/PySide6 apenas como frontend de configuração e controle.

## Escopo
- Inventariar caminhos ativos de áudio, facades PyO3, crates Rust, APIs do Windows e dependências Python.
- Transferir para Rust produção/consumo de PCM, resampling, mixagem, volume/balance, filas, sincronização A/V, abertura/recuperação de dispositivos, hotplug e seleção/fallback de backend.
- Reutilizar WASAPI/XAudio2 e componentes existentes quando adequados, sem duplicar implementações.
- Reduzir PyO3 a comandos de ciclo de vida/configuração e snapshots/telemetria; proibir PCM por chamada Python no caminho quente.
- Remover dependências e caminhos Python de áudio após equivalência funcional.

## Critérios de aceitação
- Áudio gerado e entregue sem drenagem, conversão, cópia ou despacho de PCM por Python.
- Ownership, threads, COM apartment, buffers e encerramento definidos em Rust.
- Device loss, underruns, overflows, hotplug, fallback e desligamento testados.
- Testes determinísticos de mixer/resampler/filas e testes Windows com backend real.
- Medir latência ponta a ponta, underruns e estabilidade contra baseline equivalente.
- Python altera preferências sem participar do runtime de áudio.

## Plano
1. Inventário e contrato de sessão/dispositivo.
2. Consolidar pipeline em Rust reutilizando audio_runtime/audio_output.
3. Migrar orquestração frame/áudio para runtime Rust.
4. Reduzir PyO3 e remover caminho Python ativo.
5. Validar Rust/Python e integração Windows.

## Estado
Draft de migração. Este commit estabelece escopo e aceitação; não declara o port implementado nem validado.