# Input: perfis por periférico, XInput e regressões observadas

**Status:** PROPOSAL / INTEGRATION_PENDING  
**Issue:** #335  
**Base auditada:** `main` em `399034d65171a9465e044694c04f63ca9e9ee51a`  
**Runtime implementado nesta proposta:** nenhum. Este documento registra escopo e critérios; não declara correções concluídas.

## 1. Histórico e estado real

- PR #300 foi encerrada como proposta supersedida.
- PR #313 integrou o backend nativo SDL3 e a ponte Qt/PyO3.
- PR #131 descreveu seleção de driver incluindo XInput, mas os documentos atuais do runtime descrevem SDL3 como backend Windows implementado e GameInput como futuro. A existência de uma opção histórica de UI não prova a existência de um backend XInput direto ativo.
- `docs/architecture/native-input-runtime.md` descreve identidade de sessão e chave física opaca comum planejada para deduplicação entre backends, enquanto a integração atual ainda limita a atribuição a um dispositivo por jogador.
- `docs/architecture/controller-input-backends.md` descreve SDL3 como caminho físico ativo.

Referências: PRs #131, #300, #313; issue #309; documentos `native-input-runtime.md` e `controller-input-backends.md`.

## 2. Perfis persistentes por dispositivo

### Problema observado

Os perfis persistidos estão organizados por jogador/slot. Ao trocar o periférico atribuído ao jogador, o mapeamento anterior pode continuar aplicado ao novo dispositivo. A identidade SDL de sessão é temporária e não pode ser usada como chave persistente.

### Contrato desejado

- Identidade persistente por periférico, separada do ID efêmero de sessão.
- Preferir identificadores estáveis disponibilizados pelo backend: GUID, VID/PID, serial quando disponível e caminho/identificador físico opaco quando confiável.
- Nome amigável apenas para apresentação. Dois dispositivos idênticos devem poder manter perfis independentes quando houver identificador estável que os diferencie.
- Perfil físico acompanha o dispositivo; roteamento para Player 1/2 é configuração independente.
- Reconexão restaura o perfil correspondente; periférico desconhecido recebe perfil padrão.
- Esquema JSON versionado, migração da configuração legada por jogador e testes para preservar dados.
- Quando a plataforma não fornecer identidade estável suficiente, expor a ambiguidade em vez de prometer associação física determinística.

## 3. Backend XInput direto

A lacuna deve ser confirmada por inspeção do código, não inferida apenas pela existência de uma opção de driver na interface.

### Trabalho requerido

1. Auditar seleção de driver, carregamento de bibliotecas, enumeração e polling no runtime atual.
2. Se não houver backend XInput direto, implementar um backend Windows em Rust atrás dos contratos host-independent de `sega-input-runtime`.
3. Definir seleção explícita/automática e política de fallback com SDL3.
4. Deduplicar o mesmo controle quando exposto por mais de um backend, usando identidade física opaca confiável.
5. Testar conexão, desconexão, reconexão, quatro slots XInput, estado de botões, sticks e triggers.
6. Documentar que XInput não substitui HID/SDL3 para volantes, joysticks genéricos, sensores, hats ou periféricos avançados.

Nenhum backend XInput é declarado implementado por este documento.

## 4. Captura analógica do mapper

### Problema observado

A captura analógica foi relatada como excessivamente sensível, aceitando desvios/pequenos movimentos. Não se deve resolver isso aumentando a deadzone global aplicada ao gameplay.

### Critérios

- Threshold de captura configurável e independente da deadzone do perfil.
- Eixos centrados: ignorar repouso e ruído; aceitar apenas excursão intencional.
- Triggers unipolares: detectar a direção/faixa corretamente, sem exigir cruzamento de zero.
- Captura com histerese/estabilidade temporal se necessária, sem polling por instrução e sem introduzir latência no caminho normal de gameplay.
- Testes para repouso, ruído, excursão gradual, excursão intencional, retorno ao centro e triggers.
- A UX deve permitir iniciar captura antes/depois de clicar em Gravar sem exigir que o usuário mantenha previamente o botão pressionado.

