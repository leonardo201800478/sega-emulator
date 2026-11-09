# Andre Agassi Tennis (Europe, Brazil) (En) — tela preta e ruído de áudio

Data: 2026-10-29  
Estado: **INVESTIGATION_PENDING**  
Runtime alterado: **não**  
ROM no repositório: **não**

## Relato

O usuário relata que **Andre Agassi Tennis (Europe, Brazil) (En)** para Sega Master System não inicia no Sega Emulator: a tela permanece preta e há um chiado/ruído de fundo.

O sintoma não foi reproduzido independentemente nesta fatia. O áudio audível pode indicar que parte do runtime e do PSG está ativa, mas não prova que a CPU executa corretamente, que o VDP recebe a inicialização esperada ou que a ROM está íntegra. Também não é possível concluir, apenas pelo ruído, que o problema seja exclusivamente de áudio.

## Hipóteses concorrentes

1. **ROM/dump:** tamanho, SHA-256, padding, cabeçalho inesperado ou imagem de cartucho diferente da esperada.
2. **Inicialização CPU/Z80:** reset, execução inicial, tratamento de interrupções ou ciclo preso em uma rotina que deixa o PSG em estado audível.
3. **Mapper e memória:** seleção do mapeamento de cartucho, bancos ou leituras iniciais incompatíveis com o hardware esperado.
4. **VDP/estado de vídeo:** registradores, VRAM, modo de vídeo ou timing inicial incorretos, mantendo a tela preta apesar de atividade da CPU.
5. **Região e padrão de vídeo:** perfil selecionado não corresponde ao comportamento esperado pela ROM; testar região e PAL/NTSC como dimensões separadas.
6. **PSG/integração temporal:** áudio contínuo pode resultar de escrita de registrador, estado travado ou pacing, mas precisa ser distinguido de ruído intencional/estado de boot.

Nenhuma dessas hipóteses é declarada causa antes da evidência.

## Plano de investigação

1. Confirmar a mesma cópia local da ROM em todos os emuladores; registrar tamanho e SHA-256 localmente, sem incluir a ROM nem dados que permitam reconstruí-la no Git.
2. Testar em Ares e GearSystem, se disponíveis, com a mesma ROM, região e estado de execução; registrar se ambos iniciam ou reproduzem tela preta/ruído.
3. Registrar as opções de região/padrão de vídeo e repetir Automatic, Europe, USA e Japan quando aplicável, sem confundir região do sistema com padrão PAL/NTSC.
4. Instrumentar um diagnóstico curto do core: PC/SP/registradores Z80 em checkpoints, contagem de instruções, leituras/escritas iniciais de memória e I/O, estado/IRQ do VDP, estado e escritas do PSG. Manter os logs locais e evitar despejar conteúdo de ROM.
5. Identificar se a CPU avança, se o VDP recebe escritas de configuração, se o framebuffer muda e se o PSG recebe comandos repetitivos.
6. Construir teste sintético para a causa isolada (reset, mapeamento, I/O, VDP ou áudio); não transformar uma ROM comercial em fixture.
7. Implementar a correção mínima e executar regressões de CPU, memória/cartucho, VDP, PSG, interrupções, região/PAL-NTSC e SG-1000.

## Critérios de aceitação

- [ ] Reprodução confirmada com configuração documentada.
- [ ] ROM local identificada por tamanho/hash sem publicar o arquivo.
- [ ] Comparação com ao menos um emulador de referência, se a ROM executar nele.
- [ ] Primeira divergência identificada entre execução da CPU, mapeamento, inicialização do VDP e estado do PSG.
- [ ] Teste sintético determinístico que falha antes e passa depois da correção.
- [ ] Sem regressões nos testes SMS existentes e preservação de SG-1000.
- [ ] Testes relevantes no SHA candidato e boot manual no Windows.

## Validação e pendências

