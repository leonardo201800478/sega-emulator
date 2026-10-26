# SG-1000 — marco de qualidade de áudio e vídeo

**Data do marco:** 2026-10-26  
**Escopo:** SG-1000 no frontend Qt/PySide6, com o runtime integrado disponível no projeto.

## Resumo

Após teste manual do desenvolvedor na versão atualizada, a qualidade percebida de áudio e vídeo foi considerada satisfatória para marcar a primeira etapa do SG-1000. O áudio foi descrito como muito próximo ao Gearsystem, que serve como referência comparativa de estudo. Este resultado é um marco de progresso, não uma certificação de equivalência absoluta com hardware original.

## Estado observado

- **Áudio:** PSG SN76489AN integrado; saída PCM mono no frontend Qt via WASAPI compartilhado, usando PortAudio/sounddevice. O som foi considerado muito próximo ao Gearsystem em teste manual.
- **Vídeo:** apresentação do framebuffer pela cena Qt Quick, com Vulkan solicitado e fallback raster disponível. O backend efetivamente ativo deve ser confirmado no log da máquina de teste.
- **Integração:** a aplicação conecta runtime da máquina, entrada, vídeo e áudio. O crate standalone `native/sg1000_core` continua parcial e não deve ser descrito como um runtime completo independente.
- **Experiência prolongada:** foram observados pequenos engasgos e possível descompasso que parecem se tornar mais perceptíveis durante sessões longas. A causa ainda não foi isolada.
- **CPU/temporização:** a execução usa T-states nominais por instrução e pacing de frame; isso não constitui uma implementação completa ciclo a ciclo de barramento.

## Limites da evidência

A comparação com Gearsystem é uma observação prática do desenvolvedor. Ela não demonstra por si só:
- equivalência de amostras PCM com hardware original;
- equivalência de temporização de barramento ou CPU ciclo a ciclo;
- compatibilidade com todas as ROMs e revisões de hardware;
- estabilidade de sincronismo durante sessões prolongadas;
- inicialização de Vulkan em toda combinação de GPU, driver e Windows.

Ao publicar testes futuros, registrar versão/commit, ROM identificada por SHA-256 (sem distribuir ROM comercial), backend de vídeo efetivo, dispositivo/taxa de áudio, duração da sessão e evidências observáveis. Usar ROMs homebrew ou materiais legalmente obtidos.

## Plano de validação para estabilidade

1. Instrumentar a fila PCM com ocupação mínima/máxima/média, underruns, overflows e blocos descartados.
2. Registrar tempos entre frames, frames atrasados e duração do trabalho de emulação/apresentação.
3. Repetir a mesma sequência de teste por 15–30 minutos e comparar início, meio e fim.
4. Determinar se a primeira divergência surge no scheduler, na produção de amostras, no callback de áudio ou na apresentação de vídeo.
5. Corrigir o componente identificado com teste de regressão; evitar aumentar buffers ou adicionar threads sem evidência.
6. Repetir testes funcionais, determinismo e integração no mesmo SHA candidato.

## Interface Qt

O modo de tela cheia imersivo é destinado a exibir somente a área do jogo. Deve ocultar a barra de menus, a barra de ações, a barra de status e as margens externas da área de vídeo. F11 e Alt+Enter alternam o modo; Esc retorna à janela normal. O restante da interface continua acessível fora do modo imersivo.

## Critério para o primeiro release público

O marco de qualidade justifica preparar um primeiro release público versionado, desde que antes:
- a licença do projeto seja escolhida explicitamente e incluída no repositório;
- a caixa **Sobre** do Qt mostre a versão, o escopo do SG-1000, os créditos e a licença correta;
- os workflows aplicáveis passem no SHA exato candidato;
- a documentação e o changelog distingam comportamento observado, testes automatizados e limitações;
- o release seja identificado como inicial/experimental, sem alegar precisão ciclo a ciclo nem validação universal.

## Estado após a preparação da PR #113

- A licença MIT foi adicionada em LICENSE e declarada nos metadados do pacote.
- O diálogo Sobre do Qt foi atualizado com versão, escopo SG-1000, créditos tecnológicos, licença e aviso de projeto independente.
- A tela cheia imersiva e um teste de UI foram adicionados na PR #113.
- Os testes e workflows desta revisão ainda precisam ser executados no SHA candidato. Não há, neste documento, declaração de aprovação dos checks.

## Próximos passos

1. Executar lint, formatação, suíte Python e os workflows aplicáveis no SHA final da PR.
2. Validar manualmente o diálogo Sobre e o modo imersivo no Windows, incluindo saída com Esc e restauração da janela.
3. Reproduzir e instrumentar os engasgos em sessões longas.
4. Publicar o release inicial depois que os checks exigidos forem aprovados e os limites forem registrados nas notas.
