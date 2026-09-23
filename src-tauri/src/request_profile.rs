use serde::{Deserialize, Serialize};

use crate::providers::ProviderKind;

const OUTPUT_TOKEN_MAX: u64 = 32_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DescribeModelRequest {
    pub kind: ProviderKind,
    pub base_url: String,
    pub model_id: String,
    #[serde(default)]
    pub family: Option<String>,
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
}

#[derive(Clone, Copy)]
struct Query<'a> {
    kind: ProviderKind,
    base_url: &'a str,
    model_id: &'a str,
    family: Option<&'a str>,
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

enum ReasoningContract {
    Unsupported,
    Unknown,
    Thinking {
        modes: &'static [&'static str],
        default_mode: &'static str,
        locked_on: bool,
    },
    Effort {
        levels: &'static [&'static str],
        default_level: &'static str,
    },
    GeminiLevel {
        levels: &'static [&'static str],
        default_level: &'static str,
    },
    GeminiBudget {
        modes: &'static [&'static str],
        default_mode: &'static str,
        locked_on: bool,
    },
    AnthropicExtended {
        default_on: bool,
        interleaved: bool,
    },
    Claude {
        thinking_modes: &'static [&'static str],
        default_thinking: &'static str,
        locked_on: bool,
        levels: &'static [&'static str],
        default_level: &'static str,
    },
}

struct Contract {
    vendor: &'static str,
    model_known: bool,
    reasoning: ReasoningContract,
    tiers: &'static [&'static str],
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
    describe(&Query {
        kind: query.kind,
        base_url: &query.base_url,
        model_id: &query.model_id,
        family: query.family.as_deref(),
    })
}

pub fn prepare(
    query: &DescribeModelRequest,
    choices: &ChatRequestOptions,
    max_output: u64,
) -> WirePlan {
    let contract = contract(&Query {
        kind: query.kind,
        base_url: &query.base_url,
        model_id: &query.model_id,
        family: query.family.as_deref(),
    });
    plan_for(&contract, choices, max_output)
}

pub fn quiet_query(
    kind: ProviderKind,
    base_url: &str,
    model_id: &str,
    family: Option<&str>,
) -> DescribeModelRequest {
    DescribeModelRequest {
        kind,
        base_url: base_url.to_string(),
        model_id: model_id.to_string(),
        family: family.map(str::to_string),
    }
}