## 5. Pause SMS/NMI e Road Rash

### Sintoma reportado

Em Road Rash (SMS), o letreiro de introdução que deveria deslizar da direita para a esquerda não progride visualmente, embora a música continue. Pressionamentos repetidos de Pause fazem a animação avançar em passos até chegar ao menu; depois o jogo fica jogável.

### Hipóteses a testar, não conclusões

- Caminho de entrada Pause/console_pause e geração de NMI por borda.
- Diferença entre Pause físico/tecla configurada e F5, que pausa o timer do frontend.
- Avanço de frames e coordenação frontend/runtime.
- Timing do VDP/scanline e atualização de sprites/VRAM.
- Divergência de execução do jogo ou de renderização.

Não há evidência suficiente para atribuir o sintoma a opcode Z80 inválido, corrupção de sprites ou posição de memória específica.

### Plano de diagnóstico

1. Registrar exatamente qual entrada foi acionada: Pause físico/tecla P (NMI) ou F5 (pausa do frontend).
2. Instrumentar transições de pressionado/liberado e número de solicitações NMI sem adicionar callbacks por instrução.
3. Criar replay determinístico dos mesmos inputs e registrar checkpoints de CPU, VDP, VRAM/OAM equivalente e framebuffer em fronteiras definidas.
4. Comparar execução sem Pause, com um único pulso e com pulsos repetidos.
5. Reproduzir com oráculo independente (ARES ou referência equivalente) usando a mesma ROM e sequência de entrada, sem versionar ROM comercial.
6. Corrigir somente a primeira divergência demonstrada e adicionar regressão automatizada.

Issue #309 continua sendo referência para o rework da ação dedicada `console_pause` e sua integração NMI.

## 6. Validação e gates

### Testes direcionados

- Perfis: identidade, dispositivos com nomes iguais, persistência, migração, troca de jogador, hotplug e reconexão.
- Backend: SDL3 e XInput, seleção/fallback, deduplicação e dispositivos ausentes.
- Mapper: eixos centrados, ruído, triggers e captura intencional.
- SMS: NMI edge-triggered, liberação, repetição e distinção entre Pause/NMI e F5.
- Road Rash: replay e checkpoints comparativos. O teste automatizado não deve depender de ROM comercial versionada.

### Gates obrigatórios no SHA candidato

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `python -m pytest`
- `python -m ruff check .`
- `python -m ruff format --check .`

Além da CI, exigir validação física no Windows para SDL3/XInput, troca de controles, hotplug e captura analógica. Registrar modelos, firmware, transporte USB/Bluetooth e resultados. Não inferir latência button-to-photon a partir de responsividade subjetiva.

## 7. Ordem de execução proposta

1. Auditar e documentar a implementação real da seleção XInput; reproduzir o comportamento atual.
2. Corrigir modelo de identidade e persistência de perfis por dispositivo, com migração.
3. Separar threshold de captura analógica da deadzone de gameplay.
4. Implementar/ativar backend XInput direto somente se a auditoria confirmar ausência, com deduplicação SDL3.
5. Isolar e corrigir Pause/NMI com testes específicos.
6. Reproduzir e diagnosticar Road Rash; não misturar uma correção de timing especulativa com as mudanças de input.
7. Executar todos os gates e a matriz física; manter a PR Draft até as evidências necessárias.

## 8. Restrições arquiteturais

- Captura física, backends, normalização, identidade e roteamento pertencem ao runtime Rust.
- Qt/Python permanecem interface, configuração, diagnóstico e tooling.
- Não introduzir callbacks Python por instrução, polling por instrução, JIT, frame skip ou hacks de timing.
- Preservar SG-1000 e separar os adaptadores específicos de SG-1000/SMS.
- Não versionar ROMs comerciais, BIOS proprietárias ou dumps protegidos.
- Estados de validação devem ser explícitos: PASS, FAIL, BLOCKED ou NOT RUN. Não declarar TEMPORAL_PASS sem evidência temporal independente.
