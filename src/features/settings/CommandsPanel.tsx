import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { GlassButton } from "@/components/GlassButton";
import { SLASH_COMMANDS } from "@/lib/slash-commands";
import { useSettingsStore } from "@/lib/settings";
import { PROMPT_COMMAND_NAME, type PromptCommand } from "@/types/settings";

type CommandsPanelProps = {
  query: string;
};

const matches = (text: string, query: string): boolean =>
  query.length === 0 || text.toLowerCase().includes(query);

export const CommandsPanel = ({ query }: CommandsPanelProps): ReactNode => {
  const { t } = useTranslation();
  const commands = useSettingsStore((state) => state.promptCommands);
  const setPromptCommands = useSettingsStore((state) => state.setPromptCommands);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [template, setTemplate] = useState("");
  const [error, setError] = useState("");

  const builtin = SLASH_COMMANDS.filter((command) =>
    matches(`${command.name} ${t(command.descriptionKey)}`, query),
  );
  const custom = commands.filter((command) =>
    matches(`${command.name} ${command.description} ${command.template}`, query),
  );

  const add = (): void => {
    const nextName = name.trim();
    const nextTemplate = template.trim();
    const taken = new Set([
      ...SLASH_COMMANDS.map((command) => command.name.toLowerCase()),
      ...commands.map((command) => command.name.toLowerCase()),
    ]);
    if (!PROMPT_COMMAND_NAME.test(nextName) || taken.has(nextName.toLowerCase())) {
      setError(t("settings.commands.nameTaken"));
      return;
    }
    if (!nextTemplate) {
      setError(t("settings.commands.templateRequired"));
      return;
    }
    const next: PromptCommand = {
      name: nextName,
      description: description.trim(),
      template: nextTemplate,
    };
    setPromptCommands([...commands, next]);
    setName("");
    setDescription("");
    setTemplate("");
    setError("");
  };

  const remove = (commandName: string): void => {
    setPromptCommands(commands.filter((command) => command.name !== commandName));
  };

  return (
    <div className="commands-panel">
      <p className="commands-panel__intro">{t("settings.commands.description")}</p>
      <h3 className="commands-panel__heading">{t("settings.commands.builtin")}</h3>
      <ul className="commands-panel__list">
        {builtin.map((command) => (
          <li key={command.id} className="commands-panel__row">
            <div>
              <p className="commands-panel__name">/{command.name}</p>
              <p className="commands-panel__hint">{t(command.descriptionKey)}</p>
            </div>
            <span className="commands-panel__kind">
              {t(
                command.kind === "action"
                  ? "settings.commands.kind.function"
                  : "settings.commands.kind.prompt",
              )}
            </span>
          </li>
        ))}
      </ul>
      <h3 className="commands-panel__heading">{t("settings.commands.custom")}</h3>
      {custom.length === 0 ? (
        <p className="commands-panel__hint">{t("settings.commands.empty")}</p>
      ) : (
        <ul className="commands-panel__list">
          {custom.map((command) => (
            <li key={command.name} className="commands-panel__row">
              <div>
                <p className="commands-panel__name">/{command.name}</p>
                <p className="commands-panel__hint">{command.description || command.template}</p>
              </div>
              <GlassButton variant="secondary" onClick={() => remove(command.name)}>
                {t("settings.commands.remove")}
              </GlassButton>
            </li>
          ))}
        </ul>
      )}
      <form
        className="commands-panel__form"
        onSubmit={(event) => {
          event.preventDefault();
          add();
        }}
      >
        <label className="commands-panel__field">
          <span>{t("settings.commands.name")}</span>
          <input
            className="input"
            value={name}
            onChange={(event) => setName(event.target.value)}
            autoComplete="off"
            spellCheck={false}
          />
        </label>
        <label className="commands-panel__field">
          <span>{t("settings.commands.descriptionLabel")}</span>
          <input
            className="input"
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            autoComplete="off"
          />
        </label>
        <label className="commands-panel__field">
          <span>{t("settings.commands.template")}</span>
          <textarea
            className="input input--area"
            value={template}
            onChange={(event) => setTemplate(event.target.value)}
          />
        </label>
        {error ? <p className="commands-panel__error">{error}</p> : null}
        <GlassButton type="submit">{t("settings.commands.add")}</GlassButton>
      </form>
    </div>
  );
};
