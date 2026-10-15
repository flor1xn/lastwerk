//! lastwerk: Kommandozeile für Import, Berechnung, Plan und Export
//!
//! Befehle nach docs/SPEC.md, Abschnitt 5.4: `import`, `import-recovery`, `rebuild`, `plausibility`, `garmin` (`status`, `sync`, `logout`),
//! `thresholds` (`set`, `list`, `suggest`), `athlete set`, `pmc`, `plan`, `plans`, `plan-status`,
//! `validate`, `refine`, `chat`, `export-ics`, `backup`, `restore`, `goal` (`set`, `list`) und
//! `availability` (`set`, `list`).
//!
//! Exit-Codes: 0 Erfolg, 1 Validierungsfehler, 2 Eingabefehler (Ordner fehlt, Argumente
//! falsch, Datenbank unbrauchbar), 3 interner Fehler. Fehlerhafte FIT-Dateien sind kein
//! Fehler des Befehls: sie stehen im Bericht, der Exit-Code bleibt 0.

use std::path::PathBuf;
use std::process::ExitCode;

use chrono::NaiveDate;
use clap::{Args, Parser, Subcommand, ValueEnum};
use lastwerk_core::plan::{Locale, PlanSport, Priority};
use lastwerk_core::{Sex, Sport};
use lastwerk_store::{PlanStatus, ThresholdKind};

use lastwerk_cli::chat::ChatRequest;
use lastwerk_cli::commands;
use lastwerk_cli::planning::{DEFAULT_STRENGTH_PER_WEEK, PlanRequest};
use lastwerk_cli::pmc::{PmcFormat, PmcOverrides, PmcRequest};
use lastwerk_cli::refine::{ProviderChoice, RefineRequest};
use lastwerk_cli::thresholds::{AthleteRequest, SetRequest, SuggestRequest};
use lastwerk_cli::userdata::{AvailabilityRequest, GoalRequest};

