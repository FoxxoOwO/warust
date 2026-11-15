//! Překlady uživatelského rozhraní.
//!
//! Zdrojovým jazykem je čeština: texty zůstávají v kódu tam, kde jsou, jen se
//! obalí `tr!("…")` (statický text) nebo `trf!("… {}", hodnota)` (text
//! s parametry). Klíčem překladu je tedy česká věta sama — nemůže se stát, že
//! by se klíč a text rozešly.
//!
//! Když anglický překlad chybí, vrátí se čeština. To je záměr (rozpracovaný
//! překlad nesmí rozbít UI), ale aby se na chybějící překlad nezapomnělo,
//! hlídá to test `kazdy_prelozeny_text_ma_anglicky_protějšek` — ten projde
//! všechny soubory v `src/` a každý literál z `tr!`/`trf!` porovná s tabulkou.
//!
//! Jazyk je procesová konstanta (`AtomicU8`), ne parametr: texty se objevují
//! na stovkách míst a protahovat `Language` každou funkcí UI by znamenalo
//! přepsat celé rozhraní kvůli jednomu nastavení. `app.rs` ho nastaví jednou
//! na začátku každého snímku, takže přepínač v nastavení zabere hned.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

/// Jazyk rozhraní.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Cs,
    En,
}

impl Default for Language {
    fn default() -> Self {
        // Čeština je výchozí, aby stávajícím uživatelům po aktualizaci nic
        // nepřeskočilo do angličtiny.
        Language::Cs
    }
}

impl Language {
    /// Kód jazyka pro uložení do nastavení a logy.
    pub fn code(self) -> &'static str {
        match self {
            Language::Cs => "cs",
            Language::En => "en",
        }
    }

    /// Popisek do přepínače — vždy v daném jazyce, aby ho našel i ten, kdo
    /// aktuálnímu jazyku nerozumí.
    pub fn label(self) -> &'static str {
        match self {
            Language::Cs => "Čeština",
            Language::En => "English",
        }
    }

    /// Jazyky nabízené v nastavení.
    pub fn all() -> [Language; 2] {
        [Language::Cs, Language::En]
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// Nastaví jazyk pro celý proces (volá `app.rs` před kreslením snímku).
pub fn set_language(lang: Language) {
    CURRENT.store(
        match lang {
            Language::Cs => 0,
            Language::En => 1,
        },
        Ordering::Relaxed,
    );
}

pub fn language() -> Language {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Language::En,
        _ => Language::Cs,
    }
}

pub fn is_english() -> bool {
    language() == Language::En
}

/// Přeloží statický text. Neznámý text zůstane česky.
pub fn t(czech: &'static str) -> &'static str {
    if !is_english() {
        return czech;
    }
    english(czech).unwrap_or(czech)
}

/// Anglický překlad daného českého textu, pokud existuje.
///
/// Tabulka je pole dvojic a hledá se přes `HashMap` postavenou jednou při
/// prvním použití — lineární průchod by se dělal tisíckrát za snímek.
pub fn english(czech: &str) -> Option<&'static str> {
    static LOOKUP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    LOOKUP
        .get_or_init(|| ENGLISH.iter().copied().collect())
        .get(czech)
        .copied()
}

/// Naformátuje přeloženou šablonu: každé `{}` nahradí dalším argumentem.
///
/// `format!` vyžaduje literál, takže šablonu z tabulky použít nelze; tohle je
/// jeho náhrada pro texty, které se překládají.
pub fn tf(template: &'static str, args: &[&dyn std::fmt::Display]) -> String {
    let translated = t(template);
    let mut out = String::with_capacity(translated.len() + 16);
    let mut rest = translated;
    let mut args = args.iter();
    while let Some(pos) = rest.find("{}") {
        out.push_str(&rest[..pos]);
        match args.next() {
            Some(arg) => out.push_str(&arg.to_string()),
            // Víc zástupných znaků než argumentů: zbytek necháme být, ať je
            // chyba vidět v UI a ne jako panika.
            None => {
                out.push_str("{}");
            }
        }
        rest = &rest[pos + 2..];
    }
    out.push_str(rest);
    out
}

/// Přeloží statický text (`tr!("Nastavení")`).
#[macro_export]
macro_rules! tr {
    ($text:literal) => {
        $crate::i18n::t($text)
    };
}

/// Přeloží text s parametry (`trf!("Archivované ({})", 3)`).
#[macro_export]
macro_rules! trf {
    ($template:literal $(, $arg:expr)* $(,)?) => {
        $crate::i18n::tf(
            $template,
            &[$( &$arg as &dyn ::core::fmt::Display ),*],
        )
    };
}

