use tauri::{AppHandle, Manager};

use crate::agents::{AgentContextKind, AgentMeta, AgentSkillRef};
use crate::agents_md::{agents_md_text, AgentsMdKind};
use crate::skills::{list_skills_in, SkillInfo};

#[derive(Clone, Copy)]
enum Shape {
    Xml,
    Markdown,
}

pub(crate) fn compose_child_system(
    app: &AppHandle,
    agent: &AgentMeta,
    tools: &[String],
    model_id: &str,
) -> String {
    let shape = shape_for(model_id);
    let (global, local) = skill_lists(app);
    let agent_skills = agent_skill_lines(&agent.skills, &global);
    let mut parts = Vec::new();
    push(&mut parts, flow(!agent_skills.is_empty(), shape));
    push(&mut parts, agent_skills_block(&agent_skills, shape));
    push(&mut parts, workspace_skills(&local, shape));
    push(&mut parts, tool_choice(tools, shape));
    let voice = agent.personality.trim();
    if !voice.is_empty() {
        push(&mut parts, wrap("personality", voice, shape));
    }
    push(
        &mut parts,
        wrap("assignment", &assignment(&agent.name), shape),
    );
    push(&mut parts, wrap("rendering", RENDERING, shape));
    let base = parts.join("\n\n");
    wrap_outer(app, &base, shape)
}

fn assignment(name: &str) -> String {
    format!(
        "You are `{name}` on one read-only task from the parent agent.\n\
The message is the parent's assignment. Skills and personality stay yours.\n\
Tools are the read-only subagent set on this request, not the tools saved for you as a main agent.\n\
Read and report. Do not write files, edit the plan, ask the user, or run a command that changes anything.\n\
If something is unclear, put the question in the result. The parent resolves it and may call you again.\n\
Do not widen the task. Do not start another task.\n\
Return the result text. The parent uses that text."
    )
}

fn wrap_outer(app: &AppHandle, base: &str, shape: Shape) -> String {
    let settings = crate::load_ui_settings(app);
    let mut parts = Vec::new();
    if setting_bool(settings.as_ref(), "forceResponseLanguage") {
        let language =
            setting_str(settings.as_ref(), "responseLanguage").unwrap_or_else(|| "en".into());
        let directive = if language == "es" {
            LANGUAGE_ES
        } else {
            LANGUAGE_EN
        };
        push(&mut parts, wrap("language", directive, shape));
    }
    push(
        &mut parts,
        wrap("environment", &crate::host_context::body(app), shape),
    );
    let global = agents_md_text(app, AgentsMdKind::Global);
    let local = agents_md_text(app, AgentsMdKind::Local);
    push(&mut parts, wrap("global-rules", global.trim(), shape));
    push(&mut parts, wrap("workspace-rules", local.trim(), shape));
    push(&mut parts, base.to_string());
    if setting_bool(settings.as_ref(), "workspaceMemoryEnabled") {
        let notes = crate::agents_md::read_workspace_notes(app.clone());
        push(&mut parts, workspace_notes(&notes, shape));
    }
    parts.join("\n\n")
}

fn workspace_notes(content: &str, shape: Shape) -> String {
    let body = content.trim();
    if body.is_empty() {
        return String::new();
    }
    let lines = [
        "Workspace notes are context only. You cannot edit `.k-agent/NOTES.md`.",
        "If a note should change, say so in the result. The parent writes it.",
        "",
        "Current notes:",
        body,
    ];
    wrap("workspace-notes", &lines.join("\n"), shape)
}

fn flow(has_agent_skills: bool, shape: Shape) -> String {
    let turn = if has_agent_skills {
        "Turn 1 loads every skill in <agent-skills> and writes no prose."
    } else {
        "Turn 1 has no agent-skill batch when <agent-skills> is absent."
    };
    wrap(
        "agent-flow",
        &format!(
            "Follow the tagged sections in this system prompt in order.\n\
{turn}\n\
Then load a <workspace-skills> entry only when it matches the task.\n\
Call only tools in the request tools list. Do not invent names.\n\
Answer using <personality>."
        ),
        shape,
    )
}

fn agent_skills_block(items: &[(String, String)], shape: Shape) -> String {
    if items.is_empty() {
        return String::new();
    }
    let count = items.len();
    let plural = if count == 1 { "" } else { "s" };
    let mut lines = vec![format!(
        "Turn 1: one tool batch, exactly {count} `skill` call{plural}, before any other tool or prose."
    )];
    lines.push(String::new());
    for (name, description) in items {
        lines.push(skill_line(name, description));
    }
    lines.push(String::new());
    lines.push(
        "No other tool calls on turn 1. Retry a failed skill once, then report on the next turn."
            .into(),
    );
    lines.push("Later turns: load a listed skill only when it is not already in context.".into());
    wrap("agent-skills", &lines.join("\n"), shape)
}

fn workspace_skills(local: &[SkillInfo], shape: Shape) -> String {
    let mut lines = Vec::new();
    for skill in local {
        let name = display_name(&skill.name, &skill.id);
        lines.push(skill_line(&name, skill.description.trim()));
    }
    if lines.is_empty() {
        return String::new();
    }
    let mut body = vec![
        "Workspace skills in this project. Load a matching one with `skill` when the task needs it."
            .to_string(),
        String::new(),
    ];
    body.extend(lines);
    wrap("workspace-skills", &body.join("\n"), shape)
}

