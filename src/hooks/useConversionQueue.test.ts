import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { emit } from "@tauri-apps/api/event";
import { mockTauri, resetTauriMocks } from "@/test/tauri";
import { JOB_EVENT } from "@/lib/jobs";
import { useConversionQueue } from "./useConversionQueue";

beforeEach(() => {
  mockTauri();
});

afterEach(() => {
  resetTauriMocks();
});

async function renderQueue() {
  const view = renderHook(() => useConversionQueue());
  // 구독이 비동기라 등록이 끝날 때까지 한 틱 흘려보낸다.
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
  return view;
}

async function sendJobEvent(payload: unknown) {
  await act(async () => {
    await emit(JOB_EVENT, payload);
  });
}

describe("useConversionQueue", () => {
  // ── happy path ───────────────────────────────────────────────

  it("추적을 시작한 작업이 대기 상태로 목록에 뜬다", async () => {
    const { result } = await renderQueue();

    act(() => result.current.track(1, "/tmp/계약서.hwp", "/out/계약서.pdf"));

    expect(result.current.items).toEqual([
      {
        id: 1,
        source: "/tmp/계약서.hwp",
        outPath: "/out/계약서.pdf",
        name: "계약서.hwp",
        status: "queued",
        progress: 0,
        message: null,
        note: null,
      },
    ]);
  });

  it("진행률과 완료가 목록에 반영된다", async () => {
    const { result } = await renderQueue();
    act(() => result.current.track(1, "/tmp/a.hwp", "/out/a.pdf"));

    await sendJobEvent({ kind: "progress", id: 1, progress: 40 });
    expect(result.current.items[0].progress).toBe(40);

    await sendJobEvent({ kind: "completed", id: 1 });
    expect(result.current.items[0].status).toBe("completed");
    expect(result.current.items[0].progress).toBe(100);
  });

  // ── edge cases ───────────────────────────────────────────────

  it("실패 사유를 보존한다", async () => {
    const { result } = await renderQueue();
    act(() => result.current.track(1, "/tmp/a.hwp", "/out/a.pdf"));

    await sendJobEvent({
      kind: "failed",
      id: 1,
      message: "암호가 설정된 한글 문서입니다.",
    });

    expect(result.current.items[0].status).toBe("failed");
    expect(result.current.items[0].message).toBe(
      "암호가 설정된 한글 문서입니다.",
    );
  });

  it("추적하지 않은 작업의 이벤트는 무시한다", async () => {
    const { result } = await renderQueue();

    await sendJobEvent({ kind: "progress", id: 99, progress: 50 });

    expect(result.current.items).toHaveLength(0);
  });

  it("등록보다 먼저 도착한 이벤트도 등록 직후 반영된다", async () => {
    // 변환이 즉시 실패하면 커맨드가 id 를 돌려주기도 전에 이벤트가 온다.
    const { result } = await renderQueue();

    await sendJobEvent({
      kind: "failed",
      id: 7,
      message: "암호가 설정된 한글 문서입니다.",
    });
    act(() => result.current.track(7, "/tmp/암호.hwp", "/out/암호.pdf"));

    expect(result.current.items[0].status).toBe("failed");
    expect(result.current.items[0].message).toBe(
      "암호가 설정된 한글 문서입니다.",
    );
  });

  it("여러 작업이 등록 순서대로 유지된다", async () => {
    const { result } = await renderQueue();

    act(() => {
      result.current.track(1, "/tmp/a.hwp", "/out/a.pdf");
      result.current.track(2, "/tmp/b.hwpx", "/out/b.pdf");
    });

    expect(result.current.items.map((item) => item.id)).toEqual([1, 2]);
  });

  it("취소 요청과 확정이 상태로 드러난다", async () => {
    const { result } = await renderQueue();
    act(() => result.current.track(1, "/tmp/a.hwp", "/out/a.pdf"));

    await sendJobEvent({ kind: "cancelling", id: 1 });
    expect(result.current.items[0].status).toBe("cancelling");

    await sendJobEvent({ kind: "cancelled", id: 1 });
    expect(result.current.items[0].status).toBe("cancelled");
  });

  it("경로가 아닌 파일 이름만 표시용으로 뽑는다", async () => {
    const { result } = await renderQueue();

    act(() =>
      result.current.track(1, "/Users/kim/문서/보고서.v2.hwp", "/out/a.pdf"),
    );

    expect(result.current.items[0].name).toBe("보고서.v2.hwp");
  });

  // ── 프리플라이트 안내 ────────────────────────────────────────

  it("안내 이벤트가 항목에 남는다", async () => {
    const { result } = await renderQueue();
    act(() => result.current.track(1, "/tmp/배포용.hwp", "/out/배포용.pdf"));

    await sendJobEvent({
      kind: "note",
      id: 1,
      message: "배포용(읽기 전용) 한글 문서입니다.",
    });

    expect(result.current.items[0].note).toBe(
      "배포용(읽기 전용) 한글 문서입니다.",
    );
  });

  it("안내는 상태나 진행률을 바꾸지 않는다", async () => {
    const { result } = await renderQueue();
    act(() => result.current.track(1, "/tmp/a.hwp", "/out/a.pdf"));
    await sendJobEvent({ kind: "progress", id: 1, progress: 5 });

    await sendJobEvent({ kind: "note", id: 1, message: "안내" });

    // 안내를 실패처럼 보이게 하면 사용자는 변환이 멈춘 줄 안다.
    expect(result.current.items[0].status).toBe("running");
    expect(result.current.items[0].progress).toBe(5);
    expect(result.current.items[0].message).toBeNull();
  });

  it("완료된 뒤에도 안내가 남는다", async () => {
    // 완료가 안내를 지워버리면 경고를 볼 기회가 사라진다.
    const { result } = await renderQueue();
    act(() => result.current.track(1, "/tmp/배포용.hwp", "/out/배포용.pdf"));

    await sendJobEvent({ kind: "note", id: 1, message: "안내" });
    await sendJobEvent({ kind: "completed", id: 1 });

    expect(result.current.items[0].status).toBe("completed");
    expect(result.current.items[0].note).toBe("안내");
  });

  it("등록보다 먼저 온 안내도 반영된다", async () => {
    const { result } = await renderQueue();

    await sendJobEvent({ kind: "note", id: 7, message: "안내" });
    act(() => result.current.track(7, "/tmp/배포용.hwp", "/out/배포용.pdf"));

    expect(result.current.items[0].note).toBe("안내");
  });

  // ── 늦게 도착한 진행률 ────────────────────────────────────────

  it("완료된 항목은 늦게 온 진행 이벤트로 되돌아가지 않는다", async () => {
    // 하트비트의 마지막 추정치는 완료 뒤에 도착할 수 있다.
    const { result } = await renderQueue();
    act(() => result.current.track(1, "/tmp/a.hwp", "/out/a.pdf"));

    await sendJobEvent({ kind: "completed", id: 1 });
    await sendJobEvent({ kind: "progress", id: 1, progress: 40 });

    expect(result.current.items[0].status).toBe("completed");
    expect(result.current.items[0].progress).toBe(100);
  });

  it("취소 중인 항목은 진행 이벤트로 다시 변환 중이 되지 않는다", async () => {
    // 취소를 눌렀는데 막대가 다시 오르면 취소가 씹힌 것처럼 보인다.
    const { result } = await renderQueue();
    act(() => result.current.track(1, "/tmp/a.hwp", "/out/a.pdf"));

    await sendJobEvent({ kind: "cancelling", id: 1 });
    await sendJobEvent({ kind: "progress", id: 1, progress: 60 });

    expect(result.current.items[0].status).toBe("cancelling");
  });

  it("실패한 항목도 늦은 진행률에 되살아나지 않는다", async () => {
    const { result } = await renderQueue();
    act(() => result.current.track(1, "/tmp/a.hwp", "/out/a.pdf"));

    await sendJobEvent({ kind: "failed", id: 1, message: "변환 실패" });
    await sendJobEvent({ kind: "progress", id: 1, progress: 80 });

    expect(result.current.items[0].status).toBe("failed");
    expect(result.current.items[0].message).toBe("변환 실패");
  });

  it("언마운트하면 이벤트를 더 받지 않는다", async () => {
    const { result, unmount } = await renderQueue();
    act(() => result.current.track(1, "/tmp/a.hwp", "/out/a.pdf"));
    const before = result.current.items[0].progress;

    unmount();
    await sendJobEvent({ kind: "progress", id: 1, progress: 80 });

    expect(result.current.items[0].progress).toBe(before);
  });
});
