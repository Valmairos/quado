## Verifiche del Cestino su R:\ (volume {24c69897-74a3-4ec2-b864-7101bc40cd71}, Cestino ridotto a 20 MB)

### `PreDeleteItem` e rifiuto del singolo elemento

Tre elementi in un'unica operazione: `a.bin` (2 MB), `grande.bin` (30 MB, oltre la capacità), `b.bin` (2 MB).

- **nessuna protezione (controllo)**: dopo l'operazione a.bin assente, grande.bin assente, b.bin assente
  - PreDeleteItem `a.bin`: flag 0x282 (nel Cestino)
  - PostDeleteItem `a.bin`: flag 0x282, esito 0x00270008, nel Cestino: false
  - PreDeleteItem `grande.bin`: flag 0x202 (eliminazione definitiva)
  - PostDeleteItem `grande.bin`: flag 0x202, esito 0x00270008, nel Cestino: false
  - PreDeleteItem `b.bin`: flag 0x202 (eliminazione definitiva)
  - PostDeleteItem `b.bin`: flag 0x202, esito 0x00270008, nel Cestino: false
  - PerformOperations 0x00000000, operazioni annullate: false
  - Cestino secondo Windows: 0 elementi, 0 byte

- **E_ABORT**: dopo l'operazione a.bin assente, grande.bin presente, b.bin presente
  - PreDeleteItem `a.bin`: flag 0x282 (nel Cestino)
  - PostDeleteItem `a.bin`: flag 0x282, esito 0x00270008, nel Cestino: false
  - PreDeleteItem `grande.bin`: flag 0x202 (eliminazione definitiva), rifiutato
  - PostDeleteItem `grande.bin`: flag 0x202, esito 0x80004004, nel Cestino: false
  - PerformOperations 0x80004004, operazioni annullate: false
  - Cestino secondo Windows: 0 elementi, 0 byte

- **HRESULT_FROM_WIN32(ERROR_CANCELLED)**: dopo l'operazione a.bin assente, grande.bin presente, b.bin presente
  - PreDeleteItem `a.bin`: flag 0x282 (nel Cestino)
  - PostDeleteItem `a.bin`: flag 0x282, esito 0x00270008, nel Cestino: false
  - PreDeleteItem `grande.bin`: flag 0x202 (eliminazione definitiva), rifiutato
  - PostDeleteItem `grande.bin`: flag 0x202, esito 0x800704c7, nel Cestino: false
  - PerformOperations 0x800704c7, operazioni annullate: false
  - Cestino secondo Windows: 0 elementi, 0 byte

- **COPYENGINE_E_USER_CANCELLED**: dopo l'operazione a.bin assente, grande.bin presente, b.bin presente
  - PreDeleteItem `a.bin`: flag 0x282 (nel Cestino)
  - PostDeleteItem `a.bin`: flag 0x282, esito 0x00270008, nel Cestino: false
  - PreDeleteItem `grande.bin`: flag 0x202 (eliminazione definitiva), rifiutato
  - PostDeleteItem `grande.bin`: flag 0x202, esito 0x80270000, nel Cestino: false
  - PerformOperations 0x80270000, operazioni annullate: true
  - Cestino secondo Windows: 0 elementi, 0 byte

- **COPYENGINE_S_USER_IGNORED**: dopo l'operazione a.bin assente, grande.bin assente, b.bin assente
  - PreDeleteItem `a.bin`: flag 0x282 (nel Cestino)
  - PostDeleteItem `a.bin`: flag 0x282, esito 0x00270008, nel Cestino: false
  - PreDeleteItem `grande.bin`: flag 0x202 (eliminazione definitiva), rifiutato
  - PostDeleteItem `grande.bin`: flag 0x202, esito 0x00270008, nel Cestino: false
  - PreDeleteItem `b.bin`: flag 0x202 (eliminazione definitiva), rifiutato
  - PostDeleteItem `b.bin`: flag 0x202, esito 0x00270008, nel Cestino: false
  - PerformOperations 0x00000000, operazioni annullate: false
  - Cestino secondo Windows: 0 elementi, 0 byte

