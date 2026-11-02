# SMS1 VDP 315-5124 — matriz de validação T0.1

**Data:** 2026-11-02  
**Escopo:** preparação de testes funcionais e de observabilidade para o novo VDP SMS1.  
**Pré-requisito:** PR #365 (T0 parcial).  
**Estado:** matriz proposta; nenhum comportamento temporal desconhecido é promovido a fato.  
**Runtime:** sem alterações nesta etapa.

## 1. Objetivo e regras de segurança

Esta matriz transforma a caracterização T0 em casos reproduzíveis, com estímulo, observável, expectativa, proveniência e limites explícitos. O objetivo é reduzir falsos positivos, testes tautológicos e regressões que passem porque o teste reproduz a mesma suposição do código.

Regras obrigatórias:

1. Testes do novo VDP não podem chamar nem reutilizar o renderer legado como implementação de referência.
2. Fixtures e oráculos devem ser independentes da implementação sob teste.
3. Um teste que verifica determinismo prova apenas determinismo; não prova fidelidade ao hardware.
4. Comportamento documentado funcionalmente pode receber teste funcional sem fixar timestamp ou fase.
5. Todo teste que dependa de posição horizontal, fase, fetch slot, WAIT ou desempate simultâneo fica BLOCKED enquanto não houver evidência adequada.
6. Nenhum teste pode atualizar automaticamente golden output para transformar FAIL em PASS.
7. A ausência de console físico 315-5124 deve ser registrada como limitação, não contornada por constantes copiadas de outro emulador.

## 2. Classes de teste

| Classe | Propósito | Pode fixar timing? | Resultado que autoriza |
|---|---|---:|---|
| UNIT_FUNCTIONAL | Registradores, portas, máscaras e memórias | Não | Semântica funcional |
| PROPERTY | Invariantes e limites de endereçamento | Não | Robustez/invariantes |
| STATE_TRANSITION | Transições de estado e clear/acknowledge | Apenas se documentado | Semântica de estado |
| RASTER_TRACE | Estímulo versus posição raster | Com oráculo apropriado | Comportamento temporal |
| BUS_TRACE | Ordem e amostragem de transações CPU/VDP | Com trace adequado | Integração temporal |
| DIFFERENTIAL | Comparação entre implementações | Não isoladamente | Divergência investigável |
| DETERMINISM | Repetição e save/restore | Não | Reprodutibilidade |
| HARDWARE_ORACLE | Captura de 315-5124 identificado | Sim, conforme setup | Evidência medida |

## 3. Manifest de casos funcionais iniciais

As expectativas abaixo são limitadas ao que as fontes citadas documentam. Elas não fixam ordem de eventos sublinha.

