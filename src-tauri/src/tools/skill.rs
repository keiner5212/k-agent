use serde_json::{json, Value};

use super::{
    yaml_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, YamlValue, TOOL_KIND_CONTEXT,
};

pub const NAME: &str = "skill";

const DESCRIPTION: &str = "Load a skill by name. Returns SKILL.md body and dir.";

pub struct SkillTool;

impl Tool for SkillTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Skill name"
                    }
                },
                "required": ["name"]
            }),
        }
    }

    fn execute(&self, args: &Value, ctx: &ToolContext<'_>) -> ToolOutcome {
        let Some(name) = args.get("name").and_then(Value::as_str).map(str::trim) else {
            return super::context_error(None, "skill tool requires a string `name`.");
        };
        if name.is_empty() {
            return super::context_error(None, "skill name is empty.");
        }
        let Some(app) = ctx.app else {
            return super::context_error(None, "skill tool requires the app handle.");
        };
        match crate::skills::find_skill_by_name(app, name) {
            Ok(Some(skill)) => {
                let body = skill.body.trim();
                ToolOutcome {
                    text: yaml_doc(&[
                        ("name", YamlValue::Str(name)),
                        ("path", YamlValue::Str(&skill.path)),
                        ("body", YamlValue::Block(body)),
                    ]),
                    display: ToolDisplay {
                        kind: TOOL_KIND_CONTEXT.to_string(),
                        path: Some(skill.path.clone()),
                        skill_name: Some(name.to_string()),
                        status: Some("ok".into()),
                        ..ToolDisplay::default()
                    },
                    snapshot: None,
                }
            }
            Ok(None) => with_skill_name(
                super::context_error(None, &format!("Skill `{name}` was not found.")),
                name,
            ),
            Err(error) => with_skill_name(
                super::context_error(None, &format!("Unable to load skill `{name}`: {error}")),
                name,
            ),
        }
    }
}

fn with_skill_name(mut outcome: ToolOutcome, name: &str) -> ToolOutcome {
    outcome.display.skill_name = Some(name.to_string());
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_empty_name() {
        let dir = std::env::temp_dir();
        let ctx = crate::tools::ToolContext::for_test(dir, 1);
        let outcome = SkillTool.execute(&json!({"name": "  "}), &ctx);
        assert_eq!(outcome.display.status.as_deref(), Some("error"));
        assert!(outcome.text.contains("empty"));
    }

    #[test]
    fn rejects_missing_app_handle() {
        let dir = std::env::temp_dir();
        let ctx = crate::tools::ToolContext::for_test(dir, 1);
        let outcome = SkillTool.execute(&json!({"name": "demo"}), &ctx);
        assert_eq!(outcome.display.status.as_deref(), Some("error"));
        assert!(outcome.text.contains("app handle"));
    }
}
