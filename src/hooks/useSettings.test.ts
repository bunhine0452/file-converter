import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { mockTauri, resetTauriMocks, type IpcCall } from "@/test/tauri";
import { DEFAULT_SETTINGS } from "@/lib/settings";
import { useSettings } from "./useSettings";

let calls: IpcCall[];

function mockWith(stored: unknown) {
  mockTauri(
    (command) => (command === "get_settings" ? stored : undefined),
    calls,
  );
}

beforeEach(() => {
  calls = [];
  mockWith(DEFAULT_SETTINGS);
});

afterEach(() => {
  resetTauriMocks();
  delete document.documentElement.dataset.theme;
});

async function renderSettings() {
  const view = renderHook(() => useSettings());
  await waitFor(() => expect(view.result.current.isLoading).toBe(false));
  return view;
}

describe("useSettings", () => {
  // ── happy path ───────────────────────────────────────────────

  it("저장돼 있던 설정을 불러온다", async () => {
    mockWith({ ...DEFAULT_SETTINGS, saveMode: "sameAsSource" });

    const { result } = await renderSettings();

    expect(result.current.settings.saveMode).toBe("sameAsSource");
  });

  it("값을 바꾸면 화면에 바로 반영하고 저장한다", async () => {
    const { result } = await renderSettings();

    await act(async () => {
      await result.current.update({ nameSuffix: "_변환" });
    });

    expect(result.current.settings.nameSuffix).toBe("_변환");
    const saved = calls.find((call) => call.command === "save_settings");
    expect(saved?.payload).toMatchObject({
      settings: { nameSuffix: "_변환" },
    });
  });

  it("바꾸지 않은 값은 그대로 유지된다", async () => {
    mockWith({ ...DEFAULT_SETTINGS, outputDir: "/out" });
    const { result } = await renderSettings();

    await act(async () => {
      await result.current.update({ theme: "dark" });
    });

    expect(result.current.settings.outputDir).toBe("/out");
  });

  // ── edge cases ───────────────────────────────────────────────

  it("설정을 못 읽어도 기본값으로 시작한다", async () => {
    // 설정 하나 때문에 앱이 멈추면 사용자는 복구할 방법이 없다.
    mockWith(undefined);

    const { result } = await renderSettings();

    expect(result.current.settings).toEqual(DEFAULT_SETTINGS);
  });

  it("고른 테마를 문서에 적용한다", async () => {
    mockWith({ ...DEFAULT_SETTINGS, theme: "dark" });

    await renderSettings();

    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("테마를 바꾸면 문서 표시도 따라 바뀐다", async () => {
    const { result } = await renderSettings();

    await act(async () => {
      await result.current.update({ theme: "dark" });
    });

    expect(document.documentElement.dataset.theme).toBe("dark");
  });
});
