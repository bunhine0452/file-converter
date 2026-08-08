import { useCallback, useEffect, useRef, useState } from "react";
import {
  DEFAULT_SETTINGS,
  getSettings,
  saveSettings,
  type Settings,
} from "@/lib/settings";
import {
  applyTheme,
  prefersDark,
  resolveTheme,
  watchSystemTheme,
} from "@/lib/theme";

/**
 * 사용자 설정. 읽기는 절대 실패하지 않는다 — 못 읽으면 기본값으로 시작한다.
 *
 * 저장은 화면 반영과 분리돼 있다. 디스크 쓰기가 실패해도 이번 세션의 선택은 살아 있어야
 * 사용자가 하던 일을 마칠 수 있다 (실패는 `saveError` 로 드러낸다).
 */
export function useSettings() {
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [isLoading, setIsLoading] = useState(true);
  const [saveError, setSaveError] = useState<string | null>(null);
  const isMountedRef = useRef(true);

  useEffect(() => {
    isMountedRef.current = true;

    getSettings()
      .catch(() => DEFAULT_SETTINGS)
      .then((loaded) => {
        if (!isMountedRef.current) return;
        setSettings(loaded);
        setIsLoading(false);
        applyTheme(resolveTheme(loaded.theme, prefersDark()));
      });

    return () => {
      isMountedRef.current = false;
    };
  }, []);

  // "시스템"을 골랐으면 OS 를 바꿨을 때 앱도 따라가야 한다.
  useEffect(() => {
    if (settings.theme !== "system") return;

    return watchSystemTheme(() =>
      applyTheme(resolveTheme("system", prefersDark())),
    );
  }, [settings.theme]);

  const update = useCallback(
    async (patch: Partial<Settings>) => {
      const next = { ...settings, ...patch };
      setSettings(next);
      applyTheme(resolveTheme(next.theme, prefersDark()));

      try {
        await saveSettings(next);
        setSaveError(null);
      } catch (cause) {
        setSaveError(cause instanceof Error ? cause.message : String(cause));
      }
    },
    [settings],
  );

  return { settings, update, isLoading, saveError };
}
