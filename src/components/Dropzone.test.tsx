import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { emit } from "@tauri-apps/api/event";
import {
  mockTauri,
  resetTauriMocks,
  type IpcCall,
  type IpcHandler,
} from "@/test/tauri";
import { Dropzone } from "./Dropzone";

let ipcCalls: IpcCall[] = [];

/** 커맨드별 응답을 갈아끼우면서 호출 기록도 남긴다. */
function respondTo(handler: IpcHandler) {
  mockTauri(handler, ipcCalls);
}

beforeEach(() => {
  ipcCalls = [];
  respondTo(() => undefined);
});

afterEach(() => {
  resetTauriMocks();
});

/** 구독이 끝날 때까지 한 틱 흘려보낸다 (onDragDropEvent 는 비동기 구독). */
async function renderDropzone(
  props: Partial<Parameters<typeof Dropzone>[0]> = {},
) {
  const onFiles = props.onFiles ?? vi.fn();
  const view = render(<Dropzone onFiles={onFiles} disabled={props.disabled} />);
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
  return { ...view, onFiles };
}

async function dragEnter(paths: string[]) {
  await act(async () => {
    await emit("tauri://drag-enter", { paths, position: { x: 1, y: 1 } });
  });
}

async function drop(paths: string[]) {
  await act(async () => {
    await emit("tauri://drag-drop", { paths, position: { x: 1, y: 1 } });
  });
}

describe("Dropzone", () => {
  // ── happy path ───────────────────────────────────────────────

  it("드롭 안내와 파일 선택 버튼을 보여준다", async () => {
    await renderDropzone();

    expect(screen.getByText(/여기에 놓으세요/)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "파일 선택" }),
    ).toBeInTheDocument();
  });

  it("드롭된 한글 문서를 상위로 넘긴다", async () => {
    const { onFiles } = await renderDropzone();

    await drop(["/tmp/계약서.hwp"]);

    expect(onFiles).toHaveBeenCalledWith(["/tmp/계약서.hwp"]);
  });

  it("파일 선택 버튼은 한글 문서 필터로 다이얼로그를 연다", async () => {
    const { onFiles } = await renderDropzone();
    respondTo((command) =>
      command === "plugin:dialog|open" ? ["/tmp/고른.hwpx"] : undefined,
    );

    await userEvent.click(screen.getByRole("button", { name: "파일 선택" }));

    const call = ipcCalls.find((c) => c.command === "plugin:dialog|open");
    expect(call).toBeDefined();
    expect(call?.payload).toMatchObject({
      options: {
        multiple: true,
        directory: false,
        filters: [{ name: "한글 문서", extensions: ["hwp", "hwpx"] }],
      },
    });
    expect(onFiles).toHaveBeenCalledWith(["/tmp/고른.hwpx"]);
  });

  // ── edge cases ───────────────────────────────────────────────

  it("드래그가 올라오면 놓으라는 안내로 바뀐다", async () => {
    await renderDropzone();

    await dragEnter(["/tmp/a.hwp"]);

    expect(screen.getByText(/놓으면 변환을 시작합니다/)).toBeInTheDocument();
  });

  it("지원하지 않는 파일을 끌고 오면 경고를 보여준다", async () => {
    await renderDropzone();

    await dragEnter(["/tmp/사진.png"]);

    expect(
      screen.getByText(/한글 문서.*만 변환할 수 있습니다/),
    ).toBeInTheDocument();
  });

  it("상태 안내는 스크린리더에 알려진다", async () => {
    await renderDropzone();

    expect(screen.getByRole("status")).toBeInTheDocument();
  });

  it("다이얼로그를 취소하면 아무 일도 일어나지 않는다", async () => {
    const { onFiles } = await renderDropzone();
    respondTo((command) =>
      command === "plugin:dialog|open" ? null : undefined,
    );

    await userEvent.click(screen.getByRole("button", { name: "파일 선택" }));

    expect(onFiles).not.toHaveBeenCalled();
  });

  it("비활성 상태면 버튼을 누를 수 없고 드롭도 무시한다", async () => {
    const { onFiles } = await renderDropzone({ disabled: true });

    expect(screen.getByRole("button", { name: "파일 선택" })).toBeDisabled();

    await drop(["/tmp/a.hwp"]);

    expect(onFiles).not.toHaveBeenCalled();
  });
});
