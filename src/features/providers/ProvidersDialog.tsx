import { useEffect, type ReactNode } from "react";
import { Dialog } from "@/components/Dialog";
import { useProvidersStore } from "@/lib/providers";
import { ProviderList } from "./ProviderList";

type ProvidersDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export const ProvidersDialog = ({ open, onOpenChange }: ProvidersDialogProps): ReactNode => {
  const load = useProvidersStore((state) => state.load);

  useEffect(() => {
    if (open) void load();
  }, [open, load]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange} titleKey="providers.title">
      <ProviderList onClose={() => onOpenChange(false)} />
    </Dialog>
  );
};
