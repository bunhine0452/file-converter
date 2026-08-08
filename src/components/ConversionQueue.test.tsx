import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mockTauri, resetTauriMocks, type IpcCall } from "@/test/tauri";
import type { ConversionItem } from "@/hooks/useConversionQueue";
import { ConversionQueue } from "./ConversionQueue";

let calls: IpcCall[];

beforeEach(() => {
  calls = [];
  mockTauri(() => undefined, calls);
});

afterEach(() => {
  resetTauriMocks();
});

function item(overrides: Partial<ConversionItem> = {}): ConversionItem {
  return {
    id: 1,
    source: "/tmp/보고서.hwp",
    outPath: "/out/보고서.pdf",
    name: "보고서.hwp",
    status: "completed",
    progress: 100,
    message: null,
    note: null,
    ...overrides,
  };
}

describe("ConversionQueue", () => {
  // ── happy path ───────────────────────────────────────────────

  it("안내가 있으면 화면에 보여준다", () => {
    // 안내를 렌더하지 않으면 코어까지 실어 나른 경고가 마지막에 버려진다.
    render(
      <ConversionQueue
        items={[item({ note: "배포용(읽기 전용) 한글 문서입니다." })]}
      />,
    );

    expect(
      screen.getByText("배포용(읽기 전용) 한글 문서입니다."),
    ).toBeInTheDocument();
  });

  // ── edge cases ───────────────────────────────────────────────

  it("안내가 없으면 아무것도 덧붙이지 않는다", () => {
    render(<ConversionQueue items={[item()]} />);

    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("안내는 실패 메시지와 다른 위상으로 보여준다", () => {
    // 둘 다 같은 스타일이면 "진행됨"과 "실패"를 구분할 수 없다.
    render(
      <ConversionQueue
        items={[
          item({ id: 1, note: "배포용 문서입니다." }),
          item({
            id: 2,
            status: "failed",
            message: "암호가 설정된 문서입니다.",
          }),
        ]}
      />,
    );

    const note = screen.getByText("배포용 문서입니다.");
    const failure = screen.getByText("암호가 설정된 문서입니다.");

    // 안내는 폴라이트 라이브 리전으로 읽히고, 에러 색을 입지 않는다.
    expect(note).toHaveAttribute("role", "status");
    expect(note.className).not.toMatch(/destructive/);
    expect(failure).not.toHaveAttribute("role", "status");
    expect(failure.className).toMatch(/destructive/);
  });

  it("안내와 실패가 한 항목에 함께 있어도 둘 다 보여준다", () => {
    render(
      <ConversionQueue
        items={[
          item({
            status: "failed",
            note: "배포용 문서입니다.",
            message: "변환에 실패했습니다.",
          }),
        ]}
      />,
    );

    expect(screen.getByText("배포용 문서입니다.")).toBeInTheDocument();
    expect(screen.getByText("변환에 실패했습니다.")).toBeInTheDocument();
  });
});

describe("ConversionQueue — 조작", () => {
  // ── 취소 ─────────────────────────────────────────────────────

  it("변환 중인 항목은 취소할 수 있다", async () => {
    // 취소 커맨드는 진작 있었는데 화면에 붙어 있지 않아 아무도 쓸 수 없었다.
    render(
      <ConversionQueue items={[item({ status: "running", progress: 40 })]} />,
    );

    await userEvent.click(screen.getByRole("button", { name: "변환 취소" }));

    expect(calls.map((call) => call.command)).toContain("cancel_job");
    expect(
      calls.find((call) => call.command === "cancel_job")?.payload,
    ).toMatchObject({ id: 1 });
  });

  it("이미 끝난 항목에는 취소 버튼이 없다", () => {
    render(<ConversionQueue items={[item({ status: "completed" })]} />);

    expect(
      screen.queryByRole("button", { name: "변환 취소" }),
    ).not.toBeInTheDocument();
  });

  // ── 결과 열기 ────────────────────────────────────────────────

  it("완료된 항목은 결과 PDF 를 바로 열 수 있다", async () => {
    // 저장 위치만 열어 주면 사용자가 파일을 또 찾아야 한다.
    render(<ConversionQueue items={[item({ status: "completed" })]} />);

    await userEvent.click(screen.getByRole("button", { name: "PDF 열기" }));

    const opener = calls.find((call) => call.command.includes("opener"));
    expect(JSON.stringify(opener?.payload)).toContain("/out/보고서.pdf");
  });

  // ── 진행 표시 ────────────────────────────────────────────────

  it("진행률은 숫자와 막대로 함께 보인다", () => {
    // 대용량 변환은 몇 분씩 걸린다 — 숫자만으로는 살아있는지 알기 어렵다.
    render(
      <ConversionQueue items={[item({ status: "running", progress: 42 })]} />,
    );

    const bar = screen.getByRole("progressbar");
    expect(bar).toHaveAttribute("aria-valuenow", "42");
    expect(screen.getByText(/42%/)).toBeInTheDocument();
  });

  it("끝난 항목에는 진행 막대를 남기지 않는다", () => {
    render(<ConversionQueue items={[item({ status: "completed" })]} />);

    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
  });

  // ── 목록 정리 ────────────────────────────────────────────────

  it("끝난 항목이 있으면 목록을 정리할 수 있다", async () => {
    const onClearFinished = vi.fn();
    render(
      <ConversionQueue
        items={[
          item({ id: 1, status: "completed" }),
          item({ id: 2, status: "running" }),
        ]}
        onClearFinished={onClearFinished}
      />,
    );

    await userEvent.click(
      screen.getByRole("button", { name: "끝난 항목 지우기" }),
    );

    expect(onClearFinished).toHaveBeenCalledOnce();
  });

  it("진행 중인 항목만 있으면 정리 버튼을 보이지 않는다", () => {
    render(
      <ConversionQueue
        items={[item({ status: "running" })]}
        onClearFinished={vi.fn()}
      />,
    );

    expect(
      screen.queryByRole("button", { name: "끝난 항목 지우기" }),
    ).not.toBeInTheDocument();
  });
});
