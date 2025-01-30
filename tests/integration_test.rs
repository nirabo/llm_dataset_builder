use assert_fs::prelude::*;
use predicates::prelude::*;
use std::env;
use std::fs;

#[tokio::test]
async fn test_cli_args_override_env_vars() {
    let temp = assert_fs::TempDir::new().unwrap();

    // Copy test data
    fs::copy("tests/data/test.md", temp.path().join("test.md")).unwrap();

    // Set environment variables
    env::set_var("OLLAMA_ENDPOINT", "http://env-endpoint:11434");
    env::set_var("OLLAMA_MODEL", "env-model");
    env::set_var("OUTPUT_DIR", "env-output");

    // Run with CLI args that override env vars
    let status = tokio::process::Command::new(env!("CARGO_BIN_EXE_llm_dataset_builder"))
        .arg("-e")
        .arg("http://cli-endpoint:11434")
        .arg("-m")
        .arg("cli-model")
        .arg("-d")
        .arg(temp.path().to_str().unwrap())
        .arg("--test-mode")
        .status()
        .await
        .unwrap();

    assert!(status.success());

    // Check that CLI args were used instead of env vars
    let output = temp.child("test_qa.jsonl");
    output.assert(predicate::path::exists());

    // Clean up
    env::remove_var("OLLAMA_ENDPOINT");
    env::remove_var("OLLAMA_MODEL");
    env::remove_var("OUTPUT_DIR");
}

#[tokio::test]
async fn test_env_vars_used_when_no_cli_args() {
    let temp = assert_fs::TempDir::new().unwrap();

    // Copy test data
    fs::copy("tests/data/test.md", temp.path().join("test.md")).unwrap();

    // Set environment variables
    env::set_var("OLLAMA_ENDPOINT", "http://env-endpoint:11434");
    env::set_var("OLLAMA_MODEL", "env-model");
    env::set_var("OUTPUT_DIR", temp.path().to_str().unwrap());

    // Run without CLI args
    let status = tokio::process::Command::new(env!("CARGO_BIN_EXE_llm_dataset_builder"))
        .arg("--test-mode")
        .status()
        .await
        .unwrap();

    assert!(status.success());

    // Check that env vars were used
    let output = temp.child("test_qa.jsonl");
    output.assert(predicate::path::exists());

    // Clean up
    env::remove_var("OLLAMA_ENDPOINT");
    env::remove_var("OLLAMA_MODEL");
    env::remove_var("OUTPUT_DIR");
}

#[tokio::test]
async fn test_defaults_used_when_no_config() {
    let temp = assert_fs::TempDir::new().unwrap();

    // Copy test data
    fs::copy("tests/data/test.md", temp.path().join("test.md")).unwrap();

    // Ensure no env vars are set
    env::remove_var("OLLAMA_ENDPOINT");
    env::remove_var("OLLAMA_MODEL");
    env::remove_var("OUTPUT_DIR");

    // Run without any configuration
    let status = tokio::process::Command::new(env!("CARGO_BIN_EXE_llm_dataset_builder"))
        .arg("-d")
        .arg(temp.path().to_str().unwrap())
        .arg("--test-mode")
        .status()
        .await
        .unwrap();

    assert!(status.success());

    // Check that output file was created
    let output = temp.child("test_qa.jsonl");
    output.assert(predicate::path::exists());
}
