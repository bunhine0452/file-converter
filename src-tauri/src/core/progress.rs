//! 변환 중 진행 표시를 살려 두는 하트비트.
//!
//! soffice 는 변환 도중 아무 진행 정보도 주지 않는다. 그래서 지금까지는 시작할 때
//! 5% 한 번만 보내고 끝날 때까지 침묵했다 — 100MB 짜리 문서에서는 막대가 몇 분 동안
//! 5% 에 멈춰 있어 사용자가 앱이 죽은 줄 안다.
//!
//! 여기서는 "얼마나 걸릴 것 같은가"로 추정 진행률을 만들어 살아있음을 보인다.
//! 추정이므로 절대 100% 에 닿지 않는다 — 완료는 실제로 끝났을 때만 쓴다.

use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// 변환을 시작했음을 알리는 최소 진행률.
pub const CONVERT_STARTED_PERCENT: u8 = 5;

/// 추정 진행률의 상한. 100 을 추정으로 채우면 완료와 구분되지 않는다.
pub const HEARTBEAT_CEILING_PERCENT: u8 = 95;

/// 제한 시간 대비 예상 소요 시간의 비율 (1/n).
///
/// 제한 시간은 "이보다 오래 걸리면 죽은 것"이라 실제 소요보다 넉넉하다.
/// 그 절반을 기준으로 잡아야 정상 변환에서 막대가 끝까지 차오른다.
const EXPECTED_DIVISOR: u32 = 2;

/// 경과 시간으로 추정한 진행률.
///
/// 시작 진행률에서 출발해 예상 시간에 상한까지 선형으로 차오르고, 예상보다
/// 오래 걸리면 상한에 머문다 (더 이상 진척을 약속하지 않되 살아있음은 유지).
pub fn heartbeat_percent(elapsed: Duration, expected: Duration) -> u8 {
    let span = u128::from(HEARTBEAT_CEILING_PERCENT - CONVERT_STARTED_PERCENT);
    let expected_ms = expected.as_millis();

    // 예상 시간을 모르면(0) 진척을 흉내내지 않고 상한만 알린다.
    if expected_ms == 0 {
        return HEARTBEAT_CEILING_PERCENT;
    }

    let elapsed_ms = elapsed.as_millis().min(expected_ms);
    let gained = (span * elapsed_ms / expected_ms) as u8;

    CONVERT_STARTED_PERCENT + gained
}

/// 제한 시간에서 뽑아낸 예상 소요 시간 — 크기 비례 규칙을 한 곳(제한 시간)에만 둔다.
pub fn expected_duration(timeout: Duration) -> Duration {
    timeout / EXPECTED_DIVISOR
}

/// 일정 간격으로 경과 시간을 알려 주는 배경 스레드.
///
/// 정지 요청은 [`Condvar`] 로 즉시 전달된다 — 간격만큼 자고 있으면 변환이 끝난 뒤에도
/// 그만큼 완료 통지가 늦어지고, 그 사이 늦은 진행률이 완료를 덮어쓴다.
pub struct Heartbeat {
    signal: Arc<(Mutex<bool>, Condvar)>,
    handle: Option<JoinHandle<()>>,
}

impl Heartbeat {
    /// `interval` 마다 시작 이후 경과 시간을 `tick` 에 넘긴다. 첫 알림도 한 간격 뒤다.
    pub fn start(interval: Duration, mut tick: impl FnMut(Duration) + Send + 'static) -> Self {
        let signal = Arc::new((Mutex::new(false), Condvar::new()));
        let waiter = Arc::clone(&signal);

        let handle = std::thread::spawn(move || {
            let started = Instant::now();
            let (stopped, wake) = &*waiter;

            loop {
                let guard = stopped.lock().unwrap_or_else(|e| e.into_inner());
                // 조건을 함께 넘겨야 한다 — 기다리기 전에 도착한 정지 신호는 깨울 대상이
                // 없어 사라지고, 그러면 다음 간격까지(최대 몇 분) 매달린다.
                let (guard, _) = wake
                    .wait_timeout_while(guard, interval, |stopped| !*stopped)
                    .unwrap_or_else(|e| e.into_inner());
                if *guard {
                    return;
                }
                drop(guard);

                tick(started.elapsed());
            }
        });

        Self {
            signal,
            handle: Some(handle),
        }
    }