#[derive(Debug, Parser)]
#[command(
    name = "lastwerk",
    version,
    about = "Lastwerk: Belastung, Fitness und Wochenplan aus FIT-Dateien, lokal und ohne Cloud"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Liest einen Ordner mit FIT-Dateien ein (rekursiv, unveränderte Dateien werden übersprungen)
    Import {
        /// Ordner mit den FIT-Dateien
        #[arg(long)]
        dir: PathBuf,
        /// Datenbankdatei (Standard: Lastwerk/lastwerk.db im lokalen Datenordner)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Liest Erholungstage (`YYYY-MM-DD.json`, Schema recovery-day) aus dem Austauschordner des
    /// Zusatzprogramms. Ungültige Dateien werden abgelehnt und gezählt.
    ImportRecovery {
        /// Ordner mit den Dateien, zum Beispiel exchange/out/recovery
        #[arg(long)]
        dir: PathBuf,
        /// Datenbankdatei (Standard: Lastwerk/lastwerk.db im lokalen Datenordner)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Baut den Cache neu auf: leert alle abgeleiteten Tabellen und liest den Ordner neu ein
    Rebuild {
        /// Ordner mit den FIT-Dateien
        #[arg(long)]
        dir: PathBuf,
        /// Datenbankdatei (Standard: Lastwerk/lastwerk.db im lokalen Datenordner)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Gibt nur Kennzahlen zu einem Ordner aus (nichts wird gespeichert, keine Dateinamen)
    Plausibility {
        /// Ordner mit den FIT-Dateien
        #[arg(long)]
        dir: PathBuf,
        /// Datenbank, aus der nur die Schwellenwerte gelesen werden, damit Belastungen
        /// berechenbar sind. Ohne Angabe sind alle Belastungen "nicht berechenbar".
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Schwellenwerte mit Gültigkeitsdatum setzen, auflisten und vorschlagen lassen
    Thresholds {
        /// Datenbankdatei (Standard: Lastwerk/lastwerk.db im lokalen Datenordner)
        #[arg(long, global = true)]
        db: Option<PathBuf>,
        #[command(subcommand)]
        action: ThresholdAction,
    },
    /// Angaben zur Person, die nur Berechnungen brauchen (Geschlecht, Geburtsjahr)
    Athlete {
        /// Datenbankdatei (Standard: Lastwerk/lastwerk.db im lokalen Datenordner)
        #[arg(long, global = true)]
        db: Option<PathBuf>,
        #[command(subcommand)]
        action: AthleteAction,
    },
    /// Gibt Fitness (CTL), Ermüdung (ATL) und Form (TSB) je Tag aus
    Pmc {
        /// Datenbankdatei (Standard: Lastwerk/lastwerk.db im lokalen Datenordner)
        #[arg(long)]
        db: Option<PathBuf>,
        /// Erster Tag (JJJJ-MM-TT)
        #[arg(long, value_parser = parse_date)]
        from: NaiveDate,
        /// Letzter Tag einschließlich (JJJJ-MM-TT)
        #[arg(long, value_parser = parse_date)]
        to: NaiveDate,
        /// Ausgabeformat
        #[arg(long, value_enum, default_value_t = FormatArg::Csv)]
        format: FormatArg,
        /// Zeitkonstante der Fitness in Tagen (Standard: Einstellung `pmc.tau_ctl_d`, sonst 42,
        /// Konvention der Praxis)
        #[arg(long)]
        tau_ctl: Option<f32>,
        /// Zeitkonstante der Ermüdung in Tagen (Standard: Einstellung `pmc.tau_atl_d`, sonst 7,
        /// Konvention der Praxis)
        #[arg(long)]
        tau_atl: Option<f32>,
        /// Startwert der Fitness am ersten Tag mit Daten (Standard: Einstellung `pmc.ctl0`, sonst 0)
        #[arg(long)]
        ctl0: Option<f32>,
        /// Startwert der Ermüdung am ersten Tag mit Daten (Standard: Einstellung `pmc.atl0`, sonst 0)
        #[arg(long)]
        atl0: Option<f32>,
    },
    /// Erzeugt einen regelbasierten Wochenplan, schreibt ihn als JSON und speichert ihn als
    /// Entwurf
    Plan {
        /// Datenbankdatei (Standard: Lastwerk/lastwerk.db im lokalen Datenordner)
        #[arg(long)]
        db: Option<PathBuf>,
        /// Montag der ersten Planwoche (JJJJ-MM-TT)
        #[arg(long, value_parser = parse_date)]
        week_start: NaiveDate,
        /// Termin des Ziels (JJJJ-MM-TT). Ohne Angabe gilt das aktive Ziel der Datenbank,
        /// sonst wird fortlaufend geplant.
        #[arg(long, value_parser = parse_date)]
        goal_date: Option<NaiveDate>,
        /// Sportart des Termins (Standard: die Sportart mit dem größten Gewicht)
        #[arg(long, value_enum)]
        goal_sport: Option<EnduranceArg>,
        /// Zahl der Wochen, 1 bis 26 (Standard: bis zur Woche des Termins, ohne Termin 8)
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..=26))]
        weeks: Option<u32>,
        /// Sprache der Texte im Plan
        #[arg(long, value_enum, default_value_t = LocaleArg::De)]
        locale: LocaleArg,
        /// Sportarten, kommagetrennt (Standard: die der letzten zwölf Wochen)
        #[arg(long, value_enum, value_delimiter = ',')]
        sports: Vec<EnduranceArg>,
        /// Gewünschte Krafteinheiten je Woche (R7 begrenzt auf 1 bis 2)
        #[arg(long, default_value_t = DEFAULT_STRENGTH_PER_WEEK, value_parser = clap::value_parser!(u8).range(0..=2))]
        strength: u8,
        /// Ausgabedatei für den Plan (JSON nach schema/plan.schema.json)
        #[arg(long)]
        out: PathBuf,
    },
    /// Listet die gespeicherten Pläne (ID, Status, Erstellungsdatum, Wochen), die neuesten zuerst
    Plans {
        /// Datenbankdatei (Standard: Lastwerk/lastwerk.db im lokalen Datenordner)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Setzt den Status eines gespeicherten Plans. `active` archiviert den bisherigen aktiven
    /// Plan, nur ein aktiver Plan ist für den MCP-Server sichtbar. Unbekannte ID: Exit 2.
    PlanStatus {
        /// Datenbankdatei (Standard: Lastwerk/lastwerk.db im lokalen Datenordner)
        #[arg(long)]
        db: Option<PathBuf>,
        /// Plan-ID (siehe `lastwerk plans`)
        #[arg(long)]
        id: String,
        /// Neuer Status
        #[arg(long, value_enum)]
        status: PlanStatusArg,
    },
    /// Prüft einen Plan gegen das Schema und die Leitplanken. Exit 1 bei Verletzung.
    Validate {
        /// Plan als JSON
        plan: PathBuf,
        /// Datenbankdatei (Standard: Lastwerk/lastwerk.db im lokalen Datenordner)
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Lässt ein lokales Sprachmodell einen Plan anpassen und erklären (Ollama oder lokaler
    /// OpenAI-kompatibler Server). Zeigt vorher den exakten Eingang und fragt nach. Bei jedem
    /// Fehler des Modells bleibt der Regelplan (Exit 0). Exit 2 bei nicht erlaubtem Host.
    Refine {
        /// Plan als JSON
        plan: PathBuf,
        /// Anbieter des lokalen Modells
        #[arg(long, value_enum)]
        provider: ProviderArg,
        /// Name des Modells, zum Beispiel qwen3:8b
        #[arg(long)]
        model: String,
        /// Adresse des Servers (Standard bei ollama: http://127.0.0.1:11434, bei
        /// openai-compatible keine). Nur Loopback, im lokalen Netz nur mit --allow-lan.
        #[arg(long)]
        base_url: Option<String>,
        /// Bestätigt, dass der Server im lokalen Netz steht (private Adresse)
        #[arg(long)]
        allow_lan: bool,
        /// Datenbankdatei (Standard: Lastwerk/lastwerk.db im lokalen Datenordner)
        #[arg(long)]
        db: Option<PathBuf>,
        /// Ausgabedatei (Standard: neben der Eingabe mit Endung .refined.json)
        #[arg(long)]
        out: Option<PathBuf>,
        /// Keine Rückfrage vor dem Senden (für Skripte)
        #[arg(long)]
        yes: bool,
        /// Zeitlimit je Aufruf an das Modell in Sekunden
        #[arg(long, default_value_t = lastwerk_coach::DEFAULT_TIMEOUT_S, value_parser = clap::value_parser!(u32).range(1..=3600))]
        timeout: u32,
    },
    /// Ein Gesprächsschritt mit dem lokalen Coach (Ollama oder lokaler OpenAI-kompatibler
    /// Server), zum Ausprobieren. Zeigt Antwort und geprüften Vorschlag, übernimmt und
    /// speichert nichts. Exit 2 bei nicht erlaubtem Host.
    Chat {
        /// Datenbankdatei (Standard: Lastwerk/lastwerk.db im lokalen Datenordner)
        #[arg(long)]
        db: Option<PathBuf>,
        /// Plan als JSON (Standard: der aktive Plan der Datenbank)
        #[arg(long)]
        plan: Option<PathBuf>,
        /// Die Nachricht an den Coach, höchstens 1000 Zeichen
        #[arg(long)]
        message: String,
        /// Anbieter des lokalen Modells
        #[arg(long, value_enum, default_value_t = ProviderArg::Ollama)]
        provider: ProviderArg,
        /// Name des Modells (Standard bei ollama: ein vorhandenes Modell, bei
        /// openai-compatible Pflicht)
        #[arg(long)]
        model: Option<String>,
        /// Adresse des Servers (Standard bei ollama: http://127.0.0.1:11434, bei
        /// openai-compatible keine). Nur Loopback, im lokalen Netz nur mit --allow-lan.
        #[arg(long)]
        base_url: Option<String>,
        /// Bestätigt, dass der Server im lokalen Netz steht (private Adresse)
        #[arg(long)]
        allow_lan: bool,
        /// Zeigt vor dem Senden genau den Text, der zum Modell geht
        #[arg(long)]
        show_input: bool,
        /// Zeitlimit je Aufruf an das Modell in Sekunden
        #[arg(long, default_value_t = lastwerk_coach::DEFAULT_TIMEOUT_S, value_parser = clap::value_parser!(u32).range(1..=3600))]
        timeout: u32,
    },
    /// Schreibt einen Plan als ICS-Datei (nur Titel, Zeit und Dauer, keine Orte)
    ExportIcs {
        /// Plan als JSON
        plan: PathBuf,
        /// Ausgabedatei (ICS)
        #[arg(long)]
        out: PathBuf,
    },
    /// Sichert die Nutzerdaten (Schwellenwerte, Ziele, Verfügbarkeit, Pläne, Einstellungen) als
    /// JSON
    Backup {
        /// Datenbankdatei (Standard: Lastwerk/lastwerk.db im lokalen Datenordner)
        #[arg(long)]
        db: Option<PathBuf>,
        /// Ausgabedatei (JSON)
        #[arg(long)]
        out: PathBuf,
    },
    /// Spielt eine Sicherung ein und ersetzt dabei die Nutzerdaten der Datenbank
    Restore {
        /// Datenbankdatei (Standard: Lastwerk/lastwerk.db im lokalen Datenordner)
        #[arg(long)]
        db: Option<PathBuf>,
        /// Sicherung (JSON)
        #[arg(long = "in")]
        input: PathBuf,
    },
    /// Abgleich mit Garmin: Status, Aktivitäten und Erholungswerte holen, Verbindung trennen.
    /// Die Anmeldung gibt es nur im Fenster der App.
    Garmin {
        /// Datenbankdatei (Standard: Lastwerk/lastwerk.db im lokalen Datenordner)
        #[arg(long, global = true)]
        db: Option<PathBuf>,
        #[command(subcommand)]
        action: GarminAction,
    },
    /// Ziele für den Planer setzen und auflisten
    Goal {
        /// Datenbankdatei (Standard: Lastwerk/lastwerk.db im lokalen Datenordner)
        #[arg(long, global = true)]
        db: Option<PathBuf>,
        #[command(subcommand)]
        action: GoalAction,
    },
    /// Verfügbarkeit je Wochentag für den Planer setzen und auflisten
    Availability {
        /// Datenbankdatei (Standard: Lastwerk/lastwerk.db im lokalen Datenordner)
        #[arg(long, global = true)]
        db: Option<PathBuf>,
        #[command(subcommand)]
        action: AvailabilityAction,
    },
}

#[derive(Debug, Subcommand)]
enum GarminAction {
    /// Zeigt, ob eine Verbindung besteht, und den letzten Abgleich (ohne Netzzugriff)
    Status,
    /// Holt neue Aktivitäten (FIT) und Erholungswerte (HRV, Ruhepuls, Schlaf, Body Battery)
    /// und importiert sie. Ohne --days ab dem letzten Abgleich minus 7 Tage, beim ersten Mal
    /// so weit zurück, wie die Einstellung garmin.backfill_days sagt (Standard 365).
    Sync {
        /// Zahl der Tage vor heute, die dieser Lauf abdeckt (1 bis 1825), statt des Fensters
        /// aus dem letzten Abgleich
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..=1825))]
        days: Option<u32>,
    },
    /// Trennt die Verbindung und löscht die Token aus dem Schlüsselbund
    Logout,
}

