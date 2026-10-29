# SMS homebrew validation — archived evidence

**Purpose:** preserve the inventory and bounded test evidence before removing the temporary test corpus from the repository. No ROM payloads are retained in this document.

## Evidence provenance

- Inventory/build workflow run: [#7 — SMS Homebrew ROM Inventory and WLA-DX Builds](https://github.com/leonardo201800478/sega-emulator/actions/runs/40891095007)
- Candidate source SHA: `78b01a7eb1e58869626d6513004f71300f347f09`
- Workflow result: **PASS** (the inventory/build job completed and uploaded its report; this does not mean every source compiled).
- Artifact: [sms-homebrew-validation](https://github.com/leonardo201800478/sega-emulator/actions/runs/40891095007/artifacts/12905116133), SHA-256 `f73aec6b47089d015a2f62267f051053752d9d191bb17278082d55afaaad2e61`, retention through 2026-11-05.
- The list below records SHA-256 and byte count from the workflow inventory, so the record remains useful after the short-lived artifact expires.

## Emulator smoke-test evidence

The following results were reported by Rust CI on candidate SHA `afcaa0d2ffe24cf761e44ead75138f81886faa79` before the merge of PR #241:

- `sms-sound-test.sms`: 30,000 instructions in two machine instances; PSG state and generated samples matched deterministically and samples were produced. The bounded window did **not** prove that the sample stream was non-silent or audibly correct.
- `VDPTEST.sms`: 30,000 instructions; raster advanced and VDP registers, VRAM and framebuffer matched between two instances.
- `zexdoc.sms`: 5,000-instruction bounded smoke test; CPU and memory states matched between two instances. This is **not** a full ZEXDOC conformance run.
- The Rust workspace checks and external Z80 conformance workflow were reported PASS on that candidate SHA. These results are candidate-SHA evidence, not a new test run on the later merge commit.
- Synthetic PSG unit tests independently exercised deterministic, non-silent output.

## WLA-DX compilation findings

Workflow run #7 completed successfully as a reporting job, with these individual results:

- `PFR_Detect200.asm`: **FAIL** under the inferred WLA-DX recipe. Linker reported no room for section `TEST_DATA_PAGE_Sega_Tag` (16 bytes) in ROM bank 1. The source was not changed to hide the error.
- `SMSCycleCounter-SMS-1.0/demo.asm`: build completed, but rebuilt output SHA-256 differed from the supplied `output.sms`; equivalence was not established.
- `testmain.asm`: blocked by missing `colours.inc` and `Phantasy Star decompressors.inc`.
- `VCounterTest-SMS-1.00/TEST.ASM`: supplied build uses TASM80, not WLA-DX.
- `porttest.asm` and `bgtest.asm`: legacy TASM syntax without a reproducible build recipe in the corpus.

Compiling a test, matching a file hash, or passing a bounded smoke test does not by itself establish hardware accuracy. Visual/audio comparison against an independent reference and deeper temporal tests remain pending.

## Archived corpus inventory

All entries below were inventoried under `local-assets/sms_test_roms/`. Hashes are SHA-256 of the file bytes.

| Kind | Relative path | SHA-256 | Bytes |
|---|---|---|---:|
| other | `LICENSE` | `658f7e3b90a1de0a634efa561cc2ae2f25b68d3c8986a4dc35821c480bbdb334` | 18123 |
| assembly-source | `PFR_Detect200.asm` | `72037487f5e8d02c1d990370d91aad4b2f2e9197fce4ef111d71b58e0ca5d26c` | 38123 |
| rom-binary | `PFR_Detect200.sms` | `7c56d5d84c5f701711f2254458b3ce36be7e00aa1438d46d040f21a24b595515` | 32768 |
| rom-binary | `Porttest.sms` | `6e6ef0ea9f9acca4b666e845cc170bf4eca1f28c24b75b7bc87b168c82aa3b87` | 32768 |
| other | `README.md` | `ad2c38b0049b9c9dceee63ca3cf2eee9bb8f2deed1f1e5e9c7e7180a2a4d4d6a` | 3692 |
| other | `SMSCycleCounter-SMS-1.0/Compile.bat` | `a35d351011deb055ed21768c870f4c38c669d2f7ebcc2dd626f686dd2b30ec38` | 1249 |
| assembly-source | `SMSCycleCounter-SMS-1.0/demo.asm` | `799901c57081d90899fcb97cdb6e3aef7f88317c55e113032ba9c442e385b719` | 9264 |
| rom-binary | `SMSCycleCounter-SMS-1.0/output.sms` | `0200a0a03ff56c7801c5da02612c23b1212a3a07d802afae23659a85570bf29b` | 32768 |
| rom-binary | `SMSTestSuite.sms` | `edbe62071e94cbd10faa51f9035159f36b1b2fae8e3e4bec2a0f20848d877ebb` | 32768 |
| rom-binary | `TEST(1).SMS` | `70be2135231cc24ab624975c857df64366a42668d9793e25f210b720cd3db147` | 32768 |
| rom-binary | `TEST.SMS` | `c23791e2b6dcd224092dfefa8b480884a85b87fd5667439c0a97841ab06ff52e` | 32768 |
| other | `VCounterTest-SMS-1.00/TASM80.TAB` | `398e23b4e2970433c01bce0994dc1979dec24cfd913391d710de20b142a7a161` | 14453 |
| assembly-source | `VCounterTest-SMS-1.00/TEST.ASM` | `657b42cec85c4bee750f9373a8a4881a8d2a2491323a925a116a42310dc2be79` | 20708 |
| rom-binary | `VCounterTest-SMS-1.00/TEST.SMS` | `70be2135231cc24ab624975c857df64366a42668d9793e25f210b720cd3db147` | 32768 |
| assembly-source | `VCounterTest-SMS-1.00/conio.inc` | `1678c7950eeff9a7fd6e5b68ce424baa541ce60d2cfab46812b03713771b004f` | 5280 |
| other | `VCounterTest-SMS-1.00/make.bat` | `1e693ef3ffacb757433596b0f6356f802ad1df95cddc1810f49e17210be40080` | 55 |
| other | `VCounterTest-SMS-1.00/readme.txt` | `b9fdb5c8705356455869e94947840d9e4201a4ac8367e6775e3adfec3e637546` | 403 |
| other | `VCounterTest-SMS-1.00/run.bat` | `56e813c8444d1db9d22a41fc3b633c9df4b89f48a446e89a9cc64cf74fed27d2` | 17 |
| assembly-source | `VCounterTest-SMS-1.00/vdp.inc` | `c369eea9a1e5018783a14cdff720cac10350bdc5275d8ab4e957ffcec8891f01` | 2541 |
| assembly-source | `VCounterTest-SMS-1.00/vga.inc` | `ec61876335aed8b9c7b531b58aa3a47219e1062d4cbc62daaccb4e96f7ccda15` | 5900 |
| rom-binary | `VDPTEST.sms` | `2b64b55ded7406aecf60a85547639dd4a0ab9f9c212a04676b79e15e8711f3a0` | 32768 |
| other | `VDPTest-ReadMe.txt` | `85e98512a2319b95618a6708ce87231d6ec844d8c2c4f25e545cd8d5d647d35a` | 6198 |
| assembly-source | `bgtest.asm` | `27e20d632c9a6224d432cb1a466dabbf6232430115f5ebb1fb02e79cdc6dece7` | 10246 |
| rom-binary | `bgtest.sms` | `1bf18c7a722b8d2ab46cc011127a710a3c0ee8444fe0a77a0e74f3406920e6a5` | 32768 |
| assembly-source | `eefont.inc` | `9ca27dd24990dfb25915a6644b8302667aaefb9ae169ce89b2934a9f09e1c076` | 7423 |
| rom-binary | `output.sms` | `0200a0a03ff56c7801c5da02612c23b1212a3a07d802afae23659a85570bf29b` | 32768 |
| assembly-source | `porttest.asm` | `f2f473bdede6bfc70e02b6c12c5c04ac48784eb3e2d14d2ddc5fcadde949991c` | 10927 |
| rom-binary | `sms-sound-test.sms` | `1e911ba47f37cadff37d7ae0500e481f85a40b3bb4d53309a972566c2fe7d117` | 32768 |
| rom-binary | `smscharset (palette).bin` | `bfc5da356f82336905a6829e895055b0ae7e70d2d0571f6c40f6d63755a00024` | 5 |
| other | `smscharset (tiles).pscompr` | `b08a78cb02bea28a945d45f9e7374b6a56713648e9ca7c629ea919885593c179` | 2040 |
| assembly-source | `smsfont.inc` | `3d0eb8eebe8da95b58a698a81323f14a7cd7bf976726ba8530a136ff8f39e198` | 3379 |
| rom-binary | `smspal.sms` | `7b01ec09b9a7ea2f1224ac36fb06c44dfd6d3d0f14d19b22006240e723272165` | 32768 |
| assembly-source | `testmain.asm` | `b0db6a75f1128e809fa835ab40323acf009d090704657d9fa0f8a815f6eb05ba` | 152290 |
| rom-binary | `zexall.sms` | `25e205207745781a556003a800e60d9ec52330fb1cec1639dcf554a06e622e66` | 65536 |
| rom-binary | `zexdoc.sms` | `684cab8d256d474fdd34fc21ef942c4767cb355cbc47588926cd49ef4375ef30` | 65536 |
