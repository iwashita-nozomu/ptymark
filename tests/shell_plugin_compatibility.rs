use ptymark::{
    DisplayPipeline, FencedDetector, NoopCache, OutputSegment, PreviewRenderer, RenderContext,
    RenderService, TerminalOutputGate,
};
use std::collections::HashSet;

const INVENTORIES: [(&str, &str); 5] = [
    (
        "bash",
        include_str!("../compat/shell-integrations/bash.tsv"),
    ),
    (
        "zsh",
        include_str!("../compat/shell-integrations/zsh.tsv"),
    ),
    (
        "fish",
        include_str!("../compat/shell-integrations/fish.tsv"),
    ),
    (
        "powershell",
        include_str!("../compat/shell-integrations/powershell.tsv"),
    ),
    (
        "nushell",
        include_str!("../compat/shell-integrations/nushell.tsv"),
    ),
];

const PROFILES: [&str; 8] = [
    "safe-text",
    "hook-only",
    "prompt-control",
    "right-prompt",
    "line-editor",
    "completion-menu",
    "alternate-screen",
    "progress-line",
];

fn fixture(profile: &str) -> Vec<u8> {
    match profile {
        "safe-text" => "icons   src   README.md\n".as_bytes().to_vec(),
        "hook-only" => b"direnv: export +PROJECT_ENV\n".to_vec(),
        "prompt-control" => b"\x1b]133;A\x07\x1b]7;file://host/workspace\x07\x1b[1;32muser@host\x1b[0m \x1b]0;workspace\x07$ \n".to_vec(),
        "right-prompt" => b"\x1b[s\x1b[72G\x1b[2m12:34\x1b[0m\x1b[u\n".to_vec(),
        "line-editor" => b"\x1b[?2004h\x1b[2K\r\x1b[32mgit\x1b[0m status\x1b[90m --short\x1b[0m\x1b[?2004l\n".to_vec(),
        "completion-menu" => b"\x1b[2K\rcheckout  cherry-pick  clone\x1b[1A\x1b[12C\n".to_vec(),
        "alternate-screen" => b"\x1b[?1049h\x1b[?1000hselector\n$$\nnot math\n$$\n\x1b[?1000l\x1b[?1049l\n".to_vec(),
        "progress-line" => b"indexing 1%\rindexing 50%\rindexing 100%\n".to_vec(),
        other => panic!("unknown compatibility profile: {other}"),
    }
}

fn flatten(segments: Vec<OutputSegment>) -> Vec<u8> {
    segments
        .into_iter()
        .flat_map(|segment| match segment {
            OutputSegment::SafeText(bytes) | OutputSegment::RawTerminalBytes(bytes) => bytes,
        })
        .collect()
}

fn preview_pipeline() -> DisplayPipeline {
    let detector = Box::new(FencedDetector::new(&ptymark::DetectionConfig::default()));
    let renderer = RenderService::new(Box::new(PreviewRenderer), Box::new(NoopCache::default()));
    DisplayPipeline::new(detector, renderer, RenderContext::default(), false)
}

#[test]
fn inventory_has_twenty_reviewed_integrations_per_shell() {
    let mut global = HashSet::new();

    for (shell, inventory) in INVENTORIES {
        let mut lines = inventory.lines();
        assert_eq!(
            lines.next(),
            Some("integration\tcategory\tprofile\tverification\tupstream\tnotes")
        );
        let rows: Vec<_> = lines.filter(|line| !line.is_empty()).collect();
        assert_eq!(rows.len(), 20, "{shell} must retain about twenty reviewed integrations");

        for row in rows {
            let fields: Vec<_> = row.split('\t').collect();
            assert_eq!(fields.len(), 6, "invalid {shell} inventory row: {row}");
            let name = fields[0];
            let profile = fields[2];
            let verification = fields[3];
            let upstream = fields[4];
            assert!(!name.is_empty());
            assert!(PROFILES.contains(&profile), "unknown profile {profile}");
            assert_eq!(verification, "contract-verified");
            assert!(upstream.starts_with("https://github.com/"));
            assert!(
                global.insert((shell, name)),
                "duplicate integration entry: {shell}/{name}"
            );
        }
    }
}

#[test]
fn every_behavior_profile_is_byte_exact_across_arbitrary_chunks() {
    for profile in PROFILES {
        let source = fixture(profile);
        let mut gate = TerminalOutputGate::default();
        let mut output = Vec::new();
        let mut saw_raw = false;

        for byte in &source {
            let segments = gate.feed(&[*byte]);
            saw_raw |= segments
                .iter()
                .any(|segment| matches!(segment, OutputSegment::RawTerminalBytes(_)));
            output.extend(flatten(segments));
        }

        assert_eq!(output, source, "profile {profile} changed terminal bytes");
        if !matches!(profile, "safe-text" | "hook-only") {
            assert!(saw_raw, "profile {profile} must exercise the protected raw path");
        }
    }
}

#[test]
fn prompt_controls_pass_unchanged_before_a_renderable_block() {
    let prompt = fixture("prompt-control");
    let mut source = prompt.clone();
    source.extend_from_slice(b"```mermaid\nflowchart LR\n  Shell --> Plugin --> Command\n```\n");

    let mut pipeline = preview_pipeline();
    let mut output = Vec::new();
    for byte in source {
        pipeline.feed(&[byte], &mut output).expect("feed");
    }
    pipeline.finish(&mut output).expect("finish");

    assert!(output.starts_with(&prompt));
    let rendered = String::from_utf8_lossy(&output[prompt.len()..]);
    assert!(rendered.contains("ptymark mermaid"));
    assert!(!rendered.contains("```mermaid"));
}

#[test]
fn full_screen_and_progress_interfaces_never_enter_semantic_detection() {
    for profile in ["alternate-screen", "progress-line", "line-editor", "completion-menu"] {
        let source = fixture(profile);
        let mut pipeline = preview_pipeline();
        let mut output = Vec::new();
        for byte in &source {
            pipeline.feed(&[*byte], &mut output).expect("feed");
        }
        pipeline.finish(&mut output).expect("finish");
        assert_eq!(output, source, "profile {profile} was transformed");
        assert_eq!(pipeline.report().semantic_blocks, 0);
    }
}
