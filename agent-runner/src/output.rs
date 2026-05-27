use crate::report::Report;
use std::path::Path;

pub fn write_output(
    result_text: &str,
    report: &Report,
    output_dir: &Path,
    mail_to: Option<&str>,
) -> Result<(), String> {
    println!("{}", result_text);

    report
        .write_to_file(output_dir)
        .map_err(|e| format!("Failed to write report: {}", e))?;

    if let Some(email) = mail_to {
        if let Err(e) = send_email(email, result_text, report) {
            eprintln!("Email send failed: {}", e);
        }
    }

    Ok(())
}

fn send_email(to: &str, body: &str, report: &Report) -> Result<(), String> {
    use lettre::message::header::ContentType;
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{Message, SmtpTransport, Transport};

    let smtp_host = std::env::var("SMTP_HOST").unwrap_or_else(|_| "localhost".to_string());
    let smtp_user = std::env::var("SMTP_USER").unwrap_or_default();
    let smtp_pass = std::env::var("SMTP_PASS").unwrap_or_default();
    let smtp_from = std::env::var("SMTP_FROM").unwrap_or_else(|_| smtp_user.clone());

    let subject = format!(
        "Agent Report: {} (exit {})",
        report.status, report.exit_code
    );

    let email = Message::builder()
        .from(
            smtp_from
                .parse()
                .map_err(|e| format!("Invalid from address: {}", e))?,
        )
        .to(to
            .parse()
            .map_err(|e| format!("Invalid to address: {}", e))?)
        .subject(&subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body.to_string())
        .map_err(|e| format!("Failed to build email: {}", e))?;

    let creds = Credentials::new(smtp_user, smtp_pass);

    let mailer = SmtpTransport::relay(&smtp_host)
        .map_err(|e| format!("SMTP relay error: {}", e))?
        .credentials(creds)
        .build();

    mailer
        .send(&email)
        .map_err(|e| format!("SMTP send error: {}", e))?;

    Ok(())
}
