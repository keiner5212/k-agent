use serde::{Deserialize, Serialize};

use crate::catalog::{ModelReasoningSpec, ModelRequestSpec, ModelSamplingSpec};
use crate::providers::{ProviderKind, UserParamKind, UserRequestParam, UserRequestSpec};

const OUTPUT_TOKEN_MAX: u64 = 32_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DescribeModelRequest {
    pub kind: ProviderKind,
    pub base_url: String,
    pub model_id: String,
    #[serde(default)]
    pub user_request: Option<UserRequestSpec>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequestOptions {
    #[serde(default)]
    pub reasoning_mode: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub service_tier: Option<String>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub max_output_tokens: Option<u64>,
    #[serde(default)]
    pub limit_provider_data_use: bool,
    #[serde(default)]
    pub extra: Vec<RequestExtra>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RequestExtra {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRequestView {
    pub vendor: String,
    pub model_contract: String,
    pub reasoning: ReasoningView,
    pub service_tiers: Vec<String>,
    pub sampling: SamplingView,
    pub privacy_support: String,
    pub privacy_detail_key: String,
    pub reasoning_split: bool,
    pub notes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<UserRequestParam>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum ReasoningView {
    #[serde(rename = "unsupported")]
    Unsupported,
    #[serde(rename = "unknown")]
    Unknown,
    #[serde(rename = "thinking")]
    Thinking {
        modes: Vec<String>,
        default_mode: String,
        locked_on: bool,
    },
    #[serde(rename = "effort")]
    Effort {
        levels: Vec<String>,
        default_level: String,
    },
    #[serde(rename = "gemini-level")]
    GeminiLevel {
        levels: Vec<String>,
        default_level: String,
    },
    #[serde(rename = "gemini-budget")]
    GeminiBudget {
        modes: Vec<String>,
        default_mode: String,
        locked_on: bool,
    },
    #[serde(rename = "claude")]
    Claude {
        thinking_modes: Vec<String>,
        default_thinking: String,
        locked_on: bool,
        levels: Vec<String>,
        default_level: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplingView {
    pub temperature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenField {
    MaxTokens,
    MaxCompletionTokens,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyWire {
    OpenAiStoreFalse,
    OpenRouterDataCollectionDeny,
}

#[derive(Debug, Clone)]
pub struct WirePlan {
    pub enable_reasoning: bool,
    pub openai_thinking: Option<String>,
    pub reasoning_split: bool,
    pub reasoning_effort: Option<String>,
    pub anthropic_thinking_type: Option<String>,
    pub anthropic_budget: Option<u64>,
    pub anthropic_effort: Option<String>,
    pub anthropic_interleaved: bool,
    pub gemini_level: Option<String>,
    pub gemini_budget: Option<i64>,
    pub include_thoughts: bool,
    pub service_tier: Option<String>,
    pub temperature: Option<f64>,
    pub gemini_top_p: bool,
    pub token_field: TokenField,
    pub privacy: Option<PrivacyWire>,
    pub max_output: u64,
    pub extra: Vec<(String, String)>,
}

#[derive(Clone, Copy)]
struct Query<'a> {
    kind: ProviderKind,
    base_url: &'a str,
    model_id: &'a str,
}

#[derive(Clone, Copy)]
enum Sampling {
    Omit,
    Fixed(f64),
    Range {
        min: f64,
        max: f64,
        default_value: f64,
    },
}

struct Contract {
    vendor: &'static str,
    model_known: bool,
    reasoning: ModelReasoningSpec,
    tiers: Vec<String>,
    sampling: Sampling,
    privacy_support: &'static str,
    privacy_detail_key: &'static str,
    privacy_wire: Option<PrivacyWire>,
    reasoning_split: bool,
    token_field: TokenField,
    gemini_top_p: bool,
    notes: Vec<&'static str>,
    opus5_disable_limit: bool,
}

#[tauri::command]
pub fn describe_model_request(query: DescribeModelRequest) -> ModelRequestView {
    describe(
        &Query {
            kind: query.kind,
            base_url: &query.base_url,
            model_id: &query.model_id,
        },
        query.user_request.as_ref(),
    )
}

pub fn prepare(
    query: &DescribeModelRequest,
    choices: &ChatRequestOptions,
    max_output: u64,
) -> WirePlan {
    let contract = contract(
        &Query {
            kind: query.kind,
            base_url: &query.base_url,
            model_id: &query.model_id,
        },
        query.user_request.as_ref(),
    );
    plan_for(&contract, choices, max_output)
}

pub fn quiet_query(kind: ProviderKind, base_url: &str, model_id: &str) -> DescribeModelRequest {
    DescribeModelRequest {
        kind,
        base_url: base_url.to_string(),
        model_id: model_id.to_string(),
        user_request: None,
    }
}

fn describe(query: &Query<'_>, user: Option<&UserRequestSpec>) -> ModelRequestView {
    let contract = contract(query, user);
    ModelRequestView {
        vendor: contract.vendor.to_string(),
        model_contract: if contract.model_known {
            "verified".into()
        } else {
            "unknown".into()
        },
        reasoning: view_reasoning(&contract.reasoning),
        service_tiers: contract.tiers.clone(),
        sampling: view_sampling(contract.sampling),
        privacy_support: contract.privacy_support.to_string(),
        privacy_detail_key: contract.privacy_detail_key.to_string(),
        reasoning_split: contract.reasoning_split,
        notes: contract.notes.into_iter().map(str::to_string).collect(),
        params: user
            .filter(|spec| spec.active())
            .map(|spec| spec.params.clone())
            .unwrap_or_default(),
    }
}

fn plan_for(contract: &Contract, choices: &ChatRequestOptions, max_output: u64) -> WirePlan {
    let max_output = max_output_ceiling(choices, max_output);

    let mut plan = WirePlan {
        enable_reasoning: false,
        openai_thinking: None,
        reasoning_split: contract.reasoning_split,
        reasoning_effort: None,
        anthropic_thinking_type: None,
        anthropic_budget: None,
        anthropic_effort: None,
        anthropic_interleaved: false,
        gemini_level: None,
        gemini_budget: None,
        include_thoughts: false,
        service_tier: allowed(choices.service_tier.as_deref(), &contract.tiers),
        temperature: temperature_for(contract.sampling, choices.temperature),
        gemini_top_p: contract.gemini_top_p,
        token_field: contract.token_field,
        privacy: if choices.limit_provider_data_use {
            contract.privacy_wire
        } else {
            None
        },
        max_output,
        extra: choices
            .extra
            .iter()
            .filter_map(|item| {
                let name = item.name.trim();
                if name.is_empty() {
                    None
                } else {
                    Some((name.to_string(), item.value.trim().to_string()))
                }
            })
            .collect(),
    };
    apply_reasoning(contract, choices, &mut plan);
    plan
}

fn max_output_ceiling(choices: &ChatRequestOptions, caller_max: u64) -> u64 {
    let caller = caller_max.clamp(1, OUTPUT_TOKEN_MAX);
    match choices.max_output_tokens.filter(|value| *value > 0) {
        Some(requested) => requested.min(caller).max(1),
        None => caller,
    }
}

fn apply_reasoning(contract: &Contract, choices: &ChatRequestOptions, plan: &mut WirePlan) {
    match &contract.reasoning {
        ModelReasoningSpec::Unsupported | ModelReasoningSpec::Unknown => {}
        ModelReasoningSpec::Thinking {
            modes,
            default_mode,
            locked_on,
            ..
        } => {
            let forced = if default_mode.is_empty() {
                modes
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "adaptive".to_string())
            } else {
                default_mode.clone()
            };
            let mode = if *locked_on {
                forced
            } else {
                allowed(choices.reasoning_mode.as_deref(), modes).unwrap_or(forced)
            };
            plan.enable_reasoning = mode != "disabled";
            plan.openai_thinking = Some(mode.clone());
            plan.anthropic_thinking_type = Some(mode);
        }
        ModelReasoningSpec::Effort {
            levels,
            default_level,
        } => {
            let effort = allowed(choices.effort.as_deref(), levels).or_else(|| {
                if default_level.is_empty() {
                    None
                } else {
                    Some(default_level.clone())
                }
            });
            if let Some(effort) = effort {
                plan.enable_reasoning = !matches!(effort.as_str(), "none" | "off");
                if contract.vendor == "anthropic" {
                    plan.anthropic_effort = Some(effort);
                } else {
                    plan.reasoning_effort = Some(effort);
                }
            }
        }
        ModelReasoningSpec::GeminiLevel {
            levels,
            default_level,
        } => {
            let level =
                allowed(choices.effort.as_deref(), levels).unwrap_or_else(|| default_level.clone());
            plan.gemini_level = Some(level.to_ascii_uppercase());
            plan.include_thoughts = true;
            plan.enable_reasoning = true;
        }
        ModelReasoningSpec::GeminiBudget {
            modes,
            default_mode,
            locked_on,
        } => {
            let mode = if *locked_on {
                "dynamic".to_string()
            } else {
                allowed(choices.reasoning_mode.as_deref(), modes)
                    .unwrap_or_else(|| default_mode.clone())
            };
            let mode = mode.as_str();
            plan.gemini_budget = Some(if mode == "disabled" { 0 } else { -1 });
            plan.include_thoughts = mode != "disabled";
            plan.enable_reasoning = mode != "disabled";
        }
        ModelReasoningSpec::AnthropicExtended {
            default_on,
            interleaved,
        } => {
            apply_extended_thinking(plan, choices, *default_on, *interleaved);
        }
        ModelReasoningSpec::Claude {
            thinking_modes,
            default_thinking,
            locked_on,
            levels,
            default_level,
        } => {
            let effort = allowed(choices.effort.as_deref(), levels).or_else(|| {
                if default_level.is_empty() {
                    None
                } else {
                    Some(default_level.clone())
                }
            });
            let forced = if default_thinking.is_empty() {
                thinking_modes
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "adaptive".to_string())
            } else {
                default_thinking.clone()
            };
            let mut mode = if *locked_on {
                forced
            } else {
                allowed(choices.reasoning_mode.as_deref(), thinking_modes).unwrap_or(forced)
            };
            if contract.opus5_disable_limit
                && mode == "disabled"
                && matches!(effort.as_deref(), Some("xhigh" | "max"))
            {
                mode = "adaptive".into();
            }
            plan.anthropic_thinking_type = Some(mode.clone());
            plan.anthropic_effort = effort;
            plan.enable_reasoning = mode != "disabled";
        }
    }
}

fn apply_extended_thinking(
    plan: &mut WirePlan,
    choices: &ChatRequestOptions,
    default_on: bool,
    interleaved: bool,
) {
    let enabled = match choices.reasoning_mode.as_deref().map(str::trim) {
        Some("disabled") => false,
        Some("enabled") => true,
        _ => default_on,
    };
    if !enabled {
        plan.anthropic_thinking_type = Some("disabled".into());
        return;
    }
    let budget = (plan.max_output / 2).clamp(1024, 16_384);
    let max_tokens = plan.max_output.max(budget + 1024).min(OUTPUT_TOKEN_MAX);
    plan.max_output = max_tokens;
    let budget = budget.min(plan.max_output.saturating_sub(1));
    plan.anthropic_thinking_type = Some("enabled".into());
    plan.anthropic_budget = Some(budget);
    plan.anthropic_interleaved = interleaved && budget >= 1024;
    plan.enable_reasoning = true;
}

fn view_reasoning(reasoning: &ModelReasoningSpec) -> ReasoningView {
    match reasoning {
        ModelReasoningSpec::Unsupported => ReasoningView::Unsupported,
        ModelReasoningSpec::Unknown => ReasoningView::Unknown,
        ModelReasoningSpec::Thinking {
            modes,
            default_mode,
            locked_on,
            ..
        } => ReasoningView::Thinking {
            modes: modes.clone(),
            default_mode: default_mode.clone(),
            locked_on: *locked_on,
        },
        ModelReasoningSpec::Effort {
            levels,
            default_level,
        } => ReasoningView::Effort {
            levels: levels.clone(),
            default_level: default_level.clone(),
        },
        ModelReasoningSpec::GeminiLevel {
            levels,
            default_level,
        } => ReasoningView::GeminiLevel {
            levels: levels.clone(),
            default_level: default_level.clone(),
        },
        ModelReasoningSpec::GeminiBudget {
            modes,
            default_mode,
            locked_on,
        } => ReasoningView::GeminiBudget {
            modes: modes.clone(),
            default_mode: default_mode.clone(),
            locked_on: *locked_on,
        },
        ModelReasoningSpec::AnthropicExtended { default_on, .. } => ReasoningView::Thinking {
            modes: vec!["disabled".into(), "enabled".into()],
            default_mode: if *default_on { "enabled" } else { "disabled" }.into(),
            locked_on: false,
        },
        ModelReasoningSpec::Claude {
            thinking_modes,
            default_thinking,
            locked_on,
            levels,
            default_level,
        } => ReasoningView::Claude {
            thinking_modes: thinking_modes.clone(),
            default_thinking: default_thinking.clone(),
            locked_on: *locked_on,
            levels: levels.clone(),
            default_level: default_level.clone(),
        },
    }
}

fn view_sampling(sampling: Sampling) -> SamplingView {
    match sampling {
        Sampling::Omit => SamplingView {
            temperature: "hidden".into(),
            min: None,
            max: None,
            default_value: None,
        },
        Sampling::Fixed(value) => SamplingView {
            temperature: "fixed".into(),
            min: None,
            max: None,
            default_value: Some(value),
        },
        Sampling::Range {
            min,
            max,
            default_value,
        } => SamplingView {
            temperature: "range".into(),
            min: Some(min),
            max: Some(max),
            default_value: Some(default_value),
        },
    }
}

fn temperature_for(sampling: Sampling, requested: Option<f64>) -> Option<f64> {
    match sampling {
        Sampling::Omit => None,
        Sampling::Fixed(value) => Some(value),
        Sampling::Range {
            min,
            max,
            default_value,
        } => Some(match requested {
            Some(value) if value.is_finite() => value.clamp(min, max),
            _ => default_value,
        }),
    }
}

fn allowed(choice: Option<&str>, values: &[String]) -> Option<String> {
    let choice = choice.map(str::trim).filter(|value| !value.is_empty())?;
    values
        .iter()
        .find(|value| value.as_str() == choice)
        .cloned()
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn param_named<'a>(spec: &'a UserRequestSpec, name: &str) -> Option<&'a UserRequestParam> {
    spec.params
        .iter()
        .find(|param| param.name.eq_ignore_ascii_case(name))
}

fn spec_from_user(spec: &UserRequestSpec) -> ModelRequestSpec {
    let thinking = param_named(spec, "thinking")
        .filter(|param| param.kind == UserParamKind::String)
        .map(|param| param.values.clone())
        .unwrap_or_default();
    let effort = param_named(spec, "effort")
        .filter(|param| param.kind == UserParamKind::String)
        .map(|param| param.values.clone())
        .unwrap_or_default();
    let tiers = param_named(spec, "serviceTier")
        .filter(|param| param.kind == UserParamKind::String)
        .map(|param| param.values.clone())
        .unwrap_or_default();
    let reasoning = match (thinking.is_empty(), effort.is_empty()) {
        (false, false) => Some(ModelReasoningSpec::Claude {
            thinking_modes: thinking.clone(),
            default_thinking: thinking[0].clone(),
            locked_on: false,
            levels: effort,
            default_level: String::new(),
        }),
        (false, true) => Some(ModelReasoningSpec::Thinking {
            modes: thinking.clone(),
            default_mode: thinking[0].clone(),
            locked_on: false,
            anthropic_default_mode: None,
        }),
        (true, false) => Some(ModelReasoningSpec::Effort {
            levels: effort,
            default_level: String::new(),
        }),
        (true, true) => None,
    };
    let sampling = param_named(spec, "temperature")
        .filter(|param| param.kind == UserParamKind::Number)
        .and_then(|param| param.values.first())
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite())
        .map(|value| ModelSamplingSpec::Range {
            min: 0.0,
            max: 2.0,
            default_value: value,
        });
    ModelRequestSpec {
        known: true,
        native: vec![
            "minimax".into(),
            "openai".into(),
            "anthropic".into(),
            "gemini".into(),
        ],
        reasoning,
        sampling,
        token_field: None,
        tiers,
        tier_may_reject: false,
        reasoning_split_openai: param_named(spec, "reasoningSplit")
            .is_some_and(|param| param.kind == UserParamKind::Bool),
        opus5_disable_limit: false,
        gemini_top_p: false,
    }
}

pub fn params_from_spec(spec: &ModelRequestSpec) -> Vec<UserRequestParam> {
    let mut params = Vec::new();
    let string_param = |name: &str, values: Vec<String>| UserRequestParam {
        name: name.into(),
        kind: UserParamKind::String,
        values,
    };
    match &spec.reasoning {
        Some(ModelReasoningSpec::Thinking { modes, .. })
        | Some(ModelReasoningSpec::GeminiBudget { modes, .. }) => {
            if !modes.is_empty() {
                params.push(string_param("thinking", modes.clone()));
            }
        }
        Some(ModelReasoningSpec::Effort { levels, .. })
        | Some(ModelReasoningSpec::GeminiLevel { levels, .. }) => {
            if !levels.is_empty() {
                params.push(string_param("effort", levels.clone()));
            }
        }
        Some(ModelReasoningSpec::Claude {
            thinking_modes,
            levels,
            ..
        }) => {
            if !thinking_modes.is_empty() {
                params.push(string_param("thinking", thinking_modes.clone()));
            }
            if !levels.is_empty() {
                params.push(string_param("effort", levels.clone()));
            }
        }
        Some(ModelReasoningSpec::AnthropicExtended { .. }) => {
            params.push(string_param(
                "thinking",
                vec!["disabled".into(), "enabled".into()],
            ));
        }
        _ => {}
    }
    if let Some(sampling) = &spec.sampling {
        let value = match sampling {
            ModelSamplingSpec::Fixed { value } => *value,
            ModelSamplingSpec::Range { default_value, .. } => *default_value,
        };
        params.push(UserRequestParam {
            name: "temperature".into(),
            kind: UserParamKind::Number,
            values: vec![value.to_string()],
        });
    }
    if !spec.tiers.is_empty() {
        params.push(string_param("serviceTier", spec.tiers.clone()));
    }
    if spec.reasoning_split_openai {
        params.push(UserRequestParam {
            name: "reasoningSplit".into(),
            kind: UserParamKind::Bool,
            values: Vec::new(),
        });
    }
    params
}

#[tauri::command]
pub fn catalog_request_params(model_id: String) -> Vec<UserRequestParam> {
    crate::catalog::bundled_lookup(&model_id)
        .and_then(|entry| entry.request.as_ref())
        .map(params_from_spec)
        .unwrap_or_default()
}

fn contract(query: &Query<'_>, user: Option<&UserRequestSpec>) -> Contract {
    let mut contract = match vendor(query.base_url) {
        Vendor::MiniMax => minimax_shell(),
        Vendor::OpenAi => openai_shell(),
        Vendor::Anthropic => anthropic_shell(),
        Vendor::Gemini => gemini_shell(),
        Vendor::OpenRouter => provider_shell(
            "openrouter",
            "request-flag",
            "chat.request.privacy.openrouter",
            Some(PrivacyWire::OpenRouterDataCollectionDeny),
            &[],
        ),
        Vendor::Groq => provider_shell(
            "groq",
            "account-only",
            "chat.request.privacy.accountOnly",
            None,
            &["auto", "on_demand", "flex", "performance"],
        ),
        Vendor::Xai => provider_shell(
            "xai",
            "account-only",
            "chat.request.privacy.accountOnly",
            None,
            &["default", "priority"],
        ),
        Vendor::Fireworks => provider_shell(
            "fireworks",
            "account-only",
            "chat.request.privacy.accountOnly",
            None,
            &["priority"],
        ),
        Vendor::Together | Vendor::Mistral | Vendor::Cohere | Vendor::DashScope => provider_shell(
            vendor_name(vendor(query.base_url)),
            "account-only",
            "chat.request.privacy.accountOnly",
            None,
            &[],
        ),
        Vendor::DeepSeek | Vendor::Cerebras | Vendor::Zhipu | Vendor::Moonshot => provider_shell(
            vendor_name(vendor(query.base_url)),
            "unsupported",
            "chat.request.privacy.unsupported",
            None,
            &[],
        ),
        Vendor::Unknown => provider_shell(
            "unknown",
            "unknown",
            "chat.request.privacy.unknown",
            None,
            &[],
        ),
    };
    if let Some(spec) = user.filter(|item| item.active()) {
        apply_spec(&mut contract, &spec_from_user(spec), query.kind);
    } else if let Some(spec) = active_spec(query) {
        apply_spec(&mut contract, &spec, query.kind);
    }
    contract
}

fn provider_shell(
    vendor: &'static str,
    privacy_support: &'static str,
    privacy_detail_key: &'static str,
    privacy_wire: Option<PrivacyWire>,
    tiers: &'static [&'static str],
) -> Contract {
    let mut notes = vec!["chat.request.note.modelUnknown"];
    if !tiers.is_empty() {
        notes.push("chat.request.note.tierMayReject");
    }
    Contract {
        vendor,
        model_known: false,
        reasoning: ModelReasoningSpec::Unknown,
        tiers: strings(tiers),
        sampling: Sampling::Omit,
        privacy_support,
        privacy_detail_key,
        privacy_wire,
        reasoning_split: false,
        token_field: TokenField::MaxTokens,
        gemini_top_p: false,
        notes,
        opus5_disable_limit: false,
    }
}

fn minimax_shell() -> Contract {
    let mut contract = provider_shell(
        "minimax",
        "unsupported",
        "chat.request.privacy.unsupported",
        None,
        &[],
    );
    contract.token_field = TokenField::MaxCompletionTokens;
    contract
}

fn openai_shell() -> Contract {
    provider_shell(
        "openai",
        "request-flag",
        "chat.request.privacy.openai",
        Some(PrivacyWire::OpenAiStoreFalse),
        &["auto", "default", "flex", "priority"],
    )
}

fn anthropic_shell() -> Contract {
    provider_shell(
        "anthropic",
        "account-only",
        "chat.request.privacy.accountOnly",
        None,
        &["auto", "standard_only"],
    )
}

fn gemini_shell() -> Contract {
    provider_shell(
        "gemini",
        "account-only",
        "chat.request.privacy.accountOnly",
        None,
        &[],
    )
}

fn kind_native(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::OpenAiLike => "openai",
        ProviderKind::AnthropicLike => "anthropic",
        ProviderKind::GeminiLike => "gemini",
    }
}

fn host_native(base_url: &str) -> Option<&'static str> {
    match vendor(base_url) {
        Vendor::MiniMax => Some("minimax"),
        Vendor::OpenAi => Some("openai"),
        Vendor::Anthropic => Some("anthropic"),
        Vendor::Gemini => Some("gemini"),
        _ => None,
    }
}