    /// 알림을 멈추고 스레드가 끝날 때까지 기다린다 — 반환 뒤에는 콜백이 절대 오지 않는다.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        let (stopped, wake) = &*self.signal;
        {
            let mut guard = stopped.lock().unwrap_or_else(|e| e.into_inner());
            *guard = true;
        }
        wake.notify_all();

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Heartbeat {
    /// 잊고 놓아 버려도 스레드가 남지 않는다.
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    const TIMEOUT: Duration = Duration::from_secs(60);

    // ── happy path ───────────────────────────────────────────────

    #[test]
    fn 시작_직후에는_시작_진행률이다() {
        assert_eq!(
            heartbeat_percent(Duration::ZERO, TIMEOUT),
            CONVERT_STARTED_PERCENT
        );
    }

    #[test]
    fn 예상_시간의_절반이면_시작과_상한의_중간이다() {
        // Arrange & Act
        let percent = heartbeat_percent(Duration::from_secs(30), TIMEOUT);

        // Assert — 5 + (95-5)/2
        assert_eq!(percent, 50);
    }

    // ── edge cases ───────────────────────────────────────────────

    #[test]
    fn 예상_시간을_넘겨도_상한에서_멈춘다() {
        // 추정으로 100% 를 찍으면 완료와 구분되지 않는다 — 끝나지도 않았는데 끝난 척이다.
        for elapsed in [61, 120, 3600] {
            let percent = heartbeat_percent(Duration::from_secs(elapsed), TIMEOUT);

            assert_eq!(percent, HEARTBEAT_CEILING_PERCENT, "{elapsed}초");
        }
    }

    #[test]
    fn 예상_시간이_0_이어도_나눗셈이_터지지_않는다() {
        assert_eq!(
            heartbeat_percent(Duration::from_secs(1), Duration::ZERO),
            HEARTBEAT_CEILING_PERCENT
        );
    }

    #[test]
    fn 경과가_늘면_진행률은_절대_줄지_않는다() {
        // 막대가 뒤로 가면 사용자는 변환이 되돌아간 줄 안다.
        let mut previous = 0;

        for tenth in 0..=200 {
            let percent = heartbeat_percent(Duration::from_millis(tenth * 500), TIMEOUT);

            assert!(percent >= previous, "{tenth} 번째에서 후퇴: {percent}");
            previous = percent;
        }
    }

    // ── 하트비트 스레드 ───────────────────────────────────────────

    fn recorder() -> (
        Arc<Mutex<Vec<Duration>>>,
        impl FnMut(Duration) + Send + 'static,
    ) {
        let ticks = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&ticks);

        (ticks, move |elapsed| {
            sink.lock().expect("기록 잠금").push(elapsed);
        })
    }

    #[test]
    fn 하트비트는_간격마다_경과_시간을_알린다() {
        // Arrange
        let (ticks, record) = recorder();

        // Act
        let beat = Heartbeat::start(Duration::from_millis(10), record);
        std::thread::sleep(Duration::from_millis(200));
        beat.stop();

        // Assert — 몇 번인지가 아니라 "계속 온다"와 "경과가 는다"가 계약이다.
        let ticks = ticks.lock().expect("기록 잠금");
        assert!(ticks.len() >= 2, "알림이 부족하다: {}", ticks.len());
        assert!(
            ticks.windows(2).all(|pair| pair[1] > pair[0]),
            "경과 시간이 늘지 않았다: {ticks:?}"
        );
    }

    #[test]
    fn 정지한_뒤에는_더_이상_알리지_않는다() {
        // 완료 뒤에 늦은 진행률이 도착하면 UI 가 끝난 작업을 다시 변환 중으로 되돌린다.
        let (ticks, record) = recorder();
        let beat = Heartbeat::start(Duration::from_millis(5), record);
        std::thread::sleep(Duration::from_millis(50));

        beat.stop();

        let after_stop = ticks.lock().expect("기록 잠금").len();
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(ticks.lock().expect("기록 잠금").len(), after_stop);
    }

    #[test]
    fn 정지는_다음_간격을_기다리지_않는다() {
        // 간격만큼 기다렸다 멈추면 변환이 끝나도 완료 통지가 그만큼 늦어진다.
        let (ticks, record) = recorder();
        let beat = Heartbeat::start(Duration::from_secs(30), record);

        let started = std::time::Instant::now();
        beat.stop();

        assert!(started.elapsed() < Duration::from_secs(1), "정지가 느리다");
        // 첫 간격 전에는 알림도 없다 — 시작 진행률을 두 번 보내는 셈이 된다.
        assert!(ticks.lock().expect("기록 잠금").is_empty());
    }

    #[test]
    fn 예상_시간은_제한_시간보다_짧고_비례한다() {
        // 제한 시간까지 기다려야 막대가 차면 정상 변환도 절반에서 끝난다.
        let small = expected_duration(Duration::from_secs(60));
        let large = expected_duration(Duration::from_secs(600));

        assert!(small < Duration::from_secs(60));
        assert_eq!(large, small * 10);
    }
}
