import { open } from "@tauri-apps/plugin-dialog";
import { RuntimeStatus } from "@/components/RuntimeStatus";
import { Button } from "@/components/ui/button";
import type { ConflictRule, SaveMode, Settings } from "@/lib/settings";
import type { ThemeSetting } from "@/lib/theme";
import { cn } from "@/lib/utils";

/** 미리보기에 쓸 가상의 원본 — 규칙을 글로만 설명하면 무슨 파일이 나올지 모른다. */
const PREVIEW_SOURCE = "보고서";

const SAVE_MODES: ReadonlyArray<{
  value: SaveMode;
  label: string;
  hint: string;
}> = [
  {
    value: "ask",
    label: "변환할 때마다 묻기",
    hint: "한 건은 저장 위치를, 여러 건은 폴더를 묻습니다",
  },
  {
    value: "sameAsSource",
    label: "원본과 같은 폴더",
    hint: "묻지 않고 원본 옆에 저장합니다",
  },
  {
    value: "fixedFolder",
    label: "지정한 폴더",
    hint: "정해 둔 폴더에 모읍니다",
  },
];

const CONFLICT_RULES: ReadonlyArray<{ value: ConflictRule; label: string }> = [
  { value: "number", label: "번호 붙이기 (보고서 (1).pdf)" },
  { value: "overwrite", label: "덮어쓰기" },
];

const THEMES: ReadonlyArray<{ value: ThemeSetting; label: string }> = [
  { value: "system", label: "시스템 설정" },
  { value: "light", label: "라이트" },
  { value: "dark", label: "다크" },
];

export interface SettingsPanelProps {
  settings: Settings;
  onUpdate: (patch: Partial<Settings>) => void;
}

/** 설정 화면 — 저장 위치·이름 규칙·테마·변환 런타임. */
export function SettingsPanel({ settings, onUpdate }: SettingsPanelProps) {
  async function pickFolder() {
    const dir = await open({
      title: "PDF 를 모아 둘 폴더",
      directory: true,
      multiple: false,
    });

    if (dir === null) return;

    // 폴더를 고른 이상 그 방식으로 쓰겠다는 뜻이다.
    onUpdate({ outputDir: dir, saveMode: "fixedFolder" });
  }

  return (
    <div className="flex max-w-xl flex-col gap-8">
      <Field
        legend="저장 위치"
        hint={
          settings.saveMode === "fixedFolder" && settings.outputDir
            ? settings.outputDir
            : undefined
        }
      >
        {SAVE_MODES.map((mode) => (
          <Radio
            key={mode.value}
            name="saveMode"
            label={mode.label}
            hint={mode.hint}
            checked={settings.saveMode === mode.value}
            onSelect={() => onUpdate({ saveMode: mode.value })}
          />
        ))}

        {settings.saveMode === "fixedFolder" && (
          <Button
            variant="outline"
            size="sm"
            className="mt-1 self-start"
            onClick={() => void pickFolder()}
          >
            폴더 선택
          </Button>
        )}
      </Field>

      <Field legend="파일 이름">
        <label className="flex flex-col gap-1.5 text-sm">
          이름 뒤에 붙일 말
          <input
            type="text"
            value={settings.nameSuffix}
            placeholder="비우면 원본 이름 그대로"
            onChange={(event) => onUpdate({ nameSuffix: event.target.value })}
            className="border-input focus-visible:ring-ring bg-card w-64 rounded-md border px-2.5 py-1.5 text-sm outline-none focus-visible:ring-2"
          />
        </label>
        <p className="text-muted-foreground text-xs">
          {PREVIEW_SOURCE}.hwp →{" "}
          <span className="text-foreground">
            {PREVIEW_SOURCE}
            {settings.nameSuffix}.pdf
          </span>
        </p>
      </Field>

      <Field legend="같은 이름이 있을 때">
        {CONFLICT_RULES.map((rule) => (
          <Radio
            key={rule.value}
            name="onConflict"
            label={rule.label}
            checked={settings.onConflict === rule.value}
            onSelect={() => onUpdate({ onConflict: rule.value })}
          />
        ))}
      </Field>

      <Field legend="테마">
        {THEMES.map((theme) => (
          <Radio
            key={theme.value}
            name="theme"
            label={theme.label}
            checked={settings.theme === theme.value}
            onSelect={() => onUpdate({ theme: theme.value })}
          />
        ))}
      </Field>

      <Field legend="변환 런타임">
        <RuntimeStatus />
      </Field>
    </div>
  );
}

function Field({
  legend,
  hint,
  children,
}: {
  legend: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <fieldset className="flex flex-col gap-2">
      <legend className="mb-2 text-sm font-medium">{legend}</legend>
      {children}
      {hint && (
        <p className="text-muted-foreground truncate text-xs" title={hint}>
          {hint}
        </p>
      )}
    </fieldset>
  );
}

function Radio({
  name,
  label,
  hint,
  checked,
  onSelect,
}: {
  name: string;
  label: string;
  hint?: string;
  checked: boolean;
  onSelect: () => void;
}) {
  return (
    <label
      className={cn(
        "flex cursor-pointer items-baseline gap-2.5 text-sm",
        !checked && "text-muted-foreground",
      )}
    >
      <input
        type="radio"
        name={name}
        checked={checked}
        onChange={onSelect}
        className="accent-accent-strong"
      />
      <span className="flex flex-col gap-0.5">
        <span className={cn(checked && "text-foreground")}>{label}</span>
        {hint && <span className="text-muted-foreground text-xs">{hint}</span>}
      </span>
    </label>
  );
}
