# Documents · Documenti

[English](#english) · [Italiano](#italiano)

## English

These are Quado's project documents, written in October 2026 before and during development. They are in Italian, like the code comments. They describe what Quado must do and why, how Windows uses the space on a drive, and how each part is tested. The code cites them by number and section: `06 §11` is section 11 of `06-piano-di-test.md`, `05 D7` is decision D7 in `05-decisioni-progettuali.md`, `F-AC-2` is a requirement in `03-specifiche-prodotto.md`.

Measurements of the development PC's drives were read-only, with Windows 10 22H2. The tests that move and delete files used VHDX test drives. Milestone reports and raw measurement logs stay out of this repository because they describe that PC and its files.

Suggested order:

1. `00-sintesi.md`: what Quado is, what changed after the critical review, open decisions.
2. `03-specifiche-prodotto.md`: scenarios, requirements (`F-*`) and the model behind the recommendations.
3. `05-decisioni-progettuali.md`: decisions D1-D13, each with its reason.
4. `02-studio-tecnico.md`: how Windows accounts for space, the APIs Quado uses and their limits.
5. `08-analisi-dei-rischi.md` and `06-piano-di-test.md`: what could go wrong (`R*`, `RS*`) and the tests that check it.

Other documents, for reference: `01` existing tools, `04` interface direction, `07` development plan, `09` scenario matrix, `10` critical review.

Other files:

- `testi.md`: rules for interface texts and the glossary of fixed terms. Read it before changing a `.ftl` file.
- `revisione/`: the decisions of the October 12 review, the list of requirements with the IDs that the other documents use, and the Windows tests behind the decisions.
- `m1/cestino-verifiche.md`, `m1/cestino-verifiche-bis.md`: Recycle Bin tests on a VHDX test drive, the source of the `cestino-orfane` rule.
- `brand/`: the "Metà" mark: SVG files, construction grid and colors. The app icon is generated from the code with `cargo xtask icona`.
- `images/`: the images in the README, made with test data and made-up numbers. `images/sorgenti/` has the files that generate them.

## Italiano

Questi sono i documenti di progetto di Quado, scritti a ottobre 2026 prima e durante lo sviluppo. Descrivono cosa deve fare Quado e perché, come Windows usa lo spazio di un'unità e come si prova ogni parte. Il codice li cita per numero e sezione: `06 §11` è la sezione 11 di `06-piano-di-test.md`, `05 D7` è la decisione D7 di `05-decisioni-progettuali.md`, `F-AC-2` è un requisito di `03-specifiche-prodotto.md`.

Le misure sulle unità del PC di sviluppo sono state prese in sola lettura, con Windows 10 22H2. Le prove che spostano ed eliminano file hanno usato VHDX di prova. I resoconti delle milestone e i registri grezzi delle misure non sono in questo repository, perché descrivono quel PC e i suoi file.

Ordine consigliato:

1. `00-sintesi.md`: cos'è Quado, cosa è cambiato dopo la revisione critica, decisioni aperte.
2. `03-specifiche-prodotto.md`: scenari, requisiti (`F-*`) e modello delle raccomandazioni.
3. `05-decisioni-progettuali.md`: le decisioni D1-D13, ognuna con il suo motivo.
4. `02-studio-tecnico.md`: come Windows conta lo spazio, le API che usa Quado e i loro limiti.
5. `08-analisi-dei-rischi.md` e `06-piano-di-test.md`: cosa può andare storto (`R*`, `RS*`) e i test che lo controllano.

Gli altri documenti, da consultare quando servono: `01` strumenti esistenti, `04` direzione dell'interfaccia, `07` piano di sviluppo, `09` matrice degli scenari, `10` revisione critica.

Altri file:

- `testi.md`: regole per i testi dell'interfaccia e glossario dei termini fissi. Va letto prima di modificare un file `.ftl`.
- `revisione/`: le decisioni della revisione del 12 ottobre, l'elenco dei requisiti con gli ID che usano gli altri documenti e le prove in Windows su cui si basano le decisioni.
- `m1/cestino-verifiche.md`, `m1/cestino-verifiche-bis.md`: prove del Cestino su un VHDX di prova, la fonte della regola `cestino-orfane`.
- `brand/`: il segno «Metà»: file SVG, griglia di costruzione e colori. L'icona dell'app si genera dal codice con `cargo xtask icona`.
- `images/`: le immagini del README, fatte con dati di prova e numeri inventati. In `images/sorgenti/` ci sono i file che le generano.
