# Auditoria de prontidão — ROMs externas de diagnóstico SMS

**Data:** 2026-11-13  
**Baseline consultada:** `main` = `2942abfe0d4754dd857be0bc103d594abfd9b865`  
**Escopo:** inventário e prontidão de execução das suites públicas SMS; nenhuma alteração no runtime.

## 1. Conclusão executiva

As suites externas são úteis e devem fazer parte da validação independente. Entretanto, a baseline ainda não oferece um runner que execute ROMs pelo caminho completo SMS.

O binário `native/sms_core/src/bin/z80test_runner.rs` declara e implementa um `FlatBus` de 64 KiB. Ele executa o motor Z80 em memória plana e não instancia a máquina SMS, VDP, PSG, mapper ou Scheduler. Portanto, não é um runner válido para SMSVDPTest, SMSmemtest ou SMSTestSuite.

A máquina temporal SMS já possui uma integração real de Z80, barramento, Scheduler, PSG compartilhado e VDP SMS. O caminho de barramento confirma que efeitos de portas são aplicados na conclusão programada da transação. Contudo, o próprio código delimita a implementação VDP atual como scanline-clocked: arbitragem de dot/fetch e efeitos de meio de scanline ainda não estão implementados. Isso limita quais testes de SmsVdpTest podem ser considerados executáveis.

**Decisão:** primeiro construir o harness Rust full-machine e a superfície de observação determinística; depois executar os testes funcionais compatíveis. Não promover resultados CPU-only a PASS de sistema SMS.

## 2. Evidências do código

### 2.1 Runner Z80 existente

Arquivo: `native/sms_core/src/bin/z80test_runner.rs`.

- Inicializa `FlatBus` com 65.536 bytes.
- Copia a imagem em `0x8000`.
- Define um endereço sentinela de retorno e executa `Z80::step`.
- Reporta instruções, T-states, leituras de porta e contadores B/C.
- Não instancia `SmsTemporalMachine` nem `SmsTemporalBus`.

O README `tools/z80test/README.md` já reconhece que esse binário é incompleto como harness externo e que a orquestração Rust ainda precisa ser reconstruída. A suite ali descrita (Patrik Rak) é direcionada a ZX Spectrum, não deve ser confundida com ROM de diagnóstico SMS.

### 2.2 Máquina temporal SMS

Em `native/sms_core/src/temporal_machine.rs`, `SmsTemporalMachine` executa o Z80 microsequenciado e despacha operações pelo barramento temporal. A API contém stepping de instrução e frame com limite de instruções.

Em `native/sms_core/src/temporal_bus.rs`:

- `SmsTemporalBus` possui memória/cartridge, I/O, PSG agendado, VDP e coordenador temporal.
- As escritas de porta são aplicadas na fronteira de conclusão da transação.
- O estado serializável inclui PSG, VDP, memória, coordenador e operações pendentes.
- O comentário de módulo explicita que o VDP avança em granularidade de scanline e que dot/fetch arbitration e efeitos de meio de scanline permanecem fora da fatia atual.

A integração temporal VDP→Z80 INT existente é relevante para os testes de interrupção, mas não comprova por si só a precisão de HCounter, VCounter, fetch ou IRQ dentro da linha.

### 2.3 Gate Z80

O scorecard consultado mantém E7.7 em `IN_PROGRESS` e E8 em `BLOCKED`. Permanecem lacunas de auditoria semântica global, execução integral do ROM Z80 Block Flags Test v5.0 e validação direta em hardware SMS. Assim, ZEXDOC/ZEXALL não será iniciado nesta etapa.

## 3. Classificação das suites

