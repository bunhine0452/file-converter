import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { JobEvent } from "@/lib/jobs";
import { JobProgressDemo } from "./JobProgressDemo";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

const unlisten = vi.fn(() => {});

/** 테스트에서 Rust 쪽 이벤트를 흉내내 밀어 넣는다. */
let emit: (event: JobEvent) => void;

beforeEach(() => {
  vi.mocked(invoke).mockReset().mockResolvedValue(1);
  unlisten.mockReset();
  vi.mocked(listen).mockReset();
  vi.mocked(listen).mockImplementation(async (_name, callback) => {
    const push = callback as (event: { payload: JobEvent }) => void;
    emit = (event) => act(() => push({ payload: event }));
    return unlisten;
  });
});

async function startJob() {
  await userEvent.click(screen.getByRole("button", { name: "데모 작업 시작" }));
}

describe("JobProgressDemo", () => {
  // ── happy path ─────────────────────────────────────────────

  it("시작 전에는 대기 상태를 보여준다", () => {
    render(<JobProgressDemo />);

    expect(
      screen.getByRole("button", { name: "데모 작업 시작" }),
    ).toBeInTheDocument();
    expect(screen.getByTestId("demo-progress")).toHaveTextContent("0%");
  });

  it("시작을 누르면 커맨드를 호출하고 실행 상태가 된다", async () => {
    render(<JobProgressDemo />);

    await startJob();

    expect(invoke).toHaveBeenCalledWith("start_demo_job");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "취소" })).toBeInTheDocument(),
    );
  });

  it("progress 이벤트가 진행률 표시를 갱신한다", async () => {
    render(<JobProgressDemo />);
    await startJob();
    await waitFor(() => expect(listen).toHaveBeenCalled());

    emit({ kind: "progress", id: 1, progress: 65 });

    expect(screen.getByTestId("demo-progress")).toHaveTextContent("65%");
    expect(screen.getByRole("progressbar")).toHaveAttribute(
      "aria-valuenow",
      "65",
    );
  });

  it("completed 이벤트가 오면 완료로 표시하고 진행률이 100% 가 된다", async () => {
    render(<JobProgressDemo />);
    await startJob();

    emit({ kind: "completed", id: 1 });

    expect(screen.getByTestId("demo-progress")).toHaveTextContent("100%");
    expect(screen.getByText("완료")).toBeInTheDocument();
  });

  // ── edge cases ─────────────────────────────────────────────

  it("failed 이벤트의 사유를 사용자에게 보여준다", async () => {
    render(<JobProgressDemo />);
    await startJob();

    emit({ kind: "failed", id: 1, message: "암호가 걸린 문서입니다" });

    expect(screen.getByRole("alert")).toHaveTextContent(
      "암호가 걸린 문서입니다",
    );
  });

  it("취소를 누르면 현재 작업 id 로 cancel_job 을 호출한다", async () => {
    render(<JobProgressDemo />);
    await startJob();
    await waitFor(() => screen.getByRole("button", { name: "취소" }));

    await userEvent.click(screen.getByRole("button", { name: "취소" }));

    expect(invoke).toHaveBeenCalledWith("cancel_job", { id: 1 });
  });

  it("다른 작업의 이벤트는 무시한다", async () => {
    render(<JobProgressDemo />);
    await startJob();
    emit({ kind: "progress", id: 1, progress: 30 });

    emit({ kind: "progress", id: 99, progress: 90 });

    expect(screen.getByTestId("demo-progress")).toHaveTextContent("30%");
  });

  it("언마운트하면 이벤트 구독을 해제한다", async () => {
    const view = render(<JobProgressDemo />);
    await waitFor(() => expect(listen).toHaveBeenCalled());

    view.unmount();

    await waitFor(() => expect(unlisten).toHaveBeenCalled());
  });

  it("커맨드 호출이 실패하면 사유를 보여주고 실행 상태로 넘어가지 않는다", async () => {
    vi.mocked(invoke).mockRejectedValueOnce("웹뷰가 닫혔습니다");
    render(<JobProgressDemo />);

    await startJob();

    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent("웹뷰가 닫혔습니다"),
    );
    expect(
      screen.queryByRole("button", { name: "취소" }),
    ).not.toBeInTheDocument();
  });
});
