#!/usr/bin/env python3
from __future__ import annotations

import base64
import io
import tarfile
from pathlib import Path

payload = base64.b85decode(Path("doctor_payload.b85").read_text().strip())
with tarfile.open(fileobj=io.BytesIO(payload), mode="r:gz") as archive:
    archive.extractall(".", filter="data")

doctor = Path("src/doctor.rs")
text = doctor.read_text()
old = """#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DoctorRequest {
    pub config_path: Option<PathBuf>,
    pub pipeline: PipelineOptions,
}

impl Default for DoctorRequest {
    fn default() -> Self {
        Self {
            config_path: None,
            pipeline: PipelineOptions::default(),
        }
    }
}
"""
new = """#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DoctorRequest {
    pub config_path: Option<PathBuf>,
    pub pipeline: PipelineOptions,
}
"""
if text.count(old) != 1:
    raise SystemExit("DoctorRequest default block was not found exactly once")
text = text.replace(old, new)
old = """fn load_configuration(
    explicit: Option<&Path>,
    selected_path: Option<&Path>,
    redactor: &Redactor,
    findings: &mut Vec<DiagnosticFinding>,
)"""
new = """fn load_configuration(
    explicit: Option<&Path>,
    selected_path: Option<&Path>,
    _redactor: &Redactor,
    findings: &mut Vec<DiagnosticFinding>,
)"""
if text.count(old) != 1:
    raise SystemExit("load_configuration signature was not found exactly once")
doctor.write_text(text.replace(old, new))

test = Path("tests/doctor_contract.rs")
text = test.read_text()
old = """    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!marker.exists(), "doctor started an external process");
    let json = String::from_utf8(output.stdout).expect("UTF-8 JSON");
    assert!(json.contains("\\\"state\\\": \\\"ready\\\""));
"""
new = """    assert_eq!(
        output.status.code(),
        Some(10),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!marker.exists(), "doctor started an external process");
    let json = String::from_utf8(output.stdout).expect("UTF-8 JSON");
    assert!(json.contains("\\\"status\\\": \\\"degraded\\\""));
    assert!(json.contains("\\\"state\\\": \\\"ready\\\""));
"""
if text.count(old) != 1:
    raise SystemExit("side-effect-free doctor assertion was not found exactly once")
test.write_text(text.replace(old, new))
