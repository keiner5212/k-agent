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
          {tools.map((tool) => (
            <li key={tool.name} className="mcp-tools-dialog__item">
              <span className="mcp-tools-dialog__name">{tool.name}</span>
              <p className="mcp-tools-dialog__desc">
                {tool.description?.trim() || t("mcpServers.toolsDialog.noDescription")}
              </p>
            </li>
          ))}
        </ul>
      </div>
    </Dialog>
  );
};
