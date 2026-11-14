# Plán kontroly a opravy bugů

Doplněk k `docs/QA-PLAN.md`: ten řeší, *co* projít před vydáním; tenhle řeší
*co dělat s chybou*, od nálezu po uzavření, a kde v tomhle kódu chyby vznikají.

## 1. Jak chyba vzniká — kontrolní místa

Vytipováno z kódu, ne obecně. U každého místa je pravidlo, které drží.

| Místo | Proč je rizikové | Pravidlo |
|-------|------------------|----------|
| Výměna souborů a spouštění procesů (`src/updater.rs`) | běží odpojeně po `process::exit(0)` (`src/updater.rs:289`), uživatel nevidí nic; `tar` z PATH byl GNU tar a tiše nic nerozbalil | jediná cesta: sandbox test v `%TEMP%\updtest` + log do `%TEMP%\warust-update.log`; nikdy nespoléhat na PATH, vždy plná cesta k systémovému nástroji |
| Panely s obsahem proměnné šířky (`src/ui/chat_info.rs`, `src/ui/chat_list.rs`) | řádek s dlouhou URL si vzal 944 px v panelu 300 px a rozbil celé okno | každý seznam s uživatelským textem dostane headless test šířky (viz QA-PLAN §2) |
| Ignorované výsledky — 211× `let _ =` v `src/` | selhání zápisu nebo smazání zmizí bez stopy | ignorovat jen úklid (dočasné soubory, cache); u stavu, session a logu výsledek ošetřit a zalogovat |
| `unwrap()`/`expect()` — 151×, většina v testech | na datech ze sítě nebo od uživatele spadne celá aplikace | v async cestách a v `src/backend.rs` žádný `unwrap` na cizí data; poslední výskyt je `client.pn().unwrap_or_else(\|\| client.lid().unwrap())` (`src/backend.rs:3285`) — dořešit fallbackem |
| Blokující I/O v async (`std::fs` v `src/backend.rs:282,484–495`) | zasekne runtime, projeví se jako „zamrzlé“ UI | při startu je to v pořádku, v reakci na uživatelskou akci ne |
| Kreslení přes `painter` (`MessageBubble`, dokumenty) | šířku a ořez nekontroluje layout, test ji nezměří | nové `painter` volání musí respektovat `bubble_rect` a být pokryté testem, který měří `min_rect` |
| Aktualizace ve starých verzích | verze 0.2.0–0.3.1 mají rozbitý instalátor a samy se neopraví | každá oprava instalátoru musí mít v release notes ruční postup |

## 2. Zápis nálezu

Každý bug dostane záznam, i když ho hned opravím. Do issue na GitHubu, nebo do
tabulky níže, podle rozsahu.

```
### BUG-<rrrrmmdd>-<nn>: <jedna věta, co je špatně>
- verze: 0.3.2 (release) / main@<hash>
- prostředí: Windows 11, okno 420×964
- kroky: 1. … 2. … 3. …
- čekal jsem: …
- stalo se: …
- opakovatelnost: vždy / po <akci> / jednou
- důkazy: screenshot, log (RUST_LOG=warust=debug), %TEMP%\warust-update.log
- závažnost: P0/P1/P2
```

Bez kroků a verze se bug neopravuje — odhadem se opraví špatná věc.

## 3. Triáž

| Závažnost | Význam | Co s tím |
|-----------|--------|----------|
| P0 | nejde spustit, přihlásit, ztráta dat, rozbité rozvržení celého okna | stop práce, oprava hned, vydání hned |
| P1 | funkce nefunguje (neodešle, nepřehraje, neaktualizuje se) | oprava v nejbližším vydání, do té doby v release notes |
| P2 | kosmetika, zkrácený text, animace | sbírat, opravit dávkově |

Když si nejde vybrat mezi P1 a P2: rozhoduje, jestli jde obejít. Jde-li to
obejít (ruční stažení zipu), je to P1. Nejde-li, je to P0.

## 4. Reprodukce — dokud to nemám v ruce, nic neopravuju

1. Projít kroky na **release buildu**, ne na svém čerstvém.
2. Zúžit na minimum: která akce to spustí (klik na řádek vs. „Otevřít“), jaká
   šířka okna, jaká verze.
3. Přenést do měřitelné podoby — jedna z těchto:
   - **unit test** (logika, parsery, skripty — jako `windows_script_extracts_with_the_system_tar`),
   - **headless layout test** (`ctx.run_ui`, měření `min_rect().width()` — jako `gallery_rows_fit_the_panel_width`),
   - **sandbox skript** (práce se soubory a procesy v `%TEMP%\updtest`),
   - když ani jedno nejde (kreslení, animace): zapsat do záznamu přesné kroky a screenshot a ověřit ručně před i po.
4. Test musí **nejdřív selhat** a jeho hláška musí obsahovat číslo („rows are
   944 px wide“), ne jen `assertion failed`.

## 5. Hledání příčiny

Postup, který se osvědčil u posledních dvou bugů:

