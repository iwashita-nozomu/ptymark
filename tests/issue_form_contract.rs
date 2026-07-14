use std::fs;
use std::path::PathBuf;

fn issue_template(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".github")
        .join("ISSUE_TEMPLATE")
        .join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read `{}`: {error}", path.display()))
}

#[test]
fn release_support_routes_are_present_and_security_stays_private() {
    let config = issue_template("config.yml");
    assert!(config.contains("blank_issues_enabled: false"));
    assert!(config.contains("security/advisories/new"));
    assert!(config.contains("Usage and design questions"));

    let bug = issue_template("bug-report.yml");
    assert!(bug.contains("name: Bug report"));
    assert!(bug.contains("ptymark --version"));
    assert!(bug.contains("--safe"));
    assert!(bug.contains("--source"));
    assert!(bug.contains("--private"));
    assert!(bug.contains("Security Advisories"));

    let compatibility = issue_template("terminal-compatibility.yml");
    assert!(compatibility.contains("name: Terminal compatibility report"));
    assert!(compatibility.contains("PTY/ConPTY"));
    assert!(compatibility.contains("tmux"));
    assert!(compatibility.contains("SSH"));
    assert!(compatibility.contains("CJK"));
}

#[test]
fn public_forms_require_redaction_and_reproduction_evidence() {
    for name in ["bug-report.yml", "terminal-compatibility.yml"] {
        let form = issue_template(name);
        assert!(form.contains("Minimal reproduction"), "form={name}");
        assert!(form.contains("required: true"), "form={name}");
        assert!(form.contains("secrets"), "form={name}");
        assert!(form.contains("private"), "form={name}");
        assert!(form.contains("identifying paths"), "form={name}");
    }
}