fn describe(query: &Query<'_>) -> ModelRequestView {
    let contract = contract(query);
    ModelRequestView {
        vendor: contract.vendor.to_string(),
        model_contract: if contract.model_known {
            "verified".into()
        } else {
            "unknown".into()
        },
        reasoning: view_reasoning(&contract.reasoning),
        service_tiers: contract
            .tiers
            .iter()
            .map(|item| (*item).to_string())
            .collect(),
        sampling: view_sampling(contract.sampling),
        privacy_support: contract.privacy_support.to_string(),
        privacy_detail_key: contract.privacy_detail_key.to_string(),
        reasoning_split: contract.reasoning_split,
        notes: contract.notes.into_iter().map(str::to_string).collect(),
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
        service_tier: allowed(choices.service_tier.as_deref(), contract.tiers),
        temperature: temperature_for(contract.sampling, choices.temperature),
        gemini_top_p: contract.gemini_top_p,
        token_field: contract.token_field,
        privacy: if choices.limit_provider_data_use {
            contract.privacy_wire
        } else {
            None
        },
        max_output,
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
        ReasoningContract::Unsupported | ReasoningContract::Unknown => {}
        ReasoningContract::Thinking {
            modes,
            default_mode,
            locked_on,
        } => {
            let mode = if *locked_on {
                "adaptive".to_string()
            } else {
                allowed(choices.reasoning_mode.as_deref(), modes)
                    .unwrap_or_else(|| (*default_mode).to_string())
            };
            plan.enable_reasoning = mode != "disabled";
            plan.openai_thinking = Some(mode.clone());
            plan.anthropic_thinking_type = Some(mode);
        }
        ReasoningContract::Effort {
            levels,
            default_level,
        } => {
            let effort = allowed(choices.effort.as_deref(), levels).or_else(|| {
                if default_level.is_empty() {
                    None
                } else {
                    Some((*default_level).to_string())
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
        ReasoningContract::GeminiLevel {
            levels,
            default_level,
        } => {
            let level = allowed(choices.effort.as_deref(), levels)
                .unwrap_or_else(|| (*default_level).to_string());
            plan.gemini_level = Some(level.to_ascii_uppercase());
            plan.include_thoughts = true;
            plan.enable_reasoning = true;
        }
        ReasoningContract::GeminiBudget {
            modes,
            default_mode,
            locked_on,
        } => {
            let mode = if *locked_on {
                "dynamic".to_string()
            } else {
                allowed(choices.reasoning_mode.as_deref(), modes)
                    .unwrap_or_else(|| (*default_mode).to_string())
            };
            let mode = mode.as_str();
            plan.gemini_budget = Some(if mode == "disabled" { 0 } else { -1 });
            plan.include_thoughts = mode != "disabled";
            plan.enable_reasoning = mode != "disabled";
        }
        ReasoningContract::AnthropicExtended {
            default_on,
            interleaved,
        } => {
            apply_extended_thinking(plan, choices, *default_on, *interleaved);
        }
        ReasoningContract::Claude {
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
                    Some((*default_level).to_string())
                }
            });
            let mut mode = if *locked_on {
                "adaptive".to_string()
            } else {
                allowed(choices.reasoning_mode.as_deref(), thinking_modes)
                    .unwrap_or_else(|| (*default_thinking).to_string())
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

fn view_reasoning(reasoning: &ReasoningContract) -> ReasoningView {
    match reasoning {
        ReasoningContract::Unsupported => ReasoningView::Unsupported,
        ReasoningContract::Unknown => ReasoningView::Unknown,
        ReasoningContract::Thinking {
            modes,
            default_mode,
            locked_on,
        } => ReasoningView::Thinking {
            modes: owned(modes),
            default_mode: (*default_mode).to_string(),
            locked_on: *locked_on,
        },
        ReasoningContract::Effort {
            levels,
            default_level,
        } => ReasoningView::Effort {
            levels: owned(levels),
            default_level: (*default_level).to_string(),
        },
        ReasoningContract::GeminiLevel {
            levels,
            default_level,
        } => ReasoningView::GeminiLevel {
            levels: owned(levels),
            default_level: (*default_level).to_string(),
        },
        ReasoningContract::GeminiBudget {
            modes,
            default_mode,
            locked_on,
        } => ReasoningView::GeminiBudget {
            modes: owned(modes),
            default_mode: (*default_mode).to_string(),
            locked_on: *locked_on,
        },
        ReasoningContract::AnthropicExtended { default_on, .. } => ReasoningView::Thinking {
            modes: vec!["disabled".into(), "enabled".into()],
            default_mode: if *default_on { "enabled" } else { "disabled" }.into(),
            locked_on: false,
        },
        ReasoningContract::Claude {
            thinking_modes,
            default_thinking,
            locked_on,
            levels,
            default_level,
        } => ReasoningView::Claude {
            thinking_modes: owned(thinking_modes),
            default_thinking: (*default_thinking).to_string(),
            locked_on: *locked_on,
            levels: owned(levels),
            default_level: (*default_level).to_string(),
        },
    }
}

fn view_sampling(sampling: Sampling) -> SamplingView {
    match sampling {
        Sampling::Omit | Sampling::Fixed(_) => SamplingView {
            temperature: "hidden".into(),
            min: None,
            max: None,
            default_value: None,
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

fn allowed(choice: Option<&str>, values: &[&str]) -> Option<String> {
    let choice = choice.map(str::trim).filter(|value| !value.is_empty())?;
    values
        .iter()
        .find(|value| **value == choice)
        .map(|value| (*value).to_string())
}

fn owned(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn contract(query: &Query<'_>) -> Contract {
    match vendor(query.base_url) {
        Vendor::MiniMax => minimax_contract(query),
        Vendor::OpenAi => openai_contract(query),
        Vendor::Anthropic => anthropic_contract(query),
        Vendor::Gemini => gemini_contract(query),
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
    }
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
        reasoning: ReasoningContract::Unknown,
        tiers,
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

fn minimax_contract(query: &Query<'_>) -> Contract {
    let blob = blob(query);
    let official = is_minimax_m3(&blob) || is_minimax_m2(&blob);
    let locked_on = official && !is_minimax_m3(&blob);
    let default_mode = if query.kind == ProviderKind::AnthropicLike && is_minimax_m3(&blob) {
        "disabled"
    } else {
        "adaptive"
    };
    let modes: &'static [&'static str] = if locked_on {
        &["adaptive"]
    } else if official {
        &["adaptive", "disabled"]
    } else {
        &[]
    };
    let mut notes = Vec::new();
    if query.kind == ProviderKind::OpenAiLike && official {
        notes.push("chat.request.note.reasoningSplit");
    }
    if locked_on {
        notes.push("chat.request.note.thinkingLocked");
    }
    if !official {
        notes.push("chat.request.note.modelUnknown");
    }
    Contract {
        vendor: "minimax",
        model_known: official,
        reasoning: if official {
            ReasoningContract::Thinking {
                modes,
                default_mode,
                locked_on,
            }
        } else {
            ReasoningContract::Unknown
        },
        tiers: if official {
            &["standard", "priority"]
        } else {
            &[]
        },
        sampling: if official {
            Sampling::Range {
                min: 0.0,
                max: 2.0,
                default_value: 1.0,
            }
        } else {
            Sampling::Omit
        },
        privacy_support: "unsupported",
        privacy_detail_key: "chat.request.privacy.unsupported",
        privacy_wire: None,
        reasoning_split: query.kind == ProviderKind::OpenAiLike && official,
        token_field: TokenField::MaxCompletionTokens,
        gemini_top_p: false,
        notes,
        opus5_disable_limit: false,
    }
}

fn openai_contract(query: &Query<'_>) -> Contract {
    let blob = blob(query);
    let mut contract = provider_shell(
        "openai",
        "request-flag",
        "chat.request.privacy.openai",
        Some(PrivacyWire::OpenAiStoreFalse),
        &["auto", "default", "flex", "priority"],
    );
    contract.vendor = "openai";
    if query.kind != ProviderKind::OpenAiLike {
        return contract;
    }
    if has(&blob, "gpt-6") {
        return effort_contract(
            contract,
            &["low", "medium", "high", "xhigh", "max"],
            "",
            true,
        );
    }
    if has(&blob, "gpt-5-6") {
        return effort_contract(
            contract,
            &["none", "low", "medium", "high", "xhigh", "max"],
            "medium",
            true,
        );
    }
    if has(&blob, "gpt-5-5") {
        return effort_contract(
            contract,
            &["none", "low", "medium", "high", "xhigh"],
            "medium",
            true,
        );
    }
    if has(&blob, "gpt-5-4") {
        return effort_contract(
            contract,
            &["none", "low", "medium", "high", "xhigh"],
            "none",
            true,
        );
    }
    if has(&blob, "gpt-5") {
        return effort_contract(contract, &["minimal", "low", "medium", "high"], "", true);
    }
    if has_token(&blob, "o3") || has_token(&blob, "o4") || has_token(&blob, "o1") {
        contract.sampling = Sampling::Omit;
        contract.token_field = TokenField::MaxCompletionTokens;
        contract
            .notes
            .retain(|note| *note != "chat.request.note.modelUnknown");
        contract.model_known = false;
        contract.reasoning = ReasoningContract::Unknown;
        return contract;
    }
    if has(&blob, "gpt-4-1") || has(&blob, "gpt-4o") || has(&blob, "gpt-4-o") {
        contract.model_known = true;
        contract.reasoning = ReasoningContract::Unsupported;
        contract.sampling = Sampling::Range {
            min: 0.0,
            max: 2.0,
            default_value: 1.0,
        };
        contract.token_field = TokenField::MaxTokens;
        contract
            .notes
            .retain(|note| *note != "chat.request.note.modelUnknown");
    }
    contract
}

fn anthropic_contract(query: &Query<'_>) -> Contract {
    let mut contract = provider_shell(
        "anthropic",
        "account-only",
        "chat.request.privacy.accountOnly",
        None,
        &["auto", "standard_only"],
    );
    if query.kind != ProviderKind::AnthropicLike {
        return contract;
    }
    let blob = blob(query);
    let Some(kind) = claude_kind(&blob) else {
        return contract;
    };
    contract.model_known = true;
    contract
        .notes
        .retain(|note| *note != "chat.request.note.modelUnknown");
    contract.sampling = Sampling::Fixed(1.0);
    match kind {
        ClaudeKind::AlwaysAdaptive {
            levels,
            default_level,
        } => {
            contract.reasoning = ReasoningContract::Claude {
                thinking_modes: &["adaptive"],
                default_thinking: "adaptive",
                locked_on: true,
                levels,
                default_level,
            };
            contract.notes.push("chat.request.note.thinkingLocked");
        }
        ClaudeKind::Adaptive {
            levels,
            default_level,
            default_on,
            opus5_limit,
        } => {
            contract.reasoning = ReasoningContract::Claude {
                thinking_modes: &["adaptive", "disabled"],
                default_thinking: if default_on { "adaptive" } else { "disabled" },
                locked_on: false,
                levels,
                default_level,
            };
            contract.opus5_disable_limit = opus5_limit;
            if !default_on {
                contract.notes.push("chat.request.note.thinkingDefaultOff");
            }
        }
        ClaudeKind::Extended {
            default_on,
            interleaved,
        } => {
            contract.reasoning = ReasoningContract::AnthropicExtended {
                default_on,
                interleaved,
            };
        }
    }
    contract
}

fn gemini_contract(query: &Query<'_>) -> Contract {
    let mut contract = provider_shell(
        "gemini",
        "account-only",
        "chat.request.privacy.accountOnly",
        None,
        &[],
    );
    if query.kind != ProviderKind::GeminiLike {
        return contract;
    }
    let blob = blob(query);
    let Some(kind) = gemini_kind(&blob) else {
        return contract;
    };
    contract.model_known = true;
    contract
        .notes
        .retain(|note| *note != "chat.request.note.modelUnknown");
    contract.tiers = &["standard", "flex", "priority"];
    contract.notes.push("chat.request.note.tierMayReject");
    match kind {
        GeminiKind::Level {
            levels,
            default_level,
        } => {
            contract.reasoning = ReasoningContract::GeminiLevel {
                levels,
                default_level,
            };
            contract.sampling = Sampling::Omit;
        }
        GeminiKind::Budget {
            can_disable,
            default_off,
        } => {
            contract.reasoning = if can_disable {
                ReasoningContract::GeminiBudget {
                    modes: &["disabled", "dynamic"],
                    default_mode: if default_off { "disabled" } else { "dynamic" },
                    locked_on: false,
                }
            } else {
                ReasoningContract::GeminiBudget {
                    modes: &["dynamic"],
                    default_mode: "dynamic",
                    locked_on: true,
                }
            };
            if !can_disable {
                contract.notes.push("chat.request.note.thinkingLocked");
            }
            contract.sampling = Sampling::Range {
                min: 0.0,
                max: 2.0,
                default_value: 1.0,
            };
            contract.gemini_top_p = true;
        }
    }
    contract
}

fn effort_contract(
    mut contract: Contract,
    levels: &'static [&'static str],
    default_level: &'static str,
    completion_tokens: bool,
) -> Contract {
    contract.model_known = true;
    contract.reasoning = ReasoningContract::Effort {
        levels,
        default_level,
    };
    contract.sampling = Sampling::Omit;
    contract.token_field = if completion_tokens {
        TokenField::MaxCompletionTokens
    } else {
        TokenField::MaxTokens
    };
    contract
        .notes
        .retain(|note| *note != "chat.request.note.modelUnknown");
    contract
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

fn blob(query: &Query<'_>) -> String {
    format!(
        "{} {}",
        norm(query.model_id),
        query.family.map(norm).unwrap_or_default()
    )
}

fn norm(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('.', "-")
}

fn has(blob: &str, needle: &str) -> bool {
    blob.contains(needle)
}

fn has_token(blob: &str, token: &str) -> bool {
    blob.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '-'))
        .any(|part| part == token || part.starts_with(&format!("{token}-")))
}

fn is_minimax_m3(blob: &str) -> bool {
    has(blob, "minimax-m3") || has(blob, "minimax/m3") || has_token(blob, "m3")
}

fn is_minimax_m2(blob: &str) -> bool {
    has(blob, "minimax-m2")
}

enum ClaudeKind {
    AlwaysAdaptive {
        levels: &'static [&'static str],
        default_level: &'static str,
    },
    Adaptive {
        levels: &'static [&'static str],
        default_level: &'static str,
        default_on: bool,
        opus5_limit: bool,
    },
    Extended {
        default_on: bool,
        interleaved: bool,
    },
}

fn claude_kind(blob: &str) -> Option<ClaudeKind> {
    const ALL: &[&str] = &["low", "medium", "high", "xhigh", "max"];
    const NO_XHIGH: &[&str] = &["low", "medium", "high", "max"];
    if has(blob, "fable-5") || has(blob, "mythos-5") || has(blob, "opus-5-5") {
        return Some(ClaudeKind::AlwaysAdaptive {
            levels: ALL,
            default_level: if has(blob, "opus-5-5") {
                "medium"
            } else {
                "high"
            },
        });
    }
    if has(blob, "mythos-preview") {
        return Some(ClaudeKind::AlwaysAdaptive {
            levels: NO_XHIGH,
            default_level: "high",
        });
    }
    if has(blob, "opus-5") {
        return Some(ClaudeKind::Adaptive {
            levels: ALL,
            default_level: "high",
            default_on: true,
            opus5_limit: true,
        });
    }
    if has(blob, "sonnet-5") {
        return Some(ClaudeKind::Adaptive {
            levels: ALL,
            default_level: "high",
            default_on: true,
            opus5_limit: false,
        });
    }
    if has(blob, "opus-4-8") || has(blob, "opus-4-7") {
        return Some(ClaudeKind::Adaptive {
            levels: ALL,
            default_level: "high",
            default_on: false,
            opus5_limit: false,
        });
    }
    if has(blob, "opus-4-6") || has(blob, "sonnet-4-6") {
        return Some(ClaudeKind::Adaptive {
            levels: NO_XHIGH,
            default_level: "high",
            default_on: false,
            opus5_limit: false,
        });
    }
    if has(blob, "haiku-4-5") {
        return Some(ClaudeKind::Extended {
            default_on: false,
            interleaved: false,
        });
    }
    if has(blob, "opus-4-5") || has(blob, "sonnet-4-5") {
        return Some(ClaudeKind::Extended {
            default_on: false,
            interleaved: true,
        });
    }
    None
}

enum GeminiKind {
    Level {
        levels: &'static [&'static str],
        default_level: &'static str,
    },
    Budget {
        can_disable: bool,
        default_off: bool,
    },
}

fn gemini_kind(blob: &str) -> Option<GeminiKind> {
    const FULL: &[&str] = &["minimal", "low", "medium", "high"];
    const NO_MIN: &[&str] = &["low", "medium", "high"];
    if has(blob, "gemini-3-8") || has(blob, "gemini-3-7") {
        return Some(GeminiKind::Level {
            levels: NO_MIN,
            default_level: "medium",
        });
    }
    if has(blob, "gemini-3-1-pro") {
        return Some(GeminiKind::Level {
            levels: NO_MIN,
            default_level: "high",
        });
    }
    if has(blob, "flash-lite") && has(blob, "gemini-3") {
        return Some(GeminiKind::Level {
            levels: FULL,
            default_level: "minimal",
        });
    }
    if has(blob, "gemini-3-6") || has(blob, "gemini-3-5") {
        return Some(GeminiKind::Level {
            levels: FULL,
            default_level: "medium",
        });
    }
    if has(blob, "gemini-3") {
        return Some(GeminiKind::Level {
            levels: FULL,
            default_level: "high",
        });
    }
    if has(blob, "gemini-2-5-pro") {
        return Some(GeminiKind::Budget {
            can_disable: false,
            default_off: false,
        });
    }
    if has(blob, "gemini-2-5-flash-lite") {
        return Some(GeminiKind::Budget {
            can_disable: true,
            default_off: true,
        });
    }
    if has(blob, "gemini-2-5-flash") {
        return Some(GeminiKind::Budget {
            can_disable: true,
            default_off: false,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(kind: ProviderKind, base: &str, model: &str) -> DescribeModelRequest {
        DescribeModelRequest {
            kind,
            base_url: base.into(),
            model_id: model.into(),
            family: None,
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
    fn routed_minimax_does_not_inherit_direct_contract() {
        let query = query(
            ProviderKind::OpenAiLike,
            "https://openrouter.ai/api/v1",
            "MiniMax-M3",
        );
        let plan = prepare(&query, &ChatRequestOptions::default(), 4096);
        assert!(plan.openai_thinking.is_none());
        assert!(!plan.reasoning_split);
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
    fn unknown_host_omits_unverified_effort() {
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
        assert!(plan.reasoning_effort.is_none());
    }
}
