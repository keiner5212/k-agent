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
    if has(tools, "ask_user") {
        push(&mut parts, wrap("clarify", CLARIFY, shape));
    }
    if has(tools, "todowrite") {
        push(&mut parts, wrap("todos", TODOS, shape));
    }
    if has(tools, "bash") {
        push(&mut parts, tool_choice(tools, shape));
    }
    let voice = agent.personality.trim();
    if !voice.is_empty() {
        push(&mut parts, wrap("personality", voice, shape));
    }
    push(
        &mut parts,
        wrap("assignment", &assignment(&agent.name), shape),
    );
    push(&mut parts, wrap("rendering", RENDERING, shape));
    if has(tools, "page_shot") {
        push(&mut parts, wrap("visual-check", VISUAL, shape));
    }
    let base = parts.join("\n\n");
    wrap_outer(app, &base, shape)
}

fn assignment(name: &str) -> String {
    format!(
        "You are `{name}` on one task.\n\
Do only the user message. Do not widen it. Do not start another task.\n\
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
    let mut lines = vec![
        "Workspace memory is on. Personal notes live in `.k-agent/NOTES.md`.",
        "At the end of every turn, after the answer is ready, review that file. Save only if the list changed.",
        "",
        "1. Drop. Remove a bullet that this turn contradicted, that the user overrode, or that is no longer needed.",
        "2. Add. Add a bullet only when it will still matter on a later task, the user stated it or corrected you or repeated it, and no current bullet or AGENTS.md already says it.",
        "3. Promote. If a bullet outgrows a one-line preference and is now a standing project rule, move it into `AGENTS.md`, or into `agents.md` when that file already exists. Do not edit `CLAUDE.md` or `CONTEXT.md`. Remove the bullet from NOTES.md once it is there.",
        "4. Refuse. Do not add a one-off task, a guess, a secret, chat history, or a restatement of this request.",
        "5. Cap. At most 20 bullets, one line each. To add past the cap, merge or drop a weaker bullet first.",
        "6. Save. If NOTES.md changed, write it in this turn with `write` or `edit`. If a bullet was promoted, update the workspace instruction file in the same turn. If nothing changed, leave both files alone.",
        "",
        "A bullet is a durable workspace rule, such as \"always run the formatter\". Not the file edited in this turn.",
    ];
    if body.is_empty() {
        lines.push("");
        lines.push("The file is empty. Create it when the first bullet passes step 2.");
    } else {
        lines.push("");
        lines.push("Current notes:");
        lines.push(body);
    }
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
    let mut lines = vec![
        "Use the dedicated tool. Do not use `bash` for work another tool already does.".to_string(),
    ];
    if has(tools, "list_directory") {
        lines.push(
            "List a directory with `list_directory`. Find files by name with its `glob` (`*.rs` matches any depth). Do not use `ls`, `find`, or `tree`."
                .into(),
        );
    }
    if has(tools, "read") {
        lines.push("Read a file with `read`. Do not use `cat`, `head`, `tail`, or `wc`.".into());
    }
    if has(tools, "grep") {
        lines.push(
            "Search file contents with `grep`. Do not run `grep` or `rg` in the shell.".into(),
        );
    }
    if has(tools, "lsp") {
        lines.push(
            "Use `lsp` for a definition, references, or hover when a language server is installed."
                .into(),
        );
    }
    if has(tools, "write") {
        lines.push("Create or overwrite a file with `write`.".into());
    }
    if has(tools, "edit") {
        lines.push("Change one exact span with `edit`.".into());
    }
    if has(tools, "apply_patch") {
        lines.push("Change several files in one diff with `apply_patch`.".into());
    }
    if has(tools, "create_folder") {
        lines.push("Make a directory with `create_folder`.".into());
    }
    if has(tools, "delete") {
        lines.push("Remove a file or empty directory with `delete`.".into());
    }
    lines.push(
        "`bash` is for a command that must run and finish, such as install, build, test, or git."
            .into(),
    );
    if has(tools, "http_request") {
        lines.push(
            "Call an API with `http_request`, including localhost. Do not use `curl` or `wget`."
                .into(),
        );
    }
    if has(tools, "background") {
        lines.push(
            "A process that must stay up uses `background`, not `bash`. That process is killed when the turn ends. Do not kill its pid."
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

const CLARIFY: &str = "\
Before you act, list what you would have to assume: goal, scope, files, behavior, names, and success.\n\
Call `ask_user` for every gap. One question per gap. Put the option you would have assumed first. Leave free text on.\n\
Wait for the answer. Do not start the work, and do not pick for the user.";

const TODOS: &str = "\
Keep the session todo list matched to the work when the task has several steps.\n\
Call `todowrite` with the full list. Each item is content, status, and priority (high, medium, or low). Do not invent ids.\n\
Keep at most one item in_progress. Mark an item completed only after that step is done.\n\
Send the list again when a step starts, finishes, or is dropped. An empty list clears it.";

const VISUAL: &str = "\
Use `page_shot` only on a page that is already being served. One shot per review.\n\
Do not repeat it with a different host, height, or selector.\n\
A blank or identical image is a capture miss. Do not edit the page to remove a black box from a bad shot.\n\
Start a dev server with `background`. It is killed when the turn ends. Do not kill its pid. Do not use `bash` for that.";

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
