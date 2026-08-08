//! 이미지 포맷 상호 변환.
//!
//! 파일은 기기를 떠나지 않는다 — 디코드·인코드 모두 이 프로세스 안에서 끝난다.
//! 바이트를 받아 바이트를 돌려주는 순수 함수라 실제 파일 없이 매트릭스로 검증한다.

use std::io::Cursor;

use image::{DynamicImage, ImageFormat as CrateFormat, ImageReader};

/// 앱이 다루는 이미지 포맷.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ImageFormat {
    Png,
    Jpeg,
    WebP,
    Bmp,
    Tiff,
    Gif,
}

impl ImageFormat {
    /// 화면·파일명에 쓰는 확장자.
    pub fn extension(self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::WebP => "webp",
            ImageFormat::Bmp => "bmp",
            ImageFormat::Tiff => "tiff",
            ImageFormat::Gif => "gif",
        }
    }

    /// 투명도를 담을 수 있는 포맷인가. 없는 포맷으로 가려면 알파를 먼저 합성해야 한다.
    pub fn supports_alpha(self) -> bool {
        !matches!(self, ImageFormat::Jpeg | ImageFormat::Bmp)
    }

    fn to_crate(self) -> CrateFormat {
        match self {
            ImageFormat::Png => CrateFormat::Png,
            ImageFormat::Jpeg => CrateFormat::Jpeg,
            ImageFormat::WebP => CrateFormat::WebP,
            ImageFormat::Bmp => CrateFormat::Bmp,
            ImageFormat::Tiff => CrateFormat::Tiff,
            ImageFormat::Gif => CrateFormat::Gif,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImageConvertError {
    #[error("이미지를 읽지 못했습니다")]
    Decode,
    #[error("이미지를 저장하지 못했습니다")]
    Encode,
}

/// 사용자에게 보여줄 안내. 라이브러리 원문은 절대 싣지 않는다.
pub fn convert_error_message(error: &ImageConvertError) -> &'static str {
    match error {
        ImageConvertError::Decode => {
            "이미지를 열지 못했습니다. 지원하지 않는 형식이거나 파일이 손상됐습니다."
        }
        ImageConvertError::Encode => "이미지를 저장하지 못했습니다. 다른 형식으로 시도해 주세요.",
    }
}

/// 이미지를 다른 포맷으로 바꾼 바이트.
pub fn convert_image(bytes: &[u8], target: ImageFormat) -> Result<Vec<u8>, ImageConvertError> {
    let decoded = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| ImageConvertError::Decode)?
        .decode()
        .map_err(|_| ImageConvertError::Decode)?;

    let prepared = prepare_for(decoded, target);

    let mut out = Cursor::new(Vec::new());
    prepared
        .write_to(&mut out, target.to_crate())
        .map_err(|_| ImageConvertError::Encode)?;

    Ok(out.into_inner())
}

