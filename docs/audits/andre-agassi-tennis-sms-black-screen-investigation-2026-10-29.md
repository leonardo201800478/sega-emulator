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