#[derive(Debug, Subcommand)]
enum GoalAction {
    /// Setzt ein neues aktives Ziel: mit --date ein Termin, ohne ein fortlaufendes Ziel
    Set {
        /// Termin (JJJJ-MM-TT)
        #[arg(long, value_parser = parse_date)]
        date: Option<NaiveDate>,
        /// Sportart des Termins
        #[arg(long, value_enum)]
        sport: Option<EnduranceArg>,
        /// Priorität des Termins (A, B oder C)
        #[arg(long, value_enum, ignore_case = true)]
        priority: Option<PriorityArg>,
        /// Bezeichnung (bleibt lokal)
        #[arg(long)]
        label: Option<String>,
    },
    /// Listet alle Ziele
    List,
}

#[derive(Debug, Subcommand)]
enum AvailabilityAction {
    /// Setzt die Verfügbarkeit eines Wochentags
    Set {
        /// Wochentag (mon bis sun, auch mo bis so)
        #[arg(long, value_enum)]
        weekday: WeekdayArg,
        /// Höchste Trainingszeit des Tages in Minuten (0 bis 1440)
        #[arg(long, value_parser = clap::value_parser!(u16).range(0..=1440))]
        max_minutes: u16,
        /// Harte Einheiten erlaubt
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        allow_hard: bool,
        /// Fester Ruhetag
        #[arg(long)]
        fixed_rest: bool,
    },
    /// Listet die gesetzten Wochentage
    List,
}