- Reprodução independente: **NOT RUN**.
- Teste sintético específico: **NOT IMPLEMENTED**.
- Testes automatizados/build nesta fatia documental: **NOT RUN**.
- CI: verificar os checks do SHA candidato; ausência de checks não significa aprovação.
- `QUICK_PASS` / `SHORT_PASS` / `LONG_PASS`: **NOT CLAIMED** para o jogo.
- `TEMPORAL_PASS`: **NOT ACHIEVED**.
- Nenhuma ROM comercial foi adicionada.

## Próxima etapa

Reproduzir o sintoma e coletar um checkpoint curto da CPU, do VDP e do PSG. O dado mais importante é descobrir se o Z80 está executando normalmente enquanto o VDP não produz imagem, ou se o sistema está preso durante o boot.


## Auditoria estática do boot direto e das portas — 2026-11-09

**Main auditada:** `55585bf2425f1c4eaf61c4a3a3ad42f9499c4797`.  
**Runtime alterado nesta auditoria:** não. Foi acrescentado apenas um teste sintético que demonstra a colisão de stack/mapper descrita abaixo.

### Fatos confirmados no código

1. `SmsTemporalMachine::new_with_hardware_profile()` constrói a CPU com `Z80MicroSequencer::default()`. O estado inicial do sequenciador define PC=0 e SP=0, além dos demais registradores em zero.
2. `SmsTemporalBus::new_with_hardware_profile()` usa `SmsMemory::new()`, portanto não fornece BIOS. `SmsMemory::with_bios(..., None)` escolhe `CARTRIDGE_BOOT_CONTROL = 0xAB`, bancos iniciais do mapper Sega `[0,1,2]`.
3. O valor `$AB` é compatível com a seleção de cartucho documentada para o estado pós-BIOS. Isso não reproduz, por si só, os demais efeitos do BIOS sobre CPU, RAM, VDP e periféricos.
4. A escrita em porta par de `$00-$3E` atualiza o memory-control latch (`$3E` e seus espelhos pares). A escrita em porta ímpar de `$01-$3F` atualiza o I/O-control latch (`$3F` e seus espelhos ímpares). O decoder usa o byte baixo da porta, como exigido pelo espaço I/O do SMS.
5. `SmsIo::new()` inicia `io_control = 0xFF`, estado input-only compatível com a documentação SMS Power. A leitura de `$3F` retorna `$FF`, pois o registrador é write-only.
6. `SmsTemporalBus::write_port()` encaminha corretamente portas pares de memory control para `SmsMemory::write_control_port()` e ímpares para `SmsIo::write_port()`. Já há teste temporal usando `OUT ($3E),A` e `OUT ($3F),A` com o byte alto de A não nulo (`$553E` e `$D53F`), verificando os latches e o timestamp.
7. A RAM interna é espelhada em `$E000-$FFFF`. No mapper Sega, `$FFFE` e `$FFFF` também são os registradores de banco dos slots 1 e 2. As escritas nesses endereços atualizam tanto o espelho de RAM quanto o mapper quando o cartucho está habilitado (bit 6 do latch $3E em zero).

### Risco de inicialização confirmado por teste sintético

Com o estado direto atual (SP=0 e `$3E=$AB`), um `CALL` antes de o software inicializar SP executa:

- decrementa SP para `$FFFF` e escreve o byte alto do endereço de retorno;
- decrementa SP para `$FFFE` e escreve o byte baixo;
- no mapper Sega, altera os bancos dos slots 2 e 1, respectivamente.

Foi acrescentado o teste `direct_boot_default_sp_call_overwrites_sega_mapper_registers` em `native/sms_core/src/temporal_machine.rs`. Para `CALL $0006`, o endereço de retorno `$0003` muda os bancos de `[0,1,2]` para `[0,3,0]`. O teste demonstra o risco de forma determinística; **não prova que Andre Agassi Tennis execute CALL/PUSH antes de definir SP** e não prova que o mapper detectado para a cópia local do jogo seja Sega.

Para Codemasters, o mesmo par de endereços não é decodificado como registradores de banco; portanto, o impacto precisa ser condicionado ao mapper efetivamente detectado.

### O que a documentação externa sustenta

