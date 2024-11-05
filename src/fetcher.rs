use std::{fs, path::Path};

use chrono::DateTime;
use imap::{self, types::Fetches};
use mailparse;

const DATE_TIME_OUTPUT_FORMAT: &str = "%Y-%m-%dT%H-%M";
const PORT: u16 = 993;

pub fn fetch_subject_messages(
    host: String,
    user: String,
    password: String,
    subject: String,
    output: &Path,
) {
    let client = imap::ClientBuilder::new(&host, PORT)
        .connect()
        .expect("Failed to connect to host");

    // the client we have here is unauthenticated.
    // to do anything useful with the e-mails, we need to log in
    let mut imap_session = client
        .login(&user, &password)
        .map_err(|e| e.0)
        .expect("Failed to login");

    // we want to fetch the first email in the INBOX mailbox
    imap_session
        .select("INBOX")
        .expect("Failed to select inbox");

    let sequences: Vec<String> = imap_session
        .uid_search(format!("SUBJECT {}", subject))
        .expect("Failed to search for subject")
        .into_iter()
        .map(|uid| uid.to_string())
        .collect();

    let messages = imap_session
        .uid_fetch(sequences.join(","), "RFC822")
        .expect("Failed to fetch message");

    imap_session.logout().expect("Failed to logout");

    save_attachments(messages, &subject, output);
}

fn save_attachments(messages: Fetches, subject: &str, output: &Path) {
    for message in messages.iter() {
        let mail = mailparse::parse_mail(message.body().unwrap()).expect("Failed to parse header");

        let date = mail
            .get_headers()
            .into_iter()
            .find(|header| header.get_key() == "Date")
            .expect("Failed to find date header")
            .get_value();

        let date = DateTime::parse_from_rfc2822(&date)
            .expect("Failed to parse date")
            .format(DATE_TIME_OUTPUT_FORMAT);
        let file = output.join(format!("{date}-{subject}.xlsx"));
        if file.exists() {
            continue;
        }

        let attachment = mail.subparts[1]
            .get_body_raw()
            .expect("Failed to detach attachment");
        fs::write(file, attachment).expect("Failed to write attachment to disk");
    }
}
