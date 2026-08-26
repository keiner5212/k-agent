use serde_json::{json, Value};

use super::{Tool, ToolContext, ToolOutcome, ToolSpec};

pub const NAME: &str = "skill";

const DESCRIPTION: &str = concat!(
    "Load a specialized skill when the task matches an available skill in the system context. ",
    "Use this tool to inject the skill instructions into the conversation. ",
    "The name must match a skill listed in the system prompt."
);

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
                        "description": "Skill name from the available skills list"
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
                text: to_model_output(&skill.name, &skill.path, &skill.body),
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

fn to_model_output(name: &str, path: &str, body: &str) -> String {
    let directory = std::path::Path::new(path)
        .to_string_lossy()
        .into_owned();
    format!(
        "<skill_content name=\"{name}\">\n# Skill: {name}\n\n{}\n\nBase directory for this skill: {directory}\nRelative paths in this skill (e.g., scripts/, reference/) are relative to this base directory.\n</skill_content>",
        body.trim()
    )
}
