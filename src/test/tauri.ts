import { cleanup } from "@testing-library/react";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";

export type IpcHandler = (command: string, payload?: unknown) => unknown;

export interface IpcCall {
  command: string;
  payload?: unknown;
}

/**
 * 실제 `@tauri-apps/api` 코드를 그대로 태우고 IPC 만 가로챈다.
 * `vi.mock` 으로 모듈을 통째로 갈아끼우는 것보다 강한 테스트다 — 이벤트 payload 변환
 * (`type` 필드 부착·`PhysicalPosition` 래핑)과 플러그인 인자 직렬화까지 함께 검증된다.
 */
export function mockTauri(
  handler: IpcHandler = () => undefined,
  calls?: IpcCall[],
): void {
  mockWindows("main");
  mockIPC(
    (command, payload) => {
      calls?.push({ command, payload });
      return handler(command, payload);
    },
    { shouldMockEvents: true },
  );
}

/**
 * `clearMocks()` 는 `__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener` 를 지운다.
 * 컴포넌트를 먼저 언마운트하지 않으면 구독 해제 과정에서 TypeError 가 난다.
 */
export function resetTauriMocks(): void {
  cleanup();
  clearMocks();
}
