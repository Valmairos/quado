## Seconda serie: esito per elemento, letto dal Cestino su disco

- **A: solo a.bin (2 MB)**: dopo a.bin assente
  - PreDeleteItem `a.bin`: flag 0x282 (nel Cestino)
  - PostDeleteItem `a.bin`: flag 0x282, esito 0x00270008, nel Cestino: true
  - PerformOperations 0x00000000, operazioni annullate: false
  - Cestino: Windows 1 elementi, 2097152 byte; su disco [("$IFLBCB3.bin", 74), ("$R80IA6M", 12582912), ("$RFLBCB3.bin", 2097152)]

- **B: a.bin e b.bin (2 + 2 MB) insieme**: dopo a.bin assente, b.bin assente
  - PreDeleteItem `a.bin`: flag 0x282 (nel Cestino)
  - PostDeleteItem `a.bin`: flag 0x282, esito 0x00270008, nel Cestino: true
  - PreDeleteItem `b.bin`: flag 0x282 (nel Cestino)
  - PostDeleteItem `b.bin`: flag 0x282, esito 0x00270008, nel Cestino: true
  - PerformOperations 0x00000000, operazioni annullate: false
  - Cestino: Windows 2 elementi, 4194304 byte; su disco [("$I8N5EPB.bin", 74), ("$IRL6VFJ.bin", 74), ("$R80IA6M", 12582912), ("$R8N5EPB.bin", 2097152), ("$RRL6VFJ.bin", 2097152)]

- **C: a.bin poi grande.bin (2 + 30 MB) insieme**: dopo a.bin assente, grande.bin assente
  - PreDeleteItem `a.bin`: flag 0x282 (nel Cestino)
  - PostDeleteItem `a.bin`: flag 0x282, esito 0x00270008, nel Cestino: false
  - PreDeleteItem `grande.bin`: flag 0x202 (eliminazione definitiva)
  - PostDeleteItem `grande.bin`: flag 0x202, esito 0x00270008, nel Cestino: false
  - PerformOperations 0x00000000, operazioni annullate: false
  - Cestino: Windows 0 elementi, 0 byte; su disco [("$R80IA6M", 12582912)]

- **D: grande.bin poi a.bin insieme**: dopo grande.bin assente, a.bin assente
  - PreDeleteItem `grande.bin`: flag 0x282 (nel Cestino)
  - PostDeleteItem `grande.bin`: flag 0x282, esito 0x00270008, nel Cestino: false
  - PreDeleteItem `a.bin`: flag 0x202 (eliminazione definitiva)
  - PostDeleteItem `a.bin`: flag 0x202, esito 0x00270008, nel Cestino: false
  - PerformOperations 0x00000000, operazioni annullate: false
  - Cestino: Windows 0 elementi, 0 byte; su disco [("$R80IA6M", 12582912)]

- **E: solo sparso.bin (50 MB logici, quasi nulla allocato)**: dopo sparso.bin assente
  - PreDeleteItem `sparso.bin`: flag 0x282 (nel Cestino)
  - PostDeleteItem `sparso.bin`: flag 0x282, esito 0x00270008, nel Cestino: false
  - PerformOperations 0x00000000, operazioni annullate: false
  - Cestino: Windows 0 elementi, 0 byte; su disco [("$R80IA6M", 12582912)]

- **F: solo compresso.txt (30 MB logici, circa 4 MB allocati)**: dopo compresso.txt assente
  - PreDeleteItem `compresso.txt`: flag 0x282 (nel Cestino)
  - PostDeleteItem `compresso.txt`: flag 0x282, esito 0x00270008, nel Cestino: false
  - PerformOperations 0x00000000, operazioni annullate: false
  - Cestino: Windows 0 elementi, 0 byte; su disco [("$R80IA6M", 12582912)]


