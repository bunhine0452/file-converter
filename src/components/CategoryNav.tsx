import { cn } from "@/lib/utils";

export type ConversionCategory = "document" | "image" | "pdf" | "media";

interface Category {
  id: ConversionCategory;
  label: string;
  hint: string;
  /** 아직 못 만든 분류는 눌리지 않는다 — 눌리는데 아무 일도 없는 메뉴가 제일 나쁘다. */
  ready: boolean;
}

const CATEGORIES: readonly Category[] = [
  { id: "document", label: "문서", hint: "HWP · HWPX → PDF", ready: true },
  { id: "image", label: "이미지", hint: "PNG · JPG · WebP", ready: false },
  { id: "pdf", label: "PDF", hint: "이미지 추출 · 병합", ready: false },
  { id: "media", label: "미디어", hint: "영상 · 음성", ready: false },
];

export interface CategoryNavProps {
  active: ConversionCategory;
  onSelect: (category: ConversionCategory) => void;
}

/** 변환 분류 사이드바. 지금 할 수 있는 일과 앞으로 할 일을 함께 보여 준다. */
export function CategoryNav({ active, onSelect }: CategoryNavProps) {
  return (
    <nav aria-label="변환 분류" className="flex flex-col gap-1">
      {CATEGORIES.map((category) => {
        const isActive = category.ready && category.id === active;

        return (
          <a
            key={category.id}
            role="link"
            href={category.ready ? `#${category.id}` : undefined}
            // 이름은 분류명만 — 설명까지 이름에 섞이면 "PDF(이미지 추출)"와 "이미지"가
            // 스크린리더에서 구분되지 않는다. 설명은 describedby 로 따로 읽힌다.
            aria-label={
              category.ready ? category.label : `${category.label} (준비 중)`
            }
            aria-describedby={`category-hint-${category.id}`}
            aria-current={isActive ? "page" : undefined}
            aria-disabled={category.ready ? undefined : true}
            tabIndex={category.ready ? 0 : -1}
            onClick={(event) => {
              event.preventDefault();
              if (category.ready) onSelect(category.id);
            }}
            className={cn(
              "focus-visible:ring-ring flex flex-col gap-0.5 rounded-md px-3 py-2 transition-colors duration-[var(--motion-fast)] outline-none focus-visible:ring-2",
              category.ready
                ? "hover:bg-accent cursor-pointer"
                : "cursor-default opacity-55",
              isActive && "bg-accent text-accent-foreground",
            )}
          >
            <span className="flex items-baseline justify-between gap-2 text-sm font-medium">
              {category.label}
              {!category.ready && (
                <span className="text-muted-foreground text-[10px] font-normal">
                  준비 중
                </span>
              )}
            </span>
            <span
              id={`category-hint-${category.id}`}
              className="text-muted-foreground text-xs"
            >
              {category.hint}
            </span>
          </a>
        );
      })}
    </nav>
  );
}