/// Anglické překlady. Klíčem je český text z kódu.
///
/// Pořadí je abecední, aby se dalo snadno najít, jestli tam text je.
pub const ENGLISH: &[(&str, &str)] = &[
    // --- Nastavení: záložky a obecné ---
    ("⚙️  Nastavení", "⚙️  Settings"),
    ("👤  Účet a profil", "👤  Account and profile"),
    ("💬  Konverzace a média", "💬  Chats and media"),
    ("🔔  Oznámení", "🔔  Notifications"),
    ("🔒  Soukromí", "🔒  Privacy"),
    ("🎨  Vzhled", "🎨  Appearance"),
    ("ℹ️  O aplikaci a data", "ℹ️  About and data"),
    ("🖥️  Systém a spuštění", "🖥️  System and startup"),
    ("Zavřít", "Close"),
    ("Jazyk", "Language"),
    ("Jazyk rozhraní", "Interface language"),
    ("Změna se projeví okamžitě, u ikony v oznamovací oblasti až po restartu aplikace.", "The change applies immediately; the notification-area icon updates after a restart."),
    ("Přizpůsobte si barevné schéma a styl rozhraní", "Adjust the colour scheme and look of the interface"),
    ("Vzhled aplikace", "Application appearance"),
    ("Barevné schéma", "Colour scheme"),
    ("Výchozí WhatsApp Dark (#111B21)", "Default WhatsApp Dark (#111B21)"),
    ("OLED Černá (#000000)", "OLED Black (#000000)"),
    ("Půlnoční modrá (#0A192F)", "Midnight blue (#0A192F)"),
    // --- Mazání a potvrzování ---
    ("Smazat chat", "Delete chat"),
    ("Smazat", "Delete"),
    ("Vymazat", "Clear"),
    // --- Seznam chatů ---
    ("Archivované", "Archived"),
    ("Zobrazit archivované chaty", "Show archived chats"),
    ("Synchronizovat kontakty a názvy z WhatsAppu", "Sync contacts and names from WhatsApp"),
    ("Skočit na další nepřečtenou zprávu (Alt+↓)", "Jump to the next unread message (Alt+↓)"),
    ("Nová konverzace", "New chat"),
    ("👥  Nová skupina", "👥  New group"),
    ("Připojit se do skupiny", "Join a group"),
    ("Připojit", "Join"),
    ("Zadej odkaz nebo kód pozvánky", "Enter an invite link or code"),
    ("🔔  Umlčení", "🔔  Mute"),
    ("🔕  Umlčení", "🔕  Mute"),
    ("Vypnout umlčení", "Unmute"),
    ("8 hodin", "8 hours"),
    ("1 týden", "1 week"),
    ("Vždy", "Always"),
    ("Chat zmizí ze seznamu; zprávy zůstanou na serveru.", "The chat disappears from the list; the messages stay on the server."),
    ("Vymazat zprávy v chatu {}?", "Clear the messages in chat {}?"),
    ("Historie zmizí jen u tebe, ostatní ji pořád uvidí.", "The history disappears only for you; others still see it."),
    // --- Aktualizace ---
    ("Nainstalováno: v{}", "Installed: v{}"),
    ("K dispozici je v{}", "Version v{} is available"),
    ("⬇ Stáhnout a nainstalovat", "⬇ Download and install"),
    ("Stránka vydání", "Release page"),
    ("Později", "Later"),
    (
        "Aplikace se po stažení zavře, vymění se spustitelný soubor a znovu se otevře.",
        "The application closes after the download, swaps its executable and opens again.",
    ),
    (
        "Pro tuto platformu není v tomto vydání balíček ke stažení.",
        "This release has no downloadable package for this platform.",
    ),
    (
        "Automatická instalace je zapnutá — aktualizace proběhne sama.",
        "Automatic installation is on — the update installs itself.",
    ),
    ("Není k dispozici žádná novější verze.", "No newer version is available."),
    ("Zkontrolovat znovu", "Check again"),
    (
        "Je dostupná nová verze — klikni pro podrobnosti.",
        "A new version is available — click for details.",
    ),
    ("Zjišťuji novou verzi…", "Checking for a new version…"),
    ("Máš nejnovější verzi.", "You have the newest version."),
    ("Dostupná verze {}.", "Version {} is available."),
    ("Stahuji aktualizaci…", "Downloading the update…"),
    ("Chyba", "Error"),
    (
        "vydání nemá balíček pro tuto platformu",
        "this release has no package for this platform",
    ),
    // --- Ostatní ---
    ("Přehrávání", "Playback"),
    ("Otevřít Warust", "Open Warust"),
    ("Ukončit Warust", "Quit Warust"),
    ("Skočit na nejnovější zprávy", "Jump to the newest messages"),
];

