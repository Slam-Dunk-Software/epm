mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn mock_packages(packages: serde_json::Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/packages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(packages))
        .mount(&server)
        .await;
    server
}

fn epm(registry: &str) -> Command {
    let mut cmd = Command::cargo_bin("epm").unwrap();
    cmd.env("EPM_REGISTRY", registry);
    cmd
}

// ── tests ────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn search_no_args_lists_all_packages() {
    let pkgs = json!([
        common::package_json(1, "tech_talker", "Audio transcription harness"),
        common::package_json(2, "pi", "Minimal agent harness")
    ]);
    let server = mock_packages(pkgs).await;

    epm(&server.uri())
        .arg("search")
        .assert()
        .success()
        .stdout(predicate::str::contains("tech_talker"))
        .stdout(predicate::str::contains("pi"));
}

#[tokio::test(flavor = "multi_thread")]
async fn search_output_has_name_and_description_column_headers() {
    let pkgs = json!([common::package_json(1, "tech_talker", "Audio transcription harness")]);
    let server = mock_packages(pkgs).await;

    epm(&server.uri())
        .arg("search")
        .assert()
        .success()
        .stdout(predicate::str::contains("NAME"))
        .stdout(predicate::str::contains("DESCRIPTION"));
}

#[tokio::test(flavor = "multi_thread")]
async fn search_with_query_shows_matching_package_name() {
    let pkgs = json!([
        common::package_json(1, "tech_talker", "Audio transcription harness"),
        common::package_json(2, "pi", "Minimal agent harness")
    ]);
    let server = mock_packages(pkgs).await;

    epm(&server.uri())
        .args(["search", "tech"])
        .assert()
        .success()
        .stdout(predicate::str::contains("tech_talker"));
}

#[tokio::test(flavor = "multi_thread")]
async fn search_with_query_hides_non_matching_packages() {
    let pkgs = json!([
        common::package_json(1, "tech_talker", "Audio transcription harness"),
        common::package_json(2, "pi", "Minimal agent harness")
    ]);
    let server = mock_packages(pkgs).await;

    epm(&server.uri())
        .args(["search", "tech"])
        .assert()
        .success()
        .stdout(predicate::str::contains("pi").not());
}

#[tokio::test(flavor = "multi_thread")]
async fn search_with_query_filters_by_description() {
    let pkgs = json!([
        common::package_json(1, "tech_talker", "Audio transcription harness"),
        common::package_json(2, "pi", "Minimal agent harness")
    ]);
    let server = mock_packages(pkgs).await;

    // "transcription" only appears in tech_talker's description
    epm(&server.uri())
        .args(["search", "transcription"])
        .assert()
        .success()
        .stdout(predicate::str::contains("tech_talker"))
        .stdout(predicate::str::contains("pi").not());
}

#[tokio::test(flavor = "multi_thread")]
async fn search_query_is_case_insensitive() {
    let pkgs = json!([common::package_json(1, "tech_talker", "Audio transcription harness")]);
    let server = mock_packages(pkgs).await;

    epm(&server.uri())
        .args(["search", "TECH_TALKER"])
        .assert()
        .success()
        .stdout(predicate::str::contains("tech_talker"));
}

#[tokio::test(flavor = "multi_thread")]
async fn search_no_matches_shows_expected_message() {
    let pkgs = json!([common::package_json(1, "tech_talker", "Audio transcription harness")]);
    let server = mock_packages(pkgs).await;

    epm(&server.uri())
        .args(["search", "zzz_no_match_zzz"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No packages matching 'zzz_no_match_zzz'"));
}

#[tokio::test(flavor = "multi_thread")]
async fn search_empty_registry_shows_expected_message() {
    let server = mock_packages(json!([])).await;

    epm(&server.uri())
        .arg("search")
        .assert()
        .success()
        .stdout(predicate::str::contains("No packages in registry"));
}

#[tokio::test(flavor = "multi_thread")]
async fn search_exits_zero_on_success() {
    let server = mock_packages(json!([])).await;

    epm(&server.uri()).arg("search").assert().code(0);
}

#[tokio::test(flavor = "multi_thread")]
async fn search_registry_unreachable_exits_nonzero() {
    // Port 59876: nothing should be listening there
    Command::cargo_bin("epm")
        .unwrap()
        .args(["search"])
        .env("EPM_REGISTRY", "http://127.0.0.1:59876")
        .assert()
        .failure();
}
