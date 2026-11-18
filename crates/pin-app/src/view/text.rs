use eframe::egui::{Color32, FontId, TextFormat, text::LayoutJob};

/// `text` with every match of `query` on a highlight background.
pub fn highlighted(
    text: &str,
    query: &str,
    font: FontId,
    color: Color32,
    hit: Color32,
) -> LayoutJob {
    let plain = TextFormat::simple(font, color);
    let marked = TextFormat {
        background: hit,
        ..plain.clone()
    };
    let mut job = LayoutJob::default();
    let mut at = 0;
    for range in pin_core::match_ranges(text, query) {
        job.append(&text[at..range.start], 0.0, plain.clone());
        job.append(&text[range.clone()], 0.0, marked.clone());
        at = range.end;
    }
    job.append(&text[at..], 0.0, plain);
    job
}

/// Limits `job` to one line of `width`, ending in an ellipsis when cut.
pub fn one_line(mut job: LayoutJob, width: f32) -> LayoutJob {
    job.wrap.max_width = width.max(0.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('\u{2026}');
    job
}

pub fn plain(text: &str, size: f32, color: Color32) -> LayoutJob {
    LayoutJob::single_section(
        text.to_owned(),
        TextFormat::simple(FontId::proportional(size), color),
    )
}