| ID | Área | Estímulo | Observável/assertion | Evidência | Limite |
|---|---|---|---|---|---|
| SMS1-PORT-001 | Control port | Escrever o primeiro byte; em execuções separadas, ler control port, ler data port ou escrever data port antes do segundo byte | Cada uma dessas operações limpa o estado de byte pendente; o segundo byte subsequente inicia uma nova sequência, sem completar a palavra anterior | COMMUNITY_DOC: SMS Power Control Port, seção Writes | Teste de transição funcional; não define fase elétrica nem efeitos de IRQ simultâneos |
| SMS1-PORT-002 | Control port | Enviar os dois bytes de uma palavra de controle, low byte primeiro | Os campos code (bits 15–14) e address (bits 13–0) são decodificados; apenas o segundo byte dispara a ação do code | COMMUNITY_DOC: SMS Power Control Port, seções Writes/Data format | Teste funcional, sem timestamp |
| SMS1-VRAM-001 | VRAM read-ahead | Inicializar buffer conhecido; programar endereço de leitura e executar leituras sequenciais pela data port | A programação do endereço de leitura pré-carrega o buffer e incrementa o endereço; cada leitura retorna o buffer anterior, carrega o próximo byte e incrementa novamente | COMMUNITY_DOC: SMS Power Control Port, seção VRAM read address | Estado inicial/read buffer precisa ser preparado sem assumir valor de power-on; não fixa latência física |
| SMS1-VRAM-002 | Address wrap | Iniciar leitura em 0x3FFF e observar o endereço após incremento | O endereço de 14 bits retorna a 0x0000 após 0x3FFF | COMMUNITY_DOC: SMS Power Control Port, seção VRAM read address | Separar endereço CPU e endereço interno VDP; cobrir escrita em vetor separado se regra específica for sustentada |
| SMS1-CRAM-001 | CRAM width | Escrever bytes CRAM com bits 7–6 variados, em índices conhecidos | Somente os seis bits de cor definidos pelo formato SMS são observáveis no armazenamento digital; não inferir comportamento analógico do DAC | COMMUNITY_DOC: documentação técnica SMS de CRAM; exigir referência de formato específica no fixture | Não valida DAC analógico, conversão RGB ou resposta elétrica |
| SMS1-CRAM-002 | CRAM addressing | Em SMS, escrever nos índices 0x00, 0x1F, 0x20, 0x3F e endereços com bits altos definidos | CRAM tem 32 bytes; bits acima dos cinco bits baixos do endereço são ignorados na escrita CRAM (por exemplo, 0x20 seleciona índice 0x00) | COMMUNITY_DOC: SMS Power Control Port, seção CRAM write address | Não generalizar o tamanho de 64 bytes do Game Gear; leitura da data port permanece leitura VRAM |
| SMS1-STATUS-001 | Status read | Definir flags de status; ler control port uma vez; ler novamente | Primeira leitura retorna flags pendentes e limpa flags internas; segunda leitura não repete flags já reconhecidas | COMMUNITY_DOC: SMS Power Control Port, seção Reads | Não fixa instante elétrico de clear nem amostragem de IRQ Z80 |
| SMS1-STATUS-002 | Sprite flags | Gerar overflow sem colisão e colisão sem overflow; ler status após cada cenário | Mapeamento de trabalho: overflow = bit 6; collision = bit 5; leitura do status limpa flags. A página SMS Power/Sprites contém uma afirmação contraditória que atribui collision ao bit 6 | COMMUNITY_DOC: SMS Power Control Port (bits 6/5), Collision Detection (bit 5), Sprites (linha contraditória: bit 6); confirmar contra manual primário/captura 315-5124 antes de fechar o oracle | Não implementar dois flags independentes com bit compartilhado; manter hardware-oracle pendente |
| SMS1-REG-001 | R9 | Escrever R9 durante active display e VBlank | Scroll vertical aplicado conforme latch em VBlank documentado | PRIMARY_DOC | Fronteira exata do latch UNKNOWN |
| SMS1-REG-002 | R10 | Alterar R10 antes/depois de evento de linha | Valor programado observa a latência de um interrupt descrita no manual | PRIMARY_DOC | Não fixar borda sublinha |
| SMS1-MASK-002 | R2 | Variar R2 bit 0 e observar endereço de name table | Address mask 315-5124 aplica o gate descrito | COMMUNITY_DOC: SMS Power VDP Registers | Separar efeitos conhecidos de observações não testadas |
| SMS1-MASK-003 | R3 | Variar individualmente bits de máscara documentados | Geração de endereço de pattern planes segue máscaras publicadas | COMMUNITY_DOC | Efeitos secundários não testados não são assertions |
| SMS1-MASK-004 | R4 | Variar individualmente bits de máscara documentados | Geração de endereço de pattern planes segue máscaras publicadas | COMMUNITY_DOC | Mesmo limite de R3 |
| SMS1-MASK-005 | R5/R6 | Variar bits documentados e endereços SAT/tile | Máscaras de endereço SAT/tile seguem contrato publicado | COMMUNITY_DOC | Sem inferir fetch timing |
| SMS1-SPR-001 | Sprite line limit | Preparar 7, 8 e 9 sprites elegíveis na mesma linha | Buffer lógico de até oito e overflow quando sprites adicionais qualificam | COMMUNITY_DOC: SMS Power Sprites | Elegibilidade não pode ser substituída por visibilidade final |
| SMS1-SPR-002 | Sprite overflow | Repetir com sprite transparente e/ou X fora da região visível | Overflow reflete avaliação/elegibilidade, não apenas pixels finalmente desenhados | COMMUNITY_DOC | Detalhe temporal da avaliação UNKNOWN |
| SMS1-SPR-003 | SMS1 zoom | Preparar grupos de 2, 4, 6 e 8 sprites com padrões assimétricos | Zoom horizontal não se aplica aos últimos quatro sprites encontrados; zoom vertical é distinto | COMMUNITY_DOC | Confirmar elegibilidade e ordem SAT nos casos de borda |
| SMS1-SPR-004 | Sprite collision | Sobrepor pixels opacos de dois sprites; repetir com pixels transparentes | Colisão apenas na condição opaca documentada | COMMUNITY_DOC | Primeira posição/clock de assert UNKNOWN |
| SMS1-REGION-001 | V-counter | Ler sequência para NTSC 192 e PAL 192 | Sequências verificadas contra fontes específicas, sem reutilizar dados GG ou modos SMS2 | COMMUNITY_DOC | Sem hardware oracle; manter PARTIAL |
| SMS1-ASIC-001 | Revision gate | Selecionar explicitamente 315-5124 | Perfil SMS1 rejeita recursos exclusivos do 315-5246, incluindo modos 224/240 conforme evidência atual | COMMUNITY_DOC | Não inferir ASIC pelo nome comercial |

