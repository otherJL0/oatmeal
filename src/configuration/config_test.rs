use std::env;

use anyhow::Result;
use test_utils::insta_snapshot;

use super::Config;
use crate::application::cli;

fn teardown_default_api_keys() {
    env::remove_var("ANTHROPIC_API_KEY");
    env::remove_var("OPENAI_API_KEY");
    env::remove_var("GEMINI_API_KEY");
}

#[test]
fn it_serializes_to_valid_toml() {
    teardown_default_api_keys();
    let res = Config::serialize_default(cli::build());
    let toml_res = res.parse::<toml_edit::DocumentMut>();
    assert!(toml_res.is_ok());

    insta_snapshot(|| {
        insta::assert_toml_snapshot!(res);
    });
}

#[tokio::test]
async fn it_loads_config_from_file() -> Result<()> {
    let matches = cli::build().try_get_matches_from(vec!["chat", "-c", "./config.example.toml"])?;
    Config::load(cli::build(), vec![&matches]).await?;
    return Ok(());
}

#[tokio::test]
async fn it_fails_to_loads_config_from_file() -> Result<()> {
    let matches =
        cli::build().try_get_matches_from(vec!["chat", "-c", "./test/bad-config.toml"])?;
    let res = Config::load(cli::build(), vec![&matches]).await;
    assert!(res.is_err());
    return Ok(());
}
