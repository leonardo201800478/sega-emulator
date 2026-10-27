> **Estado vigente (2026-10-27):** runtime SG-1000 ativo em Rust. O hardware Python antigo e os testes/ferramentas que dependiam dele estão arquivados em `legacy/python_reference/` e não devem ser executados/importados. Este documento preserva observações históricas; comandos apontando para os caminhos arquivados não são comandos ativos.

# Revisão da matriz de opcodes Z80

**Data:** 2026-10-24  
**Branch:** `audit/z80-test-infrastructure`  
**Escopo:** classificação arquitetural da matriz; sem alteração do executor da CPU.

## Resultado desta revisão

A comparação da matriz com o modelo de instruções documentadas no manual Zilog identificou uma lacuna na classificação de comportamentos não documentados: algumas formas DD/FD que usam IXH/IXL ou IYH/IYL dentro das famílias `LD r,r'` e ALU eram rotuladas apenas como substituições indexadas comuns.

A matriz agora classifica essas formas como `undocumented-index-half`, incluindo:

- `DD/FD 44`: LD B,IXH/IYH;
- `DD/FD 65`: LD IXH/IYH,IXL/IYL;
- `DD/FD 84`: ADD A,IXH/IYH;
- as demais combinações correspondentes das famílias `LD` e ALU que usam metades do registrador de índice.

As formas que usam memória indexada continuam separadas. Por exemplo, quando o outro operando é `(IX/IY+d)`, as codificações especiais `LD H/L,(IX/IY+d)` e `LD (IX/IY+d),H/L` não devem ser classificadas como operações com IXH/IXL ou IYH/IYL. Essa distinção evita uma classificação incorreta por simples substituição textual de H/L.

## Evidência e limites

Os manuais Zilog fornecidos para esta auditoria descrevem a arquitetura e o conjunto de instruções documentadas. Eles não bastam, isoladamente, para comprovar o comportamento de opcodes não documentados em todas as revisões NMOS/CMOS ou clones. Por isso, a matriz marca esses casos como não documentados, sem alegar equivalência universal de hardware.

Esta é uma correção da **classificação da matriz**, não uma correção comprovada do executor Z80. A implementação real deve ser validada em etapa separada com vetores diferenciais apropriados.

## Testes de regressão adicionados

`legacy/python_reference/tests/unit/test_z80_opcode_matrix.py` agora verifica explicitamente as classificações de `0x24`, `0x44`, `0x65` e `0x84`, além de manter a distinção entre substituição indexada de memória e prefixo DD/FD ignorado.

Comandos para reprodução:

```powershell
python -m pytest -q legacy/python_reference/tests/unit/test_z80_opcode_matrix.py
python tools/z80_opcode_matrix.py --json
```

O resultado do GitHub Actions deve ser consultado após o push; esta nota não declara testes aprovados antes de o workflow terminar.
