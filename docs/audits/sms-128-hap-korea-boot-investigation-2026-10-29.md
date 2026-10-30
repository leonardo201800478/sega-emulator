# Investigação: boot de 128 Hap (Korea) (Unl) no SMS

**Data:** 2026-10-29  
**Branch-base observada:** `main` em `b4a85c82b1d975a126f1dcbd3084dac51249b398`  
**Status:** INVESTIGATION_PENDING — nenhuma correção de runtime proposta nesta fatia.

## Sintoma reportado

- ROM identificada pelo usuário como `128 Hap (Korea) (Unl)`.
- O jogo inicia no GearSystem.
- O jogo não dá boot no Sega Emulator.
- O jogo também não dá boot no Ares.

A falha compartilhada pelo Sega Emulator e pelo Ares torna prioritário verificar características do dump e o caminho de inicialização/cartucho, sem presumir que o defeito seja necessariamente do CPU, do VDP ou do mapeador. O funcionamento no GearSystem é um sinal comparativo útil, mas não prova por si só qual comportamento está correto.

## Objetivo

Determinar a primeira divergência reproduzível entre o carregamento da ROM, o mapeamento do cartucho e a execução inicial da CPU, usando evidência independente e sem incluir ROM comercial ou dump protegido no repositório.

## Hipóteses a distinguir (não confirmadas)

1. **Formato/dump da ROM:** tamanho real, cabeçalho ou bytes de preenchimento, checksum, imagem overdumped/underdumped, banco duplicado ou dados iniciais diferentes entre os arquivos usados.
2. **Detecção do hardware e mapeador:** ROM coreana não licenciada pode usar uma disposição de bancos ou lógica de cartucho que não é identificada pelo caminho padrão. Não assumir que toda ROM de 128 KiB usa apenas o Sega mapper padrão sem confirmar o comportamento.
3. **Inicialização do cartucho:** estado inicial dos registradores de banco, RAM de cartucho, espelhamento e leitura dos endereços de reset podem divergir.
4. **Reset e execução inicial do Z80:** PC/SP/flags, leitura do vetor de reset, fetch inicial, interrupções ou instruções executadas antes de qualquer acesso de vídeo.
5. **Configuração do sistema:** região NTSC/PAL, modelo SMS/compatibilidade de console e estado inicial dos dispositivos.
6. **Diferença de tolerância/heurísticas entre emuladores:** GearSystem pode reconhecer ou acomodar uma variante que o Sega Emulator e o Ares não tratam da mesma forma; isso precisa ser verificado no código e nos logs, não presumido.

## Plano de investigação

### 1. Fixar a identidade do artefato local

Sem versionar nem publicar a ROM, registrar localmente:
- tamanho exato em bytes e SHA-256;
- se há cabeçalho/padding ou tamanho não múltiplo da geometria esperada;
- checksum interno, quando aplicável, e posição do cabeçalho Sega se existir;
- comparação dos primeiros bytes e das regiões de bancos, sem anexar o conteúdo da ROM a logs públicos;
- se a mesma cópia exata foi usada nos três emuladores.

Se o usuário não puder compartilhar o dump, metadados e pequenos resultados calculados localmente bastam para começar. Não solicitar upload público da ROM.

### 2. Rastrear o caminho de carregamento

Auditar no core SMS:
- validação do tamanho e normalização da imagem;
- identificação do mapper e fallback usado para ROMs sem assinatura reconhecida;
- mapeamento de `0000–BFFF` no reset e nas janelas de bancos;
- comportamento das escritas de controle do mapper e eventuais RAM/enable bits;
- acessos a `FFFC–FFFF` e a portas de I/O, sem assumir que sejam todos registradores de mapper;
- preservação de compatibilidade com ROMs SMS já validadas.

Comparar com os caminhos equivalentes no GearSystem e no Ares para entender diferenças sem copiar código. Distinguir explicitamente comportamento documentado, comportamento observado e hipótese.

