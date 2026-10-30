# Mapeamento do botão PAUSE do Sega Master System

Estado: **IMPLEMENTED / CI_PENDING / WINDOWS_MANUAL_PENDING**

## Contrato

O PAUSE físico do Master System é uma entrada dedicada do console que solicita uma NMI ao Z80. Não é um bit dos controladores e não é o comando de pausa da aplicação. O core Rust já implementa a linha por nível através de `SmsMachine::set_pause_button(bool)`: somente a transição liberado → pressionado solicita uma NMI; manter pressionado não repete a solicitação, e liberar permite uma nova borda.

## Mapeamento de entrada

- A ação configurável `console_pause` existe separadamente de `hold` nos perfis de teclado e gamepad.
- Padrões: tecla **Pause** / botão **Back (Select)** no Jogador 1; **Scroll Lock** / **Back (Select)** no Jogador 2.
- Para compatibilidade com perfis e hábitos existentes, a ação antiga `hold` também continua acionando PAUSE no SMS (P/H no teclado e Start no gamepad por padrão).
- No SMS, as duas ações são combinadas em um único nível lógico. Se uma delas permanecer pressionada, não ocorre uma segunda borda; a NMI só pode ser solicitada novamente depois que ambas forem liberadas e uma delas for pressionada.
- No SG-1000, somente `hold` mantém o comportamento NMI existente. `console_pause` é ignorada.
- Ambas as ações ficam fora de `JOYSTICK_ACTIONS` e não alteram os seis bits dos controladores.
- O comando host `pause_resume` continua independente.

## Arquitetura

O frontend Qt agrega os estados de teclado e gamepad e encaminha o nível ao adaptador Python `SMSMachine.set_pause_button()`, que valida o tipo booleano e chama o binding PyO3 existente. O core Rust detecta a borda e chama `Z80::request_nmi()`. Não foi criada uma segunda API de solicitação instantânea nem lógica de borda duplicada no Python.

## Validação

- **IMPLEMENTED:** binding dedicado, defaults, persistência pelo JSON existente, migração por merge de defaults e compatibilidade com Hold.
- **QUICK_PASS:** pendente de CI no SHA candidato.
- **SHORT_PASS:** pendente de CI no SHA candidato.
- **LONG_PASS:** não executado.
- **INTEGRATION_PENDING:** build Release/Maturin e teste manual no Windows com teclado e gamepad pendentes.
- **SG-1000:** sem mudança na semântica de Hold.
- **TEMPORAL_PASS:** não reivindicado; o SMS continua sem scheduler ciclo a ciclo.

Nenhuma ROM ou BIOS comercial foi adicionada ao repositório.
