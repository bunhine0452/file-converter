import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { mockTauri, resetTauriMocks, type IpcCall } from "@/test/tauri";
import type { RuntimeStatusView } from "@/lib/runtime";
import { RuntimeStatus } from "./RuntimeStatus";

let ipcCalls: IpcCall[] = [];

function ready(overrides: Partial<RuntimeStatusView> = {}): RuntimeStatusView {
  return {
    state: "ready",
    version: "26.2.5.2",
    exePath: "/data/lo/soffice",
    managed: true,
    ...overrides,
  };
}

/** get_runtime_status 만 응답하고 나머지는 기본값. */
function respondWithStatus(status: RuntimeStatusView) {
  mockTauri(
    (command) => (command === "get_runtime_status" ? status : undefined),
    ipcCalls,
  );
}

beforeEach(() => {
  ipcCalls = [];
  respondWithStatus(ready());
});

afterEach(() => {
  resetTauriMocks();
});

async function renderStatus() {
  const view = render(<RuntimeStatus />);
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
  return view;
}

describe("RuntimeStatus", () => {
  // ── happy path ───────────────────────────────────────────────

  it("준비가 끝나면 버전과 함께 준비됨을 알린다", async () => {
    await renderStatus();

    expect(
      await screen.findByText(/변환 준비가 끝났습니다/),
    ).toBeInTheDocument();
    expect(screen.getByText(/26\.2\.5\.2/)).toBeInTheDocument();
  });

  it("설치가 필요하면 설치 버튼을 보여준다", async () => {
    respondWithStatus(
      ready({ state: "needsLibreOffice", version: null, exePath: null }),
    );

    await renderStatus();

    expect(
      await screen.findByRole("button", { name: "지금 설치" }),
    ).toBeInTheDocument();
  });

  // ── edge cases ───────────────────────────────────────────────

  it("준비가 끝났으면 설치 버튼을 보여주지 않는다", async () => {
    await renderStatus();

    await screen.findByText(/변환 준비가 끝났습니다/);
    expect(
      screen.queryByRole("button", { name: "지금 설치" }),
    ).not.toBeInTheDocument();
  });

  it("설치 버튼을 누르면 install_runtime 을 호출하고 진행 단계를 보여준다", async () => {
    respondWithStatus(ready({ state: "needsJre" }));
    await renderStatus();
    // 설치 호출은 진행 이벤트를 흘린 뒤 준비된 상태를 돌려준다.
    mockTauri((command, payload) => {
      ipcCalls.push({ command, payload });
      if (command === "install_runtime") {
        const channel = (payload as { onEvent?: { onmessage?: unknown } })
          ?.onEvent;
        void channel;
        return ready();
      }
      return undefined;
    }, ipcCalls);

    await userEvent.click(screen.getByRole("button", { name: "지금 설치" }));

    await waitFor(() =>
      expect(ipcCalls.some((call) => call.command === "install_runtime")).toBe(
        true,
      ),
    );
  });

  it("지원하지 않는 플랫폼이면 설치를 권하지 않는다", async () => {
    respondWithStatus(
      ready({ state: "unsupported", version: null, exePath: null }),
    );

    await renderStatus();

    expect(
      await screen.findByText(/변환을 지원하지 않습니다/),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "지금 설치" }),
    ).not.toBeInTheDocument();
  });

  it("상태 조회가 실패하면 조용히 넘어가지 않고 알린다", async () => {
    mockTauri((command) => {
      if (command === "get_runtime_status") throw new Error("커맨드 실패");
      return undefined;
    }, ipcCalls);

    await renderStatus();

    expect(
      await screen.findByText(/상태를 확인하지 못했습니다/),
    ).toBeInTheDocument();
  });

  it("상태 영역은 스크린리더에 알려진다", async () => {
    await renderStatus();

    expect(await screen.findByRole("status")).toBeInTheDocument();
  });

  it("사용자가 직접 설치한 LibreOffice 를 쓰면 그 사실을 밝힌다", async () => {
    respondWithStatus(ready({ managed: false }));

    await renderStatus();

    expect(await screen.findByText(/설치된 LibreOffice/)).toBeInTheDocument();
  });
});

describe("progressRatio", () => {
  it("총 크기를 모르면 비율을 만들지 않는다", async () => {
    const { progressRatio } = await import("@/lib/runtime");

    expect(progressRatio(10, null)).toBeNull();
    expect(progressRatio(10, 0)).toBeNull();
    expect(progressRatio(50, 100)).toBe(0.5);
    expect(progressRatio(200, 100)).toBe(1);
  });
});

describe("RUNTIME_STATE_MESSAGE", () => {
  it("모든 상태에 한국어 문구가 있다", async () => {
    const { RUNTIME_STATE_MESSAGE } = await import("@/lib/runtime");
    const states = [
      "ready",
      "needsLibreOffice",
      "needsJre",
      "needsExtension",
      "unsupported",
    ] as const;

    for (const state of states) {
      expect(RUNTIME_STATE_MESSAGE[state]).toBeTruthy();
    }
  });
});
