//! File: actions/tests/links.rs
//! Purpose: Protect conservative link handling: tracking goes, everything that could matter stays; extraction finds links, emails and phones in prose.
//! Contents: tests of Clean Links and the Extract actions.

use crate::actions::Action;

/// A pasted paragraph loses click-tracking only: real parameters, fragments and signed download links are left exactly as they were (invariant: conservative links).
#[test]
fn cleaning_links_removes_tracking_and_keeps_real_and_signed_parameters() {
    let text = "Read https://example.com/post?id=7&utm_source=mail&fbclid=abc#top, then grab \
                https://files.example.com/a.zip?X-Amz-Signature=abc&utm_source=x now.";
    let cleaned = Action::CleanLinks.apply(text).expect("a tracking link changes");
    assert_eq!(
        cleaned,
        "Read https://example.com/post?id=7#top, then grab https://files.example.com/a.zip?X-Amz-Signature=abc&utm_source=x now."
    );
}

/// A link whose only parameters are tracking becomes the bare address; one with only unknown parameters is not touched at all, so the caller skips the write.
#[test]
fn a_link_with_only_tracking_is_shortened_and_one_without_is_left_alone() {
    assert_eq!(Action::CleanLinks.apply("https://a.example/x?utm_source=1&gclid=2").as_deref(), Some("https://a.example/x"));
    assert_eq!(Action::CleanLinks.apply("https://a.example/x?ref=friend&page=2"), None);
    assert_eq!(Action::CleanLinks.apply("no links here, only words"), None);
}

/// Sentence punctuation after a link is not part of it, but a bracket the link opened is (Wikipedia-style addresses).
#[test]
fn extracting_links_drops_sentence_punctuation_and_keeps_balanced_brackets() {
    let text = "See (https://en.wikipedia.org/wiki/Rust_(language)) and https://x.example/a. Again https://x.example/a!";
    let found = Action::ExtractUrls.apply(text).expect("links found");
    assert_eq!(found, "https://en.wikipedia.org/wiki/Rust_(language)\nhttps://x.example/a");
}

/// Emails and phone numbers are listed once each in order of appearance; dates that look like numbers are not phone numbers.
#[test]
fn extracting_emails_and_phones_lists_each_once_and_skips_dates() {
    let emails = Action::ExtractEmails.apply("Mail ann@example.com or bo@example.org; ann@example.com again").expect("emails");
    assert_eq!(emails, "ann@example.com\nbo@example.org");
    let phones = Action::ExtractPhones.apply("Call +1 555 123 4567 before 2024-05-06").expect("phones");
    assert_eq!(phones, "+1 555 123 4567");
    assert_eq!(Action::ExtractEmails.apply("nothing to find"), None);
}
