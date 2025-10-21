use crate::config::Settings;
use anyhow::Context;
use reqwest::Client;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct PluginRegistry {
    mistral: Option<Arc<MistralClient>>,
}

impl PluginRegistry {
    pub fn new(settings: &Settings) -> anyhow::Result<Self> {
        let mistral = if let Some(endpoint) = &settings.plugin_bus.mistral_endpoint {
            Some(Arc::new(MistralClient::new(
                endpoint.clone(),
                settings.plugin_bus.mistral_api_key.clone(),
            )?))
        } else {
            None
        };

        Ok(Self { mistral })
    }

    pub fn mistral(&self) -> Option<Arc<MistralClient>> {
        self.mistral.clone()
    }
}

#[derive(Clone)]
pub struct MistralClient {
    base_url: String,
    api_key: Option<String>,
    client: Client,
}

impl MistralClient {
    pub fn new(base_url: String, api_key: Option<String>) -> anyhow::Result<Self> {
        let client = Client::builder().build()?;
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            client,
        })
    }

    fn auth_headers(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(key) = &self.api_key {
            req.bearer_auth(key)
        } else {
            req
        }
    }

    pub async fn generate_config(&self, prompt: &str) -> anyhow::Result<RecipeResponse> {
        let payload = RecipeRequest {
            model: "gpt-4o-mini".to_string(),
            input: prompt.to_string(),
        };
        let req = self
            .client
            .post(format!("{}/v1/recipes", self.base_url))
            .json(&payload);
        let response = self
            .auth_headers(req)
            .send()
            .await
            .context("failed to call mistral recipe endpoint")?;
        let response = response.json::<RecipeResponse>().await?;
        Ok(response)
    }

    pub async fn analyze_results(
        &self,
        run_summary: &serde_json::Value,
    ) -> anyhow::Result<AnalysisResponse> {
        let payload = AnalysisRequest {
            model: "gpt-4o-mini".to_string(),
            run: run_summary.clone(),
        };
        let req = self
            .client
            .post(format!("{}/v1/analysis", self.base_url))
            .json(&payload);
        let response = self
            .auth_headers(req)
            .send()
            .await
            .context("failed to call mistral analysis endpoint")?;
        Ok(response.json::<AnalysisResponse>().await?)
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecipeRequest {
    pub model: String,
    pub input: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecipeResponse {
    pub config: serde_json::Value,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisRequest {
    pub model: String,
    pub run: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisResponse {
    pub summary: String,
    pub bottlenecks: Vec<String>,
    pub recommendations: Vec<String>,
}