IDs são estáveis para rastreabilidade; casos podem ser divididos quando uma fonte ou comportamento se mostrar ambíguo. Uma linha não autoriza implementar detalhe temporal não documentado.

## 4. Casos temporais bloqueados

Os casos abaixo devem existir no backlog, mas não podem receber valores esperados até que uma fonte ou captura independente sustente a expectativa.

| ID | Pergunta | Bloqueio atual |
|---|---|---|
| SMS1-RASTER-001 | Qual offset e sequência observável do H-counter? | UNKNOWN; offsets SMS1 não fixados |
| SMS1-RASTER-002 | Em qual fase active display/HBlank começa e termina? | UNKNOWN; descrição aproximada não fixa fase |
| SMS1-RASTER-003 | Qual a fase de reset e relação de fase clock VDP/Z80? | UNKNOWN; frequência nominal não determina fase |
| SMS1-FETCH-001 | Quando ocorrem name/pattern/attribute fetches? | UNKNOWN; sem tabela SMS1 por dot validada |
| SMS1-FETCH-002 | Qual escrita VRAM vence quando coincide com fetch? | UNKNOWN; prioridade simultânea não documentada |
| SMS1-CRAM-003 | Qual primeiro pixel é afetado por write CRAM em cada posição? | UNKNOWN; DAC/latch/fetch schedule não medidos |
| SMS1-SPR-005 | Em qual fase SAT é lida e quando o buffer é fechado? | UNKNOWN; regra funcional não determina fase |
| SMS1-SPR-006 | Qual primeiro dot torna overflow/collision observável? | UNKNOWN |
| SMS1-BUS-001 | Qual acesso CPU recebe WAIT/stall em active display? | UNKNOWN; cadência 16/29 T-states não prova WAIT automático |
| SMS1-BUS-002 | Como CPU write e VDP fetch são ordenados no mesmo instante? | UNKNOWN; não criar prioridade |
| SMS1-IRQ-001 | Em qual fase INT muda e em qual fase Z80 a amostra? | UNKNOWN; exige VDP, bus e Z80 temporal |

Um caso temporal só sai de BLOCKED quando o manifest registrar fonte/medição, setup, ASIC/região, resolução temporal, incerteza do instrumento, estímulo, observável e resultado reproduzível.

## 5. Oráculos e independência

### Proveniência corrigida das assertions de portas

As regras detalhadas de protocolo de portas, read-ahead, wrap e CRAM address alias desta matriz são referenciadas às páginas técnicas comunitárias SMS Power (Control Port), não devem ser rotuladas como PRIMARY_DOC apenas por descreverem o comportamento programável. O manual de software Mark III permanece fonte primária para os pontos efetivamente localizados nele; cada assertion deve indicar a página/seção exata antes de ser promovida a DOCUMENTED. O mapeamento de status overflow/collision permanece sujeito a confirmação por manual primário ou medição física, apesar da convergência de duas páginas comunitárias para bit 5 de collision.

Ordem de confiança:

