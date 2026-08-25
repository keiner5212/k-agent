import { useEffect, type MouseEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Folder, GitBranch } from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useSkillsStore } from "@/lib/skills";
import { useRepoInfo } from "./use-repo-info";

const shortPath = (path: string): string => {
  const home = path.includes("/home/") ? path.replace(/^\/home\/[^/]+/, "~") : path;
  const parts = home.split("/").filter(Boolean);
  if (parts.length <= 3) return home;
  return `…/${parts.slice(-2).join("/")}`;
};

export const ContextStrip = (): ReactNode => {
  const { t } = useTranslation();
  const loadSkills = useSkillsStore((state) => state.load);
  const setWorkspacePath = useSkillsStore((state) => state.setWorkspacePath);
  const workspacePath = useSkillsStore((state) => state.workspacePath);

  const repo = useRepoInfo(workspacePath);

  useEffect(() => {
    void loadSkills();
  }, [loadSkills]);

  const handlePickWorkspace = (event: MouseEvent<HTMLButtonElement>): void => {
    event.preventDefault();
    void (async () => {
      const picked = await openDialog({
        directory: true,
        multiple: false,
        title: t("workspace.pickFolder"),
      });
      if (typeof picked !== "string") return;
      await setWorkspacePath(picked);
    })();
  };

  return (
    <footer className="context-strip">
      <div className="context-strip__row">
        <button
          type="button"
          className="context-strip__chip context-strip__chip--button"
          title={workspacePath ?? t("workspace.unset")}
          onClick={handlePickWorkspace}
        >
          <Folder size={12} strokeWidth={1.5} />
          <span className="context-strip__chip-label">{t("workspace.label")}</span>
          <span className="context-strip__chip-value">
            {workspacePath ? shortPath(workspacePath) : t("workspace.unset")}
          </span>
        </button>
        {repo.isRepo && repo.branch ? (
          <span className="context-strip__chip context-strip__chip--accent" title={repo.branch}>
            <GitBranch size={12} strokeWidth={1.5} />
            <span className="context-strip__chip-value">{repo.branch}</span>
          </span>
        ) : null}
      </div>
    </footer>
  );
};
