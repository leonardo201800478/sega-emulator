# Auditoria dos workflows e plano de recuperação de validação

**Data da inspeção:** 2026-10-24  
**Repositório:** `leonardo201800478/sega-emulator`  
**Escopo:** workflows de CI e auditoria estendida associados às PRs #18, #19, #21, #23, #24, #25, #26 e #27. Este registro separa resultados observados de conclusões ainda não comprovadas.

## 1. Resumo executivo

- As execuções consultadas terminaram; isso não significa que todos os testes passaram.
- PRs #19 e #21 tiveram CI e auditoria estendida concluídos com sucesso nos commits inspecionados.
- PRs #23 a #27 tiveram falhas no passo `ruff format --check .`; nesse fluxo, os passos posteriores da job de qualidade foram ignorados. Portanto, os testes de pytest desses commits não foram validados por esses runs.
- A auditoria estendida da PR #18 chegou a executar a varredura prolongada e terminou com falha de robustez. O relatório final registrou 97.629.696 casos, 17.367.013 ocorrências `runtime_exception` e 799.950 ocorrências `unsupported_opcode`. Esses números são contagens de eventos, não necessariamente bugs únicos. A amostra retida e as categorias precisam ser examinadas antes de atribuir causa.
- O workflow da PR #18 também registrou falhas independentes de estilo: formatação em `tests/unit/test_z80.py` e no notebook diferencial; lint em `tools/audits/z80_long_stress_audit.py` e no notebook.
- A PR #27 contém somente documentação da auditoria de VRAM; não executou testes específicos da VRAM.

## 2. Execuções observadas

| PR | Escopo | Resultado observado | Interpretação |
|---|---|---|---|
| #18 | CI | Sucesso no commit `0176c8c63f6b6a3c81774c02987fd5be3303daa1` | O workflow de qualidade daquele commit terminou com sucesso; não equivale a aprovação da auditoria estendida |
| #18 | Auditoria estendida | Falha no run [40070113503](https://github.com/leonardo201800478/sega-emulator/actions/runs/40070113503) | A varredura prolongada terminou com ocorrências de robustez |
| #19 | CI + auditoria estendida | Ambos sucesso no commit `27c48911c2c15d13b12db3308799fd94ebdc866f` | Evidência positiva apenas para aquele snapshot e escopo |
| #21 | CI + auditoria estendida | Ambos sucesso no commit `a06ca5da632d4c035a00d3d82c2e2b8141e309f8` | Evidência positiva apenas para aquele snapshot e escopo |
| #23 | CI + auditoria estendida | Falha de formatação/lint | Os passos de qualidade bloquearam a validação completa |
| #24 | CI + auditoria estendida | Falha de formatação | Não considerar pytest validado nesses runs |
| #25 | CI + auditoria estendida | Falha de formatação | Não considerar pytest validado nesses runs |
| #26 | CI + auditoria estendida | Falha de formatação | Não considerar regressões de sprites aprovadas nesses runs |
| #27 | CI + auditoria estendida | Falha de formatação | PR documental; nenhuma validação de código foi executada |

Os resultados são históricos dos commits listados, não uma afirmação sobre o estado de commits posteriores.

## 3. Falhas de qualidade identificadas nos logs

### 3.1 Ruff format

O log da auditoria estendida da PR #18 identificou:
- `tests/unit/test_z80.py`: o arquivo seria reformado perto do teste `test_reset_clears_cpu_state_without_replacing_buses`;
- `z80-q-scf-ccf-mainline/tools/audits/z80_q_scf_ccf_differential_audit.ipynb`: células de inicialização, verificação de estilo e seleção do CSV de referência;
- em runs mais recentes das PRs #25–#27, a célula que define `reference_name` continuou sendo apontada.

### 3.2 Ruff check

O log da auditoria estendida da PR #18 reportou quatro erros:
- E501: linha longa no resumo final de `tools/audits/z80_long_stress_audit.py`;
- F401: import `json` não utilizado no notebook;
- E501: comentário longo na célula que executa os vetores SCF/CCF;
- E501: comentário longo na célula de upload do CSV.

A branch `fix/z80-audit-mainline` recebeu commits para normalizar o fim do arquivo de testes Z80 e quebrar a linha longa do relatório de stress. Essas alterações ainda precisam ser confirmadas por nova execução de CI; não presumir sucesso.

### 3.3 Ordem dos passos

Os workflows executam formatação e lint como passos separados. Quando um passo falha, os passos seguintes da mesma job de qualidade são ignorados. Na auditoria estendida, a geração do inventário e o upload de artefatos estão configurados para executar mesmo após falhas. Um artefato enviado, portanto, não significa que a auditoria tenha passado.

## 4. Auditoria prolongada Z80: resultado e limites

