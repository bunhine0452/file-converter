import { describe, expect, it } from "vitest";
import { detectPlatform } from "./platform";

const MAC =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15";
const WINDOWS =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36 Edg/124.0.0.0";
const LINUX =
  "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

describe("detectPlatform", () => {
  // ── happy path ───────────────────────────────────────────────

  it("mac 과 windows 를 가려낸다", () => {
    // 창 조작 버튼 위치가 달라 여백을 다르게 줘야 한다.
    expect(detectPlatform(MAC)).toBe("mac");
    expect(detectPlatform(WINDOWS)).toBe("windows");
  });

  // ── edge cases ───────────────────────────────────────────────

  it("모르는 플랫폼은 other 로 둔다", () => {
    // 판별 실패를 mac 으로 취급하면 엉뚱한 곳에 여백이 생긴다.
    expect(detectPlatform(LINUX)).toBe("other");
    expect(detectPlatform("")).toBe("other");
  });

  it("아이폰 사파리를 mac 으로 오인하지 않는다", () => {
    expect(
      detectPlatform("Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X)"),
    ).toBe("other");
  });
});
