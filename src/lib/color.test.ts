import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { contrastRatio, parseOklch } from "./color";

/** WCAG 2.2 AA — 본문 텍스트 대비 최소치. */
const AA_TEXT = 4.5;

// Vitest 는 CSS import 를 빈 스텁으로 바꾸므로(`test.css: false` 기본값) 파일을 직접 읽는다.
const css = readFileSync(resolve(process.cwd(), "src/index.css"), "utf8");

/**
 * `index.css` 에서 토큰 값을 그대로 읽는다.
 *
 * 값을 테스트에 복사해 두면 CSS 만 고쳤을 때 테스트가 옛 색을 검사한다 —
 * 통과하는데 화면은 대비 미달인 최악의 조합이 된다.
 */
function token(name: string, scope: "light" | "dark"): string {
  const darkAt = css.indexOf(':root[data-theme="dark"]');
  const block =
    scope === "light"
      ? css.slice(css.indexOf(":root {"), darkAt)
      : css.slice(darkAt, css.indexOf("@theme"));

  const match = block.match(new RegExp(`--${name}:\\s*(oklch\\([^)]*\\))`));
  if (!match) throw new Error(`${scope} 스코프에 --${name} 토큰이 없다`);

  return match[1];
}

function ratio(fg: string, bg: string, scope: "light" | "dark"): number {
  return contrastRatio(
    parseOklch(token(fg, scope)),
    parseOklch(token(bg, scope)),
  );
}

describe("parseOklch", () => {
  it("oklch 문자열에서 밝기·채도·색상을 뽑는다", () => {
    expect(parseOklch("oklch(0.52 0.15 258)")).toEqual({
      l: 0.52,
      c: 0.15,
      h: 258,
    });
  });

  it("채도와 색상이 생략된 무채색도 읽는다", () => {
    expect(parseOklch("oklch(1 0 0)")).toEqual({ l: 1, c: 0, h: 0 });
  });
});

describe("contrastRatio", () => {
  it("검정과 흰색은 21:1 이다", () => {
    const white = parseOklch("oklch(1 0 0)");
    const black = parseOklch("oklch(0 0 0)");

    expect(contrastRatio(white, black)).toBeCloseTo(21, 1);
  });

  it("같은 색끼리는 1:1 이다", () => {
    const color = parseOklch("oklch(0.5 0.1 250)");

    expect(contrastRatio(color, color)).toBeCloseTo(1, 5);
  });

  it("순서를 바꿔도 결과가 같다", () => {
    const a = parseOklch("oklch(0.22 0.012 250)");
    const b = parseOklch("oklch(0.99 0.002 250)");

    expect(contrastRatio(a, b)).toBeCloseTo(contrastRatio(b, a), 5);
  });
});

describe("디자인 토큰 대비 (#a11y-contrast)", () => {
  // 색으로 상태를 말하는 이상, 그 색이 안 보이면 정보가 사라진다.
  const pairs = [
    ["foreground", "background"],
    ["muted-foreground", "background"],
    ["accent-strong", "card"],
    ["success", "card"],
    ["destructive", "card"],
  ] as const;

  it.each(pairs)("라이트: %s 가 %s 위에서 AA 를 넘는다", (fg, bg) => {
    expect(ratio(fg, bg, "light")).toBeGreaterThanOrEqual(AA_TEXT);
  });

  it.each(pairs)("다크: %s 가 %s 위에서 AA 를 넘는다", (fg, bg) => {
    expect(ratio(fg, bg, "dark")).toBeGreaterThanOrEqual(AA_TEXT);
  });
});
