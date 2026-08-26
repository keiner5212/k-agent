import { type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { FileText, Film, X } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { attachmentPreviewUrl } from "@/lib/attachments";
import { useComposerStore } from "@/lib/composer";
import type { ChatAttachment } from "@/types/chat";

const FileGlyph = ({ item }: { item: ChatAttachment }): ReactNode => {
  const preview = attachmentPreviewUrl(item);
  if (preview) {
    return <img src={preview} alt="" className="composer-attach-chip__thumb" />;
  }
  if (item.kind === "video") {
    return <Film size={14} strokeWidth={1.5} />;
  }
  return <FileText size={14} strokeWidth={1.5} />;
};

export const ComposerAttachments = (): ReactNode => {
  const { t } = useTranslation();
  const attachments = useComposerStore((state) => state.attachments);
  const removeAttachment = useComposerStore((state) => state.removeAttachment);
  if (attachments.length === 0) return null;

  return (
    <ul className="composer-attach-list">
      {attachments.map((item) => (
        <li key={item.id} className="composer-attach-chip">
          <FileGlyph item={item} />
          <span className="composer-attach-chip__name" title={item.name}>
            {item.name}
          </span>
          <IconButton
            label={t("chat.composer.removeAttachment")}
            className="composer-attach-chip__remove"
            onClick={() => removeAttachment(item.id)}
          >
            <X size={12} strokeWidth={1.5} />
          </IconButton>
        </li>
      ))}
    </ul>
  );
};
