const ROOT_README: &str = include_str!("../README.md");
const DOCUMENT_MAP: &str = include_str!("../documents/README.md");
const DESIGN: &str = include_str!("../documents/ptymark-design.md");
const OPENMATH: &str = include_str!("../documents/openmath.md");
const EXAMPLES: &str = include_str!("../examples/README.md");
const OPENMATH_EXAMPLE: &str = include_str!("../examples/openmath.md");

#[test]
fn root_entrypoint_routes_common_user_tasks() {
    for required in [
        "## Choose a route",
        "documents/README.md",
        "documents/openmath.md",
        "documents/interactive-session.md",
        "documents/troubleshooting.md",
        "examples/README.md",
    ] {
        assert!(
            ROOT_README.contains(required),
            "root README is missing `{required}`"
        );
    }
}

#[test]
fn document_map_routes_user_tasks_to_product_owned_contracts() {
    let tasks = DOCUMENT_MAP
        .find("| Goal | Start here | Continue with |")
        .expect("task map");
    let contracts = DOCUMENT_MAP
        .find("## Product contracts")
        .expect("product contract section");
    assert!(tasks < contracts);
    for required in [
        "./openmath.md",
        "./interactive-session.md",
        "./filtered-command.md",
        "./troubleshooting.md",
        "../verification/README.md",
    ] {
        assert!(
            DOCUMENT_MAP.contains(required),
            "documentation map is missing `{required}`"
        );
    }
}

#[test]
fn current_design_describes_native_sessions_and_format_adaptation() {
    for required in [
        "ptymark -- COMMAND",
        "OpenMathAdapterRenderer",
        "SemanticFormat",
        "10 seconds",
    ] {
        assert!(DESIGN.contains(required), "design is missing `{required}`");
    }
    assert!(
        !DESIGN.contains(
            "interactive PTY host and Windows ConPTY host remain separate follow-up work"
        )
    );
    assert!(!DESIGN.contains("30-second wall-clock cold-start ceiling"));

    let fence_start = DESIGN.find("````text").expect("literal example fence");
    let fenced_example = &DESIGN[fence_start + "````text".len()..];
    let mermaid = fenced_example
        .find("```mermaid ... ```")
        .expect("Mermaid literal example");
    let openmath = fenced_example
        .find("```openmath ... ```")
        .expect("OpenMath literal example");
    let fence_end = fenced_example.find("````").expect("closing example fence");
    assert!(mermaid < openmath);
    assert!(openmath < fence_end);
}

#[test]
fn openmath_contract_and_example_are_cross_linked() {
    for required in [
        "```openmath",
        "OMOBJ",
        "DOCTYPE",
        "OMR",
        "remote Content Dictionary",
        "../examples/openmath.md",
    ] {
        assert!(
            OPENMATH.contains(required),
            "OpenMath contract is missing `{required}`"
        );
    }
    assert!(EXAMPLES.contains("openmath.md"));
    assert!(OPENMATH_EXAMPLE.contains("```openmath"));
    assert!(OPENMATH_EXAMPLE.contains("research1"));
}
