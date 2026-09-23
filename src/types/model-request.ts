export type ReasoningView =
  | { kind: "unsupported" }
  | { kind: "unknown" }
  | { kind: "thinking"; modes: string[]; defaultMode: string; lockedOn: boolean }
  | { kind: "effort"; levels: string[]; defaultLevel: string }
  | { kind: "gemini-level"; levels: string[]; defaultLevel: string }
  | { kind: "gemini-budget"; modes: string[]; defaultMode: string; lockedOn: boolean }
  | {
      kind: "claude";
      thinkingModes: string[];
      defaultThinking: string;
      lockedOn: boolean;
      levels: string[];
      defaultLevel: string;
    };

export type SamplingView = {
  temperature: string;
  min?: number;
  max?: number;
  defaultValue?: number;
};

export type ModelRequestView = {
  vendor: string;
  modelContract: string;
  reasoning: ReasoningView;
  serviceTiers: string[];
  sampling: SamplingView;
  privacySupport: string;
  privacyDetailKey: string;
  reasoningSplit: boolean;
  notes: string[];
};

export type ModelRequestOverride = {
  reasoningMode?: string;
  effort?: string;
  serviceTier?: string;
  temperature?: number;
  maxOutputTokens?: number;
};