fn active_spec(query: &Query<'_>) -> Option<ModelRequestSpec> {
    let spec = crate::catalog::bundled_lookup(query.model_id)?
        .request
        .clone()?;
    if !spec.known {
        return None;
    }
    let kind_name = kind_native(query.kind);
    let host_name = host_native(query.base_url);
    let matches = spec.native.iter().any(|name| {
        name == kind_name || host_name.is_some_and(|host| name == host)
    });
    if matches { Some(spec) } else { None }
}

fn apply_spec(contract: &mut Contract, spec: &ModelRequestSpec, kind: ProviderKind) {
    contract.model_known = spec.known;
    contract
        .notes
        .retain(|note| *note != "chat.request.note.modelUnknown");
    if !spec.tiers.is_empty() {
        contract.tiers = spec.tiers.clone();
    }
    if spec.tier_may_reject
        && !contract
            .notes
            .iter()
            .any(|note| *note == "chat.request.note.tierMayReject")
    {
        contract.notes.push("chat.request.note.tierMayReject");
    }
    contract.reasoning_split = spec.reasoning_split_openai && kind == ProviderKind::OpenAiLike;
    if contract.reasoning_split {
        contract.notes.push("chat.request.note.reasoningSplit");
    }
    if let Some(reasoning) = &spec.reasoning {
        push_reasoning_notes(contract, reasoning);
        contract.reasoning = reasoning_from_spec(reasoning, kind);
    }
    if let Some(sampling) = &spec.sampling {
        contract.sampling = match sampling {
            ModelSamplingSpec::Fixed { value } => Sampling::Fixed(*value),
            ModelSamplingSpec::Range {
                min,
                max,
                default_value,
            } => Sampling::Range {
                min: *min,
                max: *max,
                default_value: *default_value,
            },
        };
    }
    if spec.token_field.as_deref() == Some("maxCompletionTokens") {
        contract.token_field = TokenField::MaxCompletionTokens;
    } else if spec.token_field.as_deref() == Some("maxTokens") {
        contract.token_field = TokenField::MaxTokens;
    }
    contract.opus5_disable_limit = spec.opus5_disable_limit;
    contract.gemini_top_p = spec.gemini_top_p;
}

