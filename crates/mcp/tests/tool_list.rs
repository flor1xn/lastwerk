//! Werkzeugliste (SPEC 5.4, M7): genau die Werkzeuge der Tabelle, jedes mit den Hinweisen
//! `readOnlyHint = true`, `openWorldHint = false`. Ein zusätzliches oder fehlendes Werkzeug
//! lässt den Test fehlschlagen.

use std::collections::BTreeSet;

use lastwerk_mcp::LastwerkMcp;
use lastwerk_mcp::startup::list_tools_json;
use serde_json::Value;

/// Die Tabelle aus SPEC 5.4, in ihrer Reihenfolge. Bewusst nicht aus dem Programm übernommen:
/// Wer ein Werkzeug hinzufügt, muss diese Liste und SPEC 5.4 ändern.
const SPEC_TOOLS: [&str; 9] = [
    "get_fitness_status",
    "get_weekly_load",
    "list_activities",
    "get_activity",
    "get_current_plan",
    "get_plan_week",
    "get_thresholds",
    "explain_rule",
    "get_recovery_days",
];

fn tools() -> Vec<Value> {
    LastwerkMcp::tool_list()
        .iter()
        .map(|t| serde_json::to_value(t).unwrap())
        .collect()
}

#[test]
fn the_tool_list_is_exactly_the_table_in_the_spec() {
    let names: Vec<String> = tools()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, SPEC_TOOLS);
    assert_eq!(lastwerk_mcp::TOOL_NAMES, SPEC_TOOLS);
}

#[test]
fn every_tool_is_marked_read_only_closed_world_idempotent_and_not_destructive() {
    for t in tools() {
        let name = t["name"].as_str().unwrap();
        let a = &t["annotations"];
        assert_eq!(a["readOnlyHint"], true, "{name}");
        assert_eq!(a["openWorldHint"], false, "{name}");
        assert_eq!(a["destructiveHint"], false, "{name}");
        assert_eq!(a["idempotentHint"], true, "{name}");
        assert!(
            t["description"].as_str().is_some_and(|d| !d.is_empty()),
            "{name} needs a description"
        );
    }
}

#[test]
fn no_tool_name_suggests_a_write() {
    for t in tools() {
        let name = t["name"].as_str().unwrap();
        for bad in [
            "set_", "write", "delete", "update", "create", "import", "export", "run", "exec",
            "file", "sql", "query",
        ] {
            assert!(!name.contains(bad), "{name}");
        }
    }
}

#[test]
fn list_tools_prints_the_same_tools_with_their_hints() {
    let v = list_tools_json();
    let listed = v["tools"].as_array().unwrap();
    let names: Vec<&str> = listed.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, SPEC_TOOLS);
    assert!(
        listed
            .iter()
            .all(|t| t["annotations"]["readOnlyHint"] == true)
    );
}

fn props(t: &Value) -> BTreeSet<String> {
    t["inputSchema"]["properties"]
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default()
}

fn tool(name: &str) -> Value {
    tools().into_iter().find(|t| t["name"] == name).unwrap()
}

#[test]
fn arguments_are_typed_and_bounded_in_the_schema() {
    // Keine Freitextfelder: nur diese Argumente und keine weiteren.
    let expected: [(&str, &[&str]); 9] = [
        ("get_fitness_status", &["from", "to"]),
        ("get_weekly_load", &["weeks", "sport"]),
        ("list_activities", &["from", "to", "sport"]),
        ("get_activity", &["id"]),
        ("get_current_plan", &[]),
        ("get_plan_week", &["week_start"]),
        ("get_thresholds", &["on_date"]),
        ("explain_rule", &["code"]),
        ("get_recovery_days", &["from", "to"]),
    ];
    for (name, fields) in expected {
        let t = tool(name);
        let want: BTreeSet<String> = fields.iter().map(|f| (*f).to_string()).collect();
        assert_eq!(props(&t), want, "{name}");
        assert_eq!(t["inputSchema"]["type"], "object", "{name}");
        assert_eq!(t["inputSchema"]["additionalProperties"], false, "{name}");
    }

    let weeks = &tool("get_weekly_load")["inputSchema"]["properties"]["weeks"];
    assert_eq!(weeks["minimum"], 1);
    assert_eq!(weeks["maximum"], 52);
    let code = &tool("explain_rule")["inputSchema"]["properties"]["code"];
    assert_eq!(code["pattern"], "^R[0-9]{2}$");
    assert_eq!(code["minLength"], 3);
    assert_eq!(code["maxLength"], 3);
    assert_eq!(
        tool("get_activity")["inputSchema"]["properties"]["id"]["minimum"],
        1
    );
    // Daten sind Zeichenketten im Format `date`.
    assert_eq!(
        tool("get_fitness_status")["inputSchema"]["properties"]["from"]["format"],
        "date"
    );
    // Sportarten sind eine feste Aufzählung (auch hinter einem Verweis oder `anyOf`).
    let sport = tool("list_activities")["inputSchema"].to_string();
    for s in ["cycling", "running", "swimming", "strength", "other"] {
        assert!(sport.contains(s), "{s}");
    }
}