/// 투명도를 못 담는 포맷으로 갈 때는 흰 배경에 얹는다.
///
/// 알파를 그냥 버리면 투명했던 자리가 검게 나온다 — 사진을 JPG 로 바꿨더니 배경이
/// 새까맣더라는 사고가 여기서 난다.
fn prepare_for(image: DynamicImage, target: ImageFormat) -> DynamicImage {
    if target.supports_alpha() || !image.color().has_alpha() {
        return image;
    }

    let rgba = image.to_rgba8();
    let mut rgb = image::RgbImage::new(rgba.width(), rgba.height());

    for (x, y, pixel) in rgba.enumerate_pixels() {
        let alpha = f32::from(pixel[3]) / 255.0;
        let blend = |channel: u8| {
            (f32::from(channel) * alpha + 255.0 * (1.0 - alpha))
                .round()
                .clamp(0.0, 255.0) as u8
        };
        rgb.put_pixel(
            x,
            y,
            image::Rgb([blend(pixel[0]), blend(pixel[1]), blend(pixel[2])]),
        );
    }

    DynamicImage::ImageRgb8(rgb)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [ImageFormat; 6] = [
        ImageFormat::Png,
        ImageFormat::Jpeg,
        ImageFormat::WebP,
        ImageFormat::Bmp,
        ImageFormat::Tiff,
        ImageFormat::Gif,
    ];

    /// 색이 있는 작은 견본 (단색이면 포맷 차이를 못 잡는다).
    fn sample(width: u32, height: u32) -> DynamicImage {
        let mut rgba = image::RgbaImage::new(width, height);
        for (x, y, pixel) in rgba.enumerate_pixels_mut() {
            *pixel = image::Rgba([(x * 8) as u8, (y * 8) as u8, 128, 255]);
        }

        DynamicImage::ImageRgba8(rgba)
    }

    fn encode(image: &DynamicImage, format: ImageFormat) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        image
            .write_to(&mut out, format.to_crate())
            .expect("견본 인코딩");

        out.into_inner()
    }

    fn decode(bytes: &[u8]) -> DynamicImage {
        ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .expect("형식 추측")
            .decode()
            .expect("디코드")
    }

    fn format_of(bytes: &[u8]) -> CrateFormat {
        ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .expect("형식 추측")
            .format()
            .expect("형식 확정")
    }

    // ── happy path ───────────────────────────────────────────────

    #[test]
    fn 모든_조합이_서로_변환된다() {
        let source = sample(16, 12);

        for from in ALL {
            // 알파를 담을 수 없는 포맷은 견본부터 알파를 뺀다.
            let seed = if from.supports_alpha() {
                source.clone()
            } else {
                prepare_for(source.clone(), from)
            };
            let bytes = encode(&seed, from);

            for to in ALL {
                let converted =
                    convert_image(&bytes, to).unwrap_or_else(|e| panic!("{from:?}→{to:?}: {e}"));

                assert_eq!(format_of(&converted), to.to_crate(), "{from:?}→{to:?}");
                let out = decode(&converted);
                assert_eq!((out.width(), out.height()), (16, 12), "{from:?}→{to:?}");
            }
        }
    }

    #[test]
    fn 확장자는_포맷마다_다르다() {
        let mut extensions: Vec<&str> = ALL.iter().map(|f| f.extension()).collect();
        extensions.sort_unstable();
        extensions.dedup();

        assert_eq!(extensions.len(), ALL.len());
    }

    // ── edge cases ───────────────────────────────────────────────

    #[test]
    fn 투명한_그림을_jpg_로_바꾸면_검게_되지_않는다() {
        // 알파를 그냥 버리면 투명했던 자리가 새까맣게 나온다.
        let mut rgba = image::RgbaImage::new(4, 4);
        for pixel in rgba.pixels_mut() {
            *pixel = image::Rgba([0, 0, 0, 0]); // 완전 투명
        }
        let png = encode(&DynamicImage::ImageRgba8(rgba), ImageFormat::Png);

        let jpg = convert_image(&png, ImageFormat::Jpeg).expect("변환 성공");

        let pixel = decode(&jpg).to_rgb8()[(1, 1)];
        assert!(
            pixel[0] > 240 && pixel[1] > 240 && pixel[2] > 240,
            "투명 배경이 흰색이 아니다: {pixel:?}"
        );
    }

    #[test]
    fn 깨진_입력은_내부_진단_없이_거절한다() {
        for broken in [
            b"".as_slice(),
            "이건 이미지가 아니다".as_bytes(),
            &[0xFF; 64],
        ] {
            let error = convert_image(broken, ImageFormat::Png).expect_err("거절되어야 한다");

            let message = convert_error_message(&error);
            assert!(message.chars().any(|c| ('가'..='힣').contains(&c)));
            assert!(!message.contains("Decode"), "{message}");
        }
    }

    #[test]
    fn 같은_포맷으로_바꿔도_정상_결과다() {
        // 사용자가 실수로 png→png 를 고를 수 있다. 깨지지 않아야 한다.
        let png = encode(&sample(8, 8), ImageFormat::Png);

        let again = convert_image(&png, ImageFormat::Png).expect("변환 성공");

        assert_eq!(format_of(&again), CrateFormat::Png);
    }
}
