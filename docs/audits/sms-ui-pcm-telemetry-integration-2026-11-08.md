# Registro de processo — UI imersiva e telemetria PCM SMS

**Data:** 2026-11-08
**Repositório:** https://github.com/leonardo201800478/sega-emulator
**PR de implementação:** [#662](https://github.com/leonardo201800478/sega-emulator/pull/662)
**Escopo:** reorganização da interface nativa egui/eframe e integração de telemetria da saída PCM.

> Este registro documenta decisões, mudanças e evidências da execução. Não declara validação física de áudio nem compatibilidade funcional de jogos além dos testes efetivamente executados.

## 1. Solicitação e decisão de layout

A solicitação foi reduzir a poluição visual da tela do emulador, concentrar as configurações em menus simples e ampliar substancialmente a área de execução do jogo.

Foram apresentadas três alternativas de layout. A decisão explícita foi pela **Opção 2 — imersiva**, incluindo os quatro recursos adicionais aprovados:

- tela cheia;
- atalhos de teclado;
- lista de ROMs recentes;
- status técnico opcional.

A implementação manteve a interface como camada de apresentação e despacho de comandos de alto nível. Nenhuma lógica de CPU, VDP, PSG, barramento ou temporização foi transferida para a GUI.

## 2. Resultado da reorganização da interface

### 2.1 Estrutura visual

A barra superior passou a concentrar quatro menus:

| Menu | Conteúdo |
|---|---|
| **Arquivo** | Abrir ROM, ROMs recentes, fechar ROM e sair |
| **Vídeo** | Proporções 4:3, 1:1 e 16:9 |
| **Som** | Áudio ativo, WASAPI Shared, XAudio2 e volume |
| **Opções** | Status, tela cheia, região, execução, controles e diagnóstico |

O viewport recebe a área central restante da janela. O rodapé técnico pode ser ocultado para liberar ainda mais espaço vertical.

A janela nativa inicia maximizada, com tamanho interno de referência de 1440×900 e mínimo de 1024×720.

### 2.2 Tela cheia e atalhos

- **F11:** alterna tela cheia.
- **F5:** iniciar execução.
- **F6:** pausar.
- **F7:** parar.
- **F8:** reset.
- **F9:** avançar um quadro.
- **Setas / Z / X:** controles de jogo.
- **P:** solicita NMI, preservando o comportamento anterior do atalho.

Os atalhos de execução enviam comandos coarse-grained ao runtime; não executam ciclos de hardware diretamente na interface.

### 2.3 ROMs recentes

A aplicação mantém uma lista limitada às dez ROMs carregadas com sucesso. A lista:

- persiste os caminhos dos arquivos, não cópias das ROMs;
- remove duplicatas e move a ROM reaberta para o início;
- ignora entradas cujo arquivo não existe mais;
- limita o histórico a dez entradas;
- usa o diretório de dados da aplicação em `APPDATA/Sega Emulator/recent_roms.txt`.

Foi adicionado teste unitário para ordenação, deduplicação e limite da lista. O carregamento pelo menu de recentes reutiliza o fluxo normal de abertura da ROM.

## 3. Telemetria PCM incluída na mesma integração

A PR #662 também carrega o incremento de observabilidade PCM documentado em [OUT-F — telemetria da saída PCM](../audio/out-f-output-telemetry.md).

O snapshot `AudioOutputMetrics` expõe, entre outros:

- taxa de amostragem configurada;
- capacidade, ocupação e high-water da FIFO em frames estéreo;
- frames aceitos, consumidos e descartados;
- underrun em eventos e frames;
- chamadas de escrita com descarte.

A instrumentação é propagada do sink para o runtime SMS e apresentada pela GUI. Os contadores compartilhados usam atomics; o snapshot é diagnóstico e não uma transação atômica.

**Limite importante:** `queue_duration_micros()` estima somente o tempo equivalente aos frames presentes na FIFO SPSC. Não mede buffers do endpoint, driver, mixer do sistema operacional, DAC ou latência ponta a ponta.

A instrumentação não deve alterar clocks, PSG, MachineScheduler, prebuffer ou parâmetros de buffers. A thread/callback de áudio não passa a executar lógica de GUI, logging, alocação ou controle do Scheduler.

## 4. Processo de engenharia executado

### 4.1 Auditoria e reconciliação da base

- Base consultada: `main` em `d6d14cbcc78dd5d3b39cabfe7d03a857c59034ed`.
- HEAD da PR antes da reconciliação: `19c96e83b5bdc83b2a38d1984e1daf6aa71939f8`.
- A comparação identificou a branch da PR um commit atrás da `main`: o commit documental #663, `d6d14cb`, descendia de `af24e77018257b77293a28e2f8e1ac12d0804aa4`, ancestral comum da branch.
- A primeira tentativa de merge foi recusada pelas regras do repositório, que exigiram dois status checks esperados. Nenhuma alteração foi forçada na `main`.
- A branch da PR foi reconciliada com a `main` por um commit de merge normal, preservando os dois pais:
  - primeiro pai: HEAD da PR `19c96e83b5bdc83b2a38d1984e1daf6aa71939f8`;
  - segundo pai: `main` `d6d14cbcc78dd5d3b39cabfe7d03a857c59034ed`;
  - commit de reconciliação: `9881ed769dd76bca01c39072f5354ff52bbf62d1`.
- A atualização da branch foi fast-forward, com verificação do SHA esperado; não houve force-push.

### 4.2 Implementação incremental e correções

As mudanças foram feitas em commits pequenos na branch da PR #662. Durante as primeiras execuções da CI, o `rustfmt` apontou diferenças de formatação em `native/sega_app/src/app.rs`. Os pontos foram corrigidos e a formatação passou na execução posterior.

A organização do código de menus também foi refinada: o estado de apresentação foi agrupado em uma estrutura de empréstimos e os renderizadores dos menus foram separados por responsabilidade, reduzindo a complexidade da função principal de renderização.

### 4.3 Documentação

Este registro foi acrescentado ao índice `docs/audits/README.md`. O documento de telemetria OUT-F continua sendo a referência para semântica das métricas e limites da observabilidade.

## 5. Evidências de CI — execução anterior à reconciliação documental

A execução **#5760**, run ID `42483663335`, foi concluída com sucesso no SHA `19c96e83b5bdc83b2a38d1984e1daf6aa71939f8`.

| Job / suíte | Resultado observado |
|---|---|
| Change impact classification | PASS |
| Documentation patch hygiene | PASS |
| Rust workspace regression | PASS |
| Rust quality and unit regression | PASS |
| Linux audio-output host validation | PASS |
| Windows WASAPI and XAudio2 validation | PASS |
| Windows x64 D3D12 compile — formatação egui | PASS |
| Windows x64 D3D12 compile — Clippy egui e dependências Windows | PASS |
| Windows SDL3 input validation | NOT RUN (SKIPPED) |
| Windows PowerShell tooling syntax | NOT RUN (SKIPPED) |
| Z80 focused validation | NOT RUN (SKIPPED) |
| Scheduler deterministic stress | NOT RUN (SKIPPED) |
| Refresh Cargo lockfiles (manual artifact) | NOT RUN (SKIPPED) |

No job Windows D3D12, as etapas específicas de Clippy e testes do runtime D3D12/DXGI foram SKIPPED; o job terminou PASS porque as etapas aplicáveis de descoberta do workspace, formatação e Clippy da aplicação nativa passaram. Não se deve interpretar o resultado do job como execução dos testes específicos D3D12/DXGI.

**Importante:** a execução #5760 valida o SHA indicado na tabela, não o commit de reconciliação `9881ed7` nem os commits documentais posteriores. A CI deve ser concluída novamente no HEAD final da PR antes do merge. SKIPPED é sempre NOT RUN, nunca PASS.

## 6. Validação física e limites conhecidos

A CI comprova os jobs automatizados listados; ela não substitui testes em hardware e endpoint de áudio reais.

Permanecem fora do que foi comprovado por esta execução:

- captura de loopback WASAPI/XAudio2 em gameplay real;
- medição independente da taxa efetiva de amostras;
- latência ponta a ponta do dispositivo;
- confirmação auditiva de que a qualidade sonora foi corrigida;
- validação de compatibilidade de jogos em hardware real;
- execução dos jobs explicitamente classificados como NOT RUN.

A telemetria permite coletar evidências sobre a FIFO, mas não identifica sozinha a causa de distorção, descompasso ou pitch percebido. A investigação de qualidade sonora permanece separada e não deve ser considerada encerrada por esta integração.

## 7. Resultado final da integração — PR #662

A PR #662 foi integrada com sucesso à `main`.

| Referência | SHA / resultado |
|---|---|
| Base `main` antes da reconciliação | `d6d14cbcc78dd5d3b39cabfe7d03a857c59034ed` |
| HEAD final validado da PR | `3a7169dd504d7f2e8705efeb1f724e3aad207862` |
| Workflow final | #5764 — run ID `42484250031` |
| Resultado do workflow final | SUCCESS |
| Commit de merge da PR #662 | `950df944adc2a5fcc6fa297a48dc72d97e861e70` |
| SHA confirmado da `main` após merge | `950df944adc2a5fcc6fa297a48dc72d97e861e70` |
| Estado remoto da PR #662 | CLOSED / MERGED |

A primeira tentativa de merge foi recusada por falta de dois status checks esperados. A branch foi reconciliada com a base atual sem force-push. Após a inclusão deste registro e a correção de trailing whitespace apontada pelo job de higiene documental, o workflow #5764 foi executado no HEAD final da PR e terminou SUCCESS. O merge foi então realizado com proteção de SHA esperado.

### Classificação dos checks do workflow final

| Job | Classificação |
|---|---|
| Change impact classification | PASS |
| Documentation patch hygiene | PASS |
| Rust workspace regression | PASS |
| Rust quality and unit regression | PASS |
| Linux audio-output host validation | PASS |
| Windows WASAPI and XAudio2 validation | PASS |
| Windows x64 D3D12 compile | PASS |
| Windows SDL3 input validation | NOT RUN (SKIPPED) |
| Windows PowerShell tooling syntax | NOT RUN (SKIPPED) |
| Scheduler deterministic stress | NOT RUN (SKIPPED) |
| Z80 focused validation | NOT RUN (SKIPPED) |
| Refresh Cargo lockfiles (manual artifact) | NOT RUN (SKIPPED) |

No job Windows D3D12, a formatação nativa e o Clippy da aplicação egui/dependências Windows passaram. As etapas específicas de Clippy e testes do runtime D3D12/DXGI foram SKIPPED; portanto, não são declaradas como executadas.

### Estado final e pendências separadas

- A reorganização da interface, a telemetria PCM e este registro de processo estão integrados à `main`.
- Os testes automatizados classificados como PASS acima correspondem ao HEAD final da PR antes do merge.
- Os jobs NOT RUN permanecem pendentes e não foram convertidos em PASS.
- A validação física de áudio (loopback, medição de taxa efetiva, latência ponta a ponta e confirmação auditiva) continua pendente.
- A causa raiz de eventuais problemas de qualidade sonora não foi declarada resolvida por esta integração.

**Conclusão:** integração de código e documentação concluída. A validação física e os jobs especializados classificados como NOT RUN permanecem fora do escopo comprovado desta entrega.
