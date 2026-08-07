import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { emit } from "@tauri-apps/api/event";
import { mockTauri, resetTauriMocks } from "@/test/tauri";
import { HWP_PATTERN, useFileDrop } from "./useFileDrop";

beforeEach(() => {
  mockTauri();
});

afterEach(() => {
  resetTauriMocks();
});

const POSITION = { x: 10, y: 20 };

async function dragEnter(paths: string[]) {
  await act(async () => {
    await emit("tauri://drag-enter", { paths, position: POSITION });
  });
}

async function dragLeave() {
  await act(async () => {
    await emit("tauri://drag-leave", null);
  });
}

async function dragOver() {
  await act(async () => {
    await emit("tauri://drag-over", { position: POSITION });
  });
}

async function drop(paths: string[]) {
  await act(async () => {
    await emit("tauri://drag-drop", { paths, position: POSITION });
  });
}

/**
 * onDragDropEvent 는 4개 이벤트를 비동기로 구독한다.
 * 등록이 끝나기 전에 emit 하면 이벤트가 유실되므로 한 틱 흘려보낸다.
 */
async function renderDrop(onFiles: (paths: string[]) => void) {
  const view = renderHook(() => useFileDrop({ accept: HWP_PATTERN, onFiles }));
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
  return view;
}

describe("useFileDrop", () => {
  // ── happy path ───────────────────────────────────────────────

  it("드롭된 한글 문서 경로를 핸들러에 넘긴다", async () => {
    const onFiles = vi.fn();
    await renderDrop(onFiles);

    await drop(["/Users/kim/계약서.hwp"]);

    expect(onFiles).toHaveBeenCalledWith(["/Users/kim/계약서.hwp"]);
  });

  it("드래그가 들어오면 오버 상태가 되고 나가면 풀린다", async () => {
    const { result } = await renderDrop(vi.fn());

    await dragEnter(["/tmp/a.hwp"]);
    expect(result.current.isOver).toBe(true);

    await dragLeave();
    expect(result.current.isOver).toBe(false);
  });

  // ── edge cases ───────────────────────────────────────────────

  it("지원하지 않는 파일만 끌고 오면 무효 상태로 알린다", async () => {
    const { result } = await renderDrop(vi.fn());

    await dragEnter(["/tmp/사진.png"]);

    expect(result.current.isOver).toBe(true);
    expect(result.current.isInvalid).toBe(true);
  });

  it("지원 파일이 하나라도 있으면 무효가 아니다", async () => {
    const { result } = await renderDrop(vi.fn());

    await dragEnter(["/tmp/사진.png", "/tmp/문서.hwpx"]);

    expect(result.current.isInvalid).toBe(false);
  });

  it("드롭 시 지원하는 확장자만 걸러서 넘긴다", async () => {
    const onFiles = vi.fn();
    await renderDrop(onFiles);

    await drop(["/tmp/a.hwp", "/tmp/b.png", "/tmp/c.HWPX"]);

    expect(onFiles).toHaveBeenCalledWith(["/tmp/a.hwp", "/tmp/c.HWPX"]);
  });

  it("지원 파일이 없으면 핸들러를 부르지 않고 무효로 표시한다", async () => {
    const onFiles = vi.fn();
    const { result } = await renderDrop(onFiles);

    await drop(["/tmp/b.png"]);

    expect(onFiles).not.toHaveBeenCalled();
    expect(result.current.isInvalid).toBe(true);
  });

  it("같은 경로가 연달아 두 번 드롭돼도 한 번만 처리한다", async () => {
    // macOS 에서 드롭 이벤트가 2회 발화하는 미해결 이슈(tauri#14134) 방어
    const onFiles = vi.fn();
    await renderDrop(onFiles);

    await drop(["/tmp/a.hwp"]);
    await drop(["/tmp/a.hwp"]);

    expect(onFiles).toHaveBeenCalledTimes(1);
  });

  it("다른 경로가 드롭되면 다시 처리한다", async () => {
    const onFiles = vi.fn();
    await renderDrop(onFiles);

    await drop(["/tmp/a.hwp"]);
    await drop(["/tmp/b.hwp"]);

    expect(onFiles).toHaveBeenCalledTimes(2);
  });

  it("경로가 없는 over 이벤트에도 오버 상태를 유지한다", async () => {
    const { result } = await renderDrop(vi.fn());

    await dragEnter(["/tmp/a.hwp"]);
    await dragOver();

    expect(result.current.isOver).toBe(true);
    expect(result.current.isInvalid).toBe(false);
  });

  it("드롭이 끝나면 오버·무효 상태가 초기화된다", async () => {
    const { result } = await renderDrop(vi.fn());
    await dragEnter(["/tmp/사진.png"]);

    await drop(["/tmp/a.hwp"]);

    expect(result.current.isOver).toBe(false);
    expect(result.current.isInvalid).toBe(false);
  });

  it("언마운트하면 더 이상 드롭을 처리하지 않는다", async () => {
    const onFiles = vi.fn();
    const { unmount } = await renderDrop(onFiles);

    unmount();
    await drop(["/tmp/a.hwp"]);

    expect(onFiles).not.toHaveBeenCalled();
  });

  it("HWP_PATTERN 은 대소문자와 관계없이 hwp·hwpx 만 받는다", () => {
    expect(HWP_PATTERN.test("a.hwp")).toBe(true);
    expect(HWP_PATTERN.test("a.HWPX")).toBe(true);
    expect(HWP_PATTERN.test("a.hwpx.zip")).toBe(false);
    expect(HWP_PATTERN.test("hwp")).toBe(false);
  });
});
