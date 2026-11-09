//! Model catalogue for the Vly AI gateway.
//!
//! The bundled list is every text/coding model exposed by the Vercel AI
//! Gateway that the Vly gateway proxies. `VlyClient::list_models` refreshes it
//! from the deployment's own `/models` endpoint, which is authoritative.

use std::sync::OnceLock;

use serde_json::Value;

/// One chat/text model exposed by the gateway.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider: String,
}

impl ModelInfo {
    pub fn new(id: &str, name: Option<&str>) -> Self {
        let id = id.trim();
        let provider = id.split('/').next().unwrap_or("gateway").to_string();
        let name = name
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(id)
            .to_string();
        Self {
            id: id.to_string(),
            name,
            provider,
        }
    }

    pub fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_ascii_lowercase();
        query.is_empty()
            || self.id.to_ascii_lowercase().contains(&query)
            || self.name.to_ascii_lowercase().contains(&query)
    }
}

impl std::fmt::Display for ModelInfo {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} · {}", self.provider, self.name)
    }
}

/// Models worth surfacing before the user searches the full catalogue.
pub const RECOMMENDED: &[&str] = &[
    "openai/gpt-5.3-codex",
    "openai/gpt-5.4",
    "openai/gpt-5.5",
    "openai/gpt-5-codex",
    "anthropic/claude-sonnet-5",
    "anthropic/claude-opus-5",
    "google/gemini-3.1-pro-preview",
    "google/gemini-3.8-flash",
    "deepseek/deepseek-v4-pro",
    "moonshotai/kimi-k2.7-code",
    "zai/glm-5.3",
    "alibaba/qwen3-coder",
    "spacexai/grok-4.7",
    "mistral/codestral",
];

pub fn is_recommended(id: &str) -> bool {
    RECOMMENDED.contains(&id)
}

