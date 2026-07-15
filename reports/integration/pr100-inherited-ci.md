# PR 100 inherited CI diagnostic

make ci exit status: 2

```text
✅ local issue sync checks 成功
ACCUMULATED_AGENT_EVAL_PRODUCER=codex-agent-role:pass:stdout=.state/agent-eval-runs/run-all-checks/01-codex-agent-role.stdout.txt:stderr=.state/agent-eval-runs/run-all-checks/01-codex-agent-role.stderr.txt
ACCUMULATED_AGENT_EVAL_PRODUCER=skill-workflow-prompt:pass:stdout=.state/agent-eval-runs/run-all-checks/02-skill-workflow-prompt.stdout.txt:stderr=.state/agent-eval-runs/run-all-checks/02-skill-workflow-prompt.stderr.txt
ACCUMULATED_AGENT_EVAL_PRODUCER=local-llm-responsibility:pass:stdout=.state/agent-eval-runs/run-all-checks/03-local-llm-responsibility.stdout.txt:stderr=.state/agent-eval-runs/run-all-checks/03-local-llm-responsibility.stderr.txt
ACCUMULATED_AGENT_EVAL_PRODUCER=workflow-selection:pass:stdout=.state/agent-eval-runs/run-all-checks/04-workflow-selection.stdout.txt:stderr=.state/agent-eval-runs/run-all-checks/04-workflow-selection.stderr.txt
ACCUMULATED_AGENT_EVAL_PRODUCER=report-quality:pass:stdout=.state/agent-eval-runs/run-all-checks/05-report-quality.stdout.txt:stderr=.state/agent-eval-runs/run-all-checks/05-report-quality.stderr.txt
ACCUMULATED_AGENT_EVAL_PRODUCERS=5
ACCUMULATED_AGENT_EVAL_FAILED=-
ACCUMULATED_AGENT_EVAL=pass
✅ accumulated agent eval producers 成功
EVAL_ACCUMULATION_HOOK_FILES=0
EVAL_ACCUMULATION_HOOK_ENTRIES=0
EVAL_ACCUMULATION_HOOK_LEGACY_MISSING_NAMESPACE=0
EVAL_ACCUMULATION_HOOK_NAMESPACE_DEBT=0
EVAL_ACCUMULATION_SKILL_REPORTS=1
EVAL_ACCUMULATION_LOCAL_LLM_REPORTS=1
EVAL_ACCUMULATION_WORKFLOW_SELECTION_REPORTS=1
EVAL_ACCUMULATION_REPORT_QUALITY_REPORTS=1
EVAL_ACCUMULATION_CODEX_AGENT_ROLE_REPORTS=1
EVAL_ACCUMULATION_FAMILY_REPORTS=codex-agent-role:1
EVAL_ACCUMULATION_FAMILY_REPORTS=local-llm-responsibility:1
EVAL_ACCUMULATION_FAMILY_REPORTS=report-quality:1
EVAL_ACCUMULATION_FAMILY_REPORTS=skill-workflow-prompt:1
EVAL_ACCUMULATION_FAMILY_REPORTS=workflow-selection:1
EVAL_ACCUMULATION_FINDINGS=0
EVAL_ACCUMULATION_BLOCKING_FINDINGS=0
EVAL_ACCUMULATION_WARNINGS=0
EVAL_ACCUMULATION=pass
✅ eval accumulation checks 成功
✅ Rust format checks 成功
    Checking cfg-if v1.0.4
    Checking typenum v1.20.0
    Checking zerocopy v0.8.48
    Checking once_cell v1.21.4
    Checking foldhash v0.2.0
    Checking libsqlite3-sys v0.30.1
    Checking hashbrown v0.16.1
    Checking zmij v1.0.21
    Checking serde_core v1.0.228
    Checking hashlink v0.11.1
    Checking generic-array v0.14.7
    Checking encoding_rs v0.8.35
    Checking block-buffer v0.10.4
    Checking crypto-common v0.1.7
    Checking digest v0.10.7
    Checking bitflags v2.11.1
    Checking memchr v2.8.0
    Checking smallvec v1.15.1
    Checking cpufeatures v0.2.17
    Checking arraydeque v0.5.1
    Checking itoa v1.0.18
    Checking fallible-iterator v0.3.0
    Checking fallible-streaming-iterator v0.1.9
    Checking yaml-rust2 v0.11.0
    Checking sha2 v0.10.9
    Checking serde_json v1.0.149
    Checking ahash v0.8.12
    Checking hashbrown v0.14.5
    Checking hashlink v0.9.1
    Checking rusqlite v0.32.1
    Checking agent-canon v0.1.0 (/home/runner/work/ptymark/ptymark/merge/vendor/agent-canon/rust/agent-canon)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.50s
✅ Rust clippy checks 成功
   Compiling agent-canon v0.1.0 (/home/runner/work/ptymark/ptymark/merge/vendor/agent-canon/rust/agent-canon)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 7.85s
     Running unittests src/main.rs (vendor/agent-canon/rust/agent-canon/target/debug/deps/agent_canon-4f5765341fff2be6)

running 152 tests
test docs::tests::fixes_legacy_math_notation ... ok
test docs::tests::help_text_exposes_options_and_examples ... ok
test docs::tests::parses_formatter_command_arguments ... ok
test docs::tests::structured_report_tells_agents_to_use_reported_location ... ok
test docs::tests::formats_mermeid_fence_and_reserved_graph_node ... ok
test docs::tests::flags_workspace_absolute_markdown_links_even_when_target_exists ... ok
test docs::tests::renders_validation_failure_response_from_inventory ... ok
test jit_ir_to_lean::tests::render_lean_rejects_missing_public_interface ... ok
test jit_ir_to_lean::tests::render_lean_rejects_malformed_public_interface_schema ... ok
test jit_ir_to_lean::tests::render_lean_rejects_non_answer_state_info_public_return ... ok
test local_llm::tests::classify_prompt_is_native_rust_and_single_file ... ok
test local_llm::tests::build_index_adds_python_build_subcommand ... ok
test local_llm::tests::implementation_surface_route_detects_contract_only_test_policy ... ok
test local_llm::tests::implementation_surface_route_detects_document_claim_grounding ... ok
test local_llm::tests::implementation_surface_route_detects_iterative_algorithm_contract ... ok
test local_llm::tests::implementation_surface_route_detects_personal_codex_runtime ... ok
test local_llm::tests::implementation_surface_route_fallback_is_structured_without_llm ... ok
test local_llm::tests::implementation_surface_route_detects_program_contract ... ok
test local_llm::tests::implementation_surface_route_payload_is_structured ... ok
test local_llm::tests::implementation_surface_route_prioritizes_local_llm_tool ... ok
test local_llm::tests::local_llm_command_envelope_hides_accelerator_devices ... ok
test local_llm::tests::parses_batch_prose_ir_inputs ... ok
test local_llm::tests::parses_classify_alias_and_root ... ok
test local_llm::tests::parses_implementation_surface_route_request_file ... ok
test local_llm::tests::prose_ir_does_not_mark_activity_word_only_as_experiment_plan ... ok
test local_llm::tests::prose_ir_does_not_mark_experimental_identifier_as_experiment_plan ... ok
test local_llm::tests::prose_ir_marks_assigned_experiment_fields_as_present ... ok
test local_llm::tests::parent_repo_root_uses_vendored_agent_canon_source ... ok
test local_llm::tests::prose_ir_marks_experiment_vocabulary_as_vocabulary_only ... ok
test local_llm::tests::prose_ir_part_llm_results_skip_when_llama_cli_is_missing ... ok
test jit_ir_to_lean::tests::render_lean_omits_source_and_backend_routes_for_hlo_only_root ... ok
test local_llm::tests::prose_ir_payload_returns_intermediate_representation ... ok
test local_llm::tests::prose_ir_prompt_names_tool_document_extraction_contracts ... ok
test local_llm::tests::prose_ir_part_llm_results_run_fake_llama_per_part_in_input_order ... ok
test migration_audit::tests::audit_passes_for_complete_fixture ... ok
test migration_audit::tests::parse_root_argument ... ok
test python_algorithm_contract::tests::amp_algorithm_module_requires_standard_surface_and_callable_algorithm ... ok
test python_algorithm_contract::tests::compliant_nested_and_stopping_contract_passes ... ok
test python_algorithm_contract::tests::direct_base_stopping_policy_is_reported ... ok
test python_algorithm_contract::tests::missing_child_info_is_reported ... ok
test migration_audit::tests::audit_rejects_dockerfile_cargo_test ... ok
test python_algorithm_contract::tests::stopping_primitive_definition_module_is_not_legacy_usage ... ok
test python_module_groups::tests::maps_path_to_longest_matching_group ... ok
test python_module_groups::tests::parses_module_group_contract ... ok
test python_structure_hash::tests::import_hash_tracks_import_surface_separately ... ok
test migration_audit::tests::audit_reports_missing_wrapper ... ok
test python_structure_hash::tests::module_scope_detects_cross_module_groups ... ok
test python_structure_hash::tests::single_callee_ignores_annotation_only_references ... ok
test python_structure_hash::tests::one_shared_module_level_call_does_not_create_similar_caller ... ok
test python_module_groups::tests::discovers_python_submodules_without_tests ... ok
test python_structure_hash::tests::single_caller_class_target_detects_annotation_owner ... ok
test python_structure_hash::tests::single_callee_reports_only_non_public_wrappers ... ok
test python_structure_hash::tests::stable_hash_ignores_names_when_canonical_payload_is_equal ... ok
test python_structure_hash_impact::tests::compares_removed_and_added_hashes ... ok
test python_structure_hash::tests::single_caller_groups_multiple_call_sites_by_enclosing_block ... ok
test python_structure_hash::tests::source_files_expand_repo_import_neighborhood ... ok
test python_structure_hash_report::tests::class_single_owner_struct_allows_single_callee_attribute_scope ... ok
test python_structure_hash_report::tests::class_single_owner_struct_gets_inline_candidate_when_usage_is_local ... ok
test python_structure_hash_report::tests::class_single_owner_struct_preserves_protocol_contracts ... ok
test python_structure_hash_report::tests::cross_owner_small_methods_are_review_blocked ... ok
test python_structure_hash_report::tests::class_single_owner_struct_preserves_pytree_contracts ... ok
test python_structure_hash_report::tests::parses_finding_without_losing_raw_text ... ok
test python_structure_hash_report::tests::class_single_owner_struct_review_blocks_returned_private_instance ... ok
test python_structure_hash_report::tests::module_group_graph_aggregates_file_dependencies ... ok
test python_structure_hash_report::tests::parses_legacy_single_caller_without_similar_callers ... ok
test python_structure_hash_report::tests::parses_single_callee_wrapper_with_callee_evidence ... ok
test python_structure_hash_report::tests::resolves_relative_import_modules ... ok
test python_structure_hash_report::tests::same_owner_cond_body_pair_is_review_blocked ... ok
test python_structure_hash_report::tests::same_owner_parameterless_methods_are_review_blocked ... ok
test python_structure_hash_report::tests::parses_single_caller_finding_with_call_site_evidence ... ok
test python_structure_hash_report::tests::repo_import_targets_are_part_of_analysis ... ok
test python_structure_hash_scope_plan::tests::builds_blocks_from_mechanical_clusters ... ok
test jit_ir_to_lean::tests::render_lean_expands_llvm_modules_and_functions ... ok
test python_structure_hash_report::tests::structures_mechanical_problem_cluster_for_shared_callee_wrappers ... ok
test python_structure_hash_report::tests::structures_summary_lines ... ok
test python_structure_hash_scope_plan::tests::keeps_edit_scope_paths_out_of_block_allowed_files ... ok
test python_structure_hash_scope_plan::tests::selects_larger_disjoint_actionable_candidate ... ok
test python_structure_hash_scope_plan::tests::parses_dependency_edit_scope_paths ... ok
test rust_migration_plan::tests::parse_defaults ... ok
test rust_migration_plan::tests::foundation_reports_missing_post_create ... ok
test rust_migration_plan::tests::plan_keeps_orchestration_python ... ok
test rust_migration_plan::tests::plan_includes_policy_first_target ... ok
test rust_migration_plan::tests::plan_counts_hook_candidate_tools ... ok
test rust_migration_plan::tests::plan_marks_document_inventory_completed ... ok
test semantic_index::tests::default_db_path_lives_under_home_cache_and_outside_repo ... ok
test semantic_index::tests::directed_kind_of_score_classifies_equivalent_and_containment ... ok
test semantic_index::tests::absolute_include_outside_root_is_rejected ... ok
test semantic_index::tests::candidate_commands_auto_resolve_provider_dimension ... ok
test semantic_index::tests::context_pack_returns_bounded_evidence_cells ... ok
test semantic_index::tests::embedding_ignores_dependency_manifest_comment ... ok
test semantic_index::tests::embedding_is_normalized_and_zero_safe ... ok
test semantic_index::tests::discourse_relations_pair_therefore_and_because_variants ... ok
test semantic_index::tests::eval_output_accepts_valid_review_artifacts ... ok
test semantic_index::tests::eval_output_rejects_cross_responsibility_and_query_echo ... ok
test semantic_index::tests::eval_missing_must_not_path_fails ... ok
test semantic_index::tests::markdown_segmentation_emits_document_sections_and_blocks ... ok
test semantic_index::tests::embed_provider_adds_vectors_without_rebuilding_nodes ... ok
test semantic_index::tests::eval_fixture_reports_pass ... ok
test semantic_index::tests::eval_run_returns_nonzero_when_quality_fails ... ok
test semantic_index::tests::merge_candidates_exclude_same_file_pairs_by_default ... ok
test semantic_index::tests::merge_candidates_skip_tiny_heading_only_sections ... ok
test semantic_index::tests::merge_candidates_stay_within_responsibility_bucket_on_full_repo_input ... ok
test semantic_index::tests::openai_embedding_response_parses_indexed_batch ... ok
test semantic_index::tests::openai_embedding_response_rejects_bad_shapes ... ok
test semantic_index::tests::parse_search_accepts_query_file_and_jsonl_for_long_text ... ok
test semantic_index::tests::parse_search_requires_query ... ok
test semantic_index::tests::parse_similar_rejects_non_positive_min_score ... ok
test semantic_index::tests::merge_candidates_skip_alignment_mirrors_and_eval_logs ... ok
test semantic_index::tests::remote_embedding_text_is_bounded_without_splitting_chars ... ok
test semantic_index::tests::responsibility_scope_bucket_tracks_manifest_surfaces ... ok
test semantic_index::tests::mismatched_search_dimension_returns_no_hits ... ok
test semantic_index::tests::responsibility_tree_detects_missing_directory_coverage ... ok
test semantic_index::tests::natural_relations_persist_directed_kind_of_analysis ... ok
test semantic_index::tests::provider_compare_reuses_existing_responsibility_buckets ... ok
test semantic_index::tests::responsibility_tree_reports_vectors_and_coverage ... ok
test semantic_index::tests::search_skips_cached_nodes_for_deleted_paths ... ok
test semantic_index::tests::similar_pairs_can_cross_responsibility_bucket_for_alignment_search ... ok
test semantic_index::tests::sqlite_build_and_search_roundtrip ... ok
test semantic_index::tests::thin_docs_marks_readme_wrappers_as_protected_entrypoints ... ok
test structured_analysis::tests::closed_issue_records_are_historical_not_active_findings ... ok
test semantic_index::tests::thin_docs_reports_short_wrapper_from_vector_db ... ok
test structured_analysis::tests::dsl_trace_can_be_satisfied_by_responsibility_coverage ... ok
test structured_analysis::tests::dsl_upstream_without_design_trace_is_responsibility_gap ... ok
test structured_analysis::tests::analyze_writes_separate_warning_database ... ok
test structured_analysis::tests::duplicate_titles_are_reported ... ok
test structured_analysis::tests::generated_reports_are_excluded_from_default_document_inventory ... ok
test structured_analysis::tests::graph_contract_cli_rejects_invalid_format_argument ... ok
test structured_analysis::tests::graph_contract_cli_returns_failure_on_blocker ... ok
test structured_analysis::tests::graph_contract_json_output_is_parseable ... ok
test structured_analysis::tests::build_cache_materializes_artifact_layer ... ok
test structured_analysis::tests::graph_contract_reports_broken_edge_endpoint ... ok
test structured_analysis::tests::graph_contract_reports_missing_required_column ... ok
test structured_analysis::tests::graph_contract_reports_missing_required_table ... ok
test structured_analysis::tests::graph_contract_reports_invalid_payload_json ... ok
test structured_analysis::tests::graph_contract_text_exposes_core_contract ... ok
test structured_analysis::tests::directory_responsibility_gap_is_reported_from_child_artifacts ... ok
test structured_analysis::tests::graph_contract_reports_non_blocker_rules ... ok
test structured_analysis::tests::tool_document_path_alone_does_not_create_design_gap ... ok
test structured_analysis::tests::upstream_coverage_rule_reports_graph_format_gap ... ok
test test_design::tests::accepts_multiline_python_test_function_header_with_unittest_assertion ... ok
test test_design::tests::allows_static_checker_behavior_contract_tests ... ok
test test_design::tests::detects_missing_oracle_in_python_test_function ... ok
test test_design::tests::detects_overspecified_mock_and_private_access ... ok
test test_design::tests::emits_property_design_hint_for_parser_examples ... ok
test test_design::tests::flags_generated_execution_only_placeholder ... ok
test test_design::tests::flags_static_analysis_duplicate_success_test ... ok
test test_design::tests::help_text_exposes_options_and_examples ... ok
test structured_analysis::tests::inventory_import_materializes_document_canon_diagnostics ... ok
test structured_analysis::tests::graph_contract_accepts_build_cache_schema ... ok
test test_design::tests::parse_check_args ... ok
test test_design::tests::scans_test_like_files_under_tests_directory ... ok
test structured_analysis::tests::responsibility_gap_diagnostic_includes_recursive_verification_route ... ok

test result: ok. 152 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s

✅ Rust tests 成功
GITHUB_WORKFLOW_FINDING severity=error path=.github/workflows/ptymark-dependency-alignment.yml message=missing_agent_canon_checkout_helper
GITHUB_WORKFLOW_FINDING severity=error path=.github/workflows/ptymark-dependency-alignment.yml message=missing_agent_canon_repo_credential_env
GITHUB_WORKFLOWS_CHECKED=17
GITHUB_WORKFLOW_ERRORS=2
GITHUB_WORKFLOW_WARNINGS=0
GITHUB_WORKFLOWS=fail
❌ GitHub workflow / PR template checks 失敗
CONTAINER_CONFIG_PACK=default-host-docker	path=docker/packs/default-host-docker.toml	dockerfile=docker/Dockerfile	context=.	workdir=/workspace	workspace_mount=/workspace
CONTAINER_CONFIG_PACK=default	path=docker/packs/default.toml	dockerfile=docker/Dockerfile	context=.	workdir=/workspace	workspace_mount=/workspace
CONTAINER_CONFIG_CHECKED=.dockerignore,docker/Dockerfile,docker/requirements.txt,docker/packs,.devcontainer,.vscode
CONTAINER_CONFIG_FINDINGS=0
CONTAINER_CONFIG=pass
✅ container configuration checks 成功

1️⃣  documentation checks を実行中...
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.02s
     Running `vendor/agent-canon/rust/agent-canon/target/debug/agent-canon docs check`
DOCS_CHECK=pass
✅ documentation checks 成功

2️⃣  experiment registry checks を実行中...
repo_root=/home/runner/work/ptymark/ptymark/merge
registry_path=/home/runner/work/ptymark/ptymark/merge/experiments/registry.toml
WARNING: _template: active_branch does not exist in the current repo: main
OK: experiment registry is valid
✅ experiment registry checks 成功

3️⃣  pytest を実行中...
........................................................................ [  7%]
................................................................. [ 14%]
.................................................................... [ 21%]
.................................................................................................................................. [ 35%]
..................................................................... [ 42%]
.................................................................... [ 49%]
................................................................... [ 56%]
.......................................................... [ 63%]
..................................................................................................................... [ 75%]
........................................................................ [ 83%]
........................................................................ [ 90%]
..............F...........................sssss......................... [ 98%]
................                                         [100%]
=================================== FAILURES ===================================
_________ ReleaseMetadataTest.test_release_tree_metadata_is_consistent _________
tests/tools/test_release_metadata.py:29: in test_release_tree_metadata_is_consistent
    result = subprocess.run(
/opt/hostedtoolcache/Python/3.11.15/x64/lib/python3.11/subprocess.py:571: in run
    raise CalledProcessError(retcode, process.args,
E   subprocess.CalledProcessError: Command '['/opt/hostedtoolcache/Python/3.11.15/x64/bin/python3', '/home/runner/work/ptymark/ptymark/merge/scripts/check-release-metadata.py', '--tag', 'v0.1.0-alpha.2']' returned non-zero exit status 1.
=========================== short test summary info ============================
FAILED tests/tools/test_release_metadata.py::ReleaseMetadataTest::test_release_tree_metadata_is_consistent - subprocess.CalledProcessError: Command '['/opt/hostedtoolcache/Python/3.11.15/x64/bin/python3', '/home/runner/work/ptymark/ptymark/merge/scripts/check-release-metadata.py', '--tag', 'v0.1.0-alpha.2']' returned non-zero exit status 1.
1 failed, 940 passed, 5 skipped, 238 subtests passed in 389.03s (0:06:29)
❌ pytest 失敗

4️⃣  pyright を実行中...
0 errors, 0 warnings, 0 informations
✅ pyright 成功

5️⃣  pydocstyle を実行中... (Docstring チェック)
✅ pydocstyle 成功

6️⃣  ruff を実行中...
   - E,F: コード品質（エラー・警告）
   - I: Import 順序チェック
   - D: Docstring 検証
   - UP: Python 最新構文チェック

All checks passed!
✅ ruff 成功

PYTHON_QUALITY_CHECKS=fail
❌ Python quality checks 失敗

════════════════════════════════════════════════════════════════
❌ CI チェック完了: 失敗あり
════════════════════════════════════════════════════════════════
make: *** [Makefile:41: ci] Error 1
```
