# SMSmemtest — auditoria de oráculo e capacidade

**Data:** 2026-11-13
**Suite:** SMSmemtest
**Upstream:** https://github.com/sverx/SMSmemtest
**Revisão fixada no manifesto:** `12771ed247b93d3a83d0c3cbe7d2d39e3ccb47d9`
**Fonte auditada:** `src/main.c` (versão declarada 0.40)
**Escopo:** determinar o que precisa ser observado para transformar execução em resultados por caso. Nenhuma ROM foi executada nesta auditoria.

## 1. Fluxo efetivo da ROM

A execução não é um teste autônomo que termina após um número fixo de frames. O fluxo em `main()` é:

1. `quick_dead_ram_test()`: verifica uma pequena área de RAM; em caso de falha, configura fundo vermelho e entra em loop infinito.
2. `SMS_init()` e inicialização do display.
3. `wait_start()`: espera um botão/direção do controle.
4. Testes de RAM interna:
   - `ram_test(0x00)`
   - `ram_test(0xFF)`
   - `ram_test(0x55)`
   - `ram_test(0xAA)`
   - `ram_test_2(1)`
   - `ram_test_2(19)`
   - `ram_test_2(71)`
5. Exibe `RAM tests completed` e espera outro botão para iniciar VIDEO RAM tests.
6. Desliga o display e executa os testes de VRAM:
   - padrões `0x00`, `0xFF`, `0x55`, `0xAA`;
   - padrão incremental `vram_test_2()`.
7. Em caso de falha, reativa display e mostra a linha específica com `FAILED!` em vermelho. Em caso de sucesso, mostra `-all tests ... OK!` em verde, exibe `VIDEO RAM tests completed` e espera outro botão.
8. Executa testes de cart SRAM para os bancos 0 e 1, com cinco padrões por banco, e termina em loop infinito.

A função `print_test_result()` é um oráculo visual: escreve `OK!` em verde ou `FAILED!` em vermelho. O resultado não é escrito em uma estrutura de memória padronizada para consumo externo.

## 2. O que o runner atual ainda não faz

O runner atual chama `step_frame()` repetidamente, drena PCM e emite contadores JSONL. Ele não chama `set_controller()`, não observa o framebuffer e não decodifica o texto/cores apresentados pela ROM.

Esse mecanismo é intencionalmente um heurístico de presença de linha, não OCR: não valida que o texto esperado esteja presente nem que `OK!`/`FAILED!` tenha sido produzido. A ROM pode continuar parada se o marcador não for detectado; o runner então retorna `INPUT_DRIVER_TIMEOUT_NO_ORACLE`. Quando detecta atividade nas três linhas esperadas, retorna `PROMPT_ROW_HEURISTIC_COMPLETED_NO_CASE_ORACLE`. O driver não aciona a última espera para SRAM.

A máquina já expõe APIs read-only/seguras para esta finalidade:
- `SmsTemporalMachine::set_controller()` atualiza níveis do controle sem avançar o tempo;
- `SmsTemporalMachine::framebuffer()` expõe o RGB24 do frame concluído;
- `SmsTemporalMachine::vdp()` fornece acesso read-only ao VDP.

Isso permite desenvolver um driver de teste em fronteiras de frame, sem callbacks por instrução ou avanço de tempo fora do Scheduler. A entrada deve ser acionada por estado observado, não por atrasos fixos em número de frames.

## 3. Limitação de cart SRAM confirmada no runtime atual

A implementação `SmsMemory` auditada contém ROM/cartridge image, mapper, registradores de bancos, 8 KiB de work RAM e BIOS opcional. Não há backing store de cartridge SRAM nem seleção de SRAM bank.

O código da ROM chama `SMS_enableSRAMBank(0)` e `SMS_enableSRAMBank(1)`, acessando o espaço `$8000-$BFFF`. No estado atual do runtime, essas operações não podem ser usadas para validar SRAM real; escritas comuns nessa janela não criam memória persistente de cartucho.

