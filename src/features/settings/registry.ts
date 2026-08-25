export type SettingItemType =
  "select" | "toggle" | "keybinding" | "action" | "list" | "modelChoice";

export type SettingOption = {
  value: string;
  labelKey: string;
};

export type SettingItem = {
  id: string;
  type: SettingItemType;
  titleKey: string;
  descriptionKey: string;
  options?: SettingOption[];
  keywords?: string[];
};

export type SettingsSectionDef = {
  id:
    | "general"
    | "chat"
    | "modelChoices"
    | "lsps"
    | "providers"
    | "skills"
    | "agents"
    | "agentsMd"
    | "keybindings"
    | "debug";
  titleKey: string;
  descriptionKey?: string;
  keywords?: string[];
  items: SettingItem[];
};
