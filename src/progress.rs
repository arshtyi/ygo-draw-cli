use indicatif::{ProgressBar, ProgressStyle};

pub fn bytes(label: &str, length: Option<u64>) -> ProgressBar {
    let bar = match length {
        Some(length) => ProgressBar::new(length),
        None => ProgressBar::no_length(),
    };
    let template = if length.is_some() {
        "{spinner:.green} {msg:24!} [{bar:28.cyan/blue}] {bytes}/{total_bytes} {bytes_per_sec} ETA {eta}"
    } else {
        "{spinner:.green} {msg:24!} {bytes} {bytes_per_sec}"
    };
    bar.set_style(
        ProgressStyle::with_template(template)
            .expect("static byte progress template should be valid")
            .progress_chars("=>-"),
    );
    bar.set_message(label.to_owned());
    bar
}

pub fn items(label: &str, length: usize) -> ProgressBar {
    let bar = ProgressBar::new(length as u64);
    bar.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} {msg:24!} [{bar:28.green/blue}] {pos}/{len} {per_sec} ETA {eta}",
        )
        .expect("static item progress template should be valid")
        .progress_chars("=>-"),
    );
    bar.set_message(label.to_owned());
    bar
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_progress_tracks_position() {
        let bar = bytes("download", Some(100));
        bar.inc(25);

        assert_eq!(bar.length(), Some(100));
        assert_eq!(bar.position(), 25);
        bar.finish_and_clear();
    }

    #[test]
    fn item_progress_tracks_position() {
        let bar = items("render", 4);
        bar.inc(1);

        assert_eq!(bar.length(), Some(4));
        assert_eq!(bar.position(), 1);
        bar.finish_and_clear();
    }
}
