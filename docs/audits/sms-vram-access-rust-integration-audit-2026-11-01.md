# Auditoria de integração — acesso temporal à VRAM do SMS

**Data:** 2026-11-01  
**Base de código examinada:** `main` conforme arquivos remotos consultados nesta auditoria; a referência histórica mais recente disponível nas PRs examinadas é `0da3098893cce4d80f7a22291d00488e4e7d0f9b`.  
**Escopo:** confrontar as pesquisas Gemini/Claude sobre acesso à VRAM/CRAM com os contratos e a implementação Rust existentes.  
**Estado:** AUDIT ONLY — nenhuma alteração no runtime nesta etapa.  
**Precisão:** não há declaração de cycle-accuracy, dot-accuracy ou TEMPORAL_PASS.

## 1. Resultado executivo

A proposta isolada do Claude não pode ser integrada diretamente. O repositório já possui um tipo canônico `MachineTime(u128)` em `native/temporal_core`; criar outro `MachineTime(u64)` dentro do SMS duplicaria a autoridade temporal e contrariaria o princípio de um único relógio da máquina.

Também não existe atualmente um crate `native/machine_scheduler` na branch principal examinada. O que está integrado em `main` é `native/temporal_core`, contendo primitivas de tempo, domínios de clock racionais e fila determinística de eventos. A PR #341 propõe uma fundação de scheduler, mas permanece aberta/Draft e não deve ser tratada como código integrado.

O SMS ativo ainda executa a CPU por instrução e avança o VDP em bloco após cada instrução. O timestamp de uma operação de porta não está disponível no callback atual do barramento. Portanto, um contador de intervalos alimentado pelo instante real de cada `OUT` não pode ser implementado corretamente apenas adicionando uma política ao VDP.

**Decisão:** documentar a pesquisa e preparar a integração temporal como trabalho futuro; não introduzir agora `VramAccessPolicy`, `MachineTime` local, slots, WAIT states ou corrupção artificial.

## 2. Inventário remoto observado

### 2.1 Pull requests relevantes

| PR | Estado observado | Relevância |
|---|---|---|
| #339 — runtime ownership/SMS profiles | Open, Draft, não merged; HEAD `1b0e0447498754e75a249a780292427fd4e18125` | Contrato de ownership e perfis SMS1/SMS2; não implementa scheduler/runtime temporal |
| #341 — scheduler foundation | Open, Draft, não merged; HEAD `e52b4bcee43fc59baa6baba70173d981a66581b9` | Proposta de scheduler; não pode ser importada como API de `main` |
| #342 — cycle-accurate architecture docs | Open, Draft, não merged; HEAD `1946cbcda8bb5b9c811cabad0c95fb3f7063f18f` | Branch documental onde esta auditoria e a atualização do evidence register são registradas |

Nenhuma das PRs foi mesclada por esta auditoria. Não houve escrita na branch `main`.

### 2.2 Temporal Core integrado em main

Arquivos observados:
- `native/temporal_core/src/lib.rs`
- `native/temporal_core/src/clock.rs`
- `native/temporal_core/src/event.rs`

