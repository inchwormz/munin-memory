use munin_memory::core::resolver;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Fixture {
    route: String,
    source_status: Option<String>,
    triggers: Vec<String>,
    negative_triggers: Vec<String>,
}

#[test]
fn resolver_trigger_fixtures_match_intent_registry() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("resolver_triggers");
    let mut fixture_count = 0usize;
    for entry in std::fs::read_dir(root).expect("fixtures dir") {
        let path = entry.expect("fixture entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        fixture_count += 1;
        let fixture: Fixture =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("fixture text"))
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(
            fixture.triggers.len() >= 5,
            "{} needs at least five positive triggers",
            path.display()
        );
        assert!(
            fixture.negative_triggers.len() >= 2,
            "{} needs at least two negative triggers",
            path.display()
        );
        for trigger in &fixture.triggers {
            let report =
                resolver::resolve_with_source_status(trigger, fixture.source_status.as_deref());
            assert_eq!(
                report.route,
                fixture.route,
                "{} positive trigger `{}` routed to `{}`",
                path.display(),
                trigger,
                report.route
            );
        }
        for trigger in &fixture.negative_triggers {
            let report =
                resolver::resolve_with_source_status(trigger, fixture.source_status.as_deref());
            assert_ne!(
                report.route,
                fixture.route,
                "{} negative trigger `{}` unexpectedly matched",
                path.display(),
                trigger
            );
        }
    }
    assert_eq!(
        fixture_count, 8,
        "expected one fixture for each narrow intent"
    );
}
