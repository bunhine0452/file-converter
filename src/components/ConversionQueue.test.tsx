import { render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { mockTauri, resetTauriMocks } from "@/test/tauri";
import type { ConversionItem } from "@/hooks/useConversionQueue";
import { ConversionQueue } from "./ConversionQueue";

beforeEach(() => {
  mockTauri();
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
