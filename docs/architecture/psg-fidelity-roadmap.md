# Roadmap de fidelidade do PSG e reutilização de hardware Sega

**Estado:** proposta técnica e inventário de lacunas, baseada em inspeção estática do código e pesquisa documental em 26/10/2026. Não é uma declaração de fidelidade validada contra hardware real.

## Objetivo e decisões

O primeiro alvo continua sendo o PSG do SG-1000. A arquitetura deve permitir compartilhar lógica realmente comum com Master System, Game Gear e Mega Drive/Genesis, sem tratar as variantes como um único chip idêntico.

Decisões para esta etapa:

- Não mudar o som padrão nem os clocks lógicos apenas para obter um resultado auditivamente mais agradável.
- Não adicionar um passa-baixa arbitrário como se fosse uma característica comprovada do SN76489AN.
- Separar **lógica digital do chip**, **reconstrução/reamostragem digital** e **resposta analógica da plataforma**.
- Manter Python como referência legível e testar qualquer mudança equivalente no backend Rust. O runtime Rust permanece a via de desempenho preferencial; JIT fica fora do escopo enquanto os resultados medidos não demonstrarem necessidade.
- Não implementar todos os consoles nesta etapa. Primeiro corrigir e validar o perfil SG-1000; perfis Sega integrados são uma etapa seguinte.

## Evidências observadas no repositório

A inspeção da main identificou os seguintes fatos:

1. src/sega_emulator/audio/sn76489.py e native/z80_core/src/audio.rs implementam um modelo de 15 bits, seed 0x4000, feedback branco a partir dos bits 0/1 e divisor de entrada por 16. Esse perfil é coerente com a família discreta SN76489AN documentada para SG-1000, mas **não deve ser reutilizado sem configuração para os PSGs integrados Sega**.
2. A máquina emite amostras em 44,1 kHz capturando o estado discreto corrente nos ticks internos. Não há, nessa etapa de geração, filtro anti-aliasing de banda limitada nem modelo de resposta analógica.
3. samples_to_pcm (quando usado pelo frontend Qt) aplica interpolação linear para adaptar a taxa de saída. A interpolação ocorre depois da geração das amostras de 44,1 kHz; portanto, não pode remover aliasing que já tenha sido introduzido na redução da taxa.
4. A mistura aplica atenuação logarítmica aproximada de 2 dB por passo e soma quatro canais com normalização por quatro. Isso é uma aproximação digital plausível, mas não valida ganho, clipping ou a resposta elétrica de uma placa de console.
5. O caminho Rust reproduz os contratos observáveis da implementação Python e tem testes diferenciais. Uma alteração no algoritmo de amostragem ou nos estados do chip deve manter essa paridade, ou atualizar explicitamente o contrato e os testes.

A descrição de som metálico/estridente é consistente com aliasing e ausência de resposta de saída, mas ainda é uma **hipótese diagnóstica**, não prova de causa única. O volume percebido deve ser medido separadamente por pico, RMS e alinhamento de ganho.

## O que a documentação técnica sustenta

### Variantes digitais

SMS Power descreve o SN76489AN discreto utilizado no SG-1000/SC-3000 como uma variante de 15 bits com feedback branco nos bits 0/1, enquanto o PSG Sega integrado usado no Master System, Game Gear e Mega Drive usa uma sequência de ruído de 16 bits com taps 0/3. O Game Gear acrescenta um registrador de roteamento estéreo por canal. O protocolo latch/data e os níveis de atenuação são comuns, mas esses detalhes tornam incorreto presumir que uma configuração fixa representa todos os consoles.

O MAME contém perfis explícitos para as variantes TI e Sega e é uma boa referência comparativa para o comportamento digital. A documentação SMS Power também registra comportamento não ideal de saída: decaimento de níveis DC, alterações sutis na forma de onda, diferenças no ruído e uso de offsets para reprodução de amostras. Esses fenômenos devem ser tratados como parte da resposta física, não como substitutos da lógica digital.

### Filtragem e amostragem

A frequência interna de atualização após o divisor /16, com clock NTSC de 3.579.545 Hz, é aproximadamente 223.721,6 atualizações por segundo. A taxa PCM de 44,1 kHz tem Nyquist de 22,05 kHz. Se o estado do sinal é simplesmente amostrado a 44,1 kHz, componentes acima de Nyquist podem dobrar para frequências audíveis (aliasing).

É necessário distinguir:

- **Filtro anti-aliasing do reamostrador:** requisito matemático para reduzir uma fonte de maior taxa a PCM de saída. Deve preservar estado entre blocos e não depender da divisão arbitrária dos frames.
- **Filtro analógico/decadência:** aproxima o circuito e os efeitos elétricos de uma plataforma concreta. A curva correta não está demonstrada para o SG-1000 por medições neste repositório.
- **Passa-alta para DC:** pode representar acoplamento AC, mas frequência de corte e resposta transitória devem ser apoiadas por esquema ou medição da plataforma.