/// Ausdauersportarten, die ein Ziel oder ein Plan wählen darf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum EnduranceArg {
    Cycling,
    Running,
    Swimming,
}

impl From<EnduranceArg> for PlanSport {
    fn from(s: EnduranceArg) -> Self {
        match s {
            EnduranceArg::Cycling => Self::Cycling,
            EnduranceArg::Running => Self::Running,
            EnduranceArg::Swimming => Self::Swimming,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ProviderArg {
    Ollama,
    OpenaiCompatible,
}

impl From<ProviderArg> for ProviderChoice {
    fn from(p: ProviderArg) -> Self {
        match p {
            ProviderArg::Ollama => Self::Ollama,
            ProviderArg::OpenaiCompatible => Self::OpenAiCompatible,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum LocaleArg {
    De,
    En,
}

impl From<LocaleArg> for Locale {
    fn from(l: LocaleArg) -> Self {
        match l {
            LocaleArg::De => Self::De,
            LocaleArg::En => Self::En,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum PlanStatusArg {
    Draft,
    Active,
    Archived,
}

impl From<PlanStatusArg> for PlanStatus {
    fn from(s: PlanStatusArg) -> Self {
        match s {
            PlanStatusArg::Draft => Self::Draft,
            PlanStatusArg::Active => Self::Active,
            PlanStatusArg::Archived => Self::Archived,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum PriorityArg {
    A,
    B,
    C,
}

impl From<PriorityArg> for Priority {
    fn from(p: PriorityArg) -> Self {
        match p {
            PriorityArg::A => Self::A,
            PriorityArg::B => Self::B,
            PriorityArg::C => Self::C,
        }
    }
}

/// Wochentag, Montag zuerst. Die deutschen Kürzel sind Aliase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum WeekdayArg {
    #[value(alias = "mo")]
    Mon,
    #[value(alias = "di")]
    Tue,
    #[value(alias = "mi")]
    Wed,
    #[value(alias = "do")]
    Thu,
    #[value(alias = "fr")]
    Fri,
    #[value(alias = "sa")]
    Sat,
    #[value(alias = "so")]
    Sun,
}

impl WeekdayArg {
    /// 0 = Montag, wie in `availability.weekday`.
    const fn index(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Subcommand)]
enum ThresholdAction {
    /// Setzt einen Schwellenwert ab einem Datum. Frühere Aktivitäten behalten ihre Schwelle.
    Set(SetArgs),
    /// Listet alle gesetzten Schwellenwerte
    List,
    /// Zeigt Vorschläge nach SPEC 8.6 an. Übernommen wird nichts.
    Suggest {
        /// Nur für diese Sportart vorschlagen
        #[arg(long, value_enum)]
        sport: Option<SportArg>,
        /// Zeit in Sekunden für 400 m Schwimmen (CSS-Vorschlag, mit --css-t200)
        #[arg(long, requires = "css_t200")]
        css_t400: Option<f32>,
        /// Zeit in Sekunden für 200 m Schwimmen (CSS-Vorschlag, mit --css-t400)
        #[arg(long, requires = "css_t400")]
        css_t200: Option<f32>,
    },
}

#[derive(Debug, Args)]
struct SetArgs {
    /// Sportart, für die die Schwelle gilt (`all` für alle, zum Beispiel Ruhepuls)
    #[arg(long, value_enum)]
    sport: SportArg,
    /// Art der Schwelle (mit --value), alternativ eine der Kurzformen
    #[arg(long, value_enum, requires = "value", conflicts_with_all = ["ftp", "lthr", "hr_max", "hr_rest", "run_speed", "css"])]
    kind: Option<KindArg>,
    /// Wert zu --kind
    #[arg(long, requires = "kind", allow_negative_numbers = true)]
    value: Option<f64>,
    /// FTP in Watt
    #[arg(long)]
    ftp: Option<f64>,
    /// Schwellenherzfrequenz in bpm
    #[arg(long)]
    lthr: Option<f64>,
    /// Maximale Herzfrequenz in bpm
    #[arg(long)]
    hr_max: Option<f64>,
    /// Ruhepuls in bpm
    #[arg(long)]
    hr_rest: Option<f64>,
    /// Laufschwellengeschwindigkeit in m/s
    #[arg(long)]
    run_speed: Option<f64>,
    /// Kritische Schwimmgeschwindigkeit (CSS) in m/s
    #[arg(long)]
    css: Option<f64>,
    /// Gültig ab diesem Datum (JJJJ-MM-TT)
    #[arg(long = "from", value_parser = parse_date)]
    valid_from: NaiveDate,
    /// Freitext zur Schwelle
    #[arg(long)]
    note: Option<String>,
    /// Der Wert stammt aus einem Vorschlag, den du ausdrücklich übernimmst
    #[arg(long)]
    from_suggestion: bool,
    /// Ordner mit den FIT-Dateien: berechnet betroffene Aktivitäten neu
    #[arg(long)]
    dir: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum AthleteAction {
    /// Setzt Geschlecht (TRIMP-Konstanten) und/oder Geburtsjahr (Schätzung der maximalen
    /// Herzfrequenz)
    Set {
        /// Geschlecht, nur für die Wahl der TRIMP-Konstanten
        #[arg(long, value_enum)]
        sex: Option<SexArg>,
        /// Geburtsjahr, nur für `208 - 0,7 * Alter` bei fehlender maximaler Herzfrequenz
        #[arg(long)]
        birth_year: Option<i32>,
        /// Ordner mit den FIT-Dateien: berechnet alle Aktivitäten neu
        #[arg(long)]
        dir: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SportArg {
    Cycling,
    Running,
    Swimming,
    All,
}

impl SportArg {
    /// `None` heißt alle Sportarten.
    fn sport(self) -> Option<Sport> {
        match self {
            Self::Cycling => Some(Sport::Cycling),
            Self::Running => Some(Sport::Running),
            Self::Swimming => Some(Sport::Swimming),
            Self::All => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum KindArg {
    #[value(name = "ftp_w")]
    FtpW,
    #[value(name = "lthr_bpm")]
    LthrBpm,
    #[value(name = "hr_max_bpm")]
    HrMaxBpm,
    #[value(name = "hr_rest_bpm")]
    HrRestBpm,
    #[value(name = "run_speed_mps")]
    RunSpeedMps,
    #[value(name = "css_mps")]
    CssMps,
}

impl From<KindArg> for ThresholdKind {
    fn from(k: KindArg) -> Self {
        match k {
            KindArg::FtpW => Self::FtpW,
            KindArg::LthrBpm => Self::LthrBpm,
            KindArg::HrMaxBpm => Self::HrMaxBpm,
            KindArg::HrRestBpm => Self::HrRestBpm,
            KindArg::RunSpeedMps => Self::RunSpeedMps,
            KindArg::CssMps => Self::CssMps,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SexArg {
    Male,
    Female,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum FormatArg {
    Csv,
    Json,
}

fn parse_date(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| format!("'{s}' ist kein Datum im Format JJJJ-MM-TT"))
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let now = chrono::Utc::now();
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    let mut out = stdout.lock();
    let mut err = stderr.lock();
    match commands::run(cli.command_args(), now, &mut out, &mut err) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Fehler: {}", e.message());
            ExitCode::from(e.exit_code())
        }
    }
}

impl Cli {
    fn command_args(self) -> commands::Invocation {
        match self.command {
            Command::Import { dir, db } => commands::Invocation::Import { dir, db },
            Command::ImportRecovery { dir, db } => commands::Invocation::ImportRecovery { dir, db },
            Command::Rebuild { dir, db } => commands::Invocation::Rebuild { dir, db },
            Command::Plausibility { dir, db } => commands::Invocation::Plausibility { dir, db },
            Command::Thresholds { db, action } => match action {
                ThresholdAction::Set(args) => commands::Invocation::ThresholdsSet {
                    db,
                    request: args.into_request(),
                },
                ThresholdAction::List => commands::Invocation::ThresholdsList { db },
                ThresholdAction::Suggest {
                    sport,
                    css_t400,
                    css_t200,
                } => commands::Invocation::ThresholdsSuggest {
                    db,
                    request: SuggestRequest {
                        sport: sport.and_then(SportArg::sport),
                        css_trial_s: css_t400.zip(css_t200),
                    },
                },
            },
            Command::Athlete { db, action } => match action {
                AthleteAction::Set {
                    sex,
                    birth_year,
                    dir,
                } => commands::Invocation::AthleteSet {
                    db,
                    request: AthleteRequest {
                        sex: sex.map(|s| match s {
                            SexArg::Male => Sex::Male,
                            SexArg::Female => Sex::Female,
                        }),
                        birth_year,
                        dir,
                    },
                },
            },
            Command::Pmc {
                db,
                from,
                to,
                format,
                tau_ctl,
                tau_atl,
                ctl0,
                atl0,
            } => commands::Invocation::Pmc {
                db,
                request: PmcRequest {
                    from,
                    to,
                    format: match format {
                        FormatArg::Csv => PmcFormat::Csv,
                        FormatArg::Json => PmcFormat::Json,
                    },
                    overrides: PmcOverrides {
                        tau_ctl_d: tau_ctl,
                        tau_atl_d: tau_atl,
                        ctl0,
                        atl0,
                    },
                },
            },
            Command::Plan {
                db,
                week_start,
                goal_date,
                goal_sport,
                weeks,
                locale,
                sports,
                strength,
                out,
            } => commands::Invocation::Plan {
                db,
                request: PlanRequest {
                    week_start,
                    goal_date,
                    goal_sport: goal_sport.map(Into::into),
                    weeks,
                    locale: locale.into(),
                    sports: sports.into_iter().map(Into::into).collect(),
                    strength_per_week: strength,
                },
                out,
            },
            Command::Plans { db } => commands::Invocation::Plans { db },
            Command::PlanStatus { db, id, status } => commands::Invocation::PlanStatus {
                db,
                id,
                status: status.into(),
            },
            Command::Validate { plan, db } => commands::Invocation::Validate { db, plan },
            Command::ExportIcs { plan, out } => commands::Invocation::ExportIcs { plan, out },
            Command::Refine {
                plan,
                provider,
                model,
                base_url,
                allow_lan,
                db,
                out,
                yes,
                timeout,
            } => commands::Invocation::Refine {
                db,
                request: RefineRequest {
                    plan,
                    provider: provider.into(),
                    model,
                    base_url,
                    allow_lan,
                    out,
                    yes,
                    timeout_s: timeout,
                },
            },
            Command::Chat {
                db,
                plan,
                message,
                provider,
                model,
                base_url,
                allow_lan,
                show_input,
                timeout,
            } => commands::Invocation::Chat {
                db,
                request: ChatRequest {
                    plan,
                    message,
                    provider: provider.into(),
                    model,
                    base_url,
                    allow_lan,
                    show_input,
                    timeout_s: timeout,
                },
            },
            Command::Backup { db, out } => commands::Invocation::Backup { db, out },
            Command::Restore { db, input } => commands::Invocation::Restore { db, input },
            Command::Garmin { db, action } => match action {
                GarminAction::Status => commands::Invocation::GarminStatus { db },
                GarminAction::Sync { days } => commands::Invocation::GarminSync { db, days },
                GarminAction::Logout => commands::Invocation::GarminLogout,
            },
            Command::Goal { db, action } => match action {
                GoalAction::Set {
                    date,
                    sport,
                    priority,
                    label,
                } => commands::Invocation::GoalSet {
                    db,
                    request: GoalRequest {
                        date,
                        sport: sport.map(Into::into),
                        priority: priority.map(Into::into),
                        label,
                    },
                },
                GoalAction::List => commands::Invocation::GoalList { db },
            },
            Command::Availability { db, action } => match action {
                AvailabilityAction::Set {
                    weekday,
                    max_minutes,
                    allow_hard,
                    fixed_rest,
                } => commands::Invocation::AvailabilitySet {
                    db,
                    request: AvailabilityRequest {
                        weekday: weekday.index(),
                        max_minutes,
                        allow_hard,
                        fixed_rest,
                    },
                },
                AvailabilityAction::List => commands::Invocation::AvailabilityList { db },
            },
        }
    }
}

impl SetArgs {
    /// Sammelt die angegebenen Schwellen: `--kind` mit `--value` oder die Kurzformen
    /// (mehrere zugleich erlaubt, jede wird ein eigener Eintrag).
    fn into_request(self) -> SetRequest {
        let mut entries: Vec<(ThresholdKind, f64)> = Vec::new();
        if let (Some(kind), Some(value)) = (self.kind, self.value) {
            entries.push((kind.into(), value));
        }
        for (kind, value) in [
            (ThresholdKind::FtpW, self.ftp),
            (ThresholdKind::LthrBpm, self.lthr),
            (ThresholdKind::HrMaxBpm, self.hr_max),
            (ThresholdKind::HrRestBpm, self.hr_rest),
            (ThresholdKind::RunSpeedMps, self.run_speed),
            (ThresholdKind::CssMps, self.css),
        ] {
            if let Some(v) = value {
                entries.push((kind, v));
            }
        }
        SetRequest {
            sport: self.sport.sport(),
            entries,
            valid_from: self.valid_from,
            note: self.note,
            from_suggestion: self.from_suggestion,
            dir: self.dir,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn command_definition_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn missing_arguments_are_input_errors() {
        // clap beendet mit Exit-Code 2, das passt zu "Eingabefehler".
        let err = Cli::try_parse_from(["lastwerk", "import"]).unwrap_err();
        assert_eq!(err.exit_code(), 2);
        let err = Cli::try_parse_from(["lastwerk", "unbekannt"]).unwrap_err();
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn import_arguments_are_parsed() {
        let cli =
            Cli::try_parse_from(["lastwerk", "import", "--dir", "a", "--db", "b.db"]).unwrap();
        match cli.command {
            Command::Import { dir, db } => {
                assert_eq!(dir, PathBuf::from("a"));
                assert_eq!(db, Some(PathBuf::from("b.db")));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn garmin_commands_are_parsed_and_days_are_limited() {
        let sync = |args: &[&str]| {
            let mut argv = vec!["lastwerk", "garmin"];
            argv.extend_from_slice(args);
            Cli::try_parse_from(argv)
        };
        match sync(&["sync", "--days", "30", "--db", "x.db"])
            .unwrap()
            .command
        {
            Command::Garmin {
                db,
                action: GarminAction::Sync { days },
            } => {
                assert_eq!(days, Some(30));
                assert_eq!(db, Some(PathBuf::from("x.db")));
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            sync(&["sync"]).unwrap().command,
            Command::Garmin {
                action: GarminAction::Sync { days: None },
                ..
            }
        ));
        assert!(matches!(
            sync(&["status"]).unwrap().command,
            Command::Garmin {
                action: GarminAction::Status,
                ..
            }
        ));
        assert!(matches!(
            sync(&["logout"]).unwrap().command,
            Command::Garmin {
                action: GarminAction::Logout,
                ..
            }
        ));
        // 1 bis 1825 Tage, wie die Einstellung garmin.backfill_days. Eine Anmeldung gibt es
        // in der Kommandozeile nicht.
        for bad in ["0", "1826", "-1", "x"] {
            assert_eq!(sync(&["sync", "--days", bad]).unwrap_err().exit_code(), 2);
        }
        assert_eq!(sync(&["login"]).unwrap_err().exit_code(), 2);
    }

    fn set_request(args: &[&str]) -> Result<SetRequest, clap::Error> {
        let mut argv = vec!["lastwerk", "thresholds"];
        argv.extend_from_slice(args);
        match Cli::try_parse_from(argv)?.command {
            Command::Thresholds {
                action: ThresholdAction::Set(a),
                ..
            } => Ok(a.into_request()),
            other => panic!("{other:?}"),
        }
    }

    /// Das Beispiel aus SPEC 5.4: `thresholds --db <datei> set --sport cycling --ftp 250 --from
    /// 2026-01-01`.
    #[test]
    fn spec_example_for_thresholds_set_is_accepted() {
        let r = set_request(&[
            "--db",
            "x.db",
            "set",
            "--sport",
            "cycling",
            "--ftp",
            "250",
            "--from",
            "2026-01-01",
        ])
        .unwrap();
        assert_eq!(r.sport, Some(Sport::Cycling));
        assert_eq!(r.entries, vec![(ThresholdKind::FtpW, 250.0)]);
        assert_eq!(r.valid_from, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        assert!(!r.from_suggestion);
    }

    #[test]
    fn kind_and_value_or_shortcuts_but_not_both() {
        let r = set_request(&[
            "set",
            "--sport",
            "all",
            "--kind",
            "hr_rest_bpm",
            "--value",
            "48",
            "--from",
            "2026-02-01",
        ])
        .unwrap();
        assert_eq!(r.sport, None);
        assert_eq!(r.entries, vec![(ThresholdKind::HrRestBpm, 48.0)]);

        let r = set_request(&[
            "set",
            "--sport",
            "all",
            "--hr-max",
            "190",
            "--hr-rest",
            "50",
            "--from",
            "2026-02-01",
        ])
        .unwrap();
        assert_eq!(
            r.entries,
            vec![
                (ThresholdKind::HrMaxBpm, 190.0),
                (ThresholdKind::HrRestBpm, 50.0)
            ]
        );

        // kind ohne value, value ohne kind, Mischung und falsches Datum sind Eingabefehler.
        for bad in [
            vec![
                "set",
                "--sport",
                "all",
                "--kind",
                "ftp_w",
                "--from",
                "2026-01-01",
            ],
            vec![
                "set",
                "--sport",
                "all",
                "--value",
                "5",
                "--from",
                "2026-01-01",
            ],
            vec![
                "set",
                "--sport",
                "all",
                "--kind",
                "ftp_w",
                "--value",
                "250",
                "--ftp",
                "250",
                "--from",
                "2026-01-01",
            ],
            vec![
                "set",
                "--sport",
                "all",
                "--ftp",
                "250",
                "--from",
                "01.01.2026",
            ],
            vec![
                "set",
                "--sport",
                "rowing",
                "--ftp",
                "250",
                "--from",
                "2026-01-01",
            ],
        ] {
            let err = set_request(&bad).unwrap_err();
            assert_eq!(err.exit_code(), 2, "{bad:?}");
        }
    }

    #[test]
    fn pmc_arguments_have_the_documented_defaults() {
        let cli = Cli::try_parse_from([
            "lastwerk",
            "pmc",
            "--db",
            "x.db",
            "--from",
            "2026-01-01",
            "--to",
            "2026-03-31",
        ])
        .unwrap();
        match cli.command {
            Command::Pmc {
                format,
                tau_ctl,
                tau_atl,
                ctl0,
                atl0,
                ..
            } => {
                assert_eq!(format, FormatArg::Csv);
                // Ohne Option gelten die Einstellungen der Datenbank (Standard 42 und 7 Tage, Start 0).
                assert_eq!((tau_ctl, tau_atl, ctl0, atl0), (None, None, None, None));
            }
            other => panic!("{other:?}"),
        }
    }
}