- **COPYENGINE_S_DONT_PROCESS_CHILDREN**: dopo l'operazione a.bin assente, grande.bin assente, b.bin assente
  - PreDeleteItem `a.bin`: flag 0x282 (nel Cestino)
  - PostDeleteItem `a.bin`: flag 0x282, esito 0x00270008, nel Cestino: false
  - PreDeleteItem `grande.bin`: flag 0x202 (eliminazione definitiva), rifiutato
  - PostDeleteItem `grande.bin`: flag 0x202, esito 0x00270008, nel Cestino: false
  - PreDeleteItem `b.bin`: flag 0x202 (eliminazione definitiva), rifiutato
  - PostDeleteItem `b.bin`: flag 0x202, esito 0x00270008, nel Cestino: false
  - PerformOperations 0x00000000, operazioni annullate: false
  - Cestino secondo Windows: 0 elementi, 0 byte

### Come Windows conta la capacità

- `sparso.bin`: logica 50 MB, quasi nessun cluster allocato; `compresso.txt`: logica 30 MB, compresso NTFS (31457280 The compression ratio is 8,0)
- `sparso.bin`:
  - PreDeleteItem `sparso.bin`: flag 0x282 (nel Cestino)
  - PostDeleteItem `sparso.bin`: flag 0x282, esito 0x00270008, nel Cestino: false
  - PerformOperations 0x00000000, operazioni annullate: false
- `compresso.txt`:
  - PreDeleteItem `compresso.txt`: flag 0x282 (nel Cestino)
  - PostDeleteItem `compresso.txt`: flag 0x282, esito 0x00270008, nel Cestino: false
  - PerformOperations 0x00000000, operazioni annullate: false

- Riempimento: tre file da 8 MB spostati uno alla volta in un Cestino da 20 MB.
  - PreDeleteItem `f1.bin`: flag 0x282 (nel Cestino)
  - PostDeleteItem `f1.bin`: flag 0x282, esito 0x00270008, nel Cestino: true
  - PerformOperations 0x00000000, operazioni annullate: false
  - dopo f1: Cestino secondo Windows 1 elementi, 8388608 byte; su disco [("$IXQI8WF.bin", 76), ("$RXQI8WF.bin", 8388608)]
  - PreDeleteItem `f2.bin`: flag 0x282 (nel Cestino)
  - PostDeleteItem `f2.bin`: flag 0x282, esito 0x00270008, nel Cestino: true
  - PerformOperations 0x00000000, operazioni annullate: false
  - dopo f2: Cestino secondo Windows 2 elementi, 16777216 byte; su disco [("$IVPWIV7.bin", 76), ("$IXQI8WF.bin", 76), ("$RVPWIV7.bin", 8388608), ("$RXQI8WF.bin", 8388608)]
  - PreDeleteItem `f3.bin`: flag 0x282 (nel Cestino)
  - PostDeleteItem `f3.bin`: flag 0x282, esito 0x00270008, nel Cestino: true
  - PerformOperations 0x00000000, operazioni annullate: false
  - dopo f3: Cestino secondo Windows 3 elementi, 25165824 byte; su disco [("$IO1MWZ5.bin", 76), ("$IVPWIV7.bin", 76), ("$IXQI8WF.bin", 76), ("$RO1MWZ5.bin", 8388608), ("$RVPWIV7.bin", 8388608), ("$RXQI8WF.bin", 8388608)]

### Cartella orfana e svuotamento

  - PreDeleteItem `cartella12`: flag 0x282 (nel Cestino)
  - PostDeleteItem `cartella12`: flag 0x282, esito 0x00270008, nel Cestino: true
  - PerformOperations 0x00000000, operazioni annullate: false
- Tolto `$I80IA6M`: ora la cartella è orfana. Windows vede 0 elementi, 0 byte; su disco [("$R80IA6M", 12582912)]
- Spostato `f4.bin` (10 MB): con l'orfana da 12 MB il totale supererebbe 20 MB se l'orfana contasse.
  - PreDeleteItem `f4.bin`: flag 0x282 (nel Cestino)
  - PostDeleteItem `f4.bin`: flag 0x282, esito 0x00270008, nel Cestino: true
  - PerformOperations 0x00000000, operazioni annullate: false
  - Windows vede 1 elementi, 10485760 byte; su disco [("$IZXSI96.bin", 76), ("$R80IA6M", 12582912), ("$RZXSI96.bin", 10485760)]
- Svuotamento del Cestino (`SHEmptyRecycleBinW`, lo stesso dell'opzione di Windows): esito Ok(()); su disco dopo: [("$R80IA6M", 12582912)]

Capacità ripristinata: valore assente, come prima