/// Jazyk je procesová konstanta, ale testy běží paralelně — ty, které ho
/// přepínají nebo kontrolují české texty, se musí navzájem vyloučit.
#[cfg(test)]
pub(crate) fn lock_language() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    set_language(Language::Cs);
    guard
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vypíše dvojice (soubor, literál) pro každé `tr!`/`trf!` v `src/`.
    ///
    /// Vlastní soubory přeskočíme — obsahují české texty jako klíče tabulky,
    /// ne jako volání v UI.
    fn literals_in_source() -> Vec<(String, String)> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect_rs_files(&root, &mut files);

        let mut found = Vec::new();
        for path in files {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            if name == "i18n.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            for marker in ["tr!(", "trf!("] {
                let mut rest = text.as_str();
                while let Some(pos) = rest.find(marker) {
                    rest = &rest[pos + marker.len()..];
                    let Some(literal) = take_string_literal(rest) else {
                        continue;
                    };
                    found.push((name.clone(), literal));
                }
            }
        }
        found
    }

    fn collect_rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    /// Přečte literál v uvozovkách na začátku `text` a vrátí ho i s escapy
    /// převedenými na skutečné znaky.
    fn take_string_literal(text: &str) -> Option<String> {
        let mut chars = text.trim_start().strip_prefix('"')?.chars();
        let mut out = String::new();
        while let Some(c) = chars.next() {
            match c {
                '"' => return Some(out),
                '\\' => match chars.next()? {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    other => out.push(other),
                },
                other => out.push(other),
            }
        }
        None
    }

    fn placeholder_count(text: &str) -> usize {
        text.matches("{}").count()
    }


    #[test]
    fn anglictina_se_nacte_az_po_prepnuti() {
        let _guard = lock_language();
        assert_eq!(t("Zavřít"), "Zavřít");

        set_language(Language::En);
        assert_eq!(t("Zavřít"), "Close");

        // Neznámý text zůstane česky, aby chybějící překlad nerozbil UI.
        assert_eq!(t("Text, který v tabulce není"), "Text, který v tabulce není");

        set_language(Language::Cs);
    }

    #[test]
    fn sablona_se_dosadi_ve_spravnem_poradi() {
        let _guard = lock_language();
        set_language(Language::En);
        assert_eq!(
            tf("Vymazat zprávy v chatu {}?", &[&"Adam"]),
            "Clear the messages in chat Adam?"
        );
        assert_eq!(tf("Zavřít", &[]), "Close");
        // Víc zástupných znaků než argumentů nesmí panikařit.
        assert_eq!(tf("A {} B {}", &[&1]), "A 1 B {}");
    }

    #[test]
    fn kazdy_prelozeny_text_ma_anglicky_protejsek() {
        let mut missing = Vec::new();
        for (file, literal) in literals_in_source() {
            if english(&literal).is_none() {
                missing.push(format!("{file}: {literal}"));
            }
        }
        assert!(
            missing.is_empty(),
            "tyto texty nemají anglický překlad v i18n::ENGLISH:\n{}",
            missing.join("\n")
        );
    }

    #[test]
    fn tabulka_neobsahuje_nepouzite_preklady() {
        let used: std::collections::HashSet<String> = literals_in_source()
            .into_iter()
            .map(|(_, literal)| literal)
            .collect();
        let unused: Vec<&str> = ENGLISH
            .iter()
            .map(|(cs, _)| *cs)
            .filter(|cs| !used.contains(*cs))
            .collect();
        assert!(
            unused.is_empty(),
            "tyto překlady už v kódu nikdo nepoužívá, smaž je z i18n::ENGLISH:\n{}",
            unused.join("\n")
        );
    }

    #[test]
    fn tabulka_ma_jedinecne_klice() {
        let mut seen = std::collections::HashSet::new();
        let mut duplicates = Vec::new();
        for (cs, _) in ENGLISH {
            if !seen.insert(*cs) {
                duplicates.push(*cs);
            }
        }
        assert!(duplicates.is_empty(), "duplicitní klíče: {duplicates:?}");
    }

    #[test]
    fn preklady_zachovavaji_pocet_zastupnych_znaku() {
        let mut broken = Vec::new();
        for (cs, en) in ENGLISH {
            if placeholder_count(cs) != placeholder_count(en) {
                broken.push(format!("{cs} → {en}"));
            }
        }
        assert!(
            broken.is_empty(),
            "česká a anglická verze nemají stejný počet `{{}}`:\n{}",
            broken.join("\n")
        );
    }
}
