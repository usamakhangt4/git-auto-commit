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
    #[serde(default)]
    thinking: String,
    #[serde(default)]
    done_reason: Option<String>,
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

    let chat_models: Vec<_> = tags_res
        .models
        .iter()
        .filter(|m| is_chat_model(&m.name))
        .collect();

    if chat_models.is_empty() {
        bail!("no chat models downloaded. Run 'ollama pull llama3.2:3b'");
    }

    let preferred = ["llama3", "qwen3", "qwen", "gemma3", "gemma"];
    for p in preferred {
        if let Some(m) = chat_models
            .iter()
            .find(|m| m.name.contains(p) && !is_small_model(&m.name))
        {
            return Ok(m.name.clone());
        }
    }

    chat_models
        .iter()
        .min_by_key(|m| model_preference_rank(&m.name))
        .map(|m| m.name.clone())
        .ok_or_else(|| anyhow::anyhow!("no usable chat model found"))
}

pub fn generate_commit_message(
    client: &reqwest::blocking::Client,
    host: &str,
    model: &str,
    format_spec: &str,
    prompt: &str,
    fallback_prompt: Option<&str>,
) -> Result<String> {
    match generate_once(client, host, model, format_spec, prompt) {
        Ok(message) => Ok(message),
        Err(first_error) => {
            let Some(fallback) = fallback_prompt.filter(|p| !p.trim().is_empty() && *p != prompt)
            else {
                return Err(first_error);
            };

            eprintln!("Warning: model returned no usable message, retrying without diffs");
            generate_once(client, host, model, format_spec, fallback)
                .with_context(|| format!("retry failed after: {first_error:#}"))
        }
    }
}

fn generate_once(
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
        options: OllamaOptions { num_predict: 120 },
    };

    let url = format!("{host}/api/generate");
    let response = client
        .post(url)
        .json(&request_body)
        .send()
        .map_err(map_request_error)?;

    if response.status().as_u16() == 404 {
        bail!(
            "model '{model}' not found. Run 'ollama list' and 'git-auto-commit set-model <name>'"
        );
    }

    if !response.status().is_success() {
        bail!("ollama API error: {}", response.status());
    }

    let res = response
        .json::<OllamaResponse>()
        .context("failed to parse Ollama response")?;

    message_from_response(&res).with_context(|| {
        format!(
            "model returned an empty or unparseable response (done_reason={:?}, raw={:?})",
            res.done_reason,
            preview(&res.response)
        )
    })
}

fn message_from_response(res: &OllamaResponse) -> Option<String> {
    clean_commit_message(&res.response).or_else(|| clean_commit_message(&res.thinking))
}

fn preview(text: &str) -> String {
    let trimmed: String = text.chars().take(120).collect();
    if text.chars().count() > 120 {
        format!("{trimmed}…")
    } else {
        trimmed
    }
}

fn is_chat_model(name: &str) -> bool {
    let lower = name.to_lowercase();
    !(lower.contains("embed") || lower.contains("cloud"))
}

fn is_small_model(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(":1b") || lower.ends_with(":300m")
}

fn model_preference_rank(name: &str) -> u8 {
    if is_small_model(name) {
        return 200;
    }
    if name.contains("llama3") {
        return 0;
    }
    if name.contains("qwen3") || name.contains("qwen") {
        return 1;
    }
    if name.contains("gemma3") || name.contains("gemma") {
        return 2;
    }
    50
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

fn map_request_error(err: reqwest::Error) -> anyhow::Error {
    if err.is_connect() {
        return map_connect_error(err);
    }
    if err.is_timeout() {
        return anyhow::anyhow!(
            "ollama request timed out. Try again, use a smaller model, or raise GIT_AUTO_COMMIT_TIMEOUT_SECS"
        );
    }
    err.into()
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

    #[test]
    fn message_from_response_uses_thinking_fallback() {
        let res = OllamaResponse {
            response: String::new(),
            thinking: "feat: add retry logic".to_string(),
            done_reason: Some("stop".to_string()),
        };
        assert_eq!(
            message_from_response(&res),
            Some("feat: add retry logic".to_string())
        );
    }

    #[test]
    fn is_chat_model_rejects_embeddings_and_cloud() {
        assert!(!is_chat_model("embeddinggemma:300m"));
        assert!(!is_chat_model("glm-5.1:cloud"));
        assert!(is_chat_model("llama3.2:3b"));
    }
}
