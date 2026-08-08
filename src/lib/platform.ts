/**
 * 플랫폼 판별.
 *
 * macOS 는 타이틀바를 투명하게 깔고 신호등 버튼만 띄우므로(`titleBarStyle: "Overlay"`)
 * 창 왼쪽 위에 그 버튼들이 앉을 자리를 비워 줘야 한다. Windows 는 시스템 타이틀바를
 * 그대로 쓰므로 비울 필요가 없다 — 같은 여백을 주면 위쪽이 휑해진다.
 */

export type Platform = "mac" | "windows" | "other";

export function detectPlatform(userAgent: string): Platform {
  // iPhone/iPad 의 UA 에도 "Mac OS X" 가 들어간다 — Macintosh 로 판별한다.
  if (userAgent.includes("Macintosh")) return "mac";
  if (userAgent.includes("Windows")) return "windows";

  return "other";
}

/** 문서 루트에 플랫폼을 표시해 CSS 가 창 여백을 조절할 수 있게 한다. */
export function applyPlatform(platform: Platform): void {
  document.documentElement.dataset.platform = platform;
}
