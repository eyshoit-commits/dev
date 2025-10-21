use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use chrono::Utc;
use config::FileFormat;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Settings {
    #[serde(default = "Settings::default_host")]
    host: IpAddr,
    #[serde(default = "Settings::default_port")]
    port: u16,
}

impl Settings {
    fn load() -> anyhow::Result<Self> {
        let mut builder = config::Config::builder();
        let path = PathBuf::from("config.mistral.json");
        if path.exists() {
            builder = builder.add_source(config::File::from(path).format(config::FileFormat::Json));
        }
        let runtime = PathBuf::from("config.mistral.runtime.json");
        if runtime.exists() {
            builder =
                builder.add_source(config::File::from(runtime).format(config::FileFormat::Json));
        }
        builder = builder.add_source(
            config::Environment::with_prefix("MAID_MISTRAL")
                .separator("__")
                .try_parsing(true)
                .list_separator(","),
        );
        let settings: Settings = builder.build()?.try_deserialize()?;
        Ok(settings)
    }

    fn default_host() -> IpAddr {
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    }

    fn default_port() -> u16 {
        43140
    }

    fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }
}

#[derive(Clone)]
struct AppState {
    settings: Settings,
}

impl AppState {
    fn new(settings: Settings) -> Self {
        Self { settings }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();

    let settings = Settings::load()?;
    info!(?settings, "MAID mistral.rs plugin starting");

    let state = Arc::new(AppState::new(settings.clone()));

    let router = Router::new()
        .route("/v1/recipes", post(generate_recipe))
        .route("/v1/analysis", post(analyze_run))
        .route("/api/inference/text", post(text_inference))
        .route("/api/inference/image", post(image_inference))
        .route("/api/inference/speech", post(speech_inference))
        .route("/v1/chat/completions", post(chat_completions))
        .with_state(state.clone());

    let addr = settings.socket_addr();
    info!(%addr, "starting mistral.rs HTTP server");

    let server = axum::Server::bind(&addr).serve(router.into_make_service());

    tokio::select! {
        result = server => {
            if let Err(err) = result {
                error!(?err, "server error");
                return Err(err.into());
            }
        }
        _ = signal::ctrl_c() => {
            info!("ctrl-c received, shutting down");
        }
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
struct RecipeRequest {
    model: Option<String>,
    input: String,
}

#[derive(Debug, Serialize)]
struct RecipeResponse {
    config: serde_json::Value,
    notes: Option<String>,
}

async fn generate_recipe(Json(request): Json<RecipeRequest>) -> Json<RecipeResponse> {
    let summary = request.input.to_lowercase();
    let target = extract_target(&summary).unwrap_or_else(|| "https://example.com".to_string());
    let scenario = if summary.contains("checkout") {
        "checkout"
    } else if summary.contains("search") {
        "search"
    } else {
        "baseline"
    };
    let config = serde_json::json!({
        "target_base_url": target,
        "users": determine_users(&summary),
        "hatch_rate": 20,
        "duration": { "seconds": 300 },
        "think_time_seconds": 2,
        "scheduler": "round-robin",
        "scenarios": [
            {
                "name": format!("{}-scenario", scenario),
                "weight": 1,
                "transactions": [
                    {
                        "name": "get-root",
                        "weight": 1,
                        "request": {
                            "method": "GET",
                            "path": "/",
                            "headers": [
                                {"name": "accept", "value": "application/json"}
                            ],
                            "allowRedirects": true
                        }
                    }
                ]
            }
        ],
        "reports": { "formats": ["json", "csv", "html"] }
    });
    let notes = Some(format!("Generated {} scenario based on prompt", scenario));
    Json(RecipeResponse { config, notes })
}

#[derive(Debug, Deserialize)]
struct AnalysisRequest {
    model: Option<String>,
    run: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct AnalysisResponse {
    summary: String,
    bottlenecks: Vec<String>,
    recommendations: Vec<String>,
}

async fn analyze_run(Json(request): Json<AnalysisRequest>) -> Json<AnalysisResponse> {
    let stats = compute_run_stats(&request.run);
    let mut bottlenecks = Vec::new();
    let mut recommendations = Vec::new();
    if stats.error_rate > 1.0 {
        bottlenecks.push(format!(
            "High error rate detected: {:.2}%, investigate upstream services",
            stats.error_rate
        ));
        recommendations.push("Enable retry/backoff strategy and inspect error logs".to_string());
    }
    if stats.p95_latency > 350.0 {
        bottlenecks.push(format!(
            "p95 latency {:.2}ms exceeds target",
            stats.p95_latency
        ));
        recommendations.push("Scale load generators or tune target service caching".to_string());
    }
    if bottlenecks.is_empty() {
        bottlenecks.push("No critical bottlenecks detected".to_string());
        recommendations.push("Increase user count gradually to explore upper limits".to_string());
    }
    let summary = format!(
        "Processed {} samples. Avg RPS {:.2}, error rate {:.2}%.",
        stats.samples, stats.avg_rps, stats.error_rate
    );
    Json(AnalysisResponse {
        summary,
        bottlenecks,
        recommendations,
    })
}

#[derive(Debug, Deserialize)]
struct TextInferenceRequest {
    prompt: String,
}

#[derive(Debug, Serialize)]
struct TextInferenceResponse {
    id: String,
    created: i64,
    result: String,
}

async fn text_inference(Json(request): Json<TextInferenceRequest>) -> Json<TextInferenceResponse> {
    let result = format!(
        "mistral.rs synthesised response for prompt '{}': ensure load patterns include ramp-up and maintain phases.",
        request.prompt
    );
    Json(TextInferenceResponse {
        id: Uuid::new_v4().to_string(),
        created: Utc::now().timestamp(),
        result,
    })
}

#[derive(Debug, Deserialize)]
struct ImageInferenceRequest {
    prompt: String,
}

#[derive(Debug, Serialize)]
struct ImageInferenceResponse {
    id: String,
    created: i64,
    url: String,
    description: String,
}

async fn image_inference(
    Json(request): Json<ImageInferenceRequest>,
) -> Json<ImageInferenceResponse> {
    let url = format!(
        "https://images.maid.example/{}/{}.png",
        Uuid::new_v4(),
        request.prompt.len()
    );
    Json(ImageInferenceResponse {
        id: Uuid::new_v4().to_string(),
        created: Utc::now().timestamp(),
        url,
        description: "Generated performance heatmap highlighting latency hotspots".to_string(),
    })
}

#[derive(Debug, Deserialize)]
struct SpeechInferenceRequest {
    text: String,
    voice: Option<String>,
}

#[derive(Debug, Serialize)]
struct SpeechInferenceResponse {
    id: String,
    created: i64,
    codec: String,
    url: String,
}

async fn speech_inference(
    Json(request): Json<SpeechInferenceRequest>,
) -> Json<SpeechInferenceResponse> {
    let voice = request.voice.unwrap_or_else(|| "dia-1.6b".to_string());
    let url = format!(
        "https://audio.maid.example/{}/{}.wav",
        voice,
        Uuid::new_v4()
    );
    Json(SpeechInferenceResponse {
        id: Uuid::new_v4().to_string(),
        created: Utc::now().timestamp(),
        codec: "pcm16".to_string(),
        url,
    })
}

#[derive(Debug, Deserialize)]
struct ChatCompletionsRequest {
    model: Option<String>,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatCompletionsResponse {
    id: String,
    created: i64,
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Serialize)]
struct ChatChoice {
    index: u32,
    message: ChatMessage,
    finish_reason: String,
}

async fn chat_completions(
    Json(request): Json<ChatCompletionsRequest>,
) -> Json<ChatCompletionsResponse> {
    let last = request
        .messages
        .last()
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let reply = format!(
        "For '{}', schedule ramp-up over 60s, maintain for 5m, ensure error budget <1%.",
        last
    );
    Json(ChatCompletionsResponse {
        id: Uuid::new_v4().to_string(),
        created: Utc::now().timestamp(),
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: reply,
            },
            finish_reason: "stop".to_string(),
        }],
    })
}

fn extract_target(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|token| token.starts_with("http"))
        .map(|value| value.trim_matches(|c| c == ',' || c == '.').to_string())
}

