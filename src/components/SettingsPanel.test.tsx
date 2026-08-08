import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mockTauri, resetTauriMocks, type IpcCall } from "@/test/tauri";
import { DEFAULT_SETTINGS, type Settings } from "@/lib/settings";
import { SettingsPanel } from "./SettingsPanel";

let calls: IpcCall[];

beforeEach(() => {
  calls = [];
  mockTauri((command) => {
    if (command === "get_runtime_status") {
      return {
        state: "ready",
        version: "26.2.5.2",
        exePath: null,
        managed: true,
      };
    }
    if (command === "plugin:dialog|open") return "/Users/kim/변환결과";
    return undefined;
  }, calls);
});

afterEach(() => {
  resetTauriMocks();
});

/**
 * 실제 앱처럼 갱신을 되먹임하는 하네스.
 *
 * 프롭을 고정한 채로 입력을 타이핑하면 제어 컴포넌트라 매 글자가 첫 글자로 덮어써진다 —
 * 그 상태로 세운 기대는 앱에서 일어나지 않는 일을 검사하게 된다.
 */
function renderPanel(initial: Partial<Settings> = {}) {
  const onUpdate = vi.fn();

  function Harness() {
    const [settings, setSettings] = useState<Settings>({
      ...DEFAULT_SETTINGS,
      ...initial,
    });

    return (
      <SettingsPanel
        settings={settings}
        onUpdate={(patch) => {
          onUpdate(patch);
          setSettings((current) => ({ ...current, ...patch }));
        }}
      />
    );
  }

  render(<Harness />);

  return onUpdate;
}

describe("SettingsPanel", () => {
  // ── happy path ───────────────────────────────────────────────

  it("저장 방식을 바꾸면 그 값만 갱신한다", async () => {
    const onUpdate = renderPanel();

    await userEvent.click(
      screen.getByRole("radio", { name: /원본과 같은 폴더/ }),
    );

    expect(onUpdate).toHaveBeenCalledWith({ saveMode: "sameAsSource" });
  });

  it("지금 고른 값이 선택 상태로 보인다", () => {
    renderPanel({ saveMode: "fixedFolder", onConflict: "overwrite" });

    expect(screen.getByRole("radio", { name: /지정한 폴더/ })).toBeChecked();
    expect(screen.getByRole("radio", { name: /덮어쓰기/ })).toBeChecked();
  });

  it("이름 접미사를 입력하면 결과 이름 미리보기가 바뀐다", async () => {
    // 규칙을 글로만 설명하면 무슨 파일이 나올지 모른다.
    const onUpdate = renderPanel();

    await userEvent.type(screen.getByLabelText(/이름 뒤에 붙일 말/), "_변환");

    expect(onUpdate).toHaveBeenLastCalledWith({ nameSuffix: "_변환" });
    expect(screen.getByText("보고서_변환.pdf")).toBeInTheDocument();
  });

  it("미리보기는 실제 접미사를 반영한다", () => {
    renderPanel({ nameSuffix: "_변환" });

    expect(screen.getByText(/보고서_변환\.pdf/)).toBeInTheDocument();
  });

  // ── edge cases ───────────────────────────────────────────────

  it("지정 폴더를 고르면 경로를 저장한다", async () => {
    const onUpdate = renderPanel({ saveMode: "fixedFolder" });

    await userEvent.click(screen.getByRole("button", { name: "폴더 선택" }));

    expect(onUpdate).toHaveBeenCalledWith({
      outputDir: "/Users/kim/변환결과",
      saveMode: "fixedFolder",
    });
  });

  it("지정 폴더 방식이 아니면 폴더 선택을 보여주지 않는다", () => {
    renderPanel({ saveMode: "ask" });

    expect(
      screen.queryByRole("button", { name: "폴더 선택" }),
    ).not.toBeInTheDocument();
  });

  it("런타임 상태도 설정 화면에서 확인할 수 있다", async () => {
    renderPanel();

    expect(
      await screen.findByText(/변환 준비가 끝났습니다/),
    ).toBeInTheDocument();
  });
});
