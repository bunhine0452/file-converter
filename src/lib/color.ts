/**
 * 디자인 토큰의 색 대비를 계산한다.
 *
 * 토큰이 oklch 로 적혀 있어 눈으로는 대비를 가늠할 수 없다. 색이 곧 상태 정보인
 * 화면에서 대비가 무너지면 정보가 사라지므로, 값을 바꿀 때마다 테스트가 잡게 한다.
 */

export interface Oklch {
  /** 밝기 0~1 */
  l: number;
  /** 채도 */
  c: number;
  /** 색상 각도(도) */
  h: number;
}

/** `oklch(0.52 0.15 258)` 형태의 CSS 값을 숫자로 바꾼다. */
export function parseOklch(value: string): Oklch {
  const numbers = value
    .replace(/^oklch\(/, "")
    .replace(/\)$/, "")
    .trim()
    .split(/[\s/]+/)
    .map(Number);

  const [l, c = 0, h = 0] = numbers;
  if (!Number.isFinite(l))
    throw new Error(`oklch 값을 읽지 못했습니다: ${value}`);

  return { l, c, h };
}

/** OKLCH → 선형 sRGB. 감마 인코딩 전 값이라 그대로 상대 휘도에 쓸 수 있다. */
function toLinearRgb({ l, c, h }: Oklch): [number, number, number] {
  const radians = (h * Math.PI) / 180;
  const a = c * Math.cos(radians);
  const b = c * Math.sin(radians);

  const lms = [
    (l + 0.3963377774 * a + 0.2158037573 * b) ** 3,
    (l - 0.1055613458 * a - 0.0638541728 * b) ** 3,
    (l - 0.0894841775 * a - 1.291485548 * b) ** 3,
  ] as const;

  const [long, medium, short] = lms;

  return [
    4.0767416621 * long - 3.3077115913 * medium + 0.2309699292 * short,
    -1.2684380046 * long + 2.6097574011 * medium - 0.3413193965 * short,
    -0.0041960863 * long - 0.7034186147 * medium + 1.707614701 * short,
    // 색역을 벗어난 값은 화면에서도 잘려 보이므로 같이 자른다.
  ].map((channel) => Math.min(1, Math.max(0, channel))) as [
    number,
    number,
    number,
  ];
}

function relativeLuminance(color: Oklch): number {
  const [r, g, b] = toLinearRgb(color);

  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG 대비비 (1~21). 순서는 상관없다. */
export function contrastRatio(a: Oklch, b: Oklch): number {
  const first = relativeLuminance(a);
  const second = relativeLuminance(b);
  const lighter = Math.max(first, second);
  const darker = Math.min(first, second);

  return (lighter + 0.05) / (darker + 0.05);
}
