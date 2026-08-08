import { invoke } from "@tauri-apps/api/core";
import type { ThemeSetting } from "@/lib/theme";

/** 변환 결과를 어디에 둘지. Rust `SaveMode` 와 같은 값이어야 한다. */
export type SaveMode = "ask" | "sameAsSource" | "fixedFolder";

/** 같은 이름이 이미 있을 때. */
export type ConflictRule = "number" | "overwrite";

export interface Settings {
  saveMode: SaveMode;
  /** `fixedFolder` 일 때 쓸 폴더. 고르지 않았으면 null. */
  outputDir: string | null;
  /** 파일명 끝(확장자 앞)에 붙일 말. 비어 있으면 원본 이름 그대로. */
  nameSuffix: string;
  onConflict: ConflictRule;
  theme: ThemeSetting;
}

/** Rust `Settings::default()` 와 같아야 한다. */
export const DEFAULT_SETTINGS: Settings = {
  saveMode: "ask",
  outputDir: null,
  nameSuffix: "",
  onConflict: "number",
  theme: "system",
};

/** 코어가 준 값이 우리가 아는 모양인지 확인하고, 빠진 값은 기본값으로 채운다. */
export function normalizeSettings(value: unknown): Settings {
  if (typeof value !== "object" || value === null) return DEFAULT_SETTINGS;

  const raw = value as Partial<Settings>;

  return {
    saveMode: raw.saveMode ?? DEFAULT_SETTINGS.saveMode,
    outputDir: raw.outputDir ?? DEFAULT_SETTINGS.outputDir,
    nameSuffix: raw.nameSuffix ?? DEFAULT_SETTINGS.nameSuffix,
    onConflict: raw.onConflict ?? DEFAULT_SETTINGS.onConflict,
    theme: raw.theme ?? DEFAULT_SETTINGS.theme,
  };
}

export async function getSettings(): Promise<Settings> {
  return normalizeSettings(await invoke("get_settings"));
}

export function saveSettings(settings: Settings): Promise<void> {
  return invoke("save_settings", { settings });
}