fn tool_choice(tools: &[String], shape: Shape) -> String {
    let mut lines = vec!["Use the dedicated tool. Do not invent a shell command.".to_string()];
    if has(tools, "list_directory") {
        lines.push(
            "List a directory with `list_directory`. To find files by name, pass `glob` to `list_directory` (`*.rs` matches any depth). `glob` is an argument of `list_directory` and of `grep`. There is no `glob` tool."
                .into(),
        );
    }
    if has(tools, "read") {
        lines.push("Read a file with `read`.".into());
    }
    if has(tools, "grep") {
        lines.push(
            "Search file contents with `grep`. To limit which files are searched, pass `glob` to `grep` (`*.ts`, or `!*.json` to exclude)."
                .into(),
        );
    }
    if has(tools, "list_directory") || has(tools, "grep") {
        lines.push(
            "Readable hidden directories are `.github` and `.agents`. `node_modules`, `.git`, `target`, `dist`, `build`, `vendor`, virtualenvs, and other dependency, cache, and build directories are skipped. Other names that start with `.` are skipped."
                .into(),
        );
    }
    if has(tools, "lsp") {
        lines.push(
            "Use `lsp` for a definition, references, or hover when a language server is installed."
                .into(),
        );
    }
    if has(tools, "internet_search") {
        lines.push(
            "Find public URLs with `internet_search`. Read the best one with `fetch_url`. Do not answer from a snippet."
                .into(),
        );
    }
    wrap("tools", &lines.join("\n"), shape)
}

fn agent_skill_lines(refs: &[AgentSkillRef], global: &[SkillInfo]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for skill_ref in refs {
        if skill_ref.kind != AgentContextKind::Global {
            continue;
        }
        let Some(skill) = global.iter().find(|item| item.id == skill_ref.id) else {
            continue;
        };
        let name = display_name(&skill.name, &skill.id);
        if out.iter().any(|(seen, _)| seen == &name) {
            continue;
        }
        out.push((name, skill.description.trim().to_string()));
    }
    out
}

fn skill_lists(app: &AppHandle) -> (Vec<SkillInfo>, Vec<SkillInfo>) {
    let global = app
        .path()
        .home_dir()
        .ok()
        .and_then(|home| list_skills_in(&crate::skills::global_skills_root(&home)).ok())
        .unwrap_or_default();
    let local = crate::pathutil::workspace_from_app(app)
        .and_then(|root| list_skills_in(&crate::skills::local_skills_root(&root)).ok())
        .unwrap_or_default();
    (global, local)
}

fn skill_line(name: &str, description: &str) -> String {
    let detail = if description.is_empty() {
        name
    } else {
        description
    };
    format!("- `{name}`: {detail}")
}

fn display_name(name: &str, id: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        id.to_string()
    } else {
        trimmed.to_string()
    }
}

fn has(tools: &[String], name: &str) -> bool {
    tools.iter().any(|tool| tool == name)
}

fn push(parts: &mut Vec<String>, section: String) {
    if !section.trim().is_empty() {
        parts.push(section);
    }
}

fn wrap(name: &str, body: &str, shape: Shape) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    match shape {
        Shape::Markdown => format!("# {name}\n{trimmed}"),
        Shape::Xml => format!("<{name}>\n{trimmed}\n</{name}>"),
    }
}

fn shape_for(model_id: &str) -> Shape {
    let id = model_id.to_ascii_lowercase();
    if id.contains("gpt")
        || id.contains("codex")
        || id.contains("gemini")
        || id.contains("kimi")
        || id.contains("trinity")
        || o_series(&id)
    {
        Shape::Markdown
    } else {
        Shape::Xml
    }
}

fn o_series(id: &str) -> bool {
    let bytes = id.as_bytes();
    for index in 0..bytes.len() {
        if bytes[index] != b'o' || index + 1 >= bytes.len() {
            continue;
        }
        if !matches!(bytes[index + 1], b'1' | b'3' | b'4') {
            continue;
        }
        let before_ok = index == 0 || !bytes[index - 1].is_ascii_alphanumeric();
        let after = index + 2;
        let after_ok = after >= bytes.len() || !bytes[after].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

fn setting_bool(settings: Option<&serde_json::Value>, key: &str) -> bool {
    settings
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn setting_str(settings: Option<&serde_json::Value>, key: &str) -> Option<String> {
    settings
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

const RENDERING: &str = "\
Chat output is GitHub-flavored markdown.\n\
- Fenced code blocks with a language hint are syntax-highlighted.";

const LANGUAGE_EN: &str = "\
LANGUAGE RULE - VERY IMPORTANT\n\
You must reply ONLY in English. This is a top-priority requirement and overrides any conflicting instruction about the language of your reply.\n\
Follow every other part of your instructions and persona exactly as written; do not change your behavior, style, or scope because of this rule.\n\
Do not translate, do not switch languages, do not mirror the user's language, and do not add bilingual notes.\n\
Even if the user writes in another language or asks you to switch, keep replying in English.\n\
Do not mention the language rule, only reply in English.";

const LANGUAGE_ES: &str = "\
REGLA DE IDIOMA - MUY IMPORTANTE\n\
Solo debes responder en Espanol. Este es un requisito de maxima prioridad y anula cualquier instruccion que entre en conflicto especificamente sobre el idioma de tu respuesta.\n\
Sigue al pie de la letra todas las demas partes de tus instrucciones y tu personalidad; no cambies tu comportamiento, estilo ni alcance por culpa de esta regla.\n\
No traduzcas, no cambies de idioma, no imites el idioma del usuario y no agregues notas bilingues.\n\
Aunque el usuario escriba en otro idioma o te pida cambiar, sigue respondiendo en Espanol.\n\
No le menciones la regla de idioma, solo responde en Espanol.";