Contratos reais:
- `MachineTime` é um newtype de `u128), com `from_ticks`, `ticks`, `checked_add` e `checked_duration_since`.
- `ClockRatio` representa uma razão racional inteira.
- `ClockDomain` mantém tempo, ciclos acumulados e resto de fase; `advance_to` é monotônico e faz preflight aritmético.
- `ClockDomainSnapshot` preserva timestamp, ciclos e fase.
- `EventQueue` ordena por timestamp, `EventPriority` e sequência estável.
- O Temporal Core não define frequências, prioridades ou semântica específica do SMS.

Consequência: qualquer trabalho futuro deve reutilizar `sega_temporal_core::MachineTime`. Não criar wrapper temporal concorrente sem uma necessidade comprovada e uma decisão arquitetural explícita.

### 2.3 SMS VDP ativo

Arquivo: `native/sms_core/src/vdp.rs`.

Estado observado:
- `SmsVdp` possui VRAM de 16 KiB, CRAM de 32 bytes, registradores, endereço, `read_buffer`, estado de comando, raster por scanline e framebuffer.
- `read_port` na porta de dados devolve o byte antigo de `read_buffer`, carrega o próximo byte de VRAM no buffer e incrementa o endereço.
- O comando de leitura também pré-carrega VRAM e incrementa o endereço.
- `write_data` escreve imediatamente em VRAM ou CRAM, atualiza `read_buffer` e incrementa o endereço.
- `advance(tstates)` acumula T-states até 228 por linha e chama `complete_scanline()`.
- `complete_scanline()` renderiza a linha a partir do estado atual de VRAM/CRAM/registradores.
- Não há timestamp absoluto por operação, pipeline de fetch por dot, slots de arbitragem, wait states ou mecanismo de contenção.

Consequência: a política Immediate do exemplo Claude não representa uma nova abstração neutra; ela formalizaria parcialmente o comportamento já existente de escrita imediata, mas sem corrigir o problema central de temporalidade.

### 2.4 Z80 e barramento SMS ativo

Arquivos: `native/sms_core/src/cpu.rs`, `native/sms_core/src/machine.rs`, `native/z80_bus/src/lib.rs`.

Estado observado:
- `Z80::step` executa uma instrução e retorna T-states nominais.
- O próprio módulo declara que não executa cada ciclo de barramento.
- `SmsMachineBus` implementa `CpuBus` com callbacks de leitura/escrita de memória e portas sem parâmetro de timestamp.
- A máquina captura `start_tstate`, executa a instrução completa, captura `end_tstate) e só então chama `vdp.advance(cycles)`.
- O VDP não é avançado até o instante de cada operação I/O dentro da instrução.
- O exemplo existente de timestamp de PSG usa `start_tstate + 11` para um caso nominal de `OUT (n),A`, explicitamente descrito no código como aproximação, não como temporização geral de barramento.

Consequência: o contador de intervalos de escrita proposto pelo Claude não pode receber timestamps confiáveis por meio do contrato atual. A precisão do contador seria limitada pelo timestamp nominal aproximado e não pode ser rotulada como medição cycle-accurate.

## 3. Auditoria do exemplo Rust do Claude

### 3.1 Problemas que impedem integração direta

1. **Tempo duplicado:** `MachineTime(u64)` conflita com o `MachineTime(u128)` canônico do Temporal Core.
2. **Timestamp indisponível:** `CpuBus::write_port` não recebe timestamp; o core não emite evento temporal para cada operação I/O.
3. **Avanço tardio do VDP:** o VDP avança após a instrução, não antes de cada acesso no timestamp correspondente.
4. **Leitura incompleta na interface proposta:** o exemplo `data_op` retorna `()`, portanto não entrega à CPU o byte antigo do read buffer. O VDP existente já implementa read-ahead: retorna o buffer antigo, preenche com VRAM e incrementa endereço.
5. **Diagnóstico incompleto:** `last_was_write` deixa de comparar escritas separadas por uma leitura; o contador mistura VRAM e CRAM; transições de fase não têm regra documentada.
6. **Overflow:** conversões `u64 * 3` e `u64 * 2` não verificam overflow.
7. **API futura não provada:** `drain_due` e `VramEffect` foram antecipados para uma política Slotted que ainda não existe nem tem semântica medida.
8. **Perfil parcial:** a medição aproximada de 26 T-states foi reportada para SMS2 em Active Display; não deve ser aplicada a SMS1, CRAM, VBlank ou outras fases sem evidência correspondente.

### 3.2 O que pode ser aproveitado como ideia

- Diagnóstico passivo separado do comportamento emulado.
- Tempo inteiro e sem ponto flutuante.
- Política de acesso isolada somente quando existir uma necessidade real e uma segunda estratégia justificada.
- Snapshot do estado de diagnóstico, se fizer parte de um subsistema de telemetria persistente.
- Testes determinísticos de intervalos, com proveniência e escopo do perfil explícitos.

Esses pontos são princípios, não aprovação do código apresentado.

## 4. Evidência de hardware preservada — acesso VRAM/CRAM

As informações desta seção foram fornecidas pelo usuário a partir da terceira rodada de pesquisa com Claude e das leituras relatadas de tópicos SMS Power. Não foram reabertos nesta auditoria todos os tópicos, documentos citados indiretamente ou anexos originais. Portanto, a classificação abaixo preserva a proveniência declarada e não transforma fontes indiretas em confirmação independente.