O ARES emite o PSG SG numa taxa próxima ao clock dividido por 16 e aplica um passa-alta de 20 Hz no fluxo. Isso é uma referência concreta de pipeline, mas não prova que um passa-baixa fixo de 8–10 kHz seja a resposta correta do SG-1000. A faixa de 8–12 kHz encontrada em recomendações gerais deve ser avaliada como hipótese experimental, não incorporada como fato do hardware.

## Roadmap técnico

### Fase A — Referência e testes reproduzíveis (P0)

**Objetivo:** tornar as divergências mensuráveis antes de alterar o som.

- Criar vetores determinísticos para tom com períodos representativos, transições de registrador, ruído periódico, ruído branco, período 0/1 e alterações rápidas de volume.
- Registrar, por vetor, clock, variante, sequência de writes com timestamps, taxa de saída e estado de reset.
- Adicionar métricas de frequência medida, RMS, pico, componente DC, espectro e determinismo entre renderização integral e blocos.
- Fixar a origem de cada expectativa: documentação, código de referência independente ou captura de hardware. Não converter testes de paridade Python/Rust em alegação de fidelidade absoluta.
- Preservar capturas de áudio apenas quando legalmente obtidas, com metadados da cadeia de captura; não incluir ROMs comerciais.

**Aceitação:** os mesmos vetores produzem resultados reproduzíveis em Python e Rust; testes detectam diferenças de frequência, ganho, estado e divisão em blocos.

### Fase B — Reamostragem com controle de aliasing (P1)

**Objetivo:** reduzir o sinal de alta resolução para 44,1/48 kHz sem descartar transições sem filtragem.

- Definir uma representação de sinal fonte a cada tick interno do PSG, preservando a fase do divisor e os writes temporizados.
- Implementar um reamostrador anti-aliasing stateful (por exemplo, FIR/polyphase ou uma alternativa medida e documentada). Não usar interpolação linear como filtro anti-aliasing do sinal fonte.
- Manter estado do filtro entre frames e chamadas; o resultado deve ser independente de como o chamador divide os blocos.
- Testar a resposta de frequência do reamostrador com sinais sintéticos e medir atenuação de componentes acima do Nyquist de saída.
- Comparar custo CPU e latência antes de escolher número de taps, taxa interna e formato numérico. Medir também o caminho Rust real.

**Aceitação:** nenhuma alteração nos clocks lógicos; fase de áudio determinística; resposta de frequência documentada; aliasing acima da banda de saída atenuado por um limiar definido nos testes; ausência de descontinuidades artificiais nas fronteiras dos buffers.

### Fase C — Modelo de saída SG-1000 baseado em evidência (P1)

- Manter o sinal lógico do chip separado do modelo de saída.
- Investigar a resposta de decaimento DC descrita por SMS Power e os efeitos sobre tons, ruído e alterações de volume.
- Procurar esquemas, componentes e capturas de saída específicos do SG-1000/SG-1000 II; não extrapolar automaticamente circuitos do Master System.
- Comparar variantes: saída digital ideal, decaimento/DC e resposta de frequência experimental, sempre com parâmetros explícitos e estado persistente.
- Ajustar ganho apenas após comparar pico/RMS com referência de ganho alinhado.

**Aceitação:** cada parâmetro analógico tem fonte, medição ou rótulo explícito de hipótese; nenhum filtro é apresentado como universal para todos os consoles Sega.

### Fase D — Perfis digitais para hardware Sega compartilhado (P2)

Implementar **depois** de estabilizar e testar o perfil SG-1000:

| Perfil | Diferenças a modelar | Prioridade |
|---|---|---|
| SN76489AN discreto — SG-1000/SC-3000 | LFSR de 15 bits, taps 0/1, sem roteamento estéreo GG | Atual |
| PSG Sega integrado — Master System/Mark III | LFSR de 16 bits, taps 0/3; diferenças de revisão a investigar | Seguinte |
| PSG Game Gear | Perfil Sega integrado + registro estéreo em I/O e saída L/R | Após SMS |
| PSG Mega Drive/Genesis | PSG integrado, clock e mapa de escrita da plataforma, integração com mixer YM2612 | Após SMS/GG |

O perfil deve expressar diferenças confirmadas como dados/configuração: largura e taps do LFSR, comportamento de período zero, inversão/saída, clock/divisor, roteamento e regras de reset. Não criar uma abstração genérica de todas as famílias TI sem testes que demonstrem quais campos são necessários.

### Fase E — Validação diferencial e auditiva (P2)

- Comparar sequências digitais contra implementações independentes como MAME e ARES, respeitando as diferenças de variante.
- Usar VGM ou vetores sintéticos como estímulos controlados quando o formato e o perfil forem aplicáveis.
- Comparar capturas com mesma ROM/revisão, região, sequência de entradas, duração, taxa de captura e ganho; alinhar atraso antes de comparar.
- Relatar erro de frequência, RMS, pico, DC, espectro e diferenças temporais. Teste auditivo é complementar, não substitui métricas.
- Separar a validação de áudio do backend (underruns, buffering, volume do mixer) da validação do chip.

