# Sega Master System — plano de implementação

**Estado:** planejamento arquitetural; a implementação funcional SMS ainda não foi iniciada por este documento.  
**Base de trabalho:** branch `feat/rust-master-system-foundation`, criada a partir de `main` em `fec7a6d66e80693655d5295cb499ec3c572a02dc`.  
**Rastreabilidade:** [Issue #115](https://github.com/leonardo201800478/sega-emulator/issues/115).

## Objetivo e decisões fixas

1. O SG-1000 existente permanece funcionalmente inalterado.
2. O núcleo SMS será um crate Rust independente; não dependerá de `sg1000_core` para reutilizar hardware. Componentes compartilhados devem vir de crates neutros e contratos validados.
3. Preservar o renderer Vulkan já consolidado e a saída de áudio WASAPI que corrigiu o PSG. A integração será feita por interfaces de áudio/vídeo existentes, sem introduzir um segundo backend paralelo.
4. O VDP SMS será próprio, distinto do TMS9918A do SG-1000. O renderer Vulkan apresenta o framebuffer; não define o comportamento do VDP.
5. Implementar o YM2413/OPLL para perfis de hardware que realmente disponham de FM — em particular Master System japonês e Mark III com FM Unit. FM será uma capacidade de hardware configurável/detectável, não uma consequência automática de selecionar qualquer ROM japonesa.
6. O limite físico padrão de sprites por linha permanecerá ativado. Uma opção de compatibilidade poderá desativá-lo; essa opção é uma melhoria não fiel ao hardware e deve ser identificada como tal.
7. Região/território, padrão de vídeo (PAL/NTSC/SECAM quando aplicável), revisão/modelo de console e presença de FM são dimensões distintas. Não inferir uma dimensão cegamente a partir de outra.
8. Não declarar precisão ciclo a ciclo até existir um modelo temporal de barramento e testes independentes que sustentem a afirmação.

## Arquitetura de alto nível

- `native/sms_core`: máquina SMS, mapper/cartucho, memória, I/O, VDP e integração temporal.
- CPU Z80 e abstrações de áudio compartilhadas apenas depois da auditoria de cobertura e dos contratos; o núcleo SMS não deve importar `sg1000_core` para obter esses componentes.
- PSG SN76489: reutilizar a implementação validada e o caminho de áudio existente, confirmando clock, divisor, perfil de chip e sincronização no SMS.
- YM2413: componente FM separado, com temporização própria, estado de registradores e mixagem determinística com o PSG.
- Vulkan: continuar como backend de apresentação. VDP gera os pixels e eventos de vídeo; Vulkan não deve implementar limites de sprites ou decisões de hardware.
- WASAPI: continuar como backend de saída. PSG e FM produzem/amostram áudio a partir do tempo de emulação e entregam áudio mixado ao caminho existente, sem chamadas de áudio por instrução Python.
- Qt/PySide6: adicionar menu `Sistema` para selecionar SG-1000 ou Master System. Os outros menus devem refletir capacidades do sistema ativo, mantendo opções comuns e ocultando/desabilitando opções incompatíveis através de metadados de capacidades, não de condicionais dispersas.

## Contratos de configuração

O estado efetivo da máquina deve ser explícito e persistível. Separar, no mínimo:

- **Sistema:** SG-1000 ou Master System.
- **Território/modelo:** por exemplo, japonês/domestic ou exportação; incluir diferenças de I/O e disponibilidade de FM.
- **Padrão de vídeo:** NTSC/PAL e outros perfis apenas quando sustentados por documentação e testes.
- **Capacidades de hardware:** FM YM2413 presente/ausente; demais diferenças de revisão somente quando necessárias e documentadas.
- **Compatibilidade de vídeo:** limite de sprites por linha ativado por padrão; opção para desativá-lo.
- **Cartucho:** tamanho original do arquivo, mapper identificado/selecionado, ROM/RAM de cartucho e persistência quando suportadas.

Ao trocar de sistema, pausar e desmontar com segurança a máquina ativa, aplicar as preferências específicas do destino, atualizar menus/título/diagnósticos e limpar os estados temporais de áudio/vídeo/entrada. Não iniciar automaticamente a ROM anterior no outro console.

## Inventário inicial do repositório

| Componente | Decisão inicial | Evidência/pendência |
|---|---|---|
| Z80 | Reutilizar somente após confirmar cobertura de instruções, estados de interrupção, T-states e contrato de barramento | O repositório contém uma CPU Rust no `native/sg1000_core` e também uma extensão PyO3 Z80. Auditar ambas antes de eleger o caminho oficial. |
| Barramento | Contrato neutro somente se a semântica servir às duas máquinas | Os valores padrão de portas do contrato SG-1000 não são um modelo SMS suficiente. |
| Memória/cartucho | Módulo SMS próprio e mapper explícito | Validar mapa, banking, boot/BIOS quando aplicável, cart RAM e registradores contra documentação técnica. |
| PSG SN76489 | Reutilizar a implementação existente após testes de perfil e temporização | Preservar o caminho WASAPI já corrigido; não alterar a implementação SG-1000 sem regressões. |
| YM2413 | Implementar/adotar componente compatível com licença verificada, isolado do PSG | Testar portas `0xF0`, `0xF1`, detecção/controle em `0xF2`, estado de registradores e coexistência/mute conforme o modelo. |
| VDP | Implementação SMS separada | Mode 4, VRAM/CRAM, registradores, sprites, contadores, interrupções e diferenças regionais devem ser modelados explicitamente. |
| Vulkan | Reutilizar o renderer existente | Confirmar contrato de framebuffer, pitch/format, dimensões dinâmicas e mudanças de resolução sem cópias grandes desnecessárias. |
| WASAPI | Reutilizar o backend de áudio existente | Validar taxas de amostragem, bufferização, underruns, latência e mistura PSG/FM. |
| I/O e controles | Decodificador SMS próprio | Verificar portas, bits, espelhamentos, território e efeitos do controle de FM. |
| Scheduler | Infraestrutura neutra apenas após explicitar clocks e ordem dos eventos | Determinismo primeiro; refinamento temporal guiado por testes. |
| Frontend | Qt/PySide6 com menu `Sistema` e menus dirigidos por capacidades | Guardar preferências por sistema e exibir claramente território, padrão de vídeo e capacidades opcionais. |

## Sequência de marcos

### SMS-0 — inventário, contratos e fontes

- Inspecionar os runtimes Z80, PSG, Vulkan, WASAPI, preferências, carregamento de ROM, menus e workflows atuais antes de modificar código.
- Documentar contratos de fronteira e identificar testes existentes.
- Fixar referências independentes para memória/cartucho, temporização PAL/NTSC, VDP/sprites e YM2413; registrar para cada comportamento se é documentado, observado, inferido ou ainda hipótese.
- Definir perfis de hardware e regras de configuração sem confundir território japonês com PAL/NTSC.

### SMS-1 — crate SMS, memória e cartucho

Criar `native/sms_core` sem dependência de `sg1000_core`. Implementar memória/cartucho com ROMs sintéticas e testes fixos:

- janelas ROM, banking e registradores de controle conforme mapper/documentação validados;
- RAM interna, espelhamento e cart RAM quando presentes;
- limites, ROM vazia/curta, áreas não graváveis, banking inválido e ausência de mapper suportado;
- tamanho de arquivo preservado sem truncamento e sem limite arbitrário de 8 MiB. A aceitação deve depender do tamanho representável e do mapper efetivamente implementado: não prometer suporte a cartuchos maiores do que o hardware/mapeador modelado;
- testes de fronteira e determinismo.
- para ROMs com tamanho em múltiplos de 16 KiB e quantidade de bancos potência de dois, selecionar bancos além do tamanho físico deve reproduzir o aliasing dos bits de endereço não conectados; imagens com quantidade não potência de dois mantêm fallback determinístico `0xFF`, ainda sem alegação de equivalência com hardware para esses bancos.
- o estado inicial do mapper (`FFFC=00`, bancos `[0,1,2]`) corresponde ao perfil documentado do 315-5235; outras revisões podem ter registradores indefinidos ou reset diferente. O construtor atual escolhe esse perfil determinístico provisório, sem alegar universalidade.

Não codificar expectativas de banking ou valores de reset antes de confirmá-los com documentação e/ou vetores independentes.

### SMS-2 — CPU, barramento, território e I/O

Conectar Z80, memória e I/O SMS. Testar instruções, flags, PC/SP, T-states, portas, interrupções, reset e perfis de território. Definir uma configuração de máquina explícita para que os mesmos bytes de ROM possam ser executados em perfis distintos sem estado global oculto.

### SMS-3 — VDP Mode 4, dimensões e sprites

Implementar VDP SMS separado com VRAM, CRAM, portas/registradores, Mode 4, scroll, status, interrupções e sprites. Modelar os modos de vídeo e a contagem de linhas conforme o perfil de hardware; o tamanho do framebuffer deve ser dinâmico e compatível com o renderer Vulkan existente.

**Sprites por linha:**
- modo padrão: respeitar o limite do hardware e sinalizar overflow/status como o VDP faria;
- modo opcional “Desbloquear limite de sprites por linha”: processar sprites além do limite físico para reduzir flicker, marcado como não fiel ao hardware;
- testar o limite, o primeiro sprite excedente, prioridades/ordem, colisões quando aplicáveis e leitura/limpeza dos flags de status;
- não deixar a opção de compatibilidade alterar silenciosamente o modelo temporal ou o estado dos registradores físicos sem decisão documentada.

### SMS-4 — perfis PAL/NTSC e scheduler determinístico

Tratar PAL/NTSC como perfis temporais, não como mero multiplicador de FPS. Validar para cada perfil suportado:

- clock do Z80 e relação de clocks entre CPU, VDP e áudio;
- total de linhas por quadro, linhas ativas, períodos de blanking e dimensões de imagem;
- geração e reconhecimento de VBlank/HBlank/IRQs, execução de CPU por linha/quadro e ordem dos eventos;
- timestamps/amostras de PSG e FM, duração real do quadro e ausência de deriva acumulada;
- determinismo após reset e em execuções repetidas.

Usar constantes nomeadas e referências por perfil, evitando arredondamentos de frequência de quadro que causem deriva. Os valores numéricos finais devem vir de documentação confiável e testes, não de suposições.

### SMS-5 — PSG, FM YM2413 e mixagem

- Reutilizar PSG e WASAPI já existentes, após provar compatibilidade temporal do perfil SMS.
- Adicionar YM2413/OPLL com registros e portas compatíveis, controle/detecção conforme hardware, clock, reset e comportamento de mute.
- Permitir FM somente quando o perfil de hardware oferece esse chip; expor seleção manual/override de compatibilidade sem alterar silenciosamente o território do console.
- Definir mixagem, ganhos e formato interno com testes de amostras determinísticas. Distinguir fidelidade digital do chip, temporização e características analógicas/níveis de saída.
- Testar PSG apenas, FM apenas quando permitido, PSG+FM quando o hardware permite ambos, mute, reset, e jogos/ROMs sintéticas com escritas de registradores.

### SMS-6 — Qt, menu Sistema e integração Vulkan/WASAPI

- Adicionar menu `Sistema` com SG-1000 e Master System; manter o sistema ativo visível.
- Dirigir os demais menus por capacidades declaradas pela máquina ativa. Não mostrar comandos de VDP/áudio inexistentes no sistema selecionado.
- Manter preferências e ROMs recentes por sistema, sem misturar opções exclusivas.
- Integrar o framebuffer SMS ao renderer Vulkan e o áudio mixado à saída WASAPI existentes.
- Integrar via PyO3 em operações por bloco/frame, evitando callbacks Python por instrução/byte e cópias desnecessárias de buffers grandes.
- Trocar sistema deve encerrar/pausar a instância anterior com segurança, zerar estado temporal e atualizar vídeo/áudio/controles/diagnósticos.

### SMS-7 — diferencial, compatibilidade e aceitação

Comparar com oráculo independente (por exemplo, Ares/Gearsystem e testes públicos pertinentes), sem copiar código ou materiais incompatíveis com a licença. Usar ROMs sintéticas/homebrew e materiais legalmente obtidos. Investigar a primeira divergência de CPU, portas, estado do VDP, framebuffer ou amostras.

A matriz de aceitação deve incluir:
- SG-1000 sem regressão;
- SMS em perfis NTSC e PAL suportados;
- território japonês com FM presente e perfil sem FM;
- limite padrão e modo opcional de sprites;
- ROMs pequenas, fronteiras de banking, ROMs acima de 8 MiB com mapper suportado e rejeição explícita de mapper não suportado;
- repetibilidade, reset, áudio contínuo, dimensões de framebuffer e mudança de sistema;
- regressões do frontend, Vulkan e WASAPI.

## Validação obrigatória

Para cada fatia, registrar PASS / FAIL / BLOCKED / NOT RUN. Comandos Rust do workspace:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Para a extensão PyO3, acrescentar build Release/Maturin e testes de integração. Executar testes SG-1000 após alterações em componentes compartilhados. Para mudanças em Python, executar `python -m pytest`, `python -m ruff check .` e `python -m ruff format --check .` conforme aplicável. Se um check anterior falhar e impedir o seguinte, marcar o check bloqueado como tal; não reportar aprovação não observada.

## Referências técnicas iniciais

- SMS Power!, [Documents](https://www.smspower.org/Development/Documents): documentação técnica de hardware SMS/GG.
- Charles MacDonald, [Sega Master System VDP documentation](https://www.smspower.org/Development/VDP-Index): VDP, modos, sprites, registradores, interrupções e temporização.
- SMS Power!, [Audio Control Port](https://www.smspower.org/Development/AudioControlPort): controle e detecção de FM/PSG em hardware compatível.
- SMS Power!, [YM2413](https://www.smspower.org/Development/YM2413): funcionamento do chip FM.
- [Nuked-SMS](https://github.com/nukeykt/Nuked-SMS) e [Gearsystem](https://github.com/drhelius/Gearsystem): referências de estudo/teste; verificar licenças antes de reutilizar qualquer material.
- [Mednafen SMS documentation](https://sources.debian.org/data/main/m/mednafen/1.32.1%2Bdfsg-3/Documentation/sms.html): referência comparativa de opções de território/FM; não substitui evidência de hardware.

Essas referências orientam a investigação; cada comportamento implementado ainda precisa de uma fonte específica, testes e uma classificação clara de evidência.

## Auditoria e modelo do código existente

Antes de iniciar a implementação funcional, foram registrados os resultados da inspeção remota da base SG-1000:

- [Auditoria do código-base para Master System](../audits/master-system-codebase-audit.md): componentes existentes, dependências, limites reais, riscos e estado de validação.
- [Modelo arquitetural do core Master System](master-system-code-model.md): dependências permitidas, estrutura proposta, contratos, fronteiras PyO3/Vulkan/WASAPI e sequência de implementação.

A auditoria confirma que o core Rust standalone SG-1000 é parcial; o runtime PyO3 tem maior cobertura Z80, mas está acoplado à memória/VDP/I/O SG-1000. O loader atual limita ROMs a 48 KiB e o frontend de vídeo assume 256×192. Esses limites devem ser resolvidos com contratos específicos SMS, sem alterar a política SG-1000 globalmente.

**Validação:** leitura remota de arquivos concluída; compilação, testes Rust/Python e validação manual não foram executados nesta etapa.


## Estado da implementação — SMS-A inicial

Foi criado o crate `native/sms_core` e adicionado ao workspace Rust, sem dependência do crate SG-1000. A primeira fatia contém:

- configuração explícita de território, padrão de vídeo, presença de FM e política de limite de sprites;
- `CartridgeImage`, que preserva os bytes completos, inclusive imagens acima de 8 MiB;
- mapa inicial com mapper Sega padrão, três slots de ROM de 16 KiB, 1 KiB inicial fixo e 8 KiB de RAM espelhada;
- testes unitários sintéticos para configuração, preservação de bytes, seleção de bancos, fronteiras de RAM e rejeição explícita de ROM maior que o alcance deste mapper.

**Limites conhecidos desta fatia:** não implementa SRAM de cartucho, variantes de mapper, CPU, I/O, VDP, PSG/FM nem scheduler. A rejeição de ROM maior que 1 MiB é o limite superior conservador desta fatia, baseado na maior revisão conhecida do mapper Sega descrita pela documentação; revisões menores existem. A imagem continua preservada sem truncamento, e suporte maior exige outro mapper validado. O mapa de memória é inicial e precisa de revisão contra documentação técnica/oráculo independente antes de ser tratado como validado em hardware.

**Validação após esta alteração:** os testes foram escritos, mas não foram executados localmente nesta sessão. Aguardar CI no SHA candidato e inspecionar os logs; não declarar aprovação até haver resultados.


### Revisão técnica do mapa de memória

A revisão da referência [SMS Power — Memory Map](https://www.smspower.org/Development/MemoryMap) e [SMS Power — Mappers](https://www.smspower.org/Development/Mappers) encontrou um detalhe que precisava ser explícito no código: os registradores `$FFFC-$FFFF` sobrepõem o espelhamento da RAM. Escritas devem alcançar o registrador de mapper e a cópia de RAM; leituras nesses endereços retornam a cópia da RAM. O registrador `$FFFC` é o controle do mapper, enquanto `$FFFD-$FFFF` selecionam os bancos. O código foi atualizado para modelar a sobreposição e adicionou um teste de regressão.

O teste de formatação inicial passou após a correção de sintaxe, mas a execução de Clippy revelou `clippy::chunks_exact_to_as_chunks` no helper de ROM sintética. O helper foi reescrito com fatias indexadas, e a correção do mapper e do lint foi publicada no SHA seguinte. Aguardando nova execução do CI; compilação/testes locais continuam **NOT RUN**.


## Modelo completo do Z80 e perfis SMS

A modelagem de ISA, registradores auxiliares, flags, prefixos, interrupções, níveis de fidelidade temporal, matriz de testes e perfis de plataforma (exportação americana, doméstico japonês com FM quando presente e Master System III brasileiro) está especificada em [Modelo Z80 para o Sega Master System](z80-sms-platform-model.md). Esse documento é um contrato de trabalho, não uma declaração de implementação concluída. A presença de FM deve ser estabelecida por variante física, não inferida exclusivamente pela região.


## Estado da implementação Z80 (PR #143)

A branch de trabalho agora contém uma cópia independente de `alu.rs` e `cpu.rs` sob `native/sms_core/src`, exportada pela API pública do crate e conectada ao contrato neutro `sega-z80-bus`. O código SG-1000 não foi alterado. Foram incluídos os testes existentes da CPU copiados junto com o módulo, uma matriz de classificação de todos os opcodes base/prefixados e um teste adicional que percorre todos os 256 opcodes DDCB/FDCB para IX e IY. Um teste de integração executa instruções carregadas de um cartucho sintético e verifica leitura/escrita da RAM do SMS.

**Limite:** esses testes demonstram cobertura de execução/classificação e integração básica, não correção integral de todas as flags, timings ou acessos de barramento. Os checks do GitHub Actions devem passar no SHA candidato antes de aceitar esta fatia. CPU, I/O real, VDP, PSG/FM e scheduler ainda não formam uma máquina SMS completa.


### Validação da fatia Z80 — atualização em 2026-10-28

SHA validado: `e5a86c58c689d2f309a99e837d8ef95abe699764` (branch da PR #143).

- **PASS —** `cargo fmt --all -- --check` (workflow SMS Z80 Platform Validation).
- **PASS —** `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- **PASS —** `cargo test --workspace --locked`.
- **PASS —** workflow SMS Core Fast Validation: 52 testes unitários do crate SMS, 1 teste de integração do cartucho/RAM e 1 teste de referência exaustivo da ALU; sem falhas.
- **PASS —** workflow CI geral no mesmo SHA.
- **PASS —** teste de referência cobre ADD/ADC/SUB/SBC/CP/AND/OR/XOR para todos os 65.536 pares de operandos, incluindo carry/borrow onde aplicável. O oráculo usa equações separadas, mas ainda é um modelo algébrico interno, não uma captura direta de hardware.

**Revisão semântica — resultado parcial:** os testes existentes cobrem grupos base/prefixados, flags de operações selecionadas, DAA, registrador R, WZ/Q em casos específicos, interrupções, prefixos indexados e timings nominais. A nova prova exaustiva valida as funções da ALU, não toda a integração de cada opcode nem os ciclos de barramento. O nível temporal continua **A (T-states nominais por instrução)**; não há alegação de precisão M-cycle/T-state.

**Próxima auditoria independente:** integrar um runner pequeno e reprodutível para vetores públicos SingleStepTests/Z80, respeitando a licença MIT do conjunto e mantendo fixtures/versionamento rastreáveis. Começar por flags não documentadas, WZ/MEMPTR, Q, prefixos, estados finais e, separadamente, sequência de barramento. Não importar código de outro emulador para o runtime SMS. Esse trabalho é gate de uma futura declaração de conformidade da CPU, não deve ser confundido com a aprovação da fatia de integração desta PR.

**Limite de plataforma:** os perfis e a capacidade FM permanecem configuração/modelagem; o crate SMS ainda não integra I/O, VDP, PSG/YM2413 ou scheduler e não é uma máquina completa executável.
