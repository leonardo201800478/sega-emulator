# SingleStepTests/Z80 — vetores externos DD/FD ALU

**Status:** PR #691; validação externa pendente no HEAD desta branch.
**Sega Emulator baseline:** `95dacafb083847991b7fbc59bcfa099e8f297685`.
**Upstream:** [SingleStepTests/z80](https://github.com/SingleStepTests/z80).
**Revisão fixada:** `ebe1875d48f374bcfd4b505d8eb8ee751568b5f7` (10/04/2026).
**Licença:** MIT — Copyright (c) 2024 SingleStepTests. O workflow faz checkout do `LICENSE` original junto aos fixtures.
**Proveniência:** o README upstream declara que os vetores foram produzidos por tradução do núcleo Z80 do Ares e correção de bugs. São uma referência externa útil, mas não um oráculo independente do Ares nem evidência de execução em silício.

## Configuração e formato

- Variante CMOS: desabilitada (perfil NMOS padrão).
- Simplified memory access T-states: habilitado.
- REFRESH values on address pins during opcode fetch: habilitado.
- Null address during wait state: desabilitado.
- Cada arquivo JSON contém exatamente 1.000 casos.
- Cada caso contém `initial`, `final` e `cycles`; os estados incluem registradores principais e alternativos, flags, `Q`, `P`, `WZ`, `R`, IFF, IM, EI e RAM.
- `cycles` representa amostras de estados do barramento entre ciclos. Para estas instruções register-only, a suíte compara a duração agregada e as duas requisições lógicas de opcode-fetch; não afirma equivalência elétrica dos pinos.

## Cobertura inicial

32 arquivos × 1.000 casos = **32.000 vetores**:

- DD/FD 84, 85: ADD A,IXH/IXL ou IYH/IYL.
- DD/FD 8C, 8D: ADC A,IXH/IXL ou IYH/IYL.
- DD/FD 94, 95: SUB A,IXH/IXL ou IYH/IYL.
- DD/FD 9C, 9D: SBC A,IXH/IXL ou IYH/IYL.
- DD/FD A4, A5: AND IXH/IXL ou IYH/IYL.
- DD/FD AC, AD: XOR IXH/IXL ou IYH/IYL.
- DD/FD B4, B5: OR IXH/IXL ou IYH/IYL.
- DD/FD BC, BD: CP IXH/IXL ou IYH/IYL.

O harness `native/z80_cpu/tests/singlestep_indexed_alu.rs` executa o **microsequenciador retomável**. O executor atômico não é usado como oráculo externo; permanece apenas como diferencial interno nas suítes já existentes. São comparados estado arquitetural final, memória indicada pelo vetor, PC/R/WZ/Q/P e T-states agregados.

## Hashes dos fixtures

Os valores abaixo são os Git blob SHA-1 retornados pela API de conteúdo do GitHub para a revisão upstream fixada. O teste verifica o SHA do checkout e o blob SHA-1 de cada arquivo antes de executar os vetores.

| Arquivo | Git blob SHA-1 |
|---|---|
| `dd 84.json` | `9d9b002ef4b7e05fe290c71bbd87b0a622411040` |
| `dd 85.json` | `60e745c68c9560799c240a0d8583fdde331f1fb2` |
| `dd 8c.json` | `b306f9ebfaa88322ac60ff4ec64f9b71c3aa9a55` |
| `dd 8d.json` | `0b7d4e0a7b599399ca3d5b3aca64d18f1e2ab678` |
| `dd 94.json` | `777f25a5e20c9f0a5fe532e52982a29a888f6f73` |
| `dd 95.json` | `83623f50960d9bf1feec5a39552c183da7aba520` |
| `dd 9c.json` | `02d6df95b142cf4baecca039fb794bc7ff4c9692` |
| `dd 9d.json` | `058cbb0a541e29557fcdf99c8e67f1aaafa42dc8` |
| `dd a4.json` | `bca6b0ade586ae24f86b236ef4674b0dc8e67cbb` |
| `dd a5.json` | `0feb3576fb14f6a1bcc2c32cd25c05e92ecb8565` |
| `dd ac.json` | `82920d0f208cdffc74bca67a146909590e6052df` |
| `dd ad.json` | `1a0cbd1af7e97880a15eafd8bb80376bd9b0818f` |
| `dd b4.json` | `970b214b8f514f5841927d741ee3637f154625d0` |
| `dd b5.json` | `ad895f9f596014a99e6268c94fa60f90a876c167` |
| `dd bc.json` | `252cec202c8391d57758e61f3873ae227245978d` |
| `dd bd.json` | `a97a7ad27e6203c0585334b0e900e312862db5c3` |
| `fd 84.json` | `43a2ecc73f122d5c8b4e4c18e13edcf57968f189` |
| `fd 85.json` | `0568bcd3cc5d0332130a4a81be1808ede272461f` |
| `fd 8c.json` | `a163e17a0c4612ae3404f3e9708601850e2fd050` |
| `fd 8d.json` | `6c0f42dde50ebb899bc4c625f1edc68f1c442cd6` |
| `fd 94.json` | `b468591bd7c5b7edd2b291cd755065c92a91fdc3` |
| `fd 95.json` | `e929b84476a724a1215b1d6681e4c7126b141dd8` |
| `fd 9c.json` | `329b92c30dbb61f5bee586a17db5d7cacbee4357` |
| `fd 9d.json` | `ee7dad056b01225d856c8e4edf61efe7872d0731` |
| `fd a4.json` | `fdce6084a234a7fb745aef43d4da8521bdead5d5` |
| `fd a5.json` | `b887386ccd3ba9462f6ba5f751f1a2612085b43d` |
| `fd ac.json` | `a4bededc713e86d21a02db4102088a5ae57ebeb3` |
| `fd ad.json` | `8c8ebf197bd234404725f456451ac96963e4f918` |
| `fd b4.json` | `0573fb238e601e5502f55e7e40a65eba257e4482` |
| `fd b5.json` | `fc4a5ed76439393c804298975bed7044aa1a4472` |
| `fd bc.json` | `060fd80237ce9b71a2b981973b7ecef38c29a4af` |
| `fd bd.json` | `98199f836074844f2d121e3abd1c4add644ccef8` |

## Limites

- O corpus não substitui documentação Zilog nem teste em hardware.
- A comparação dos traces é deliberadamente limitada às duas operações lógicas M1 e ao total de 8 T-states; sinais elétricos e amostras de pinos não são declarados equivalentes.
- Esta suíte cobre inicialmente as 32.000 combinações fornecidas para F-037. Prefixos repetidos/alternados, SCF/CCF, exceções de H/L e as demais famílias serão tratados em matrizes e incrementos separados.
- O resultado desta PR só poderá ser marcado PASS depois do CI no HEAD exato.
