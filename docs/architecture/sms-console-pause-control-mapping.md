# Mapeamento do botão físico Pause do Sega Master System

Data: 2026-10-29  
Estado: **INVESTIGATION_PENDING**  
Runtime alterado nesta fatia: **não**  
ROMs comerciais: **não incluídas**

## Objetivo

Adicionar uma ação configurável de controle para o botão **Pause do console Sega Master System**, permitindo acioná-lo por teclado e/ou gamepad a partir da tela de mapeamento de controles.

## Semântica de hardware

O botão Pause do Master System não é um terceiro botão de um dos controladores. Ele é um botão físico do console que solicita uma interrupção não mascarável (**NMI**) ao Z80. Portanto, a ação deve ser modelada separadamente dos botões direcional, Button 1, Button 2 e do comando de pausa do próprio emulador (que congela a execução no host).

A ação de controle deve disparar o evento de NMI de acordo com a semântica do hardware. Não deve simplesmente pausar a thread de emulação nem ser convertida em um bit de joystick.

## Estado atual observado

- O snapshot `SmsInputState` contém os dois controladores e `reset_pressed`, mas não contém um evento/estado de Pause do console.
- O adapter SMS Python encaminha operações para `NativeSMSMachine`; o adapter SMS Rust constrói `SmsMachine`.
- O adapter SG-1000 já expõe `request_nmi()`, mas isso não comprova que a mesma API ou semântica já esteja disponível no core SMS.
- A interface de controles utiliza perfis genéricos em `src/sega_emulator/frontend/controller_profiles.py`. A nova ação precisa ser acrescentada sem confundi-la com `pause/resume` da aplicação.

Esses pontos foram observados em código; a integração funcional ainda precisa ser implementada e validada.

## Proposta de comportamento

1. Adicionar uma ação distinta, por exemplo `console_pause` ou `sms_pause`, ao conjunto de ações configuráveis.
2. Permitir binding por teclado e gamepad usando o sistema existente de perfis, persistência e interface de configuração.
3. Quando a ação for acionada no SMS, encaminhar um evento de Pause para o adapter Python/PyO3 e ao core Rust.
4. Gerar o pedido de NMI conforme a semântica Z80/hardware validada. Preferir evento por borda de pressionamento para evitar emitir NMI repetidamente enquanto a tecla permanece segurada; verificar o comportamento elétrico/documentado antes de fechar esse contrato.
5. Soltar o botão não deve gerar uma segunda NMI nem interferir nos bindings de joystick.
6. No SG-1000, não expor a ação como função de hardware suportada sem uma semântica específica desse sistema.
7. Distinguir visual e funcionalmente:
   - **Pause do console SMS:** solicita NMI ao Z80;
   - **Pausar emulação:** interrompe a execução do emulador no host.

## Plano de implementação

- Auditar `ACTIONS`, `JOYSTICK_ACTIONS`, `SUPPORTED_BUTTONS`, perfis de jogador, persistência e captura de input na interface Qt.
- Auditar o estado de NMI do Z80 SMS e o caminho de execução/interruptos no core Rust.
- Definir contrato de evento de Pause com teste de borda (pressionar, manter, soltar e pressionar novamente).
- Integrar a ação na UI e no adaptador nativo sem polling do host por instrução e sem callbacks Python por instrução.
- Adicionar testes de unidade para normalização/persistência do binding, evento NMI e isolamento dos controladores.
- Validar em ROMs de teste/homebrew que utilizem o Pause; não versionar ROM comercial.
- Executar validações Rust e Python e testar manualmente no Windows.

## Critérios de aceitação

- [ ] A ação aparece no mapeamento de controles com rótulo inequívoco “Pause do console (SMS)”.
- [ ] Teclado e gamepad podem ser configurados pela infraestrutura existente.
- [ ] A ação gera a NMI prevista pelo contrato validado, sem congelar a execução no host.
- [ ] Manter o botão pressionado não gera eventos repetidos não previstos; soltar e pressionar novamente funciona.
- [ ] A ação não altera os bits de direção/Button 1/Button 2 nem os bindings de reset.
- [ ] O comando de pausa do emulador permanece separado e sem regressões.
- [ ] Preferências são persistidas e restauradas corretamente.
- [ ] Testes Rust/Python e lint pertinentes passam no SHA candidato.
- [ ] Boot/gameplay manual é verificado no Windows com um teste apropriado.
- [ ] SG-1000 permanece inalterado.

## Estado de validação

- **IMPLEMENTED:** documentação de contrato e plano nesta PR.
- **QUICK_PASS:** NOT RUN.
- **SHORT_PASS:** NOT RUN.
- **LONG_PASS:** NOT RUN.
- **INTEGRATION_PENDING:** SIM.
- **Teste manual:** PENDING.
- **CI:** verificar checks no SHA candidato; ausência de checks não é aprovação.
- **TEMPORAL_PASS:** NOT ACHIEVED; esta PR não reivindica precisão ciclo a ciclo.

## Arquivos esperados na implementação funcional

- `src/sega_emulator/frontend/controller_profiles.py`
- Captura/persistência de controles em `src/sega_emulator/frontend/qt_frontend.py` e componentes de configuração associados.
- `src/sega_emulator/machine/sms.py`
- `native/z80_core/src/sms_machine.rs`
- `native/sms_core/src/input.rs` e/ou `native/sms_core/src/machine.rs`, conforme contrato de NMI definido pelo core.

Esta primeira fatia registra a semântica e os critérios; não afirma que o binding funcional já foi implementado.
