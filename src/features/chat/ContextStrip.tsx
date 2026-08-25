import { useEffect, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Folder, GitBranch } from "lucide-react";
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
  const workspacePath = useSkillsStore((state) => state.workspacePath);

  const repo = useRepoInfo(workspacePath);

  useEffect(() => {
    void loadSkills();
  }, [loadSkills]);

  return (
    <footer className="context-strip">
      <div className="context-strip__row">
        <span className="context-strip__chip" title={workspacePath ?? t("workspace.unset")}>
          <Folder size={12} strokeWidth={1.5} />
          <span className="context-strip__chip-label">{t("workspace.label")}</span>
          <span className="context-strip__chip-value">
            {workspacePath ? shortPath(workspacePath) : t("workspace.unset")}
          </span>
        </span>
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
