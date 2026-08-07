import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { listen } from "@tauri-apps/api/event";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

beforeEach(() => {
  vi.mocked(listen)
    .mockReset()
    .mockResolvedValue(vi.fn(() => {}));
});

describe("App", () => {
  it("앱 제목을 표시한다", () => {
    render(<App />);

    expect(
      screen.getByRole("heading", { name: "파일 변환기" }),
    ).toBeInTheDocument();
  });

  it("shadcn Button 기반 데모 버튼을 렌더한다", () => {
    render(<App />);

    const button = screen.getByRole("button", { name: "데모 작업 시작" });

    expect(button).toBeInTheDocument();
    // Tailwind v4 토큰이 shadcn Button variant를 통해 실제 클래스로 적용됐는지 확인
    expect(button).toHaveClass("bg-primary");
  });

  it("진행 이벤트 브리지 데모를 포함한다", () => {
    render(<App />);

    expect(screen.getByRole("progressbar")).toBeInTheDocument();
    expect(screen.getByTestId("demo-progress")).toHaveTextContent("0%");
  });
});
