# SMS — seleção automática de região para jogos europeus

Data: 2026-10-29  
Estado: **INVESTIGATION_PENDING**  
Runtime alterado nesta fatia: **não**  
ROMs comerciais no repositório: **não**

## Sintoma relatado

O usuário relata que o emulador não muda automaticamente para a região europeia ao carregar jogos europeus de Master System. O problema pode afetar inicialização e comportamento de jogos identificados como Europe/Brazil, incluindo casos já reportados, mas não se presume que todos tenham a mesma causa.

## Comportamento atual observado no código/documentação disponível

A PR #258 adicionou seleção persistente de região com modos Automatic, USA, Europe e Japan. O fallback automático descrito nela tenta perfis na ordem **USA → Europe → Japan**, mas avança ao próximo perfil somente se o primeiro `run_frame` falhar por não concluir um quadro dentro do limite de instruções. Isso é um watchdog de progresso, não detecção semântica da região da ROM. Uma ROM que continua executando, mas apresenta tela preta, áudio anormal ou comportamento incorreto, pode não acionar o fallback.

Portanto, “Automatic” atualmente não deve ser tratado como garantia de que uma ROM europeia será reconhecida e executada com perfil Europe/PAL. É necessário auditar o código efetivo e confirmar se a implementação presente em `main` corresponde integralmente à descrição da PR #258.

## Objetivo

Definir e implementar uma política confiável e determinística para que o modo Automatic selecione o perfil apropriado para a imagem de cartucho SMS, incluindo território Export e padrão de vídeo PAL quando sustentado por metadados/evidências, sem confundir:
- região/território do console;
- padrão de vídeo PAL/NTSC;
- região indicada nos metadados do cartucho;
- progresso de execução do Z80;
- preferência explícita do usuário.

## Hipóteses e limites

1. **Automatic usa watchdog em vez de identificação:** fallback por falha de progresso não detecta um jogo que executa incorretamente.
2. **Metadados não são lidos ou não estão disponíveis:** o dump pode não conter um cabeçalho reconhecido ou os metadados podem não ser confiáveis.
3. **Região e vídeo estão acoplados incorretamente:** selecionar Europe deve resultar no perfil de território e vídeo definidos pelo contrato do projeto, mas estes parâmetros continuam conceitos distintos.
4. **Perfil correto não garante boot:** mapper, reset/Z80, VDP e PSG podem ser a verdadeira causa em alguns títulos.
5. **Forçar Europe pelo nome do arquivo não é aceitável:** nomes são editáveis e não provam conteúdo ou hardware esperado.

## Política proposta para avaliação

- **Europe/USA/Japan explícitos:** respeitar a escolha do usuário; não sobrescrever com detecção automática.
- **Automatic:** tentar primeiro inferir o perfil por metadados válidos e verificáveis do cartucho, quando a especificação técnica e a implementação suportarem essa inferência. Registrar a fonte e a confiança da decisão.
- **Metadados ausentes/ambíguos:** aplicar um fallback documentado e determinístico; não alegar detecção de região. O fallback atual USA → Europe → Japan deve ser reavaliado com testes e dados, não alterado arbitrariamente.
- **Detecção pós-boot:** não inferir região apenas a partir de tela preta, áudio ou ausência de frame; esses sinais podem indicar outras falhas.
- **Pacing e core:** aplicar o perfil completo de forma consistente ao inicializar/reinicializar a máquina, sem misturar o pacing do host com o estado temporal do hardware.
- **Diagnóstico:** expor perfil solicitado, perfil selecionado, motivo da seleção e fallback realizado.

## Plano de auditoria/implementação

1. Inspecionar o código de `main` na interface Qt, política Python, fronteira PyO3 e construtor Rust para estabelecer o comportamento real.
2. Identificar o formato e a semântica dos metadados de região presentes nos dumps SMS; documentar fonte técnica confiável e limitações.
3. Implementar um resolvedor puro e testável: preferência explícita > metadados válidos em Automatic > fallback documentado.
4. Garantir que o resolvedor forneça território e padrão de vídeo separadamente e que o perfil chegue intacto ao core Rust.
5. Adicionar testes unitários para metadados válidos, ausentes, inválidos, conflitantes, preferência explícita e reinicialização ao trocar ROM.
6. Regressar casos relatados, incluindo Ace of Aces (Europe, Brazil) e outros títulos europeus, usando os dumps apenas localmente; não incluir ROMs.
7. Executar validações Rust/Python, build Release/Maturin e validar boot no Windows.

## Critérios de aceitação

- [ ] O comportamento atual de Automatic é confirmado por inspeção e teste.
- [ ] A origem da decisão de região é explícita e diagnosticável.
- [ ] Metadados válidos determinam o perfil automático somente conforme especificação comprovada.
- [ ] Preferências explícitas USA/Europe/Japan nunca são sobrescritas por Automatic.
- [ ] Metadados ausentes/ambíguos seguem fallback determinístico testado.
- [ ] Região/território e PAL/NTSC continuam separados na API e no core.
- [ ] Testes cobrem detecção, fallback, persistência, carregamento de nova ROM e reset.
- [ ] Testes pertinentes Rust/Python, Clippy/Ruff, build Release/Maturin executados no SHA candidato.
- [ ] Boot manual de títulos europeus no Windows confirmado e regressão SG-1000 preservada.

## Estado de validação

- **IMPLEMENTED:** especificação inicial e critérios documentados nesta PR.
- **QUICK_PASS:** NOT RUN.
- **SHORT_PASS:** NOT RUN.
- **LONG_PASS:** NOT RUN.
- **INTEGRATION_PENDING:** SIM.
- **Boot manual:** PENDING.
- **CI:** verificar status do SHA candidato; ausência de status não significa aprovação.
- **TEMPORAL_PASS:** NOT ACHIEVED; esta mudança não pretende fechar a precisão temporal ciclo a ciclo.

## Referências internas

- PR #258 — Add SMS region selection and PAL-aware Qt pacing: https://github.com/leonardo201800478/sega-emulator/pull/258
- PR #263 — Investigate Ace of Aces (Europe, Brazil) SMS boot failure: https://github.com/leonardo201800478/sega-emulator/pull/263

Esta PR inicial estabelece o contrato e os critérios; não afirma que a detecção automática já foi corrigida.
