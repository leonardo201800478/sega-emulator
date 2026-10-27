# Sega Master System — plano de implementação

**Estado:** planejamento inicial; o núcleo SMS ainda não está implementado neste documento.
**Base de trabalho:** branch `feat/rust-master-system-foundation`, criada a partir de `main` em `fec7a6d66e80693655d5295cb499ec3c572a02dc`.
**Rastreabilidade:** [Issue #115](https://github.com/leonardo201800478/sega-emulator/issues/115).

## Princípios

1. O SG-1000 existente permanece inalterado funcionalmente durante a introdução do SMS.
2. Rust é a implementação oficial para novos componentes temporais e de hardware assim que os contratos e testes de aceitação estiverem definidos.
3. Compartilhar código não significa compartilhar o perfil de hardware: diferenças devem ser explícitas por tipo/configuração, não por condicionais implícitas espalhadas.
4. O VDP SMS (Mode 4) é um componente separado do TMS9918A usado no SG-1000. Compartilhar interfaces genéricas só é permitido quando não esconde diferenças de registradores, VRAM/CRAM, sprites, interrupções ou temporização.
5. Não afirmar precisão ciclo a ciclo sem um modelo temporal de barramento e evidências independentes.

## Inventário inicial do repositório

| Componente | Decisão inicial | Evidência/pendência |
|---|---|---|
| Z80 | Compartilhar a implementação somente após confirmar cobertura de instruções, estados de interrupção, T-states e contrato de barramento | O crate `native/sg1000_core` declara uma CPU Rust própria, mas o workspace puro atual tem lacunas de integração; o runtime Qt possui também uma extensão PyO3. Mapear ambos antes de escolher o caminho oficial SMS. |
| Barramento/CPU bus | Reutilizar o contrato se os métodos e a semântica de acesso servirem ao SMS; adicionar portas e dispositivos por máquina | `native/sg1000_core/src/bus.rs` define acesso de memória e I/O com valores padrão. Não usar o comportamento padrão como modelo SMS final. |
| RAM/ROM | Implementar um tipo SMS dedicado | O mapa de memória SMS, banking, RAM e mapeadores diferem do mapa SG-1000. |
| PSG SN76489 | Candidato a compartilhamento após teste de contrato, clock/divisor e sincronização | Não assumir igualdade de perfil elétrico/temporização só por pertencer à mesma família de chip. |
| VDP | Implementação SMS separada | SMS Mode 4, paleta CRAM, registradores, sprites, contadores e interrupções não são o TMS9918A do SG-1000. |
| I/O e controles | Criar decodificador SMS próprio; compartilhar abstrações de entrada quando compatíveis | Portas e bits de status específicos devem ser verificados em documentação técnica. |
| Scheduler | Compartilhar infraestrutura somente após explicitar relógios e ordem dos eventos | Primeiro alvo é determinismo e T-states nominais; acesso ciclo a ciclo será trabalho posterior com testes. |
| Frontend | Preservar Qt/PySide6; integrar a máquina SMS em etapa posterior | Não modificar UI como parte da primeira fatia de memória. |

## Sequência de marcos

### SMS-0 — inventário e contratos

- Confirmar qual implementação Z80 é completa e testada no `main` e quais instruções/interrupts ainda têm lacunas.
- Identificar testes e workflows existentes que podem ser reutilizados sem assumir cobertura não comprovada.
- Especificar perfil de máquina SMS, região/modelo inicial e política de ROM/mapper.

### SMS-1 — memória e cartridge

Implementar em Rust um módulo isolado de memória SMS com ROM sintética e banking explícito. Critérios mínimos:

- janelas ROM de 16 KiB e comportamento de banking;
- RAM de 8 KiB e espelhamento conforme o mapa de endereços validado;
- registradores de controle em `0xFFFC–0xFFFF` e seus efeitos observáveis;
- limites, imagens ROM inválidas/curtas e comportamento de leitura/escrita em áreas não graváveis;
- testes determinísticos com vetores fixos independentes da implementação.

O valor inicial dos registradores, a região sempre mapeada e detalhes de banking devem ser confirmados contra documentação de hardware antes de fixar expectativas nos testes.

### SMS-2 — CPU e I/O

Conectar Z80 e memória por um barramento SMS. Implementar portas e leitura de controles com máscaras e espelhamentos documentados. Validar instruções, flags, PC, SP, T-states, portas e interrupções em checkpoints sintéticos.

### SMS-3 — VDP Mode 4

Criar VDP SMS separado com VRAM, CRAM, registradores/portas, renderização Mode 4, sprites, status, VBlank/HBlank e interrupções. Avançar de renderização por scanline para granularidade temporal mais fina apenas quando os testes e a arquitetura exigirem.

### SMS-4 — PSG, scheduler e máquina integrada

Integrar PSG, CPU, VDP, I/O e scheduler com ordem de eventos determinística. Definir como o tempo avança e testar reset, repetibilidade, áudio e estado de vídeo em checkpoints equivalentes.

### SMS-5 — integração Qt e diferencial

Expor a máquina pela fronteira PyO3 com operações por frame/bloco, sem callbacks Python por instrução/byte. Comparar com um oráculo independente, preferencialmente Gearsystem/Ares para checkpoints observáveis, respeitando licenças e sem copiar código GPL. Validar ROMs homebrew/sintéticas legalmente obtidas e registrar divergências desde o primeiro checkpoint.

## Validação obrigatória

Para cada fatia, registrar PASS / FAIL / BLOCKED / NOT RUN para os checks aplicáveis. No workspace Rust:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Para mudanças na extensão PyO3, acrescentar build Release/Maturin e testes de integração do backend. Testes SG-1000 devem continuar passando. Benchmarks não substituem testes de correção; hashes iguais não provam fidelidade absoluta.

## Referências técnicas iniciais

- SMS Power!, [Documents](https://www.smspower.org/Development/Documents): documentação técnica de hardware SMS/GG e documentos oficiais.
- Charles MacDonald, [Sega Master System VDP documentation](https://www.smspower.org/Development/VDP-Index): portas, registradores, modos, sprites, temporização e interrupções.
- [Nuked-SMS](https://github.com/nukeykt/Nuked-SMS): estudo de emulação com foco em precisão e evidência de chips; licença GPL-2.0, não copiar código sem análise de compatibilidade.
- [Gearsystem](https://github.com/drhelius/Gearsystem): referência comparativa para SMS/GG/SG-1000; licença GPL-3.0, usar como oráculo/estudo sem derivação de código não autorizada.

Essas referências orientam a investigação; não substituem inspeção dos arquivos atuais nem testes independentes do comportamento implementado.
