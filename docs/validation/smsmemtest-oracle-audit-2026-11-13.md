# SMSmemtest — auditoria de oráculo e capacidade

**Data:** 2026-11-13
**Suite:** SMSmemtest
**Upstream:** https://github.com/sverx/SMSmemtest
**Revisão fixada no manifesto:** `12771ed247b93d3a83d0c3cbe7d2d39e3ccb47d9`
**Fonte auditada:** `src/main.c` (versão declarada 0.40)
**Escopo:** determinar o que precisa ser observado para transformar execução em resultados por caso. A ROM SMSmemtest foi construída e executada duas vezes no GitHub Actions run `43391407254`, com registros determinísticos idênticos e gate funcional aprovado. Ambas as execuções alcançaram os resultados de RAM e VRAM.

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

O runner chama `step_frame()` repetidamente, drena PCM e emite contadores JSONL. O driver SMSmemtest lê o name table do VDP por `machine.vdp().vram()` (read-only) e decodifica os prompts e resultados a partir dos índices de tiles SMSlib. Aplica pulsos de `button_1` somente em fronteiras de frame.

A navegação valida os prompts literais pelo tilemap, sem OCR ou leitura de pixels RGB. SMSlib documenta que `SMS_configureTextRenderer()` adiciona um offset ao código ASCII; a ROM carrega as 96 letras nos ranges 0–95 (branco), 96–191 (verde) e 192–287 (vermelho). O decoder remove o atributo de tile, mapeia esses ranges para ASCII e valida a cor do sufixo `OK!`/`FAILED!`. O driver usa a PNT base `0x3800`, derivada do `SMS_PNTAddress` upstream `0x7800` (flag de escrita removida). Se o prompt não for detectado, o runner retorna `INPUT_DRIVER_TIMEOUT_NO_ORACLE`. A fase VRAM identifica o rótulo exato do padrão que falhou ou `vram_all_tests` no caminho de sucesso. O driver não aciona a última espera para SRAM.

A máquina já expõe APIs read-only/seguras para esta finalidade:
- `SmsTemporalMachine::set_controller()` atualiza níveis do controle sem avançar o tempo;
- `SmsTemporalMachine::framebuffer()` expõe o RGB24 do frame concluído;
- `SmsTemporalMachine::vdp()` fornece acesso read-only ao VDP.

Isso permite desenvolver um driver de teste em fronteiras de frame, sem callbacks por instrução ou avanço de tempo fora do Scheduler. A entrada deve ser acionada por estado observado, não por atrasos fixos em número de frames.

## 3. Limitação histórica de cart SRAM no baseline 2942 — SUPERADA

A implementação `SmsMemory` auditada no baseline histórico `2942abfe0d4754dd857be0bc103d594abfd9b865` continha ROM/cartridge image, mapper, registradores de bancos, 8 KiB de work RAM e BIOS opcional, sem backing store de cartridge SRAM. Essa conclusão descreve somente aquela baseline e foi superada pelas PRs #802/#803.

A ROM chama `SMS_enableSRAMBank(0)` e `SMS_enableSRAMBank(1)`, acessando `$8000-$BFFF`. A auditoria original ocorreu antes da integração de SRAM. A capacidade de produto foi validada separadamente: Phantasy Star com 8 KiB persiste após fechar/reabrir o emulador, e o modo diagnóstico volátil aceita 8/16/32 KiB.

Portanto:
- testes de RAM interna podem ser candidatos a validação depois que o driver e o oráculo visual forem implementados;
- testes de VRAM podem ser candidatos, mas exercitam protocolo de portas e timing de VDP e precisam de oráculo e análise de capacidades próprios;
- o driver desta ROM interrompe a navegação após o resultado VRAM e não executa a fase SRAM; a classificação correta do runner é `NOT_EXERCISED_BY_DRIVER`, não `BLOCKED_CAPABILITY` do produto.

Não modificar o mapper como parte desta tarefa de harness. A cobertura SRAM deste driver deve ser explicitamente marcada como não exercitada; a capacidade persistente de produto já foi validada em trabalho separado.