1. **Ověřit předpoklad měřením, ne čtením.** U instalátoru to bylo
   `where tar` → `C:\Program Files\Git\usr\bin\tar.exe` a `System32\tar.exe -tf`
   na skutečném zipu. U layoutu změřená šířka řádku.
2. Projít celou cestu dat: kde hodnota vzniká → kdo ji předává → kde se použije.
   Bug „UI se rozbije“ byl ve `src/ui/chat_info.rs`, ne v konverzaci.
3. Když příčina nesedí na žádný řádek kódu (přeškálované písmo), **nezasahovat
   naslepo**. Zapsat jako otevřený nález a doplnit diagnostiku (`--test-update`
   flag, souborový log), aby šel příště chytit.
4. Zapsat příčinu jednou větou do commitu — bez ní se bug vrátí.

## 6. Oprava

- Větev `fix/<slug>`, jeden bug = jeden commit.
- Nejdřív test, pak oprava. Pokud test nejde, do commitu patří, čím se to
  ověřilo ručně.
- Minimální diff: žádné refaktory „když už jsem v tom“.
- Chybová cesta má být vidět: hláška do UI (`UpdateStatus::Failed`), řádek do
  logu. Tichý `return` je zakázaný — přesně tím se ztratila informace
  o nerozbaleném zipu.
- Commit message česky: `fix(oblast): co bylo špatně` + odstavec s příčinou.

## 7. Ověření opravy

1. Nový test prochází, starý sada taky (`cargo test` → 0 failed).
2. Reprodukce ze sekce 4 na **release buildu** podle původních kroků uživatele.
3. Brány z `docs/QA-PLAN.md` §1.
4. U souborů a procesů navíc: sandbox v `%TEMP%\updtest`, kontrola
   `%TEMP%\warust-update.log`, kontrola, že se soubory opravdu přepsaly
   (velikost, `MZ` hlavička).
5. Teprve pak merge a vydání; release notes řeknou, co bylo špatně a jestli je
   potřeba ruční krok.

## 8. Uzavření a prevence

- Každý opravený bug dostane test, který by ho znovu zachytil. Bez testu se bug
  nevrací do „opraveno“, ale do „opraveno, nekryto“.
- Sekce 1 se doplní, pokud bug vznikl v novém typu místa.
- Známé a neopravené bugy zůstávají v tabulce níže — nezavírat je mazáním.

## 9. Otevřené nálezy (stav k 2026-11-14)

| ID | Popis | Závažnost | Stav |
|----|-------|-----------|------|
| BUG-20261114-01 | Instalátor používal `tar` z PATH (GNU tar), zip se nerozbalil, aplikace se restartovala ve staré verzi | P0 | opraveno v 0.3.2, kryto testem + sandboxem; uživatelé na 0.2.0–0.3.1 musí jednou ručně |
| BUG-20261114-02 | Řádek v „Média, odkazy a dokumenty“ široký 944 px v panelu 300 px, přetekl přes konverzaci | P1 | opraveno (PR #26), kryto `gallery_rows_fit_the_panel_width` |
| BUG-20261114-03 | Po kliknutí na odkaz v info panelu se objevilo přeškálované písmo; po opravě šířky neověřeno | P1 | **otevřeno** — čeká na reprodukci od uživatele (které kliknutí, jaká šířka okna) |
| BUG-20261114-04 | Aplikace nepíše log do souboru, u odpojeného okna není co přiložit k hlášení | P2 | opraveno — `%APPDATA%\warust\warust.log`, rotace při 2 MB na `warust.1.log`, kryto testy v `src/logging.rs` |
| BUG-20261114-05 | 211 ignorovaných výsledků (`let _ =`), u zápisu stavu a session může selhat tiše | P2 | částečně opraveno — 26× `persist_state` jde přes `persist_or_log`, chyba se loguje; ostatní `let _ =` (kanály, úklid) zůstávají |
| BUG-20261114-06 | `client.pn().unwrap_or_else(\|\| client.lid().unwrap())` (`src/backend.rs:3285`) — poslední `unwrap` bez fallbacku na datech ze sítě | P2 | opraveno (PR #29) — vrací chybu, zpráva zůstane neodvolaná |
| BUG-20261114-07 | `cargo fmt --check` neprochází, formátování je v repu nekonzistentní | P3 | otevřeno — srovnat jednorázově, pak zapnout jako bránu |
| BUG-20261114-08 | Nečinné okno si říkalo o frame každých 100 ms (`src/app.rs:297`), takže drželo GPU vzhůru a kreslilo tytéž pixely | P1 | opraveno (PR #29), kryto `idle_state_does_not_ask_for_animation_frames` |
| BUG-20261114-09 | `extract_message_text` měl `for` přes tři poll payloady, který vracel v první iteraci (clippy `never_loop`) | P2 | opraveno (PR #29) — `or_else` řetěz se stejnou sémantikou |
| BUG-20261114-10 | Panel zpráv nemá virtualizaci: kreslí všechny bubliny historie v každém framu | P1 | opraveno (PR #33) — 2000 zpráv 4,67 → 0,08 ms; kryto `a_long_history_only_draws_the_rows_around_the_viewport`, `spacers_keep_the_content_height` |