| Suite | Evidência de origem | Uso recomendado | Estado |
|---|---|---|---|
| SMSmemtest | Repositório sverx; licença Unlicense; testa RAM interna, VRAM e cartridge RAM | Primeiro candidato funcional após runner full-machine e confirmação do mapeamento de memória | BLOCKED — runner |
| SMS Test Suite v0.38 | Repositório sverx; vídeo, PSG/FM, controles, paddle e identificação do sistema; licença não localizada no snapshot auditado | Smoke tests funcionais e validação manual; fixar dependências de build | BLOCKED — runner e proveniência de redistribuição |
| SmsVdpTest | Repositório FluBBa; protocolo de portas, VRAM/CRAM, counters, VINT/HINT e sprites; licença não localizada no snapshot auditado | Separar casos funcionais de casos dependentes de dots/fetch e timing sub-scanline | BLOCKED — runner; subset timing adicionalmente bloqueado por capacidade |
| ZEXALL-SMS | Repositório maxim-zhao; licença GPL-2.0 | Gate formal E8, depois do fechamento E7 | BLOCKED — E7 |
| NotOnlyWords | Identidade exata do projeto não confirmada | Não integrar até receber URL ou origem verificável | BLOCKED — origem |

O detalhe de cada revisão imutável e os limites de redistribuição estão em `tests/sms/manifest.toml`.

## 4. Interpretação correta do SmsVdpTest

O README upstream recomenda que o emulador passe ZEXALL (ou ZEXDOC) e modele 228 CPU cycles por scanline antes da execução da suite. Essa recomendação é um pré-requisito do autor do teste; não é autorização para ignorar o gate E7 do projeto.

A suite cobre áreas distintas:
- protocolo de portas, latch, buffer de leitura, wrap e compartilhamento de endereço VRAM/CRAM;
- valores e mudanças de H/V counters;
- momento de VINT/HINT e persistência de flags até leitura do status;
- collision e overflow de sprites, inclusive limites por scanline;
- latch de scroll.

Nem todos esses casos dependem da mesma granularidade. O harness deverá registrar o nome do caso e a capacidade exigida. Casos de protocolo podem ser avaliados antes de casos de beam timing; testes de HCounter, collision pixel-accurate e latches dentro da linha não podem ser declarados válidos enquanto o modelo correspondente não existir.

A observação publicada em fóruns SMS Power também alerta que versões/revisões de VDP podem divergir nos resultados. A execução deve declarar o perfil VDP e não generalizar uma aprovação para todos os ASICs.

## 5. Plano de implementação

### Fase A — Harness Rust full-machine

- Criar binário de diagnóstico separado do `z80test_runner` CPU-only.
- Instanciar `SmsTemporalMachine` com perfil explícito de território, VDP, clock e taxa de amostragem.
- Carregar ROM sem patch de bytes upstream.
- Permitir limite de instruções, limite de frames, timeout de host e input determinístico.
- Executar exclusivamente por APIs públicas de stepping; nenhum atalho pode avançar o Scheduler fora do caminho da máquina.
- Encerrar por condição explícita de teste, frame budget ou limite de segurança.
- Emitir JSON Lines com build SHA, ROM SHA-256, suite/revisão, perfil, frames, instruções, T-states/machine ticks e status.
- Não despejar memória ou bytes da ROM em logs.

### Fase B — Observabilidade read-only

- Expor snapshots imutáveis de status/VRAM/CRAM/counters e eventos já disponíveis, sem mutar hardware.
- Toda observação deve ocorrer em fronteira segura e ser identificada como estado do modelo, não captura de silício.
- Não adicionar callbacks por instrução ao frontend; esta ferramenta é um binário de teste offline.
- Se uma observação exigir granularidade não existente, reportar `BLOCKED_CAPABILITY`, não estimar.

### Fase C — Execução incremental

1. SMSmemtest: RAM interna, VRAM e cartridge RAM conforme mapper/perfil realmente suportados.
2. SMSTestSuite: testes de vídeo/áudio/controle que tenham oráculo observável e dependências de build fixadas.
3. SmsVdpTest: executar apenas casos compatíveis com as capacidades temporais implementadas; manter sub-scanline cases bloqueados.
4. ZEXALL-SMS: somente depois de E7 formalmente fechado e do harness compartilhado Z80 auditado.