fn push_reasoning_notes(contract: &mut Contract, reasoning: &ModelReasoningSpec) {
    let locked = match reasoning {
        ModelReasoningSpec::Thinking { locked_on, .. }
        | ModelReasoningSpec::GeminiBudget { locked_on, .. }
        | ModelReasoningSpec::Claude { locked_on, .. } => *locked_on,
        _ => false,
    };
    if locked {
        contract.notes.push("chat.request.note.thinkingLocked");
    }
    if let ModelReasoningSpec::Claude {
        default_thinking,
        locked_on,
        ..
    } = reasoning
    {
        if !locked_on && default_thinking == "disabled" {
            contract.notes.push("chat.request.note.thinkingDefaultOff");
        }
    }
}

fn reasoning_from_spec(spec: &ModelReasoningSpec, kind: ProviderKind) -> ModelReasoningSpec {
    let ModelReasoningSpec::Thinking {
        modes,
        default_mode,
        locked_on,
        anthropic_default_mode,
    } = spec
    else {
        return spec.clone();
    };
    let default_mode = if kind == ProviderKind::AnthropicLike {
        anthropic_default_mode
            .clone()
            .unwrap_or_else(|| default_mode.clone())
    } else {
        default_mode.clone()
    };
    ModelReasoningSpec::Thinking {
        modes: modes.clone(),
        default_mode,
        locked_on: *locked_on,
        anthropic_default_mode: None,
    }
}

