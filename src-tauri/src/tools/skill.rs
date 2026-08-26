use serde_json::{json, Value};

use super::{Tool, ToolContext, ToolOutcome, ToolSpec};

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
            return ToolOutcome {
                text: "skill tool requires a string `name`.".into(),
            };
        };
        if name.is_empty() {
            return ToolOutcome {
                text: "skill name is empty.".into(),
            };
        }
        match crate::skills::find_skill_by_name(ctx.app, name) {
            Ok(Some(skill)) => ToolOutcome {
                text: to_model_output(&skill.path, &skill.body),
            },
            Ok(None) => ToolOutcome {
                text: format!("Skill `{name}` was not found."),
            },
            Err(error) => ToolOutcome {
                text: format!("Unable to load skill `{name}`: {error}"),
            },
        }
    }
}

fn to_model_output(path: &str, body: &str) -> String {
    format!("{}\n\ndir: {path}", body.trim())
}