| Afirmação | Estado de evidência preservado | Limite |
|---|---|---|
| Escritas excessivamente rápidas podem corromper VRAM/CRAM em SMS2 | MEDIDO, conforme pesquisa fornecida | Setup, sequência completa e ROM de teste precisam ser arquivados para reprodução |
| Aproximadamente 26 T-states entre escritas no Active Display SMS2 | MEDIDO, conforme pesquisa fornecida | Definição do intervalo inclui custo de instrução; não generalizar |
| Manual Mark III citado indiretamente menciona 29 T no display e 16 T em VBlank | EVIDÊNCIA INDIRETA / CONFLITANTE | Documento original e página não foram conferidos nesta auditoria |
| Manual Game Gear citado indiretamente menciona 28 T | EVIDÊNCIA INDIRETA | Não transferir automaticamente para SMS |
| Estimativa analítica de 22,8 T | INFERENCE / EVIDÊNCIA INDIRETA | Não é limite medido |
| Não há WAIT states efetivos segurando o Z80 nos testes descritos | MEDIDO/RELATADO para os cenários pesquisados | Não prova ausência universal em todas as revisões e operações |
| Slots de VRAM a cada 32 pixels ou 4 pixels | HYPOTHESIS | Posição e fase dos slots não medidas |
| Fase de slots com atraso de 7 dots | HYPOTHESIS / calibração de emulador | Derivada de modelo legado, não medição |
| Mecanismo de acesso tardio (drop/delay/misaddress) | UNKNOWN | Fontes citadas divergem |
| Buffer de leitura de um byte | DOCUMENTED segundo pesquisa fornecida | O VDP atual já possui `read_buffer`; preservar sem alterar semântica |
| Buffer físico de escrita com substituição | UNKNOWN | Não implementar como fato |
| Fetch dot-by-dot Mode 4 SMS | UNKNOWN | Análise BlastEm do Mega Drive não é evidência SMS |
| Arbitragem SMS1 vs SMS2 | UNKNOWN | Medições reportadas são SMS2 |

### Regra de implementação

Não inserir slots, WAIT states, perda de byte, atraso ou corrupção artificial. A política funcional existente permanece imediata até que uma especificação experimental reproduzível autorize outro comportamento. O contador de violações só deve ser implementado quando houver timestamp de barramento confiável e definição precisa do que conta como par de acessos.

## 5. Requisitos para futura instrumentação

Antes de contar espaçamentos entre operações de porta, o runtime precisa oferecer:
- timestamp absoluto do evento de I/O, derivado do relógio canônico;
- instante de amostragem/commit da operação explicitamente definido;
- transações do Z80 com posição dentro do M-cycle/T-state;
- avanço causal do VDP até o evento, sem avançar duas vezes o mesmo intervalo;
- preservação do comportamento de read-ahead da porta de dados;
- distinção entre operações VRAM e CRAM;
- classificação de raster no timestamp da operação, sem inventar a fase de arbitragem.

O contrato temporal do Z80/barramento atual ainda é parcial: traces T-state para oito instruções ED específicas não equivalem a timestamp por operação de I/O para todas as instruções.

## 6. Plano seguro em etapas

### A0 — documentação (esta alteração)
- Registrar pesquisa, conflitos, proveniência e limites.
- Registrar inventário real do código e PRs.
- Não alterar runtime.

### A1 — contratos temporais existentes
- Revalidar o estado de `native/temporal_core` e da PR #341.
- Resolver a validação pendente da PR #341 antes de depender de qualquer API proposta nela.
- Não criar segundo tipo de tempo.

### A2 — timestamp de transações Z80
- Desenhar micro-op/transação temporal para operações de memória e I/O.
- Manter CPU independente da plataforma SMS.
- Criar testes por T-state e barramento antes de ligar ao VDP.
- Não inferir offset de I/O a partir do total nominal da instrução.

### A3 — integração VDP sem novas hipóteses físicas
- Preservar read-ahead e semântica funcional existente.
- Sincronizar o VDP no timestamp real da transação, quando disponível.
- Manter renderização scanline identificada como limitação, sem alegar precisão intralinha.

### A4 — diagnóstico experimental
- Somente após A2/A3, introduzir contador read-only.
- Perfis de evidência separados por ASIC, fase e tipo de memória.
- Save/load/replay deve reproduzir exatamente os mesmos contadores.
- Diagnóstico não altera o estado emulado nem os hashes funcionais.