- SMS Power documenta que o software deve inicializar SP antes de usar CALL/RET/interrupções e recomenda comumente `$DFF0`, porque a área superior espelhada contém registradores de hardware (`$FFF0-$FFFF`). Isso é uma convenção de software, não prova do valor de SP deixado por todas as revisões do BIOS.
- A referência de portas documenta `$3F` como controle de direção/nível TH/TR e confirma estado input-only no startup.
- A documentação de memória/mapper confirma `$FFFC-$FFFF` como registradores Sega e o espelhamento simultâneo na RAM.
- Discussões comunitárias sobre o valor exato de SP na saída do BIOS divergem (`$DFFF` aparece como valor histórico reportado; `$DFF0` é a convenção recomendada para programas). Não fixar um desses valores como verdade de hardware sem BIOS disassembly ou trace independente.

### Diagnóstico de trace já disponível e lacuna

- `SmsTemporalMachine::set_trace_enabled(true)` captura eventos reais de barramento (`BusCompleted`) com timestamp, operação, endereço, T-states base, WAITs e dado; `take_trace()` drena os eventos. O mecanismo pode revelar fetches, escritas de memória e I/O.
- A captura genérica é desabilitada por padrão e o vetor cresce sem limite enquanto habilitado. O worker de runtime não expõe controle/extração desse trace.
- `SmsTemporalBus::port_write_trace` é um buffer circular de apenas 64 escritas de I/O; ele não captura escritas de memória nem o histórico completo de boot. Não é suficiente para provar a sequência inicial do jogo.
- A issue #312 já mantém pendente a reconstrução de diagnóstico de boot limitado/opt-in. Deve ser reconciliada com o trace genérico atual, sem reintroduzir PR antiga ou registrar ROM comercial no Git.

### Próximos testes — ordem obrigatória

1. **PASS pendente de CI:** executar o novo teste sintético de CALL/SP=0 no SHA candidato; manter sua classificação separada da reprodução real.
2. **Diagnóstico local do jogo:** registrar tamanho, SHA-256, região e mapper detectado localmente; não compartilhar nem versionar a ROM.
3. **Trace de boot limitado:** coletar checkpoints CPU (PC, SP, AF, BC, DE, HL, IX/IY, I/R, IFF/IM), memory-control, mapper/bancos e eventos de memória/I/O desde o primeiro fetch até o primeiro frame ou falha. Fazer drain em blocos para limitar memória; salvar JSONL localmente, com instruções/bytes de ROM tratados como conteúdo privado.
4. **Identificar primeiro uso da pilha:** localizar o primeiro `LD SP,nn`, `PUSH`, `CALL`, `RST`, RET ou interrupção. Se houver escrita em `$FFFE/$FFFF` antes do setup de SP, correlacionar com mudança de bancos e primeira leitura/fetch divergente.
5. **Diferencial independente:** executar a mesma imagem/região em Ares ou Gearsystem, com o BIOS habilitado e desabilitado quando suportado; registrar o estado de CPU imediatamente após handoff do BIOS, sem assumir que os defaults do emulador de referência sejam prova física.
6. **Só então decidir correção:** perfil de boot pós-BIOS configurável, execução real do BIOS, ou correção específica de outro subsistema. Não impor SP=`$DFF0` globalmente por convenção, nem mascarar o problema com mapper fixo.

### Classificação atualizada

- Portas $3E/$3F — roteamento e latches básicos: **PASS por inspeção + testes existentes**; espelhos e efeitos elétricos completos ainda precisam de cobertura diferencial.
- Estado direto sem BIOS — PC/SP/registradores zerados e memory-control $AB: **CONFIRMED CODE BEHAVIOR**.
- Colisão CALL/PUSH com mapper Sega sob SP=0: **CONFIRMED SYNTHETIC TEST**, aguardando CI.
- Andre Agassi Tennis usa SP antes de inicializá-lo: **UNKNOWN / NOT RUN**.
- Mapper real da ROM local: **UNKNOWN / NOT RUN**.
- Handoff real do BIOS e valor SP específico: **NOT RUN**.
- Reprodução do black screen/ruído e primeira divergência: **NOT RUN**.
