//! Anmeldefenster für Garmin (T23, SPEC 5.6, 10.4).
//!
//! Ein eigenes Fenster lädt die Anmeldeseite von Garmin. Passwort, MFA und Captcha laufen dort,
//! Lastwerk sieht sie nie. Nach Erfolg leitet Garmin auf `https://connect.garmin.com/app?ticket=…`
//! weiter; der Navigationsfilter erkennt genau diese Adresse ([`decide_navigation`]), bricht die
//! Navigation ab, schließt das Fenster und tauscht das Ticket in einem Arbeitsthread gegen Token.
//! Das Ergebnis geht als Ereignis `garmin-login` `{ ok, code }` an das Hauptfenster.
//!
//! Schutz des Fensters:
//! - keine Capability: `capabilities/default.json` gilt nur für `main` (ein Test prüft das), also
//!   kein Aufruf eines Befehls der App und kein Ereignis aus dieser Seite;
//! - Navigation nur über `https` zu `garmin.com` und dessen Unterdomänen, alles andere wird
//!   abgelehnt; neue Fenster und Downloads werden abgelehnt;
//! - Inkognito, damit Sitzungsdaten von Garmin nicht im Datenordner der Oberfläche bleiben;
//! - Entwicklerwerkzeuge nur im Debug-Build (im Release ohne die Tauri-Funktion `devtools` ohnehin
//!   nicht vorhanden).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use lastwerk_garmin::{Endpoints, GarminClient, Ticket, ticket_from_redirect};
use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, Emitter, Manager, Url, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::dto::GarminLoginEvent;
use crate::error::{AppError, Result};

/// Bezeichnung des Fensters. Steht in keiner Capability.
pub const LOGIN_WINDOW: &str = "garmin-login";
/// Ereignis mit dem Ergebnis der Anmeldung.
pub const EVENT_GARMIN_LOGIN: &str = "garmin-login";
/// Größe des Fensters in logischen Pixeln (Faustregel: passt zur Anmeldeseite von Garmin).
const WIDTH: f64 = 480.0;
const HEIGHT: f64 = 720.0;

/// Entscheidung des Navigationsfilters.
#[derive(Debug, PartialEq, Eq)]
pub enum NavDecision {
    /// Seite von Garmin über `https`: laden.
    Allow,
    /// Weiterleitung mit gültigem Ticket: nicht laden, Fenster schließen, Ticket tauschen.
    Ticket(Ticket),
    /// `https://connect.garmin.com/app…` ohne lesbares Ticket (etwa mit einem weiteren
    /// Parameter): nicht laden und klar melden, damit der Livetest ein Signal gibt (T21:
    /// `ticket_from_redirect` ist bewusst streng).
    UnexpectedRedirect,
    /// Alles andere: nicht laden.
    Deny,
}

/// Host gehört zu Garmin: genau `garmin.com` oder eine Unterdomäne davon. Vergleich auf dem
/// geparsten, kleingeschriebenen Host, nie auf dem Adresstext (`garmin.com.evil.example`,
/// `evilgarmin.com` und IP-Adressen fallen heraus).
fn is_garmin_host(host: &str) -> bool {
    host == "garmin.com" || host.ends_with(".garmin.com")
}

/// Entscheidet über eine Navigation im Anmeldefenster. Reine Funktion, damit sie ohne Fenster
/// mit feindlichen Adressen getestet werden kann.
pub fn decide_navigation(url: &str) -> NavDecision {
    if let Some(ticket) = ticket_from_redirect(url) {
        return NavDecision::Ticket(ticket);
    }
    let Ok(parsed) = Url::parse(url) else {
        return NavDecision::Deny;
    };
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
    {
        return NavDecision::Deny;
    }
    // Nur Namen, keine IP-Adressen (`domain` ist dort `None`); der Parser schreibt sie klein.
    // Leere Bestandteile (`.garmin.com`) gelten nicht als Name.
    let Some(host) = parsed.domain() else {
        return NavDecision::Deny;
    };
    if host.split('.').any(str::is_empty) || !is_garmin_host(host) {
        return NavDecision::Deny;
    }
    let path = parsed.path();
    if host == "connect.garmin.com" && (path == "/app" || path.starts_with("/app/")) {
        return NavDecision::UnexpectedRedirect;
    }
    NavDecision::Allow
}

/// Fenstertitel aus der Sprachdatei, ohne Netz und ohne Fremdtext.
fn window_title(locale: &str) -> String {
    let raw = if locale == "en" {
        include_str!("../../../catalog/locales/en.json")
    } else {
        include_str!("../../../catalog/locales/de.json")
    };
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|v| {
            v["ui"]["garmin"]["window_title"]
                .as_str()
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "Garmin".to_owned())
}

fn emit(app: &AppHandle, ok: bool, code: &str) {
    // Ein verlorenes Ereignis ist unkritisch: Der Zustand steht in `garmin_status`.
    let _ = app.emit(
        EVENT_GARMIN_LOGIN,
        GarminLoginEvent {
            ok,
            code: code.to_owned(),
        },
    );
}

/// Schließt das Fenster außerhalb des Navigationsfilters. Im Filter selbst zu schließen kann
/// je nach System den Webview-Thread blockieren.
fn close_window(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = handle.get_webview_window(LOGIN_WINDOW) {
            let _ = w.close();
        }
    });
}

fn exchange(app: AppHandle, client: Arc<GarminClient>, ticket: Ticket) {
    close_window(&app);
    tauri::async_runtime::spawn(async move {
        let result = tauri::async_runtime::spawn_blocking(move || {
            client.connect(&ticket).map_err(|e| e.code())
        })
        .await;
        match result {
            Ok(Ok(())) => emit(&app, true, "connected"),
            Ok(Err(code)) => emit(&app, false, code),
            Err(_) => emit(&app, false, "internal"),
        }
    });
}

