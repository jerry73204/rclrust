use std::borrow::Cow;

use newline_converter::dos2unix;

pub fn fix_newlines(text: &str) -> Cow<'_, str> {
    let text = dos2unix(text);

    if text.ends_with('\n') {
        text
    } else {
        let mut text = text.into_owned();
        text.push('\n');
        Cow::Owned(text)
    }
}