#[derive(Clone, Copy)]
enum Vendor {
    OpenAi,
    Anthropic,
    Gemini,
    MiniMax,
    OpenRouter,
    Together,
    Fireworks,
    Groq,
    Cerebras,
    DeepSeek,
    Xai,
    Mistral,
    Cohere,
    DashScope,
    Zhipu,
    Moonshot,
    Unknown,
}

fn vendor(base_url: &str) -> Vendor {
    let host = base_url.to_ascii_lowercase();
    if host.contains("openrouter.ai") {
        return Vendor::OpenRouter;
    }
    if host.contains("api.openai.com") {
        return Vendor::OpenAi;
    }
    if host.contains("anthropic.com") {
        return Vendor::Anthropic;
    }
    if host.contains("generativelanguage.googleapis.com")
        || host.contains("aiplatform.googleapis.com")
    {
        return Vendor::Gemini;
    }
    if host.contains("minimax.io") || host.contains("minimaxi.com") {
        return Vendor::MiniMax;
    }
    if host.contains("api.together.ai") || host.contains("api.together.xyz") {
        return Vendor::Together;
    }
    if host.contains("fireworks.ai") {
        return Vendor::Fireworks;
    }
    if host.contains("api.groq.com") {
        return Vendor::Groq;
    }
    if host.contains("api.cerebras.ai") {
        return Vendor::Cerebras;
    }
    if host.contains("api.deepseek.com") {
        return Vendor::DeepSeek;
    }
    if host.contains("api.x.ai") {
        return Vendor::Xai;
    }
    if host.contains("api.mistral.ai") {
        return Vendor::Mistral;
    }
    if host.contains("api.cohere.ai") || host.contains("api.cohere.com") {
        return Vendor::Cohere;
    }
    if host.contains("dashscope") || host.contains("aliyuncs.com") {
        return Vendor::DashScope;
    }
    if host.contains("bigmodel.cn") {
        return Vendor::Zhipu;
    }
    if host.contains("moonshot.ai") || host.contains("moonshot.cn") {
        return Vendor::Moonshot;
    }
    Vendor::Unknown
}

