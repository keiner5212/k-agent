import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";
import type { McpServer } from "@/types/mcp-servers";

type McpServerToolsDialogProps = {
  open: boolean;
  server: McpServer | null;
  onOpenChange: (open: boolean) => void;
};

type ToolParameter = {
  name: string;
  type: string;
  required: boolean;
  description: string;
};

const schemaType = (value: unknown): string => {
  if (typeof value === "string" && value.length > 0) return value;
  if (Array.isArray(value)) {
    const parts = value.filter((item): item is string => typeof item === "string");
    if (parts.length > 0) return parts.join(" | ");
  }
  return "any";
};

const toolParameters = (schema: Record<string, unknown> | undefined): ToolParameter[] => {
  const properties = schema?.properties;
  if (!properties || typeof properties !== "object" || Array.isArray(properties)) return [];
  const requiredRaw = Array.isArray(schema?.required) ? schema.required : [];
  const required = new Set(requiredRaw.filter((item): item is string => typeof item === "string"));
  const out: ToolParameter[] = [];
  for (const [name, value] of Object.entries(properties as Record<string, unknown>)) {
    const spec =
      value && typeof value === "object" && !Array.isArray(value)
        ? (value as Record<string, unknown>)
        : {};
    out.push({
      name,
      type: schemaType(spec.type),
      required: required.has(name),
      description: typeof spec.description === "string" ? spec.description.trim() : "",
    });
  }
  return out;
};

export const McpServerToolsDialog = ({
  open,
  server,
  onOpenChange,
}: McpServerToolsDialogProps): ReactNode => {
  const { t } = useTranslation();
  if (!open || !server) return null;
  const tools = server.tools ?? [];

  return (
    <Dialog
      open
      onOpenChange={onOpenChange}
      titleKey="mcpServers.toolsDialog.title"
      placement="center"
      size="default"
      footer={
        <GlassButton variant="secondary" onClick={() => onOpenChange(false)}>
          {t("mcpServers.toolsDialog.close")}
        </GlassButton>
      }
    >
      <div className="mcp-tools-dialog">
        <p className="mcp-tools-dialog__server">
          {t("mcpServers.toolsDialog.server", { name: server.name })}
        </p>
        <ul className="mcp-tools-dialog__list">
          {tools.map((tool) => {
            const parameters = toolParameters(tool.inputSchema);
            return (
              <li key={tool.name} className="mcp-tools-dialog__item">
                <span className="mcp-tools-dialog__name">{tool.name}</span>
                <p className="mcp-tools-dialog__desc">
                  {tool.description?.trim() || t("mcpServers.toolsDialog.noDescription")}
                </p>
                {parameters.length > 0 ? (
                  <>
                    <p className="mcp-tools-dialog__params-title">
                      {t("mcpServers.toolsDialog.parameters")}
                    </p>
                    <ul className="mcp-tools-dialog__params">
                      {parameters.map((parameter) => (
                        <li key={parameter.name} className="mcp-tools-dialog__param">
                          <span className="mcp-tools-dialog__param-name">{parameter.name}</span>
                          <span className="mcp-tools-dialog__param-type">{parameter.type}</span>
                          {parameter.required ? (
                            <span className="mcp-tools-dialog__param-required">
                              {t("mcpServers.toolsDialog.required")}
                            </span>
                          ) : null}
                          {parameter.description ? (
                            <span className="mcp-tools-dialog__param-desc">
                              {parameter.description}
                            </span>
                          ) : null}
                        </li>
                      ))}
                    </ul>
                  </>
                ) : null}
              </li>
            );
          })}
        </ul>
      </div>
    </Dialog>
  );
};
