import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { CategoryNav } from "./CategoryNav";

describe("CategoryNav", () => {
  // ── happy path ───────────────────────────────────────────────

  it("지금 보고 있는 분류를 현재 위치로 알린다", () => {
    render(<CategoryNav active="document" onSelect={vi.fn()} />);

    expect(screen.getByRole("link", { name: /문서/ })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  // ── edge cases ───────────────────────────────────────────────

  it("아직 만들지 않은 분류는 준비 중이라고 말하고 누를 수 없다", () => {
    // 눌리는데 아무 일도 안 일어나는 메뉴가 제일 나쁘다.
    render(<CategoryNav active="document" onSelect={vi.fn()} />);

    const image = screen.getByRole("link", { name: /이미지/ });
    expect(image).toHaveAttribute("aria-disabled", "true");
    expect(image).toHaveTextContent("준비 중");
  });

  it("준비 중인 분류를 눌러도 화면이 바뀌지 않는다", async () => {
    const onSelect = vi.fn();
    render(<CategoryNav active="document" onSelect={onSelect} />);

    await userEvent.click(screen.getByRole("link", { name: /미디어/ }));

    expect(onSelect).not.toHaveBeenCalled();
  });

  it("네 분류를 모두 보여 준다", () => {
    // 지금 못 하는 일도 보여 준다 — 로드맵이 곧 기대 관리다.
    render(<CategoryNav active="document" onSelect={vi.fn()} />);

    expect(screen.getAllByRole("link")).toHaveLength(4);
  });
});