/// Every text/coding model in the Vercel AI Gateway catalogue, as
/// `id|display name`. Batch-only, image-only and video-only variants are
/// excluded because the interactive chat endpoint cannot use them.
const BUNDLED: &str = "\
alibaba/qwen-3-14b|Qwen3-14B
alibaba/qwen-3-235b|Qwen3 235B A22B
alibaba/qwen-3-30b|Qwen3-30B-A3B
alibaba/qwen-3-32b|Qwen 3 32B
alibaba/qwen3-coder|Qwen3 Coder 480B A35B Instruct
alibaba/qwen3-coder-30b-a3b|Qwen 3 Coder 30B A3B Instruct
alibaba/qwen3-coder-next|Qwen3 Coder Next
alibaba/qwen3-max-thinking|Qwen 3 Max Thinking
alibaba/qwen3-next-80b-a3b-instruct|Qwen3 Next 80B A3B Instruct
alibaba/qwen3-next-80b-a3b-thinking|Qwen3 Next 80B A3B Thinking
alibaba/qwen3-vl-instruct|Qwen3 VL 235B A22B Instruct
alibaba/qwen3.5-flash|Qwen 3.5 Flash
alibaba/qwen3.5-plus|Qwen 3.5 Plus
alibaba/qwen3.6-27b|Qwen 3.6 27B
alibaba/qwen3.6-plus|Qwen 3.6 Plus
alibaba/qwen3.7-flash|Qwen 3.7 Flash
alibaba/qwen3.7-max|Qwen 3.7 Max
alibaba/qwen3.7-plus|Qwen 3.7 Plus
alibaba/qwen3.8-2.4t-a95b|Qwen3.8 2.4T A95B
alibaba/qwen3.8-27b|Qwen3.8 27B
alibaba/qwen3.8-flash|Qwen 3.8 Flash
alibaba/qwen3.8-max|Qwen 3.8 Max
alibaba/qwen3.8-max-0902|Qwen3.8 Max 0902
alibaba/qwen3.8-max-prime|Qwen 3.8 Max Prime
alibaba/qwen3.8-omni-flash|Qwen 3.8 Omni Flash
amazon/nova-2-lite|Nova 2 Lite
amazon/nova-lite|Nova Lite
amazon/nova-micro|Nova Micro
amazon/nova-pro|Nova Pro
anthropic/claude-fable-5|Claude Fable 5
anthropic/claude-fable-5.1|Claude Fable 5.1
anthropic/claude-haiku-4.5|Claude Haiku 4.5
anthropic/claude-haiku-5.5|Claude Haiku 5.5
anthropic/claude-opus-4.5|Claude Opus 4.5
anthropic/claude-opus-4.6|Claude Opus 4.6
anthropic/claude-opus-4.7|Claude Opus 4.7
anthropic/claude-opus-4.8|Claude Opus 4.8
anthropic/claude-opus-4.8-fast|Claude Opus 4.8 (Fast)
anthropic/claude-opus-5|Claude Opus 5
anthropic/claude-opus-5-fast|Claude Opus 5 (Fast)
anthropic/claude-opus-5.5|Claude Opus 5.5
anthropic/claude-opus-5.5-fast|Claude Opus 5.5 (Fast)
anthropic/claude-sonnet-4.5|Claude Sonnet 4.5
anthropic/claude-sonnet-4.6|Claude Sonnet 4.6
anthropic/claude-sonnet-5|Claude Sonnet 5
anthropic/claude-sonnet-5.5|Claude Sonnet 5.5
bytedance/seed-1.6|Seed 1.6
bytedance/seed-1.8|Bytedance Seed 1.8
bytedance/seed-2.1-turbo|Seed 2.1 Turbo
callstack/apex|Apex
cohere/command-a|Command A
deepseek/deepseek-r1|DeepSeek-R1
deepseek/deepseek-v3.1|DeepSeek V3.1
deepseek/deepseek-v3.2|DeepSeek V3.2
deepseek/deepseek-v3.2-thinking|DeepSeek V3.2 Thinking
deepseek/deepseek-v4-flash|DeepSeek V4 Flash
deepseek/deepseek-v4-flash-0731|DeepSeek V4 Flash 0731
deepseek/deepseek-v4-flash-vision-exp|DeepSeek V4 Flash Vision Exp
deepseek/deepseek-v4-pro|DeepSeek V4 Pro
deepseek/deepseek-v4-pro-0813|DeepSeek V4 Pro 0813
deepseek/deepseek-v4.1-flash|DeepSeek V4.1 Flash
deepseek/deepseek-v4.1-flash-1017|DeepSeek V4.1 Flash 1017
deepseek/deepseek-v4.1-flash-1017-fast|DeepSeek V4.1 Flash 1017 Fast
deepseek/deepseek-v4.1-flash-1017-ultrafast|DeepSeek V4.1 Flash 1017 Ultrafast
deepseek/deepseek-v4.1-flash-fast|DeepSeek V4.1 Flash Fast
elorian/glimpse|Glimpse
fireworks/ember-1|Ember-1
google/gemini-2.5-flash|Gemini 2.5 Flash
google/gemini-2.5-flash-lite|Gemini 2.5 Flash Lite
google/gemini-2.5-pro|Gemini 2.5 Pro
google/gemini-3-flash|Gemini 3 Flash
google/gemini-3.1-flash-lite|Gemini 3.1 Flash Lite
google/gemini-3.1-pro-preview|Gemini 3.1 Pro Preview
google/gemini-3.5-flash|Gemini 3.5 Flash
google/gemini-3.5-flash-lite|Gemini 3.5 Flash Lite
google/gemini-3.6-flash|Gemini 3.6 Flash
google/gemini-3.7-flash|Gemini 3.7 Flash
google/gemini-3.8-flash|Gemini 3.8 Flash
google/gemini-4-argon|Gemini 4 Argon
google/gemma-4-26b-a4b-it|Google Gemma 4 26B A4B
google/gemma-4-31b-it|Gemma 4 31B IT
humansand/ampersand-v0.1|Ampersand v0.1
inception/mercury-2|Mercury 2
inception/mercury-2.5|Mercury 2.5
inception/mercury-3|Mercury 3
inception/mercury-coder-small|Mercury Coder Small Beta
inclusionai/ling-3.0-flash|Ling 3.0 Flash
inclusionai/ling-3.0-flash-fin|Ling 3.0 Flash Fin
inclusionai/ling-3.0-flash-sante|Ling 3.0 Flash Sante
inclusionai/ling-3.0-flash-vl|Ling 3.0 Flash VL
inclusionai/ling-3.1-flash|Ling 3.1 Flash
inclusionai/ling-3.1-flash-1019|Ling 3.1 Flash 1019
inference-net/schematron-v2-small|Schematron V2 Small
inference-net/schematron-v2-turbo|Schematron V2 Turbo
interfaze/interfaze-beta|Interfaze Beta
liquid/lfm2.5-24b-a2b|LFM2.5-24B-A2B
meituan/longcat-2.5|LongCat 2.5
meta/llama-3.1-70b|Llama 3.1 70B Instruct
meta/llama-3.1-8b|Llama 3.1 8B Instruct
meta/llama-3.3-70b|Llama 3.3 70B Instruct
meta/llama-4-maverick|Llama 4 Maverick 17B Instruct
meta/llama-4-scout|Llama 4 Scout 17B Instruct
meta/muse-glimmer-30b|Muse Glimmer 30B
meta/muse-spark-1.1|Muse Spark 1.1
meta/muse-spark-1.2|Muse Spark 1.2
meta/muse-spark-1.2-contributor|Muse Spark 1.2 Contributor
meta/muse-spark-1.3|Muse Spark 1.3
meta/muse-spark-1.3-contributor|Muse Spark 1.3 Contributor
meta/muse-spark-open|Muse Spark Open
meta/muse-wonder-1.0|Muse Wonder
minimax/minimax-m2|MiniMax M2
minimax/minimax-m2.1|MiniMax M2.1
minimax/minimax-m2.1-lightning|MiniMax M2.1 Lightning
minimax/minimax-m2.5|MiniMax M2.5
minimax/minimax-m2.5-highspeed|MiniMax M2.5 High Speed
minimax/minimax-m2.7|MiniMax M2.7
minimax/minimax-m2.7-highspeed|MiniMax M2.7 High Speed
minimax/minimax-m3|MiniMax M3
minimax/minimax-m3.1-flash|MiniMax-M3.1-Flash
mistral/codestral|Mistral Codestral
mistral/ministral-14b|Ministral 14B
mistral/ministral-3b|Ministral 3B
mistral/ministral-8b|Ministral 8B
mistral/mistral-large-3|Mistral Large 3
mistral/mistral-large-4|Mistral Large 4
mistral/mistral-medium-3.5|Mistral Medium Latest
mistral/mistral-nemo|Mistral Nemo 12B
mistral/mistral-small|Mistral Small
mixedbread/toast-1|Toast 1
moonshotai/kimi-k2|Kimi K2 Instruct
moonshotai/kimi-k2.5|Kimi K2.5
moonshotai/kimi-k2.6|Kimi K2.6
moonshotai/kimi-k2.7-code|Kimi K2.7 Code
moonshotai/kimi-k2.7-code-highspeed|Kimi K2.7 Code High Speed
moonshotai/kimi-k3|Kimi K3
moonshotai/kimi-k3-fast|Kimi K3 Fast
moonshotai/kimi-k3.1-pro|Kimi K3.1 Pro
moonshotai/kimi-k3.1-pro-highspeed|Kimi K3.1 Pro Highspeed
morph/morph-v3-fast|Morph V3 Fast
morph/morph-v3-large|Morph V3 Large
nvidia/nemotron-3-nano-30b-a3b|Nemotron 3 Nano 30B A3B
nvidia/nemotron-3-super-120b-a12b|NVIDIA Nemotron 3 Super 120B A12B
nvidia/nemotron-3-ultra-550b-a55b|Nemotron 3 Ultra
nvidia/nemotron-3.5-lightning|Nemotron 3.5 Lightning 30B
nvidia/nemotron-3.5-lightning-free|Nemotron 3.5 Lightning Free
nvidia/nemotron-3.5-super-vl|NVIDIA Nemotron 3.5 Super VL
nvidia/nemotron-nano-12b-v2-vl|Nvidia Nemotron Nano 12B V2 VL
nvidia/nemotron-nano-9b-v2|Nvidia Nemotron Nano 9B V2
openai/gpt-4.1|GPT-4.1
openai/gpt-4.1-fast|GPT-4.1 (Fast)
openai/gpt-4.1-mini|GPT-4.1 mini
openai/gpt-4.1-mini-fast|GPT-4.1 mini (Fast)
openai/gpt-4.1-nano|GPT-4.1 nano
openai/gpt-4o|GPT-4o
openai/gpt-4o-fast|GPT-4o (Fast)
openai/gpt-4o-mini|GPT-4o mini
openai/gpt-4o-mini-fast|GPT-4o mini (Fast)
openai/gpt-5|GPT-5
openai/gpt-5-codex|GPT-5-Codex
openai/gpt-5-fast|GPT-5 (Fast)
openai/gpt-5-mini|GPT-5 mini
openai/gpt-5-mini-fast|GPT-5 mini (Fast)
openai/gpt-5-nano|GPT-5 nano
openai/gpt-5-pro|GPT-5 pro
openai/gpt-5.1-codex|GPT-5.1-Codex
openai/gpt-5.1-codex-max|GPT 5.1 Codex Max
openai/gpt-5.1-codex-mini|GPT 5.1 Codex Mini
openai/gpt-5.1-thinking|GPT 5.1 Thinking
openai/gpt-5.1-thinking-fast|GPT 5.1 Thinking (Fast)
openai/gpt-5.2|GPT 5.2
openai/gpt-5.2-codex|GPT 5.2 Codex
openai/gpt-5.2-fast|GPT 5.2 (Fast)
openai/gpt-5.2-pro|GPT 5.2 Pro
openai/gpt-5.3-codex|GPT 5.3 Codex
openai/gpt-5.3-codex-fast|GPT 5.3 Codex (Fast)
openai/gpt-5.4|GPT 5.4
openai/gpt-5.4-fast|GPT 5.4 (Fast)
openai/gpt-5.4-mini|GPT 5.4 Mini
openai/gpt-5.4-mini-fast|GPT 5.4 Mini (Fast)
openai/gpt-5.4-nano|GPT 5.4 Nano
openai/gpt-5.4-pro|GPT 5.4 Pro
openai/gpt-5.5|GPT 5.5
openai/gpt-5.5-fast|GPT 5.5 (Fast)
openai/gpt-5.5-pro|GPT 5.5 Pro
openai/gpt-5.6-luna|GPT 5.6 Luna
openai/gpt-5.6-luna-fast|GPT 5.6 Luna (Fast)
openai/gpt-5.6-sol|GPT 5.6 Sol
openai/gpt-5.6-sol-fast|GPT 5.6 Sol (Fast)
openai/gpt-5.6-terra|GPT 5.6 Terra
openai/gpt-5.6-terra-fast|GPT 5.6 Terra (Fast)
openai/gpt-6-astra|GPT-6 Astra
openai/gpt-6-astra-fast|GPT-6 Astra (Fast)
openai/gpt-6-luna|GPT-6 Luna
openai/gpt-6-luna-fast|GPT-6 Luna (Fast)
openai/gpt-6-sol|GPT-6 Sol
openai/gpt-6-sol-fast|GPT-6 Sol (Fast)
openai/gpt-6.1-sol|GPT-6.1 Sol
openai/gpt-6.1-sol-fast|GPT-6.1 Sol (Fast)
openai/gpt-6.1-terra|GPT-6.1 Terra
openai/gpt-6.1-terra-fast|GPT-6.1 Terra (Fast)
openai/gpt-oss-120b|GPT OSS 120B
openai/gpt-oss-20b|GPT OSS 20B
openai/gpt-oss-safeguard-120b|GPT OSS Safeguard 120B
openai/gpt-oss-safeguard-20b|GPT OSS Safeguard 20B
openai/o1|o1
openai/o3|o3
openai/o3-fast|o3 (Fast)
openai/o3-mini|o3-mini
openai/o3-pro|o3 Pro
openai/o4-mini|o4-mini
particle/particle-1|Particle-1
perplexity/sonar|Sonar
poolside/laguna-s-2.1|Laguna S 2.1
reflection/beam|Beam 1.0
reflection/beam-free|Beam 1.0 (Free)
sakana/fugu-max|Fugu Max
sakana/fugu-ultra|Fugu Ultra
sakana/fugu-ultra-v2|Fugu Ultra v2
sakana/namazu|Sakana Namazu
scaledown/scaledown-extract-1|Extract-1
scaledown/scaledown-summarize-1|Summarize-1
spacexai/grok-4.20-multi-agent|Grok 4.20 Multi-Agent
spacexai/grok-4.20-multi-agent-beta|Grok 4.20 Multi Agent Beta
spacexai/grok-4.20-non-reasoning|Grok 4.20 Non-Reasoning
spacexai/grok-4.20-non-reasoning-beta|Grok 4.20 Beta Non-Reasoning
spacexai/grok-4.20-reasoning|Grok 4.20 Reasoning
spacexai/grok-4.20-reasoning-beta|Grok 4.20 Beta Reasoning
spacexai/grok-4.3|Grok 4.3
spacexai/grok-4.5|Grok 4.5
spacexai/grok-4.6|Grok 4.6
spacexai/grok-4.7|Grok 4.7
spacexai/grok-build-0.1|Grok Build 0.1
stealth/glyph-cluster|Glyph Cluster
stepfun/step-3.7-flash|Step 3.7 Flash
stepfun/step-5-preview|Step 5 Preview
tencent/hy-mt2-lite|Tencent Hy-MT2-Lite
tencent/hy-mt2-plus|Tencent Hy-MT2-Plus
tencent/hy-mt2-pro|Tencent Hy-MT2-Pro
tencent/hy3|Tencent Hy3
tencent/hy4|Tencent Hy4
tencent/hy4-preview|Tencent Hy4 Preview
thinkingmachines/inkling|Inkling
thinkingmachines/inkling-small|Inkling Small
xiaomi/mimo-v2.6-flash|MiMo V2.6 Flash
xiaomi/mimo-v2.6-pro|MiMo V2.6 Pro
xiaomi/mimo-v2.6-pro-ultraspeed|MiMo V2.6 Pro UltraSpeed
zai/glm-4.5|GLM 4.5
zai/glm-4.5-air|GLM 4.5 Air
zai/glm-4.5v|GLM 4.5V
zai/glm-4.6|GLM 4.6
zai/glm-4.7|GLM 4.7
zai/glm-4.7-flash|GLM 4.7 Flash
zai/glm-4.7-flashx|GLM 4.7 FlashX
zai/glm-5|GLM 5
zai/glm-5-turbo|GLM 5 Turbo
zai/glm-5.1|GLM 5.1
zai/glm-5.2|GLM 5.2
zai/glm-5.2-fast|GLM 5.2 Fast
zai/glm-5.3|GLM 5.3
zai/glm-5.3-fast|GLM 5.3 Fast
zai/glm-5.3-flash|GLM 5.3 Flash
zai/glm-5.3-flashx|GLM 5.3 FlashX
zai/glm-5.3-flashx-legal|GLM 5.3 FlashX Legal
zai/glm-5v-turbo|GLM 5V Turbo";