Portanto:
- testes de RAM interna podem ser candidatos a validação depois que o driver e o oráculo visual forem implementados;
- testes de VRAM podem ser candidatos, mas exercitam protocolo de portas e timing de VDP e precisam de oráculo e análise de capacidades próprios;
- testes de cart SRAM dos bancos 0 e 1 ficam `BLOCKED_CAPABILITY` até a implementação e validação independente do mapper/SRAM correspondente.

Não modificar o mapper como parte desta tarefa de harness. A ausência de SRAM deve ser tratada como bloqueio explícito, não mascarada como falha da ROM.

## 4. Plano de oráculo incremental

### Fase M1 — Entrada determinística por fase

- Implementado um driver específico que aplica pulsos de `button_1` somente depois de frames completos, mantendo o botão pressionado durante o frame seguinte e liberando-o na próxima fronteira.
- A transição usa atividade de pixels nas linhas de texto documentadas (3, 14 e 12), estável por dois frames; não usa atrasos fixos como substituto de conclusão.
- O detector é heurístico e ainda precisa ser validado com uma ROM SMSmemtest construída a partir da revisão upstream fixada.
- Eventos de pressionar/liberar são registrados em JSONL com índice de frame.
- Não há OCR, leitura de texto tilemap nem classificação de `OK!`/`FAILED!`.

### Fase M2 — Captura read-only

- Capturar o framebuffer RGB24 somente depois de `step_frame()` completar.
- Armazenar hash do framebuffer por frame e, para diagnóstico, imagens fora do Git.
- Decodificar/inspecionar as linhas e regiões de resultado usadas pela ROM.
- Determinar por evidência as cores RGB produzidas pelo renderer; não assumir valores de palette sem testar o mapeamento.

### Fase M3 — Resultados por caso

- RAM: classificar os sete padrões separadamente por `OK!`/ `FAILED!`; detectar o loop de RAM fatal como estado separado.
- VRAM: classificar os cinco padrões separadamente; marcar casos não observáveis ou dependentes de granularidade ausente como `BLOCKED`.
- SRAM: sempre `BLOCKED_CAPABILITY` enquanto a máquina não possuir SRAM de cartucho validada.
- Um resultado global só pode ser `PARTIAL_PASS` se cada caso executável tiver resultado individual e todos os bloqueios estiverem explicitados; nunca converter isso em PASS da suite completa.

### Fase M4 — Validação do próprio oráculo

- Fixtures sintéticas para cores/linhas de texto e casos OK/FAILED.
- Comparar as decisões do parser com capturas independentes e documentadas.
- Repetir a ROM com o mesmo hash, perfil regional e sequência de entrada; exigir resultados idênticos.
- Separar erro do parser, limite de execução, falha do modelo e falha funcional da ROM.

## 5. Estado

| Componente | Estado |
|---|---|
| Runner full-machine via Scheduler | Implementado |
| CI de workspace e SMS runtime no HEAD anterior | PASS |
| Snapshot/hash do mesmo input ROM | Implementado no wrapper; CI do novo HEAD pendente |
| Entrada automática por fase | IMPLEMENTADA — heurística de atividade de linha; não validada em ROM real |
| Captura/oráculo visual | NOT IMPLEMENTED |
| Testes RAM | BLOCKED — oráculo por caso ausente; driver heurístico ainda não validado em ROM real |
| Testes VRAM | BLOCKED — oráculo por caso ausente; driver heurístico ainda não validado em ROM real e capacidade temporal por caso |
| Testes cart SRAM | BLOCKED_CAPABILITY — sem SRAM no mapper |
| Execução real de ROM externa | NOT RUN |

## 6. Referências upstream

- SMSmemtest README e instruções de build: https://github.com/sverx/SMSmemtest
- Fonte fixada: https://github.com/sverx/SMSmemtest/tree/12771ed247b93d3a83d0c3cbe7d2d39e3ccb47d9/src
- devkitSMS: https://github.com/sverx/devkitSMS

A fonte upstream descreve RAM interna de 8 KiB, VRAM de 16 KiB e cart RAM de até 32 KiB. A implementação de teste usa explicitamente dois bancos SRAM, e requer toolchain SDCC/devkitSMS e o `crt0_memtest.rel` modificado. Fixar também as revisões de todas as dependências antes de publicar ROM gerada ou comparar resultados.
