use crate::{MosseTrackerSettings, MultiMosseTracker};
use image::{DynamicImage, Rgba};
use imageproc::drawing::{draw_cross_mut, draw_hollow_rect_mut, draw_text_mut};
use imageproc::rect::Rect;
use rusttype::{Font, Scale};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct MultiMosseTrackerJS {
    tracker: MultiMosseTracker,
}

#[wasm_bindgen]
impl MultiMosseTrackerJS {
    #[wasm_bindgen(constructor)]
    pub fn new(width: u32, height: u32) -> Self {
        let window_size = 48;
        let psr_threshold = 7.0;
        let settings = MosseTrackerSettings {
            window_size,
            width,
            height,
            regularization: 0.002,
            learning_rate: 0.05,
            psr_threshold,
        };
        let desperation_threshold = 4;
        let multi_tracker = MultiMosseTracker::new(settings, desperation_threshold);
        Self {
            tracker: multi_tracker,
        }
    }

    #[wasm_bindgen]
    pub fn set_target(&mut self, x: u32, y: u32, img_data: &[u8]) {
        let img = image::load_from_memory_with_format(img_data, image::ImageFormat::Png).unwrap();
        self.tracker
            .add_or_replace_target(1, (x, y), &img.to_luma8());
    }

    #[wasm_bindgen]
    pub fn track(&mut self, img_data: &[u8]) -> Vec<u8> {
        let mut image =
            image::load_from_memory_with_format(img_data, image::ImageFormat::Png).unwrap();
        let predictions = self.tracker.track(&image.to_luma8());
        let mut img_copy = image.to_rgba8();
        for (obj_id, pred) in predictions.iter() {
            let mut color = Rgba([125u8, 255u8, 0u8, 0u8]);
            if pred.psr < self.tracker.settings.psr_threshold {
                color = Rgba([255u8, 0u8, 0u8, 0u8])
            }
            draw_cross_mut(
                &mut img_copy,
                Rgba([255u8, 0u8, 0u8, 0u8]),
                pred.location.0 as i32,
                pred.location.1 as i32,
            );
            let window_size = self.tracker.settings.window_size;
            draw_hollow_rect_mut(
                &mut img_copy,
                Rect::at(
                    pred.location.0.saturating_sub(window_size / 2) as i32,
                    pred.location.1.saturating_sub(window_size / 2) as i32,
                )
                .of_size(window_size, window_size),
                color,
            );

            let font_data = include_bytes!("../examples/Arial.ttf");
            let font = Font::try_from_bytes(font_data as &[u8]).unwrap();

            const FONT_SCALE: f32 = 10.0;

            draw_text_mut(
                &mut img_copy,
                Rgba([125u8, 255u8, 0u8, 0u8]),
                (pred.location.0 - (window_size / 2)).try_into().unwrap(),
                (pred.location.1 - (window_size / 2)).try_into().unwrap(),
                Scale::uniform(FONT_SCALE),
                &font,
                &format!("#{}", obj_id),
            );

            draw_text_mut(
                &mut img_copy,
                color,
                (pred.location.0 - (window_size / 2)).try_into().unwrap(),
                (pred.location.1 - (window_size / 2) + FONT_SCALE as u32)
                    .try_into()
                    .unwrap(),
                Scale::uniform(FONT_SCALE),
                &font,
                &format!("PSR: {:.2}", pred.psr),
            );
        }

        let mut image_data: Vec<u8> = Vec::new();
        img_copy
            .write_to(
                &mut std::io::Cursor::new(&mut image_data),
                image::ImageFormat::Png,
            )
            .unwrap();
        image_data
    }
}