/// The built-in catalogue, parsed once per process.
pub fn bundled_models() -> Vec<ModelInfo> {
    static CACHE: OnceLock<Vec<ModelInfo>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            BUNDLED
                .lines()
                .filter_map(|line| {
                    let mut parts = line.splitn(2, '|');
                    let id = parts.next()?.trim();
                    if id.is_empty() {
                        return None;
                    }
                    Some(ModelInfo::new(id, parts.next()))
                })
                .collect()
        })
        .clone()
}

/// Parse an OpenAI-compatible `/models` response. Accepts `data`, `models`
/// or a bare array, with objects or id strings, and drops anything that is not
/// an interactive text model.
pub fn parse_models(body: &Value) -> Vec<ModelInfo> {
    let list = body
        .get("data")
        .or_else(|| body.get("models"))
        .unwrap_or(body);
    let Some(entries) = list.as_array() else {
        return Vec::new();
    };

    let mut models: Vec<ModelInfo> = Vec::new();
    for entry in entries {
        if let Some(id) = entry.as_str() {
            if is_text_model(id, &[]) {
                models.push(ModelInfo::new(id, None));
            }
            continue;
        }

        let Some(id) = entry
            .get("id")
            .and_then(Value::as_str)
            .or_else(|| entry.get("model").and_then(Value::as_str))
        else {
            continue;
        };

        if let Some(kind) = entry.get("type").and_then(Value::as_str) {
            let kind = kind.to_ascii_lowercase();
            if !matches!(kind.as_str(), "language" | "chat" | "text" | "llm") {
                continue;
            }
        }

        let tags: Vec<&str> = entry
            .get("tags")
            .and_then(Value::as_array)
            .map(|tags| tags.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        if !is_text_model(id, &tags) {
            continue;
        }

        let name = entry
            .get("name")
            .or_else(|| entry.get("display_name"))
            .and_then(Value::as_str);
        models.push(ModelInfo::new(id, name));
    }

    models.sort_by(|a, b| a.provider.cmp(&b.provider).then(a.id.cmp(&b.id)));
    models.dedup_by(|a, b| a.id == b.id);
    models
}

fn is_text_model(id: &str, tags: &[&str]) -> bool {
    let lower = id.to_ascii_lowercase();
    if lower.trim().is_empty() || lower.ends_with("-batch") {
        return false;
    }
    if tags
        .iter()
        .any(|tag| matches!(*tag, "image-generation" | "video-generation"))
    {
        return false;
    }
    ![
        "embedding",
        "embed-",
        "tts",
        "whisper",
        "dall-e",
        "rerank",
        "moderation",
        "image-generation",
        "video-generation",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bundled_list_contains_every_provider() {
        let models = bundled_models();
        assert!(models.len() > 250, "bundled catalogue is too small");
        for provider in [
            "openai",
            "anthropic",
            "google",
            "deepseek",
            "moonshotai",
            "zai",
            "mistral",
            "meta",
        ] {
            assert!(
                models.iter().any(|model| model.provider == provider),
                "missing provider {provider}"
            );
        }
        assert!(models.iter().all(|model| !model.id.ends_with("-batch")));
        assert!(models.iter().all(|model| !model.name.trim().is_empty()));
    }

    #[test]
    fn recommended_models_exist_in_bundled_list() {
        let models = bundled_models();
        for id in RECOMMENDED {
            assert!(
                models.iter().any(|model| model.id == *id),
                "recommended model {id} is not in the catalogue"
            );
        }
    }

    #[test]
    fn parses_openai_style_model_list() {
        let body = json!({
            "object": "list",
            "data": [
                {"id": "openai/gpt-5", "name": "GPT-5", "type": "language", "tags": ["reasoning"]},
                {"id": "openai/text-embedding-3-large", "type": "embedding"},
                {"id": "google/gemini-3-flash-image", "type": "language", "tags": ["image-generation"]},
                {"id": "anthropic/claude-sonnet-5", "type": "language"}
            ]
        });
        let models = parse_models(&body);
        let ids: Vec<&str> = models.iter().map(|model| model.id.as_str()).collect();
        assert_eq!(ids, vec!["anthropic/claude-sonnet-5", "openai/gpt-5"]);
        assert_eq!(models[1].name, "GPT-5");
    }

    #[test]
    fn parses_string_and_bare_array_lists() {
        let models = parse_models(&json!(["zai/glm-5", "openai/gpt-5-codex"]));
        assert_eq!(models.len(), 2);
        let nested = parse_models(&json!({"models": [{"id": "mistral/codestral"}]}));
        assert_eq!(nested[0].provider, "mistral");
    }

    #[test]
    fn filters_batch_models() {
        let models = parse_models(&json!({"data": [
            {"id": "openai/gpt-5-batch", "type": "language"},
            {"id": "openai/gpt-5", "type": "language"}
        ]}));
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "openai/gpt-5");
    }

    #[test]
    fn display_uses_provider_and_name() {
        let model = ModelInfo::new("openai/gpt-5", Some("GPT-5"));
        assert_eq!(model.to_string(), "openai · GPT-5");
        assert!(model.matches("gpt"));
        assert!(model.matches("openai"));
        assert!(!model.matches("claude"));
    }
}