### A5 — política Slotted
- Bloqueada até existirem medições reproduzíveis em hardware real.
- Não adotar o padrão 32-dot/4-dot nem a fase de 7 dots como verdade.
- Qualquer política futura deve entrar como hipótese experimental, com testes e comparação contra hardware.

## 7. Critérios de aceitação documental e de implementação

- Esta auditoria e o evidence register são versionados antes de qualquer mudança de runtime.
- Cada dado externo tem classe de evidência, escopo e proveniência.
- Nenhum valor medido em SMS2 é aplicado automaticamente a SMS1.
- Nenhum código do Claude é copiado sem adaptação aos contratos reais.
- Não há duplicação de `MachineTime`.
- Nenhuma alteração reivindica WAIT, slots, fetch pipeline ou corrupção sem evidência.
- Toda futura implementação deve passar pelos checks Rust/Python obrigatórios no SHA candidato exato.
- A documentação não equivale a QUICK_PASS, SHORT_PASS, LONG_PASS ou TEMPORAL_PASS de runtime.

## 8. Fontes e referências

### Código do repositório
- `native/temporal_core/src/clock.rs` — MachineTime u128 e ClockDomain racional.
- `native/temporal_core/src/event.rs` — EventQueue e ordenação estável.
- `native/sms_core/src/vdp.rs` — portas, read buffer, escrita imediata e avanço scanline.
- `native/sms_core/src/machine.rs` — SmsMachineBus e avanço do VDP após instrução.
- `native/sms_core/src/cpu.rs` — executor Z80 por instrução e T-states nominais.
- `native/z80_bus/src/lib.rs` — contrato CpuBus e traces temporais parciais.

### Documentação e pesquisa
- SMS Power — VDP documentation: https://www.smspower.org/maxim/Documents/VDP
- SMS Power — Software Reference Manual for SEGA Mark III: https://www.smspower.org/Development/Documents
- ZiLOG Z80 CPU User Manual UM008011-0816: https://www.zilog.com/docs/z80/um0080.pdf
- MAME 315-5124 implementation (comparativo, não prova de hardware): https://github.com/mamedev/mame/blob/master/src/devices/video/315_5124.cpp
- emu198x PR #1626 (referência indireta citada na pesquisa do usuário; conteúdo original não revalidado nesta auditoria): https://github.com/emu198x/emu198x/pull/1626

### Proveniência da pesquisa
- Rodadas de pesquisa Gemini/Claude e sínteses técnicas fornecidas pelo usuário nesta conversa, em 2026-11-01.
- Os tópicos SMS Power 2126, 10523, 16298 e 11689 foram relatados como lidos diretamente pelo usuário.
- MacDonald §15, tópicos 14599, 13374, 14599 e 13374, e manual Mark III/Game Gear citados via PR #1626 foram relatados como fontes indiretas, não reabertas nesta auditoria.
- As classificações preservam a descrição do usuário; não constituem nova medição independente realizada pelo projeto.

## 9. Estado final desta auditoria

**AUDIT DOCUMENTED — runtime unchanged.**

A pesquisa técnica foi preservada, o exemplo Rust foi confrontado com os contratos existentes e a integração foi bloqueada nos pontos em que o runtime ainda não fornece timestamps de barramento confiáveis. Próximo gate: concluir e validar a fundação temporal/scheduler e, em trilha separada, evoluir o Z80 para transações temporais observáveis.


## 10. Aditivo — revalidação da fundação Machine Scheduler (2026-11-01)

**Snapshot remoto atualizado:** PR #341 permanece Open/Draft, agora no HEAD
`9f45f6444d19f0d4ab775912ca8e7454cf5280ed`. O workflow CI
`41353993811` terminou com sucesso: Rust formatting, Clippy, testes do
workspace, Ruff formatting, Ruff lint e regressões Python passaram. A correção
em relação ao snapshot anterior foi restrita ao teste
`registration_and_past_events_are_validated`; não alterou a semântica do
scheduler. PR #339 também permanece Open/Draft e seu CI
`41349282720` passou. PR #342 continua sendo a branch documental desta
auditoria.

### 10.1 Contratos efetivamente presentes na PR #341

O crate `native/machine_scheduler` compõe as primitivas de
`sega_temporal_core` e implementa:

- uma timeline `MachineTime` por instância de `MachineScheduler<T>`;
- domínios registrados em `BTreeMap<ClockDomainId, ClockDomain>`, com ordem
  de apresentação determinística pelo ID;