### 3. Encontrar a primeira divergência

Executar com a mesma ROM local e a mesma região/configuração:
- registrar o estado pós-reset e os primeiros N passos/instruções do Z80;
- registrar PC, opcode/bytes lidos, registradores essenciais e endereço físico/banco de ROM que atende cada fetch;
- registrar escritas de mapper e leituras/escritas de memória relevantes;
- determinar se a CPU fica presa em loop, executa dados como código, salta para endereço inválido ou chega a inicializar o VDP;
- comparar com GearSystem, quando for possível obter logs equivalentes, e com Ares; se não houver tracing equivalente, usar checkpoints observáveis e inspeção de código.

Evitar dumps completos de ROM, snapshots com conteúdo da ROM e logs públicos que revelem dados protegidos.

### 4. Regressão independente

Criar um teste sintético/licenciado que represente a característica técnica isolada descoberta (por exemplo, janela de bancos, valor inicial do mapper ou imagem com cabeçalho/padding), sem incorporar a ROM do jogo. O teste deve verificar:
- bytes visíveis em cada janela antes/depois das escritas do mapper;
- fetch no vetor de reset e sequência curta determinística;
- ausência de regressões nas configurações padrão e nos títulos de referência existentes.

Só adicionar uma regressão derivada do jogo se a fixture for legalmente redistribuível e a procedência/licença estiver confirmada.

## Critérios de aceitação

- [ ] Registrar tamanho e SHA-256 da cópia testada localmente, sem publicar a ROM.
- [ ] Confirmar que os três emuladores testaram exatamente o mesmo artefato e configuração relevante.
- [ ] Identificar o primeiro endereço/etapa em que o caminho do Sega Emulator diverge do comportamento esperado.
- [ ] Confirmar por evidência se a causa está no parser/detecção de mapper, no mapeamento, no reset/Z80, ou em outra camada.
- [ ] Implementar uma correção pequena e coesa em PR/fatia separada, apenas depois de identificar a causa.
- [ ] Adicionar teste determinístico independente que falhe antes e passe depois da correção.
- [ ] Executar os testes direcionados e a suíte pertinente no SHA candidato.
- [ ] Validar o boot manualmente no Windows com o mesmo arquivo e registrar o resultado como evidência separada.

## Validação e status

- **IMPLEMENTED:** apenas esta especificação de investigação documental.
- **QUICK_PASS:** NOT RUN.
- **SHORT_PASS:** NOT RUN.
- **LONG_PASS:** NOT RUN.
- **INTEGRATION_PENDING:** SIM — precisa de tracing e confirmação com o artefato local.
- **TEMPORAL_PASS:** não aplicável nesta fase; esta investigação não demonstra precisão temporal.
- **Boot manual no emulador:** PENDING; não executado nesta sessão.
- **CI:** será verificado no SHA da PR após sua criação; não considerar checks ausentes ou pendentes como aprovação.

## Limites

- Não modificar `main` diretamente.
- Não introduzir workaround específico para o nome do jogo.
- Não inferir o mapper somente pelo nome ou tamanho nominal da ROM.
- Não declarar que o Ares está correto/errado sem localizar a divergência.
- Não adicionar ROMs comerciais, BIOS protegidas ou logs que exponham conteúdo da ROM ao Git.

## Fontes técnicas a consultar

- Código do Sega Emulator no SHA-base registrado acima, especialmente carregamento de ROM, mapper, bus/memória e estado inicial do SMS/Z80.
- GearSystem: https://github.com/drhelius/Gearsystem
- Ares: https://github.com/ares-emulator/ares
- Documentação técnica do Sega Master System e especificações do Z80, usadas para confirmar os contratos de reset, mapeamento e acessos de barramento.

As referências a emuladores externos são pontos de investigação, não oráculos automaticamente independentes nem autorização para copiar implementação ou testes.
