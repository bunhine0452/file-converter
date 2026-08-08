import { afterEach, describe, expect, it } from "vitest";
import { applyTheme, resolveTheme } from "./theme";

afterEach(() => {
  delete document.documentElement.dataset.theme;
});

describe("resolveTheme", () => {
  // ── happy path ───────────────────────────────────────────────

  it("시스템 설정이면 OS 를 따른다", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
  });

  // ── edge cases ───────────────────────────────────────────────

  it("직접 고른 테마는 OS 설정을 이긴다", () => {
    // 사용자가 라이트를 골랐으면 OS 가 다크여도 라이트다.
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
  });
});

describe("applyTheme", () => {
  it("문서 루트에 테마를 표시한다", () => {
    applyTheme("dark");

    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("다시 적용하면 이전 테마를 덮어쓴다", () => {
    applyTheme("dark");
    applyTheme("light");

    expect(document.documentElement.dataset.theme).toBe("light");
  });
});