**Aceitação:** regressões automatizadas cobrem o modelo digital e o reamostrador; relatório de referência documenta método, versão da fonte e incertezas; validação física permanece explicitamente pendente quando não houver hardware/captura.

## Roadmap de lacunas do emulador

| Subsistema | Estado observado / lacuna | Próximo marco verificável |
|---|---|---|
| CPU Z80 | Núcleos Python/Rust e testes diferenciais existem; o estado atual de testes não demonstra todas as variantes, interrupções nem precisão ciclo a ciclo | Manter matriz de opcodes; testes de diagnóstico e ciclos; documentar cobertura e resultados por commit |
| Barramento e scheduler | T-states nominais e alguns writes temporizados; ainda não equivalem a execução completa por ciclo de barramento | Definir modelo explícito de eventos e acessos temporais antes de alegar precisão ciclo a ciclo |
| VDP SG-1000 | Implementações em evolução e testes de modos, raster/sprites; falta validação ampla contra padrões de hardware | Homebrew/test ROMs legais, testes diferenciais por modo, timing e snapshots comparáveis |
| PSG SG-1000 | Mistura digital implementada; amostragem/saída analógica não modeladas com fidelidade validada | Fases A–C deste documento |
| Frontend/áudio | Há caminhos de saída diferentes e adaptação de taxa; buffer e mixer podem afetar entrega, volume e latência | Medir o PCM antes do backend e o sinal entregue; testar underruns e configurações reais no Windows |
| Máquina e determinismo | Backend Rust integra mais do runtime, mas é necessário preservar paridade após alterações em dispositivos | Testes diferenciais completos, snapshots/saídas determinísticas e benchmarks comparáveis |
| Master System/Game Gear | Não tratar como sistemas prontos apenas por compartilharem Z80/PSG/VDP em parte | Só iniciar depois de perfil SG-1000 validado; implementar mapa de I/O, VDP, áudio e diferenças por plataforma |
| Mega Drive/Genesis | Requer 68000, Z80 auxiliar, YM2612, PSG, barramentos e sincronização entre processadores | Projeto de plataforma próprio após SG-1000/SMS; PSG compartilhado não substitui YM2612 ou scheduler |
| Sega CD/32X | Subsistemas adicionais e CPUs próprias | Não iniciar antes da plataforma Mega Drive e de seus contratos de sincronização estarem validados |
| Empacotamento e qualidade | CI e testes existem, mas CI não equivale a hardware real | Manter gates de lint/testes/build; relatórios de benchmarks e testes físicos separados |

## Política de desempenho

O relato do usuário de ganho superior a 20× ao mover o caminho de execução de Python para Rust é uma observação importante para orientar prioridades, mas deve ser acompanhado de benchmark reproduzível com workload, máquina, versão, aquecimento e backend identificados antes de ser usado como resultado geral do projeto.

Não adicionar JIT por antecipação. Priorizar o runtime Rust interpretado, profiling, precisão e testes diferenciais. Reavaliar JIT somente se benchmarks representativos mostrarem que o runtime nativo atual não atende às metas, e sem misturar a tradução de CPU com a lógica de áudio.

## Referências primárias e comparativas

1. SMS Power!, [SN76489](https://www.smspower.org/Development/SN76489?from=Development.PSG) — variantes, registros, LFSR, atenuação, decaimento DC e estéreo Game Gear.
2. MAME, [sn76496.cpp](https://github.com/mamedev/mame/blob/master/src/devices/sound/sn76496.cpp) — perfis e implementação comparativa das variantes TI/Sega; consultar a licença BSD-3-Clause antes de reutilizar qualquer código.
3. ARES, [PSG SG](https://github.com/ares-emulator/ares/blob/master/ares/sg/psg/psg.cpp) — taxa de stream e passa-alta de 20 Hz no fluxo SG.
4. ARES, [SN76489](https://github.com/ares-emulator/ares/blob/master/ares/component/audio/sn76489/sn76489.cpp) — estado digital do componente de referência.
5. Texas Instruments, [SN76489AN datasheet](https://map.grauw.nl/resources/sound/texas_instruments_sn76489an.pdf) — especificações do chip discreto.
6. VGM Specification, [VGM format](https://vgmrips.net/wiki/VGM_Specification) — metadados de clock e variantes do PSG para vetores de teste.
7. SMS Power!, [Game Gear Stereo](https://www.smspower.org/Development/GameGearStereo) — extensão de roteamento estéreo do Game Gear.

Referências de código são usadas como evidência comparativa e não como autorização para copiar código. Verificar versão, licença e atribuição antes de reutilizar qualquer implementação.