## 4. Plano de oráculo incremental

### Fase M1 — Entrada determinística por fase

- Implementado um driver específico que aplica pulsos de `button_1` somente depois de frames completos, mantendo o botão pressionado durante o frame seguinte e liberando-o na próxima fronteira.
- A transição exige os textos exatos `Press any button to start`, `Press any controller button` e `to start VIDEO RAM tests`, estáveis por dois frames. O fim da fase VRAM exige a heading `Testing system VIDEO RAM:` e um resultado textual reconhecido na linha 4.
- O detector de prompts é heurístico e ainda precisa ser validado com uma ROM SMSmemtest construída a partir da revisão upstream fixada.
- Eventos de pressionar/liberar são registrados em JSONL com índice de frame.
- A revisão fixada do devkitSMS (`561bc2cb9829cdf2e31ad8cd43835037c8c44a63`) define `SMS_getKeysPressed()` como `KeysStatus & ~PreviousKeysStatus`; por isso o driver aplica uma transição press/release, mantendo o botão durante um frame completo para que o handler de frame a amostre.
- O decoder lê 32 entradas do name table por linha, remove o atributo de tile e mapeia os ranges 0–95/96–191/192–287 de volta a ASCII branco/verde/vermelho.
- O classificador RAM exige o rótulo exato de cada uma das sete linhas e o sufixo `OK!`/`FAILED!` com a variante de cor correspondente.
- O classificador VRAM reconhece `vram_all_zero`, `vram_all_one`, `vram_pattern_55`, `vram_pattern_aa`, `vram_variable` ou `vram_all_tests`.
- Não há OCR nem leitura/modificação de portas VDP; o acesso é read-only por `SmsVdp::vram()`.

### Fase M2 — Captura read-only

- Capturar o framebuffer RGB24 somente depois de `step_frame()` completar.
- Armazenar hash do framebuffer por frame e, para diagnóstico, imagens fora do Git.
- Decodificar/inspecionar as linhas e regiões de resultado usadas pela ROM.
- Determinar por evidência as cores RGB produzidas pelo renderer; não assumir valores de palette sem testar o mapeamento.

### Fase M3 — Resultados por caso

- RAM: classificar os sete padrões separadamente por `OK!`/ `FAILED!`; detectar o loop de RAM fatal como estado separado.
- VRAM: classificar os cinco padrões separadamente; marcar casos não observáveis ou dependentes de granularidade ausente como `BLOCKED`.
- SRAM: `NOT_EXERCISED_BY_DRIVER`; esta execução termina após VRAM. A persistência SRAM do produto foi validada separadamente em #802/#803.
- Um resultado global só pode ser `PARTIAL_PASS` se cada caso executável tiver resultado individual e todos os bloqueios estiverem explicitados; nunca converter isso em PASS da suite completa.

### Fase M4 — Validação do próprio oráculo

- Fixtures sintéticas para cores/linhas de texto e casos OK/FAILED.
- Comparar as decisões do parser com capturas independentes e documentadas.
- Repetir a ROM com o mesmo hash, perfil regional e sequência de entrada; exigir resultados idênticos.
- Separar erro do parser, limite de execução, falha do modelo e falha funcional da ROM.

## 5. Estado

### Execução real e replay determinístico (GitHub Actions run 43391407254)

