/**
 * 테마 적용.
 *
 * CSS 는 `:root[data-theme="dark"]` 하나만 본다 — "시스템"을 실제 밝기로 바꾸는 일은
 * 여기서 한다. 미디어 쿼리와 명시 선택을 둘 다 CSS 에 두면 다크 토큰을 두 벌 적어야 하고,
 * 그 순간부터 둘이 어긋나기 시작한다.
 */

export type ThemeSetting = "system" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";

const DARK_QUERY = "(prefers-color-scheme: dark)";

/** 설정과 OS 상태를 합쳐 실제로 그릴 테마를 정한다. */
export function resolveTheme(
  setting: ThemeSetting,
  prefersDark: boolean,
): ResolvedTheme {
  if (setting === "system") return prefersDark ? "dark" : "light";

  return setting;
}

/** 지금 OS 가 다크인가. */
export function prefersDark(): boolean {
  return window.matchMedia?.(DARK_QUERY).matches ?? false;
}

export function applyTheme(theme: ResolvedTheme): void {
  document.documentElement.dataset.theme = theme;
}

/** OS 테마가 바뀌면 알려 준다. 반환값을 부르면 구독이 끊긴다. */
export function watchSystemTheme(onChange: () => void): () => void {
  const query = window.matchMedia?.(DARK_QUERY);
  if (!query) return () => {};

  query.addEventListener("change", onChange);

  return () => query.removeEventListener("change", onChange);
}
