# Pause físico do console Sega Master System

Estado: **IMPLEMENTED / CI_PENDING / WINDOWS_MANUAL_PENDING**

## Contrato

O botão Pause do SMS é uma entrada dedicada do console e solicita uma NMI ao Z80. Não é um bit dos controladores e não é o comando de pausa da aplicação. A CPU existente já implementa o latch NMI e o vetor `$0066`; o core SMS agora expõe a solicitação por `SmsMachine::request_console_pause`.

## Integração

- A ação configurável `console_pause` foi adicionada aos perfis existentes de teclado e gamepad.
- Padrões: tecla Pause / botão Back no jogador 1; Scroll Lock / botão Back no jogador 2.
- O frontend agrega os dois perfis e solicita NMI somente na borda de pressionamento.
- Repetição automática de tecla e manutenção do botão não solicitam NMIs adicionais; soltar e pressionar novamente gera nova solicitação.
- A ação é excluída de `JOYSTICK_ACTIONS` e nunca altera os seis bits dos controladores.
- O evento só é encaminhado quando o sistema ativo é Master System. SG-1000 conserva seu comportamento atual, inclusive Hold como NMI.
- O caminho Qt → adaptador Python → PyO3 → core Rust é explícito e não adiciona polling do host por instrução nem callback Python no caminho crítico.

## Distinção dos comandos

- **Pause do console (SMS / NMI):** solicita interrupção não mascarável; o Z80 continua executando e atende a NMI no limite da instrução.
- **Pausar emulação:** congela o avanço dos quadros no host, sem solicitar NMI.

## Validação

- **IMPLEMENTED:** perfil configurável, persistência via JSON existente, UI de captura, borda de pressionamento e API nativa.
- **QUICK_PASS:** pendente de CI no SHA candidato.
- **SHORT_PASS:** pendente de CI no SHA candidato.
- **LONG_PASS:** não executado.
- **INTEGRATION_PENDING:** build Windows/PyO3 e teste com controle físico pendentes.
- **Teste manual Windows:** pendente.
- **SG-1000:** sem alteração na semântica de Hold.
- **TEMPORAL_PASS:** não reivindicado; o core SMS permanece em execução por instruções, sem scheduler ciclo a ciclo.

Nenhuma ROM ou BIOS comercial foi adicionada ao repositório.