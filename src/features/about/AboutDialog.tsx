import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";
import { APP_CREATORS, APP_VERSION } from "@/lib/app-meta";
import easterEgg from "@/assets/easter-egg.png";

const EASTER_EGG_CLICKS = 7;

type AboutDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export const AboutDialog = ({ open, onOpenChange }: AboutDialogProps): ReactNode => {
  const { t } = useTranslation();
  const [clicks, setClicks] = useState(0);
  const unlocked = clicks >= EASTER_EGG_CLICKS;

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      titleKey="about.title"
      size="narrow"
      placement="center"
    >
      <div className="about-dialog">
        <p className="about-dialog__version" onClick={() => setClicks((count) => count + 1)}>
          {t("about.version", { version: APP_VERSION })}
        </p>
        {unlocked ? <img className="about-dialog__egg" src={easterEgg} alt="" /> : null}
        <h3 className="about-dialog__heading">{t("about.creators")}</h3>
        <ul className="about-dialog__list">
          {APP_CREATORS.map((creator) => (
            <li key={creator.url}>
              <span className="about-dialog__name">{creator.name}</span>
              <a
                className="about-dialog__link"
                href={creator.url}
                target="_blank"
                rel="noreferrer"
                title={t("links.openInBrowserHint")}
              >
                {creator.url.replace("https://github.com/", "@")}
              </a>
            </li>
          ))}
        </ul>
        <div className="form-actions">
          <GlassButton variant="primary" onClick={() => onOpenChange(false)}>
            {t("about.close")}
          </GlassButton>
        </div>
      </div>
    </Dialog>
  );
};
