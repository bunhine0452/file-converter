import { useCallback, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { CategoryNav, type ConversionCategory } from "@/components/CategoryNav";
import { ConversionQueue } from "@/components/ConversionQueue";
import { Dropzone } from "@/components/Dropzone";
import { RuntimeStatus } from "@/components/RuntimeStatus";
import { SettingsPanel } from "@/components/SettingsPanel";
import { Button } from "@/components/ui/button";
import { useConversionQueue } from "@/hooks/useConversionQueue";
import { useSettings } from "@/hooks/useSettings";
import { convertHwp, planOutputPath } from "@/lib/runtime";
import type { Settings } from "@/lib/settings";

/** `.hwp` / `.hwpx` 를 `.pdf` 로 바꾼 기본 저장 이름. */
function defaultOutputPath(source: string): string {
  return source.replace(/\.(hwp|hwpx)$/i, ".pdf");
}

/** 경로에서 폴더 부분만. 구분자는 OS 마다 다르다. */
function parentDirOf(path: string): string {
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));

  return cut > 0 ? path.slice(0, cut) : path;
}

/**
 * 설정이 정해 둔 저장 폴더. `null` 이면 사용자에게 물어야 한다.
 *
 * "지정한 폴더"인데 폴더를 아직 안 골랐으면 묻는 쪽으로 되돌린다 — 반쯤 빈 설정으로
 * 말없이 아무 데나 저장하면 사용자가 파일을 잃어버린다.
 */
function plannedDirFor(source: string, settings: Settings): string | null {
  if (settings.saveMode === "sameAsSource") return parentDirOf(source);
  if (settings.saveMode === "fixedFolder" && settings.outputDir) {
    return settings.outputDir;
  }

  return null;
}

type View = "convert" | "settings";

function App() {
  const { items, track, clearFinished } = useConversionQueue();
  const { settings, update } = useSettings();
  const [category, setCategory] = useState<ConversionCategory>("document");
  const [view, setView] = useState<View>("convert");

  const startOne = useCallback(
    async (source: string, outPath: string) => {
      const id = await convertHwp(source, outPath);
      track(id, source, outPath);
    },
    [track],
  );

  /**
   * 저장 위치를 정해 둔 설정이면 묻지 않는다. 물어야 할 때는 한 건이면 저장 위치를,
   * 여러 건이면 폴더를 **한 번만** 묻는다 — 파일마다 대화상자를 띄우면 10개를 드롭한
   * 사용자는 10번 답해야 한다.
   */
  const handleFiles = useCallback(
    async (paths: string[]) => {
      if (paths.length === 0) return;

      if (plannedDirFor(paths[0], settings) !== null) {
        for (const source of paths) {
          const dir = plannedDirFor(source, settings);
          if (dir === null) continue;

          await startOne(source, await planOutputPath(source, dir));
        }
        return;
      }

      if (paths.length === 1) {
        const outPath = await save({
          title: "PDF 저장 위치",
          defaultPath: defaultOutputPath(paths[0]),
          filters: [{ name: "PDF 문서", extensions: ["pdf"] }],
        });

        // 사용자가 취소하면 변환하지 않는다.
        if (outPath === null) return;

        await startOne(paths[0], outPath);
        return;
      }

      const dir = await open({
        title: `PDF ${paths.length}개를 저장할 폴더`,
        directory: true,
        multiple: false,
      });

      if (dir === null) return;

      for (const source of paths) {
        await startOne(source, await planOutputPath(source, dir));
      }
    },
    [settings, startOne],
  );

  const isSettings = view === "settings";

  return (
    <div className="grid min-h-svh grid-cols-1 sm:grid-cols-[13.5rem_1fr]">
      {/* 사이드바 — 좁은 창에서는 위쪽 가로 목록으로 접힌다. */}
      <aside className="bg-muted/40 flex flex-col gap-4 border-b p-4 sm:border-r sm:border-b-0 sm:p-5">
        <div className="flex flex-col gap-0.5">
          <h1 className="text-sm font-semibold tracking-tight">파일 변환기</h1>
          <span className="text-muted-foreground text-xs">
            기기 안에서만 변환합니다
          </span>
        </div>

        <CategoryNav
          active={category}
          onSelect={(next) => {
            setCategory(next);
            setView("convert");
          }}
        />

        <div className="mt-auto flex flex-col gap-3">
          {/* 런타임 상태는 설정 화면이 자세히 보여 준다 — 같은 것을 두 번 조회하지 않는다. */}
          {!isSettings && <RuntimeStatus />}

          <Button
            variant={isSettings ? "secondary" : "ghost"}
            size="sm"
            className="justify-start"
            aria-pressed={isSettings}
            onClick={() => setView(isSettings ? "convert" : "settings")}
          >
            설정
          </Button>
        </div>
      </aside>

      <main className="flex flex-col gap-6 p-6 sm:p-8">
        {isSettings ? (
          <>
            <header className="flex flex-col gap-1">
              <h2 className="text-2xl font-semibold tracking-tight">설정</h2>
              <p className="text-muted-foreground text-sm">
                바꾸는 즉시 저장되고 다음 실행에도 유지됩니다.
              </p>
            </header>

            <SettingsPanel
              settings={settings}
              onUpdate={(patch) => void update(patch)}
            />
          </>
        ) : (
          <>
            <header className="flex flex-col gap-1">
              <h2 className="text-2xl font-semibold tracking-tight">
                한글 문서를 PDF 로
              </h2>
              <p className="text-muted-foreground text-sm">
                파일은 이 기기를 떠나지 않습니다. 변환은 전부 로컬에서
                실행됩니다.
              </p>
            </header>

            <Dropzone onFiles={(paths) => void handleFiles(paths)} />

            <ConversionQueue items={items} onClearFinished={clearFinished} />
          </>
        )}
      </main>
    </div>
  );
}

export default App;
