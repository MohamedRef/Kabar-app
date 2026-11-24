use super::Message;
use crate::i18n::Text;

#[test]
fn calendar_labels_read_correctly_in_all_languages() {
    crate::i18n::checks::catalog(Message::ALL, |lang, message| message.text(lang), |_| false);
}
