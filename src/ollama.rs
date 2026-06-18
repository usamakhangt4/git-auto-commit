use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Deserialize)]
struct OllamaStatusResponse {
    models: Vec<OllamaModelItem>,
}

#[derive(Deserialize)]
struct OllamaModelItem {
    name: String,
}

#[derive(Serialize)]
struct OllamaOptions {
    num_predict: i32,
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    system: String,
    prompt: String,
    stream: bool,
    keep_alive: String,
    think: bool,
    options: OllamaOptions,
}

#[derive(Deserialize, Debug)]
struct OllamaResponse {
    #[serde(default)]
    response: String,
}

pub fn ollama_host() -> String {
    env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string())
}

pub fn discover_model(client: &reqwest::blocking::Client, host: &str) -> Result<String> {
    let url = format!("{host}/api/tags");
    let tags_res = client
        .get(url)
        .send()
        .map_err(map_connect_error)?
        .json::<OllamaStatusResponse>()
        .context("failed to parse Ollama model list")?;

    if tags_res.models.is_empty() {
        bail!("no models downloaded. Run 'ollama pull llama3'");
    }

    let preferred = ["llama3", "qwen", "gemma"];
    for p in preferred {
        if let Some(m) = tags_res.models.iter().find(|m| m.name.contains(p)) {
            return Ok(m.name.clone());
        }
    }

    Ok(tags_res.models[0].name.clone())
}

pub fn generate_commit_message(
    client: &reqwest::blocking::Client,
    host: &str,
    model: &str,
    format_spec: &str,
    prompt: &str,
) -> Result<String> {
    let system_prompt = format!(
        "You are a professional Git commit message generator. \
        You receive a summarized view of staged git changes. \
        Files marked 'content omitted' or 'ignored' were excluded on purpose — infer from path and status only. \
        For bulk/initial commits with many added files, summarize the project intent, not every file. \
        Ignore lockfiles, build artifacts, and binaries unless they are the main change. \
        Your ONLY task is to write a single-line commit message. \
        Format: '{format_spec}'. \
        Return NOTHING else — no explanations, no conversation, no summaries, \
        no surrounding quotes or backticks. Just the raw commit message on one line."
    );

    let request_body = OllamaRequest {
        model: model.to_string(),
        system: system_prompt,
        prompt: prompt.to_string(),
        stream: false,
        keep_alive: "30m".to_string(),
        think: false,
        options: OllamaOptions { num_predict: 80 },
    };

    let url = format!("{host}/api/generate");
    let response = client
        .post(url)
        .json(&request_body)
        .send()
        .map_err(map_connect_error)?;

    if !response.status().is_success() {
        bail!("ollama API error: {}", response.status());
    }

    let res = response
        .json::<OllamaResponse>()
        .context("failed to parse Ollama response")?;

    clean_commit_message(&res.response).context("model returned an empty or unparseable response")
}

pub fn clean_commit_message(raw: &str) -> Option<String> {
    let first_line = raw.lines().map(str::trim).find(|line| !line.is_empty())?;

    let cleaned = first_line
        .trim_matches('`')
        .trim_matches('"')
        .trim_matches('\'')
        .trim();

    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

fn map_connect_error(err: reqwest::Error) -> anyhow::Error {
    if err.is_connect() {
        anyhow::anyhow!("ollama is not running. Start it with 'ollama serve'")
    } else {
        err.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_commit_message_strips_wrapping_and_blank_lines() {
        assert_eq!(
            clean_commit_message("  `feat: add parser`\n"),
            Some("feat: add parser".to_string())
        );
        assert_eq!(
            clean_commit_message("\n\n\"fix: handle empty model\"\n"),
            Some("fix: handle empty model".to_string())
        );
    }

    #[test]
    fn clean_commit_message_rejects_empty_output() {
        assert_eq!(clean_commit_message("   \n\t"), None);
        assert_eq!(clean_commit_message("``"), None);
    }
}