1. PRIMARY_DOC com escopo de revisão e comportamento explícitos.
2. MEASURED_HARDWARE com placa/ASIC 315-5124, região, setup e dados suficientes para reprodução.
3. INDEPENDENT_TEST cujo resultado esperado tenha sido verificado em hardware 315-5124 ou sustentado por fonte primária.
4. COMMUNITY_DOC com proveniência e ressalvas.
5. EMULATOR_SOURCE apenas para comparação e geração de hipóteses.

### Ares, MAME, MEKA e GearSystem

- Fixar commit/tag antes de qualquer comparação.
- Registrar ancestralidade conhecida e algoritmo comparado.
- Não contar implementações derivadas como confirmações independentes.
- Divergência abre investigação; não é votação.
- Concordância entre emuladores não eleva UNKNOWN a MEASURED_HARDWARE.

### SmsVdpTest

O repositório FluBBaOfWard/SmsVdpTest foi identificado no T0 como candidato, commit 490c00b38c83012dee5a7a582097f080c210a3e7, sem licença declarada e com TODOs de testes físicos pendentes. Até resolver proveniência/licença:
- não copiar fonte, assembly ou ROM binária;
- não adicionar submodule/vendor;
- não assumir 228 CPU cycles/scanline como fato físico;
- usar a descrição pública apenas como pista de cobertura, não como oráculo aprovado.

## 6. Organização futura dos testes Rust

Quando o novo VDP tiver tipos testáveis, preferir testes externos ao módulo de implementação:

```text
crates/video/sms_vdp/
├── src/
│   ├── lib.rs
│   ├── revision.rs
│   ├── registers.rs
│   ├── memory.rs
│   ├── timing.rs
│   ├── fetch.rs
│   ├── sprites.rs
│   ├── pixels.rs
│   └── interrupts.rs
└── tests/
    ├── functional/
    ├── properties/
    ├── state_transitions/
    ├── traces/
    ├── save_restore/
    └── fixtures/
```

A estrutura é proposta, não uma decisão de granularidade final. Não criar módulos vazios. Cada módulo nasce com responsabilidade definida, testes e dependências mínimas.

Fixtures devem ser geradas por helpers independentes e legíveis. Expected e actual não podem chamar o mesmo algoritmo. Para endereços/máscaras, incluir vetores explícitos e property tests limitados. Para renderização, comparar buffer bruto e relatar primeira coordenada divergente.

## 7. Protocolo contra testes tautológicos

Antes de aceitar cada teste:

- Revisar se expected e actual não chamam a mesma função/algoritmo.
- Incluir caso nominal, limite e negativo.
- Verificar que o estímulo distingue implementação correta de incorreta.
- Manter o teste falhando quando a regra for deliberadamente invertida (mutation check manual ou ferramenta apropriada).
- Não derivar expected values do renderer legado.
- Registrar se valida semântica, determinismo, raster, barramento ou hardware.
- Guardar primeiro diff e contexto, não apenas hash final.

## 8. Gate T0.1

T0.1 será aceito quando:
- cada caso tiver ID, classe, fonte, estímulo, observável e limite;
- casos temporais desconhecidos estiverem explicitamente bloqueados;
- nenhum conteúdo externo sem licença for incorporado;
- expectativas de status conflitantes estiverem isoladas;
- testes não dependerem do VDP legado;
- CI documental passar sem warnings.

T0.1 não conclui T0 e não autoriza H-counter offsets, dot fetch schedule, WAIT ou prioridades de eventos simultâneos.

## 9. Próxima sequência após T0.1

1. Revisar assertions funcionais contra as fontes e transformar casos não ambíguos em testes independentes.
2. Preparar fixtures próprias para portas, VRAM/CRAM, masks, status e semântica de sprites.
3. Definir tipos/estado inicial SMS1 sem assumir fase desconhecida.
4. Iniciar VDP-T1: estado temporal serializável, sem renderer scanline como autoridade e sem relógio autônomo.
5. Integrar Scheduler/Z80/bus somente quando contrato temporal e granularidade forem suficientes.

**Resultado pretendido:** reduzir bugs introduzidos por interpretação incorreta, detectar regressões reais e evitar falsas alegações de precisão.
