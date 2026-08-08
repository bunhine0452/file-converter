import { Channel, invoke } from "@tauri-apps/api/core";

/** 변환에 필요한 런타임이 어디까지 준비됐는가. */
export type RuntimeState =
  | "ready"
  | "needsLibreOffice"
  | "needsJre"
  | "needsExtension"
  | "needsFonts"
  | "unsupported";

export interface RuntimeStatusView {
  state: RuntimeState;
  version: string | null;
  exePath: string | null;
  /** 앱이 직접 설치한 LibreOffice 인가 (아니면 사용자가 설치한 것) */
  managed: boolean;
}

export type InstallEvent =
  | { kind: "started"; step: string }
  | { kind: "progress"; step: string; received: number; total: number | null }
  | { kind: "stepDone"; step: string }
  | { kind: "finished" }
  | { kind: "failed"; message: string };

export function getRuntimeStatus(refresh = false): Promise<RuntimeStatusView> {
  return invoke<RuntimeStatusView>("get_runtime_status", { refresh });
}

/**
 * 부족한 런타임을 내려받아 설치한다.
 * 진행 상황은 채널로 오고, 완료 시 갱신된 상태가 반환된다.
 */
export function installRuntime(
  onEvent: (event: InstallEvent) => void,
): Promise<RuntimeStatusView> {
  const channel = new Channel<InstallEvent>();
  channel.onmessage = onEvent;

  return invoke<RuntimeStatusView>("install_runtime", { onEvent: channel });
}

export function convertHwp(source: string, outPath: string): Promise<number> {
  return invoke<number>("convert_hwp", { source, outPath });
}

/**
 * 폴더 하나에 여러 건을 저장할 때 쓸 경로를 코어에서 받아온다.
 *
 * 이름이 겹치면 코어가 번호를 붙인다 — 폴더만 고른 일괄 변환에서는
 * 사용자가 덮어쓰기에 동의한 적이 없다.
 */
export function planOutputPath(source: string, dir: string): Promise<string> {
  return invoke<string>("plan_output_path", { source, dir });
}

/** 상태별 사용자 대면 문구. `unsupported` 를 빼먹지 않도록 전부 채워 둔다. */
export const RUNTIME_STATE_MESSAGE: Record<RuntimeState, string> = {
  ready: "변환 준비가 끝났습니다",
  needsLibreOffice: "한글 문서 변환기를 아직 내려받지 않았습니다",
  needsJre: "Java 런타임이 필요합니다",
  needsExtension: "한글 문서 확장을 설치해야 합니다",
  needsFonts: "한글 글꼴을 내려받아야 합니다 (없으면 글자가 깨집니다)",
  unsupported: "이 플랫폼에서는 변환을 지원하지 않습니다",
};

/** 진행 막대에 쓸 비율. 총 크기를 모르면 null. */
export function progressRatio(
  received: number,
  total: number | null,
): number | null {
  if (total === null || total <= 0) return null;

  return Math.min(1, received / total);
}
