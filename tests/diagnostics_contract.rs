use ptymark::{
    DiagnosticComponent, DiagnosticEvidence, DiagnosticFinding, DiagnosticSeverity,
    DiagnosticStatus, Redactor,
};
use std::path::PathBuf;

#[test]
fn public_diagnostics_are_deterministic_and_source_redacted() {
    let mut redactor = Redactor::with_home(Some(PathBuf::from("/home/example")));
    redactor.add_sensitive("PRIVATE SEMANTIC SOURCE");
    redactor.add_sensitive("token-123");

    let evidence = redactor.public_text(
        b"PRIVATE SEMANTIC SOURCE token-123 /home/example/work\x1b]8;;https://secret\x07",
    );
    assert!(evidence.redacted);
    assert!(!evidence.value.contains("PRIVATE SEMANTIC SOURCE"));
    assert!(!evidence.value.contains("token-123"));
    assert!(!evidence.value.as_bytes().contains(&0x1b));
    assert!(evidence.value.contains("~/work"));

    let finding = DiagnosticFinding::new(
        "engine.missing",
        DiagnosticSeverity::Warning,
        DiagnosticComponent::Engine,
        "configured engine is unavailable",
    )
    .with_remedy("select preview or install the configured engine")
    .with_evidence("path", evidence);

    assert_eq!(DiagnosticStatus::Ready.exit_code(), 0);
    assert_eq!(
        DiagnosticStatus::from_findings(&[finding.clone()]).exit_code(),
        10
    );
    assert!(finding.human_line().starts_with("[warning] engine.missing"));
    assert_eq!(
        finding.evidence.get("missing").cloned(),
        None::<DiagnosticEvidence>
    );
}