Run: [Z80 extended audit — PR #18](https://github.com/leonardo201800478/sega-emulator/actions/runs/40070113503)

Contagem final registrada no log:
- casos: `97,629,696`;
- tempo decorrido reportado: `20,700 s`;
- `executed`: `79,462,733`;
- `determinism_checks`: `79,462,733`;
- `runtime_exception`: `17,367,013`;
- `unsupported_opcode`: `799,950`.

A auditoria foi desenhada para registrar exceções e instruções não suportadas como achados. O contador fatal soma `invariant_violation`, `runtime_exception`, `invalid_cycle_count` e `determinism_divergence`. O log final declara falha porque o total fatal não foi zero.

**Não concluir ainda:** que todas as exceções são defeitos de emulação; que os 799.950 eventos representam opcodes únicos; ou que o teste provou equivalência com hardware. O script é uma auditoria de robustez/invariantes e não um oracle independente do Z80.

### Próxima análise obrigatória

1. Obter o JSON do artefato `z80-extended-audit-40070113503`.
2. Agrupar as amostras retidas por `status`, `error_type`, opcode/prefixo e traceback.
3. Determinar se as exceções derivam de opcodes não suportados, estados artificiais/aleatórios, defeitos de execução ou falhas do próprio harness.
4. Criar testes mínimos determinísticos para cada defeito confirmado.
5. Manter `unsupported_opcode` como lacuna de cobertura separada, sem confundi-la com exceções inesperadas.

## 5. Correções iniciadas

- Branch `fix/workflow-audit-validation`: corrigir formatação e problemas de lint identificados no notebook diferencial, sem alterar a semântica do teste.
- Branch `fix/z80-audit-mainline`: normalizar formatação do arquivo de testes e corrigir a linha longa no script de stress.
- As correções devem ser validadas por novos runs de CI; a criação dos commits não é evidência de que os checks passaram.

## 6. Plano para recuperar a validação

### Fase 1 — restaurar CI básico
- Corrigir formatação e lint no notebook, testes e script de stress.
- Confirmar que `ruff format --check .` e `ruff check .` passam.
- Executar `pytest` até o fim; registrar separadamente testes aprovados, falhas e não executados.

### Fase 2 — entender os achados do stress test
- Baixar e inspecionar o artefato do run #18.
- Reduzir cada achado real a um caso mínimo reproduzível.
- Corrigir apenas defeitos confirmados e adicionar regressões.
- Reexecutar a auditoria longa depois que os testes curtos passarem.

### Fase 3 — fechar lacunas do Z80
- Separar cobertura de opcodes, robustez, determinismo, flags, ciclos e equivalência diferencial.
- Usar uma referência independente para os testes diferenciais; o próprio emulador não pode ser seu único oracle.
- Documentar as limitações remanescentes sem declarar precisão por ciclo sem evidência.

### Fase 4 — validar VDP/VRAM separadamente
- Adicionar testes curtos para protocolo de portas, read-ahead, incremento e wrap de endereço.
- Testar endereçamento de tabelas, modos gráficos e atributos/padrões de sprites.
- Investigar sincronização CPU–VDP e temporização por scanline após estabilizar os testes funcionais.
- Medir desempenho antes de otimizar o caminho de renderização.

## 7. Critério de encerramento

Uma lacuna só será considerada resolvida quando:
1. houver um teste reproduzível que cubra o comportamento;
2. a expectativa estiver fundamentada em documentação técnica ou referência independente;
3. o teste for executado e passar no commit candidato;
4. CI e auditoria aplicáveis terminarem com resultado confirmado;
5. a documentação refletir o resultado real, inclusive limitações ainda abertas.

## 8. Referências

- [PR #18 — Z80 audit fixes](https://github.com/leonardo201800478/sega-emulator/pull/18)
- [PR #19 — base de VRAM e interface VDP](https://github.com/leonardo201800478/sega-emulator/pull/19)
- [PR #21 — integração executável](https://github.com/leonardo201800478/sega-emulator/pull/21)
- [PR #23 — harness diferencial Q/SCF/CCF](https://github.com/leonardo201800478/sega-emulator/pull/23)
- [PR #24 — opcodes ED não atribuídos](https://github.com/leonardo201800478/sega-emulator/pull/24)
- [PR #25 — fundação PSG](https://github.com/leonardo201800478/sega-emulator/pull/25)
- [PR #26 — regressões de sprites VDP](https://github.com/leonardo201800478/sega-emulator/pull/26)
- [PR #27 — auditoria VRAM/VDP](https://github.com/leonardo201800478/sega-emulator/pull/27)
- [Run prolongado #40070113503](https://github.com/leonardo201800478/sega-emulator/actions/runs/40070113503)