- conversão racional com preflight de overflow antes do despacho;
- despacho do próximo evento após avançar todos os domínios até seu timestamp;
- eventos ordenados pela fila compartilhada por timestamp, prioridade e
  sequência de inserção;
- avanço idle que recusa atravessar evento pendente;
- payload tipado, sem callback de dispositivo dentro do crate.

O CI verde valida os gates configurados no workflow para esse SHA; não prova
integração de hardware nem precisão temporal física.

### 10.2 Lacunas e restrições para integração

1. **Clock edges ainda agregadas:** `MachineDispatch` entrega
   `elapsed_cycles`, total e fase por domínio. Não fornece timestamp de cada
   borda local nem API de próxima borda. Isso não é suficiente, sozinho, para
   sequenciar micro-operações Z80 ou fetches VDP que dependam da posição exata
   de cada borda. Antes da integração, implementar/definir a API de próxima
   borda ou eventos de borda e seus testes.
2. **Fase inicial de domínio:** `register_clock_domain` cria o domínio com
   fase zero no `now` corrente. O scheduler não expõe
   `ClockDomain::with_phase`. Até existir contrato explícito para fase inicial,
   registrar todos os domínios estáticos antes do primeiro avanço. Registro
   dinâmico após avanço não deve ser usado para simular um clock cuja fase
   física deva continuar ou ser derivada de outra fonte.
3. **Fase de evento não é dimensão independente:** a fila ordena
   `(timestamp, EventPriority, sequence)`; o modelo temporal documenta
   `(timestamp, hardware_phase, platform_priority, stable_sequence)`.
   Não presumir que o campo único `EventPriority` resolve ambas as dimensões.
   Antes de integrar eventos simultâneos de CPU/VDP/PSG, reconciliar o contrato
   e justificar cada precedência por perfil e evidência.
4. **Eventos delta sem limite explícito:** eventos causais no mesmo timestamp
   são permitidos na mesma prioridade ou em prioridade posterior; prioridade
   anterior é rejeitada. Não há limite de iterações/eventos delta por timestamp.
   Definir proteção contra ciclos causais infinitos no loop proprietário da
   máquina antes de permitir produtores de eventos recursivos.
5. **Save state incompleto:** o scheduler deriva `Clone`, mas não oferece
   snapshot/restore completo serializável da timeline e da fila. O snapshot
   individual de clock domain não captura eventos pendentes, sequência global
   ou histórico de despacho. Não declarar save-state determinístico da máquina
   com base apenas nessa API.
6. **Sem semântica de dispositivo:** o crate não executa callbacks e não
   determina prioridades SMS, WAIT, contenção ou arbitragem. A máquina
   consumidora ainda precisa estabelecer causalidade e sincronização antes de
   ler/escrever estado de dispositivos.

### 10.3 Testes presentes e próximos testes necessários

Testes existentes cobrem avanço racional, fase fracionária, ordem de IDs,
idle sem saltar evento, preflight de overflow, rejeição de domínio duplicado,
evento passado e equivalência entre avanço único e avanço em blocos.

Antes de integrar a máquina, acrescentar testes direcionados para:

- despacho de múltiplos eventos no mesmo timestamp e mesma prioridade;
- evento causal criado pelo consumidor durante o despacho, no mesmo timestamp;
- rejeição de prioridade regressiva no timestamp já despachado através da API
  `MachineScheduler`;
- evento causal com prioridade posterior e preservação da sequência;
- falha de preflight com múltiplos domínios, provando que nenhum domínio muda;
- fase inicial explícita e/ou proibição documentada de registro após início;
- limite determinístico para cadeias delta;
- snapshot/restore incluindo fila, sequência, timestamp e fases.

### 10.4 Decisão e estado

A PR #341 é uma **fundação de scheduler genérica validada por CI**, não um
scheduler de máquina SMS integrado e não um componente TEMPORAL_PASS. O
resultado do CI permite avançar para revisão de contrato e testes adicionais,
mas não autoriza ligar o Z80/VDP ao scheduler usando apenas contagens agregadas
de ciclos.

**Próximo gate:** fechar as decisões de bordas, fase de evento, causalidade
delta e snapshot/restore; depois definir transações temporais de barramento
Z80. Nenhuma alteração no runtime SMS/VDP foi feita por este aditivo.
