//! `mime`'s tests.
#![allow(clippy::unwrap_used)]
use super::*;

/// Attachments a mail brings: base64 or not, a path in the name dropped, at most `FILES_MAX` kept (each counted).
#[test]
fn a_mails_attachments_are_kept_with_safe_names_within_the_limits() {
    use std::fmt::Write as _;
    let mut raw = String::from(
        "From: a@b.co\r\nContent-Type: multipart/mixed; boundary=\"B\"\r\n\r\n--B\r\nContent-Type: text/plain\r\n\r\nsee attached\r\n",
    );
    raw.push_str("--B\r\nContent-Type: image/png\r\nContent-Disposition: attachment; filename=\"../../etc/shot.png\"\r\nContent-Transfer-Encoding: base64\r\n\r\naGVsbG8=\r\n");
    for i in 0..12 {
        let _ = write!(raw, "--B\r\nContent-Type: text/plain; name=n{i}.txt\r\n\r\nfile {i}\r\n");
    }
    raw.push_str("--B--\r\n");
    let m = parse_mail(raw.as_bytes());
    assert_eq!((m.text.as_str(), m.attachments, m.files.len()), ("see attached", 13, FILES_MAX));
    assert_eq!(
        (m.files[0].name.as_str(), m.files[0].mime.as_str(), m.files[0].bytes.as_slice()),
        ("shot.png", "image/png", b"hello".as_slice())
    );
    assert_eq!(m.files[1].name, "n0.txt");
}

/// v1 test/mail.test.js `parseMail`: who it's from, strictly; the text whichever way it came.
#[test]
fn a_message_is_read_strictly_and_its_text_whichever_way_it_came() {
    let raw = "From: \"Ada L\" <Ada@Example.com>\r\nSubject: =?UTF-8?B?Zml4IMOp?= =?UTF-8?Q?_now?=\r\nMessage-ID: <m1@x.y>\r\nIn-Reply-To: <r1@w>\r\nReferences: <a@b> <r1@w>\r\nDate: Tue, 2 Jan 2024\r\nContent-Type: multipart/alternative; boundary=\"B\"\r\n\r\n--B\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\ncaf=C3=A9 =\r\nok\r\n--B\r\nContent-Type: text/html\r\n\r\n<p>hi</p>\r\n--B\r\nContent-Type: application/pdf; name=x.pdf\r\n\r\nPDF\r\n--B--\r\n";
    let m = parse_mail(raw.as_bytes());
    assert_eq!((m.from.as_str(), m.subject.as_str(), m.message_id.as_str()), ("ada@example.com", "fix é now", "<m1@x.y>"));
    assert_eq!((m.in_reply_to.as_str(), m.references.len()), ("<r1@w>", 2));
    assert_eq!((m.text.as_str(), m.html.as_str(), m.attachments), ("café ok", "<p>hi</p>", 1));
    // the attachment is kept to hand on: its name, type and bytes (round 17)
    assert_eq!(m.files, vec![MailFile { name: "x.pdf".into(), mime: "application/pdf".into(), bytes: b"PDF".to_vec() }]);
    assert!(!m.auto);
    let two = raw.replacen("Subject:", "Subject: other\r\nSubject:", 1);
    assert_eq!(parse_mail(two.as_bytes()).from, "", "a second Subject: nobody it's certainly from");
    assert!(parse_mail(b"From: a@b.co\r\nAuto-Submitted: auto-replied\r\n\r\nx").auto);
    for (f, want) in [
        ("Ada <ada@x.io>", "ada@x.io"),
        ("ada@x.io", "ada@x.io"),
        ("\"evil <eve@x.io>\" <ada@x.io>", ""),
        ("a@x.io, b@x.io", ""),
        ("Ada <ada@localhost>", ""),
        ("(c) ada@x.io", ""),
    ] {
        assert_eq!(mailbox(f), want, "{f}");
    }
}

#[test]
fn a_reply_is_written_line_by_line_and_a_line_break_in_a_header_is_refused() {
    let refs = vec!["<a@b>".to_owned()];
    let o = Out {
        from: "wisp <k@grid.dev>",
        to: "ada@x.io",
        subject: "Re: fix\nBcc: eve@x.io",
        text: "done ✓",
        message_id: "<n@grid.dev>",
        in_reply_to: "<m1@x.y>",
        references: &refs,
        date: "Tue, 02 Jan 2024 03:04:05 GMT",
    };
    let raw = compose_mail(&o).unwrap();
    assert!(raw.contains("\r\nSubject: Re: fix Bcc: eve@x.io\r\n"), "the subject is flattened: {raw}");
    assert!(raw.contains("Auto-Submitted: auto-replied"));
    let back = parse_mail(raw.as_bytes());
    assert_eq!((back.text.as_str(), back.in_reply_to.as_str()), ("done ✓", "<m1@x.y>"));
    assert!(compose_mail(&Out { to: "ada@x.io\r\nBcc: eve@x.io", ..o }).is_none());
}
