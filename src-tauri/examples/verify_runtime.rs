//! 실환경 검증용 하네스 — 앱을 띄우지 않고 런타임 설치·변환을 그대로 돌려본다.
//!
//! ```bash
//! cargo run --example verify_runtime -- install
//! cargo run --example verify_runtime -- convert <입력.hwp> <출력.pdf>
//! ```
//!
//! 설치물은 전부 앱 데이터 디렉토리 아래에만 들어간다 (시스템에 설치하지 않는다).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use file_converter_lib::core::fs_port::RealFs;
use file_converter_lib::core::progress::{heartbeat_percent, Heartbeat};
use file_converter_lib::core::runtime::assets::Platform;
use file_converter_lib::core::runtime::download::ReqwestDownloader;
use file_converter_lib::core::runtime::real_installer::RealInstaller;
use file_converter_lib::core::soffice::probe::RealProbe;
use file_converter_lib::core::soffice::runner::RealRunner;
use file_converter_lib::shell::runtime_manager::{RuntimeManager, RuntimePaths};

const APP_IDENTIFIER: &str = "io.github.bunhine0452.fileconverter";

fn app_data_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .expect("HOME 을 찾지 못했습니다");

    if cfg!(target_os = "macos") {
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join(APP_IDENTIFIER)
    } else {
        PathBuf::from(home)
            .join("AppData")
            .join("Local")
            .join(APP_IDENTIFIER)
    }
}

fn build_manager() -> (RuntimeManager, RuntimePaths) {
    let platform = Platform::host().expect("지원하지 않는 플랫폼");
    let paths = RuntimePaths::new(&app_data_dir(), platform).expect("런타임 경로");
    let runner = Arc::new(RealRunner);
    let fs = Arc::new(RealFs);

    let manager = RuntimeManager::new(
        Arc::new(RealProbe::new(Some(paths.libreoffice.clone()), None)),
        runner.clone(),
        fs.clone(),
        Arc::new(ReqwestDownloader),
        Arc::new(RealInstaller::new(runner, fs, platform.os)),
        paths.clone(),
        platform,
    );

    (manager, paths)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("status");
    let (manager, paths) = build_manager();

    println!("런타임 루트: {}", paths.root.display());
    println!("설치 전 상태: {:#?}\n", manager.status(true));

    match command {
        "install" => {
            let outcome = manager.install(&mut |event| println!("  {event:?}"), &|| false);
            match outcome {
                Ok(status) => println!("\n설치 완료. 상태: {status:#?}"),
                Err(error) => println!("\n설치 실패: {error}"),
            }
        }
        "convert" => {
            let input = Path::new(args.get(1).expect("입력 파일 경로가 필요합니다"));
            let output = Path::new(args.get(2).expect("출력 파일 경로가 필요합니다"));

            // 앱과 같은 하트비트를 걸어 둔다 — 대용량 변환에서 진행 표시가 실제로
            // 계속 흐르는지, 예상 시간이 현실과 얼마나 맞는지 여기서 눈으로 본다.
            let expected = manager.expected_conversion_time(input);
            println!("예상 소요: {:?}", expected);
            let started = std::time::Instant::now();
            let heartbeat = Heartbeat::start(Duration::from_secs(1), move |elapsed| {
                println!(
                    "  [{:>4.1}s] {}%",
                    elapsed.as_secs_f32(),
                    heartbeat_percent(elapsed, expected)
                );
            });

            let outcome = manager.convert_to_pdf(input, output);
            heartbeat.stop();
            println!("실제 소요: {:.1}s", started.elapsed().as_secs_f32());

            match outcome {
                Ok(note) => {
                    println!("변환 성공 → {}", output.display());
                    if let Some(note) = note {
                        println!("안내: {note}");
                    }
                }
                Err(message) => println!("변환 실패: {message}"),
            }
        }
        "status" => {}
        other => println!("알 수 없는 명령: {other}"),
    }
}
