import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Pause, Play, Volume2, VolumeX } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { IconButton } from "@/components/IconButton";
import { LineEditor } from "@/components/LineEditor";
import { attachmentPreviewUrl, blobFromAttachment } from "@/lib/attachments";
import type { ChatAttachment } from "@/types/chat";

type AttachmentPreviewDialogProps = {
  item: ChatAttachment | null;
  onClose: () => void;
};

const formatMediaTime = (seconds: number): string => {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, "0")}`;
};

const useObjectUrl = (item: ChatAttachment): string | null => {
  const [url, setUrl] = useState<string | null>(null);
  const id = item.id;
  const data = item.data;
  const mime = item.mime;
  useEffect(() => {
    if (!data) return;
    const blob = blobFromAttachment({
      id,
      name: "",
      mime,
      kind: "video",
      data,
    });
    if (!blob) return;
    const next = URL.createObjectURL(blob);
    const frame = requestAnimationFrame(() => setUrl(next));
    return () => {
      cancelAnimationFrame(frame);
      URL.revokeObjectURL(next);
    };
  }, [data, id, mime]);
  return url;
};

const ObjectPreview = ({ item, url }: { item: ChatAttachment; url: string }): ReactNode => {
  const data = item.kind === "pdf" ? `${url}#toolbar=0` : url;
  return (
    <object
      data={data}
      type={item.mime}
      className="attachment-preview__object"
      aria-label={item.name}
    >
      {item.name}
    </object>
  );
};

const VideoPreview = ({ url }: { url: string }): ReactNode => {
  const { t } = useTranslation();
  const videoRef = useRef<HTMLVideoElement>(null);
  const seekRef = useRef<HTMLInputElement>(null);
  const currentLabelRef = useRef<HTMLSpanElement>(null);
  const durationLabelRef = useRef<HTMLSpanElement>(null);
  const durationSec = useRef(0);
  const seeking = useRef(false);
  const [playing, setPlaying] = useState(false);
  const [muted, setMuted] = useState(false);

  useEffect(() => {
    const node = videoRef.current;
    if (!node) return;
    node.src = url;
    return () => {
      node.removeAttribute("src");
      node.load();
    };
  }, [url]);

  const syncSeek = (time: number): void => {
    if (currentLabelRef.current) {
      currentLabelRef.current.textContent = formatMediaTime(time);
    }
    const seek = seekRef.current;
    const total = durationSec.current;
    if (!seek || total <= 0) return;
    seek.value = String(time);
    seek.style.setProperty("--value", `${(time / total) * 100}%`);
  };

  const togglePlay = (): void => {
    const node = videoRef.current;
    if (!node) return;
    if (node.paused) void node.play();
    else node.pause();
  };

  return (
    <div className="attachment-preview__video">
      <video
        ref={videoRef}
        className="attachment-preview__video-el"
        preload="metadata"
        onClick={togglePlay}
        onPlay={() => setPlaying(true)}
        onPause={() => setPlaying(false)}
        onTimeUpdate={() => {
          const node = videoRef.current;
          if (!node || seeking.current) return;
          syncSeek(node.currentTime);
        }}
        onLoadedMetadata={() => {
          const node = videoRef.current;
          if (!node) return;
          durationSec.current = Number.isFinite(node.duration) ? node.duration : 0;
          const seek = seekRef.current;
          if (seek) seek.max = String(durationSec.current);
          if (durationLabelRef.current) {
            durationLabelRef.current.textContent = formatMediaTime(durationSec.current);
          }
          syncSeek(node.currentTime);
        }}
        onEnded={() => setPlaying(false)}
      />
      <div className="attachment-preview__controls">
        <IconButton
          label={playing ? t("chat.preview.pause") : t("chat.preview.play")}
          onClick={togglePlay}
        >
          {playing ? <Pause size={14} strokeWidth={1.5} /> : <Play size={14} strokeWidth={1.5} />}
        </IconButton>
        <span ref={currentLabelRef} className="attachment-preview__time">
          0:00
        </span>
        <input
          ref={seekRef}
          type="range"
          className="appearance-slider__input attachment-preview__seek"
          min={0}
          max={0}
          step={0.1}
          defaultValue={0}
          aria-label={t("chat.preview.seek")}
          onPointerDown={() => {
            seeking.current = true;
          }}
          onPointerUp={() => {
            seeking.current = false;
          }}
          onPointerCancel={() => {
            seeking.current = false;
          }}
          onChange={(event) => {
            const next = Number(event.target.value);
            const node = videoRef.current;
            if (node) node.currentTime = next;
            syncSeek(next);
          }}
        />
        <span ref={durationLabelRef} className="attachment-preview__time">
          0:00
        </span>
        <IconButton
          label={muted ? t("chat.preview.unmute") : t("chat.preview.mute")}
          onClick={() => {
            const node = videoRef.current;
            const next = !muted;
            if (node) node.muted = next;
            setMuted(next);
          }}
        >
          {muted ? (
            <VolumeX size={14} strokeWidth={1.5} />
          ) : (
            <Volume2 size={14} strokeWidth={1.5} />
          )}
        </IconButton>
      </div>
    </div>
  );
};

const PreviewBody = ({ item }: { item: ChatAttachment }): ReactNode => {
  if (item.kind === "text" || item.kind === "document") {
    return (
      <div className="attachment-preview__text">
        <LineEditor value={item.text ?? ""} onChange={() => undefined} readOnly />
      </div>
    );
  }
  if (item.kind === "image") {
    const src = attachmentPreviewUrl(item);
    if (!src) return <p className="attachment-preview__missing">{item.name}</p>;
    return <img src={src} alt={item.name} className="attachment-preview__image" />;
  }
  return <BlobPreview item={item} />;
};

const BlobPreview = ({ item }: { item: ChatAttachment }): ReactNode => {
  const url = useObjectUrl(item);
  if (!url) return <div className="attachment-preview__video" />;
  if (item.kind === "video") return <VideoPreview url={url} />;
  if (item.kind === "pdf") return <ObjectPreview item={item} url={url} />;
  return <p className="attachment-preview__missing">{item.name}</p>;
};

export const AttachmentPreviewDialog = ({
  item,
  onClose,
}: AttachmentPreviewDialogProps): ReactNode => (
  <Dialog
    open={item !== null}
    onOpenChange={(open) => {
      if (!open) onClose();
    }}
    titleKey="chat.preview.title"
    size="wide"
  >
    {item ? (
      <div className="attachment-preview">
        <p className="attachment-preview__name" title={item.name}>
          {item.name}
        </p>
        <div className="attachment-preview__stage">
          <PreviewBody item={item} />
        </div>
      </div>
    ) : null}
  </Dialog>
);