fn determine_users(text: &str) -> u32 {
    if text.contains("thousand") {
        1000
    } else if let Some(value) = text
        .split_whitespace()
        .find_map(|token| token.parse::<u32>().ok())
    {
        value
    } else {
        200
    }
}

struct RunStats {
    samples: usize,
    avg_rps: f32,
    error_rate: f32,
    p95_latency: f32,
}

fn compute_run_stats(run: &serde_json::Value) -> RunStats {
    let metrics = run.as_array().cloned().unwrap_or_default();
    let mut total_rps = 0.0;
    let mut total_error = 0.0;
    let mut max_p95 = 0.0;
    let mut samples = 0usize;
    for item in metrics {
        if let Some(obj) = item.as_object() {
            if let Some(rps) = obj.get("throughput_rps").and_then(|v| v.as_f64()) {
                total_rps += rps as f32;
            }
            if let Some(error) = obj.get("error_rate").and_then(|v| v.as_f64()) {
                total_error += error as f32;
            }
            if let Some(p95) = obj.get("latency_p95_ms").and_then(|v| v.as_f64()) {
                if p95 as f32 > max_p95 {
                    max_p95 = p95 as f32;
                }
            }
            samples += 1;
        }
    }
    if samples == 0 {
        RunStats {
            samples: 0,
            avg_rps: 0.0,
            error_rate: 0.0,
            p95_latency: 0.0,
        }
    } else {
        RunStats {
            samples,
            avg_rps: total_rps / samples as f32,
            error_rate: total_error / samples as f32,
            p95_latency: max_p95,
        }
    }
}
