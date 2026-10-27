> **Estado vigente (2026-10-27):** hardware SG-1000 ativo exclusivamente em Rust (`native/sg1000_core`). A aplicação Qt usa uma fachada Python fina para PyO3 e carregadores de ROM/perfil. Implementações de hardware Python, ferramentas e testes que dependiam delas foram arquivados sob `legacy/` e não são executados pelo projeto ativo. Descrições de estados anteriores neste documento são históricas, não instruções atuais.

# ADR: Reconstrução do frontend Qt

- **Status:** proposta em implementação
- **Branch:** `feat/qt-frontend-rebuild`
- **Base:** `main`

## Contexto

O frontend Qt acumulou regressões e contratos divergentes entre a CLI, a máquina SG-1000 e as opções experimentais de núcleo. Um erro observado foi `run() got an unexpected keyword argument 'cpu_core'`. A interface também precisa preservar as configurações de entrada e diagnóstico existentes, sem deixar a lógica inteira concentrada em uma única função extensa.

## Objetivo

Reconstruir a camada Qt em módulos pequenos, com contratos explícitos, mantendo o núcleo da emulação independente do frontend. O trabalho não deve alterar a temporização ou a semântica da máquina SG-1000.

## Inventário funcional obrigatório

1. Abrir ROM individual e ROM dentro de ZIP; histórico de ROMs recentes.
2. Pausar/continuar, reiniciar e atalhos de teclado.
3. Apresentação do framebuffer e escalas de vídeo sem alterar os clocks lógicos.
4. Perfis de entrada para jogadores 1 e 2.
5. Configuração e captura de teclas, botões, D-pad, hats/eixos quando suportados pelo backend, zona morta, analógico esquerdo e turbo.
6. Enumerar dispositivos, tolerar conexão/desconexão e informar o estado do backend sem bloquear o frontend.
7. Persistir as configurações em `QSettings`, validar dados e recuperar defaults com segurança.
8. Saída de áudio opcional: falha no dispositivo de áudio não deve desativar a entrada.
9. Diagnóstico: logs, estado da máquina e snapshot VDP JSON+PNG.
10. Encerrar timers, dispositivos e áudio corretamente.
11. Opções de núcleo só podem aparecer quando a implementação e o contrato da máquina realmente suportarem a seleção. Nunca mostrar uma opção que não seja aplicada.
12. Menus, diálogos e toolbar devem ser organizados por responsabilidade, com interface responsiva e mensagens de erro úteis.

## Arquitetura alvo

- `qt_frontend.py`: adaptador público fino e compatível com o entry point.
- `frontend/qt/app.py`: ciclo de vida da aplicação e composição da janela.
- `frontend/qt/main_window.py`: menus, toolbar, status e layout.
- `frontend/qt/settings.py`: leitura, validação e persistência de preferências.
- `frontend/qt/input_dialog.py`: configuração de teclado e gamepads, sem duplicar o backend SDL.
- `frontend/qt/video_widget.py`: apresentação do framebuffer e escalas.
- `frontend/qt/diagnostics.py`: snapshot e inspeção da máquina.
- Os backends de entrada existentes continuam independentes da camada Qt.

## Critérios de aceitação

- A chamada pública `run(initial_path=...)` e o entry point concordam exatamente sobre os argumentos.
- Não há seleção de núcleo fictícia: o seletor só é apresentado quando o núcleo pode ser injetado e usado pela máquina.
- Os testes existentes de entrada e máquina continuam passando.
- Novos testes cobrem persistência, defaults, mapeamentos de teclado, ações de menu e encerramento.
- A janela pode ser construída em modo headless/offscreen no CI.
- Ruff e pytest passam no workflow Windows/Linux aplicável.
- Testes manuais no Windows verificam teclado, controle físico, hotplug, ROM, vídeo e áudio.
- Nenhuma alteração ao `main` até revisão e aprovação da PR.

## Riscos e limites

- SDL/PySDL3 e dispositivos físicos precisam de validação real no Windows; CI não substitui testes de hardware.
- A camada Qt não deve assumir precisão de áudio/vídeo que o núcleo ainda não fornece.
- A migração deve preservar os contratos de diagnóstico utilizados por testes e ferramentas.
