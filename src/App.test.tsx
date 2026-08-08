import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { emit } from "@tauri-apps/api/event";
import { mockTauri, resetTauriMocks, type IpcCall } from "@/test/tauri";
import { DEFAULT_SETTINGS } from "@/lib/settings";
import App from "./App";

let ipcCalls: IpcCall[] = [];

const READY_STATUS = {
  state: "ready",
  version: "26.2.5.2",
  exePath: "/data/lo/soffice",
  managed: true,
};

/** 저장 다이얼로그는 경로를 돌려주고, 변환 커맨드는 작업 id 를 돌려준다. */
function respondNormally(
  savePath: string | null = "/out/계약서.pdf",
  settings: Record<string, unknown> = {},
) {
  mockTauri((command) => {
    switch (command) {
      case "get_runtime_status":
        return READY_STATUS;
      case "get_settings":
        return { ...DEFAULT_SETTINGS, ...settings };
      case "plugin:dialog|save":
        return savePath;
      case "plugin:dialog|open":
        return "/out";
      case "plan_output_path":
        return "/out/planned.pdf";
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

  it("여러 파일을 드롭하면 폴더를 한 번만 묻는다", async () => {
    // 파일마다 저장 대화상자를 띄우면 10개 드롭에 10번 답해야 한다.
    await renderApp();

    await drop(["/tmp/가.hwp", "/tmp/나.hwp", "/tmp/다.hwp"]);

    await waitFor(() =>
      expect(
        calledCommands().filter((command) => command === "convert_hwp"),
      ).toHaveLength(3),
    );
    expect(
      calledCommands().filter((command) => command === "plugin:dialog|save"),
    ).toHaveLength(0);
    expect(
      calledCommands().filter((command) => command === "plugin:dialog|open"),
    ).toHaveLength(1);
  });

  it("일괄 저장 경로는 코어가 정한다 (덮어쓰기 방지)", async () => {
    await renderApp();

    await drop(["/tmp/가.hwp", "/tmp/나.hwp"]);

    await waitFor(() => expect(calledCommands()).toContain("plan_output_path"));
    expect(
      calledCommands().filter((command) => command === "plan_output_path"),
    ).toHaveLength(2);
  });

  it("원본과 같은 폴더 설정이면 아무것도 묻지 않는다", async () => {
    // 저장 위치를 정해 둔 사용자에게 매번 묻는 것은 설정을 무시하는 것이다.
    respondNormally("/out/계약서.pdf", { saveMode: "sameAsSource" });
    await renderApp();

    await drop(["/tmp/문서/계약서.hwp"]);

    await waitFor(() => expect(calledCommands()).toContain("convert_hwp"));
    expect(calledCommands()).not.toContain("plugin:dialog|save");
    expect(calledCommands()).not.toContain("plugin:dialog|open");
    const convert = ipcCalls.find((call) => call.command === "convert_hwp");
    expect(JSON.stringify(convert?.payload)).toContain("/tmp/문서");
  });

  it("지정 폴더 설정이면 그 폴더로 바로 변환한다", async () => {
    respondNormally("/out/계약서.pdf", {
      saveMode: "fixedFolder",
      outputDir: "/모아둔곳",
    });
    await renderApp();

    await drop(["/tmp/계약서.hwp"]);

    await waitFor(() => expect(calledCommands()).toContain("convert_hwp"));
    expect(calledCommands()).not.toContain("plugin:dialog|save");
    const planned = ipcCalls.find(
      (call) => call.command === "plan_output_path",
    );
    expect(JSON.stringify(planned?.payload)).toContain("/모아둔곳");
  });

  it("지정 폴더인데 폴더를 아직 안 골랐으면 물어본다", async () => {
    // 설정이 반쯤 비어 있다고 말없이 아무 데나 저장하면 파일을 잃어버린다.
    respondNormally("/out/계약서.pdf", {
      saveMode: "fixedFolder",
      outputDir: null,
    });
    await renderApp();

    await drop(["/tmp/계약서.hwp"]);

    await waitFor(() =>
      expect(calledCommands()).toContain("plugin:dialog|save"),
    );
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