fn vendor_name(vendor: Vendor) -> &'static str {
    match vendor {
        Vendor::Together => "together",
        Vendor::Mistral => "mistral",
        Vendor::Cohere => "cohere",
        Vendor::DashScope => "dashscope",
        Vendor::DeepSeek => "deepseek",
        Vendor::Cerebras => "cerebras",
        Vendor::Zhipu => "zhipu",
        Vendor::Moonshot => "moonshot",
        Vendor::Unknown => "unknown",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(kind: ProviderKind, base: &str, model: &str) -> DescribeModelRequest {
        DescribeModelRequest {
            kind,
            base_url: base.into(),
            model_id: model.into(),
            user_request: None,
        }
    }

    #[test]
    fn minimax_m3_openai_uses_thinking_and_split() {
        let query = query(
            ProviderKind::OpenAiLike,
            "https://api.minimax.io/v1",
            "MiniMax-M3",
        );
        let view = describe_model_request(query.clone());
        assert!(view.reasoning_split);
        assert!(matches!(
            view.reasoning,
            ReasoningView::Thinking {
                locked_on: false,
                ..
            }
        ));
        let plan = prepare(&query, &ChatRequestOptions::default(), 4096);
        assert_eq!(plan.openai_thinking.as_deref(), Some("adaptive"));
        assert!(plan.reasoning_split);
        assert!(plan.reasoning_effort.is_none());
    }

    #[test]
    fn minimax_m3_anthropic_defaults_thinking_off_without_split() {
        let query = query(
            ProviderKind::AnthropicLike,
            "https://api.minimax.io/anthropic",
            "MiniMax-M3",
        );
        let plan = prepare(&query, &ChatRequestOptions::default(), 4096);
        assert_eq!(plan.anthropic_thinking_type.as_deref(), Some("disabled"));
        assert!(!plan.reasoning_split);
    }

    #[test]
    fn routed_minimax_uses_catalog_contract() {
        let query = query(
            ProviderKind::OpenAiLike,
            "https://openrouter.ai/api/v1",
            "MiniMax-M3",
        );
        let plan = prepare(&query, &ChatRequestOptions::default(), 4096);
        assert_eq!(plan.openai_thinking.as_deref(), Some("adaptive"));
        assert!(plan.reasoning_split);
        assert!(plan.reasoning_effort.is_none());
        let opted = ChatRequestOptions {
            limit_provider_data_use: true,
            ..ChatRequestOptions::default()
        };
        let plan = prepare(&query, &opted, 4096);
        assert_eq!(
            plan.privacy,
            Some(PrivacyWire::OpenRouterDataCollectionDeny)
        );
    }

    #[test]
    fn unknown_host_uses_openai_catalog_contract() {
        let query = query(
            ProviderKind::OpenAiLike,
            "https://proxy.example.com/v1",
            "gpt-5.4",
        );
        let choices = ChatRequestOptions {
            effort: Some("high".into()),
            ..ChatRequestOptions::default()
        };
        let plan = prepare(&query, &choices, 4096);
        assert_eq!(plan.reasoning_effort.as_deref(), Some("high"));
    }

    #[test]
    fn openai_effort_comes_from_catalog() {
        let query = query(
            ProviderKind::OpenAiLike,
            "https://api.openai.com/v1",
            "gpt-5.4-pro",
        );
        let view = describe_model_request(query.clone());
        assert!(matches!(view.reasoning, ReasoningView::Effort { .. }));
        let choices = ChatRequestOptions {
            effort: Some("high".into()),
            ..ChatRequestOptions::default()
        };
        let plan = prepare(&query, &choices, 4096);
        assert_eq!(plan.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(plan.token_field, TokenField::MaxCompletionTokens);
    }
}
