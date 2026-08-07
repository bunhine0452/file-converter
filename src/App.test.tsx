import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { emit } from "@tauri-apps/api/event";
import { mockTauri, resetTauriMocks, type IpcCall } from "@/test/tauri";
import App from "./App";

let ipcCalls: IpcCall[] = [];

const READY_STATUS = {
  state: "ready",
  version: "26.2.5.2",
  exePath: "/data/lo/soffice",
  managed: true,
};

/** 저장 다이얼로그는 경로를 돌려주고, 변환 커맨드는 작업 id 를 돌려준다. */
function respondNormally(savePath: string | null = "/out/계약서.pdf") {
  mockTauri((command) => {
    switch (command) {
      case "get_runtime_status":
        return READY_STATUS;
      case "plugin:dialog|save":
        return savePath;
      case "convert_hwp":
        return 1;
      default:
        return undefined;
    }
  }, ipcCalls);
}

beforeEach(() => {
  ipcCalls = [];
  respondNormally();
});

afterEach(() => {
  resetTauriMocks();
});

async function renderApp() {
  const view = render(<App />);
  // 런타임 상태 조회와 드롭 구독이 모두 비동기다.
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
  return view;
}

async function drop(paths: string[]) {
  await act(async () => {
    await emit("tauri://drag-drop", { paths, position: { x: 1, y: 1 } });
  });
}

function calledCommands() {
  return ipcCalls.map((call) => call.command);
}

describe("App", () => {
  // ── happy path ───────────────────────────────────────────────

  it("제목과 로컬 변환 원칙을 밝힌다", async () => {
    await renderApp();

    expect(
      screen.getByRole("heading", { name: "파일 변환기" }),
    ).toBeInTheDocument();
    expect(screen.getByText(/이 기기를 떠나지 않습니다/)).toBeInTheDocument();
  });

  it("런타임 상태와 드롭 영역을 함께 보여준다", async () => {
    await renderApp();

    expect(
      await screen.findByText(/변환 준비가 끝났습니다/),
    ).toBeInTheDocument();
    expect(screen.getByText(/여기에 놓으세요/)).toBeInTheDocument();
  });

  it("드롭하면 저장 위치를 묻고 변환을 시작해 목록에 올린다", async () => {
    await renderApp();

    await drop(["/tmp/계약서.hwp"]);

    await waitFor(() =>
      expect(calledCommands()).toContain("plugin:dialog|save"),
    );
    await waitFor(() => expect(calledCommands()).toContain("convert_hwp"));
    expect(
      await screen.findByRole("list", { name: "변환 목록" }),
    ).toBeInTheDocument();
    expect(screen.getByText("계약서.hwp")).toBeInTheDocument();
  });

  // ── edge cases ───────────────────────────────────────────────

  it("저장 위치를 취소하면 변환하지 않는다", async () => {
    respondNormally(null);
    await renderApp();

    await drop(["/tmp/계약서.hwp"]);

    await waitFor(() =>
      expect(calledCommands()).toContain("plugin:dialog|save"),
    );
    expect(calledCommands()).not.toContain("convert_hwp");
  });

  it("변환 실패 사유를 목록에 그대로 보여준다", async () => {
    await renderApp();
    await drop(["/tmp/암호.hwp"]);
    await waitFor(() => expect(calledCommands()).toContain("convert_hwp"));

    await act(async () => {
      await emit("job://event", {
        kind: "failed",
        id: 1,
        message: "암호가 설정된 한글 문서입니다.",
      });
    });

    expect(
      await screen.findByText("암호가 설정된 한글 문서입니다."),
    ).toBeInTheDocument();
  });

  it("완료되면 저장 위치를 열 수 있다", async () => {
    await renderApp();
    await drop(["/tmp/계약서.hwp"]);
    await waitFor(() => expect(calledCommands()).toContain("convert_hwp"));

    await act(async () => {
      await emit("job://event", { kind: "completed", id: 1 });
    });

    expect(
      await screen.findByRole("button", { name: "저장 위치 열기" }),
    ).toBeInTheDocument();
  });

  it("변환할 것이 없으면 목록을 그리지 않는다", async () => {
    await renderApp();

    expect(
      screen.queryByRole("list", { name: "변환 목록" }),
    ).not.toBeInTheDocument();
  });
});