- ROM SHA-256: `f7654d1dea07ce195e1d57da373a86b3f1ca9869af0d9d7dcb4de6039eb52c71`.
- Perfil: USA/NTSC; resultado RAM disponível no frame 441; resultado VRAM no frame 800.
- RAM: `PASS` em `ram_all_zero`, `ram_all_one`, `ram_pattern_55`, `ram_pattern_aa`, `ram_variable_1`, `ram_variable_19` e `ram_variable_71`.
- VRAM: `PASS_ALL_REPORTED_BY_ROM` — a própria ROM entrou no ramo de sucesso agregado após executar os cinco padrões.
- Instruções: 8.496.776; áudio drenado: 640.825 amostras. O JSONL histórico registrou `sram_phase=BLOCKED_CAPABILITY`; esse rótulo significava que o driver não executou a fase, não que a capacidade atual do produto esteja bloqueada.
- Essa execução valida o fluxo ROM → entrada → framebuffer → classificador, mas não constitui comparação com hardware físico ou outro emulador de referência.
- As duas execuções usam `git rev-parse HEAD` e registraram `emulator_sha=323c492ec98dae7cbed5d1d8eac0e47df8ba8f5f`. A comparação de `start`, todos os registros `frame`, eventos de entrada, resultados por caso e `summary` retornou `determinism=IDENTICAL`. O job CI também exigiu e confirmou 7/7 casos RAM `PASS` e VRAM `PASS_ALL_REPORTED_BY_ROM`.

## 6. Estado

| Componente | Estado |
|---|---|
| Runner full-machine via Scheduler | Implementado |
| CI workspace, SMS runtime Release e ROM regression gate no HEAD `323c492` | PASS |
| Snapshot/hash do mesmo input ROM | PASS — wrapper e build ROM registram SHA-256 do snapshot/imagem executados |
| Entrada automática por fase | IMPLEMENTADA e repetível; prompts detectados nos frames 26 e 441 nas duas execuções |
| Captura/oráculo visual | Exercitado duas vezes em ROM real: sete linhas RAM e resultado agregado VRAM classificados por cor; logs idênticos |
| Testes RAM | PASS reportado pela ROM em 7/7 casos nas duas execuções idênticas |
| Testes VRAM | PASS_ALL_REPORTED_BY_ROM nas duas execuções idênticas; falha individual ainda não identifica qual dos cinco padrões falhou |
| Fase cart SRAM neste runner | NOT_EXERCISED_BY_DRIVER — o driver encerra após VRAM; persistência do produto validada separadamente em #802/#803 |
| Execução real de ROM externa | EXECUTED — 800 frames; 8.496.776 instruções; runner exit code 0 |

## 7. Referências upstream

- SMSmemtest README e instruções de build: https://github.com/sverx/SMSmemtest
- Fonte fixada: https://github.com/sverx/SMSmemtest/tree/12771ed247b93d3a83d0c3cbe7d2d39e3ccb47d9/src
- devkitSMS: https://github.com/sverx/devkitSMS

A fonte upstream descreve RAM interna de 8 KiB, VRAM de 16 KiB e cart RAM de até 32 KiB. A implementação de teste usa explicitamente dois bancos SRAM, e requer toolchain SDCC/devkitSMS e o `crt0_memtest.rel` modificado. Fixar também as revisões de todas as dependências antes de publicar ROM gerada ou comparar resultados.

## 12. Atualização após o fechamento de SRAM — 2026-11-13

A seção 3 acima é um registro histórico da baseline `2942abfe0d4754dd857be0bc103d594abfd9b865` e não descreve o estado atual do produto:

- PR #802 integrou catálogo/perfil e persistência SRAM `.srm`; Phantasy Star com 8 KiB foi testado salvando, fechando e reabrindo o emulador, com restauração confirmada.
- PR #803 integrou o override diagnóstico volátil `SEGA_SMS_DIAGNOSTIC_SRAM_BYTES` para 8192/16384/32768 bytes, sem persistência, e o SMSmemtest v0.40 passou dez verificações de padrões (cinco em cada banco).
- Portanto, SRAM não é mais `BLOCKED_CAPABILITY` no produto. Não reabrir essa etapa sem regressão demonstrada.
- O runner SMSmemtest desta PR deliberadamente termina após o resultado VRAM e não injeta o pulso que inicia a fase SRAM. O campo de resumo foi corrigido para `NOT_EXERCISED_BY_DRIVER`; o histórico antigo `BLOCKED_CAPABILITY` é apenas a classificação anterior dessa cobertura ausente.
- A aprovação de SRAM do produto vem dos testes específicos #802/#803; não deve ser atribuída às duas execuções do runner SMSmemtest descritas neste documento.