/// Öffnet das Anmeldefenster. Ist es schon offen, holt es nur nach vorn.
pub fn open(app: &AppHandle, client: Arc<GarminClient>, locale: &str) -> Result<()> {
    if let Some(existing) = app.get_webview_window(LOGIN_WINDOW) {
        let _ = existing.set_focus();
        return Ok(());
    }
    let language = if locale == "en" { "en-US" } else { "de-DE" };
    let start = Endpoints::default()
        .sign_in_url(language)
        .and_then(|u| Url::parse(&u).ok())
        .ok_or_else(|| AppError::new("garmin_window_failed"))?;

    // Wer zuerst ein Ergebnis meldet, gewinnt: Ticket, unerwartete Weiterleitung oder das
    // Schließen durch den Nutzer. Danach meldet das Fenster nichts mehr.
    let finished = Arc::new(AtomicBool::new(false));
    let nav_finished = Arc::clone(&finished);
    let nav_app = app.clone();
    let window = WebviewWindowBuilder::new(app, LOGIN_WINDOW, WebviewUrl::External(start))
        .title(window_title(locale))
        .inner_size(WIDTH, HEIGHT)
        .center()
        .focused(true)
        .incognito(true)
        .devtools(cfg!(debug_assertions))
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .on_download(|_, _| false)
        .on_navigation(move |url| match decide_navigation(url.as_str()) {
            NavDecision::Allow => true,
            NavDecision::Deny => false,
            NavDecision::Ticket(ticket) => {
                if !nav_finished.swap(true, Ordering::AcqRel) {
                    exchange(nav_app.clone(), Arc::clone(&client), ticket);
                }
                false
            }
            NavDecision::UnexpectedRedirect => {
                // Nur der Pfad, nie die Query: Sie könnte ein Ticket enthalten.
                eprintln!("garmin-login: unexpected redirect, path {}", url.path());
                if !nav_finished.swap(true, Ordering::AcqRel) {
                    emit(&nav_app, false, "unexpected_redirect");
                    close_window(&nav_app);
                }
                false
            }
        })
        .build()
        .map_err(|_| AppError::new("garmin_window_failed"))?;

    let close_app = app.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) && !finished.swap(true, Ordering::AcqRel) {
            emit(&close_app, false, "cancelled");
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TICKET: &str = "https://connect.garmin.com/app?ticket=ST-0123456-AbCdEf-cas";

    #[test]
    fn the_redirect_with_a_ticket_is_caught() {
        assert!(matches!(decide_navigation(TICKET), NavDecision::Ticket(_)));
        assert!(matches!(
            decide_navigation("https://connect.garmin.com/app/?ticket=ST-1-x"),
            NavDecision::Ticket(_)
        ));
    }

    #[test]
    fn garmin_pages_over_https_are_allowed() {
        for url in [
            "https://sso.garmin.com/portal/sso/de-DE/sign-in?clientId=GarminConnect&service=https://connect.garmin.com/app",
            "https://sso.garmin.com/mfa/verify",
            "https://www.garmin.com/de-DE/privacy/connect/",
            "https://garmin.com/",
            "https://SSO.GARMIN.COM/portal",
            "https://static.garmincdn.garmin.com/x",
        ] {
            assert_eq!(decide_navigation(url), NavDecision::Allow, "{url}");
        }
    }

    #[test]
    fn the_app_address_without_a_valid_ticket_is_reported() {
        for url in [
            "https://connect.garmin.com/app",
            "https://connect.garmin.com/app/",
            "https://connect.garmin.com/app?ticket=ST-1-x&extra=1",
            "https://connect.garmin.com/app?ticket=ST-1-x#frag",
            "https://connect.garmin.com/app?ticket=",
            "https://connect.garmin.com/app/modern/",
        ] {
            assert_eq!(
                decide_navigation(url),
                NavDecision::UnexpectedRedirect,
                "{url}"
            );
        }
        // Andere Seiten von Connect sind gewöhnliche Garmin-Seiten.
        assert_eq!(
            decide_navigation("https://connect.garmin.com/signin"),
            NavDecision::Allow
        );
        assert_eq!(
            decide_navigation("https://connect.garmin.com/apps"),
            NavDecision::Allow
        );
    }

    #[test]
    fn hostile_addresses_are_denied() {
        for url in [
            "http://sso.garmin.com/portal",
            "http://connect.garmin.com/app?ticket=ST-1-x",
            "https://garmin.com.evil.example/",
            "https://connect.garmin.com.evil.example/app?ticket=ST-1-x",
            "https://evilgarmin.com/",
            "https://notgarmin.com/",
            "https://garmin.co/",
            "https://xn--garmn-9ta.com/",
            "https://user:pw@sso.garmin.com/",
            "https://sso.garmin.com@evil.example/",
            "https://sso.garmin.com:8443/",
            "https://127.0.0.1/",
            "https://[::1]/",
            "https://203.0.113.7/app?ticket=ST-1-x",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,<script>1</script>",
            "blob:https://sso.garmin.com/1234",
            "about:blank",
            "tauri://localhost/",
            "http://ipc.localhost/plugin",
            "ipc://localhost/",
            "ftp://sso.garmin.com/",
            "",
            "not a url",
            "https://",
            "https://.garmin.com/",
        ] {
            assert_eq!(decide_navigation(url), NavDecision::Deny, "{url}");
        }
    }

    #[test]
    fn the_window_title_comes_from_the_language_files() {
        assert!(!window_title("de").is_empty());
        assert_ne!(window_title("de"), "Garmin");
        assert_ne!(window_title("en"), window_title("de"));
    }
}