### Fase D — Evidência independente

- Fazer captura separada no hardware SMS quando disponível.
- Registrar revisão/ASIC e região reais.
- Não usar a comparação entre dois caminhos internos do próprio emulador como oráculo independente.
- Manter PASS por caso e perfil; não converter um subconjunto aprovado em PASS da suite inteira.

## 6. Não objetivos desta entrega

- Não modificar Z80, VDP, PSG, Mapper, Scheduler, clocks ou WAIT states.
- Não alterar o pacing P0 ou a GUI Slint/WGPU.
- Não incorporar ROMs binárias ou fontes de terceiros.
- Não executar ZEXALL antes de E7.
- Não afirmar que o runtime SMS já passa suites externas.

## 7. Referências

- FluBBa, SmsVdpTest: https://github.com/FluBBaOfWard/SmsVdpTest
- sverx, SMSmemtest: https://github.com/sverx/SMSmemtest
- sverx, SMSTestSuite: https://github.com/sverx/SMSTestSuite
- maxim-zhao, ZEXALL-SMS: https://github.com/maxim-zhao/zexall-sms
- SMS Power, tópico SMS VDP Tester: https://www.smspower.org/forums/10695-SMSVDPTester
- SMS Power, tópico SMS Test Suite: https://www.smspower.org/forums/17116-SMSTestSuite

Esta auditoria é um inventário de prontidão, não um relatório de execução de ROMs.

## 8. Atualização — runner full-machine implementado na branch de trabalho

Após a auditoria inicial, foi criado:

- `native/sms_core/examples/sms_rom_runner.rs`: runner offline que instancia `SmsTemporalMachine::new_with_hardware_profile` e avança frames exclusivamente por `step_frame`.
- `tests/sms/scripts/run_rom.py`: wrapper que calcula SHA-256 da ROM em streaming, identifica o commit Git corrente e chama o exemplo Cargo.
- `tests/sms/README.md`: instruções de execução e semântica dos registros JSONL.

### O que o runner entrega

- perfis regionais explícitos USA/NTSC, Brasil/PAL-M, Europa/PAL e Japão/NTSC, alinhados à tabela `sms_temporal_region_profile` do worker SMS;
- limite configurável de frames e instruções por frame;
- identificação da ROM por SHA-256 e tamanho;
- identificação da revisão do emulador e revisão upstream da suite;
- registros JSON Lines de início, frames concluídos e resumo;
- drenagem dos samples PSG após cada frame para evitar crescimento ilimitado do buffer de áudio no teste offline.

### Limitações deliberadas

- inicialização direta do cartucho, sem executar BIOS ou reproduzir sua sequência de inicialização;
- não interpreta texto na tela nem buffers privados de resultado das ROMs;
- não injeta controles ou scripts de entrada nesta primeira versão;
- não classifica sucesso de uma suite. O resumo `EXECUTED_NO_ORACLE` significa apenas que a máquina completou o orçamento de frames;
- não normaliza automaticamente cabeçalhos de dumps de ROM;
- não observa dots/fetches ou efeitos sub-scanline ausentes do modelo VDP atual.

Foram adicionados testes unitários do escape JSON e da seleção/rejeição de perfis regionais. Esses testes ainda precisam ser executados pelo CI no HEAD da PR. Nenhuma ROM externa foi baixada ou executada nesta atualização.

### Próxima validação

1. CI deve compilar o exemplo e executar seus testes.
2. Foi adicionado `native/sms_core/tests/sms_rom_runner_smoke.rs`: fixture homebrew sintético em memória, sem ROM externa, que exige dois frames completos via Scheduler.
3. Ainda é necessário validar em CI a execução desse fixture e o exemplo.
4. Executar SMSmemtest em ambiente controlado e determinar seu oráculo de resultado a partir do código/documentação upstream.
5. Manter SMSTestSuite e SmsVdpTest sob a mesma regra de resultado por caso; não promover execução de frames a PASS.
