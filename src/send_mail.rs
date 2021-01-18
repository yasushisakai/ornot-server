use actix_redis::Error as ARError;
use dotenv::dotenv;
use lettre::{transport::smtp::authentication::Credentials, Message, SmtpTransport, Transport};
use redis_async::resp::RespValue;
use tokio::task;

pub struct Email {
    pub to_name: String,
    pub to_address: String,
    pub subject: String,
    pub body: String,
}

pub async fn async_send_mail(email: Email) {
    task::spawn_blocking(|| send_mail(email));
}

pub fn send_mail(email: Email) {
    dotenv().ok();

    let smtp_host = std::env::var("SMTP_HOST").unwrap();
    let email_address = std::env::var("EMAIL_ADDRESS").unwrap();
    let email_password = std::env::var("EMAIL_PASSWORD").unwrap();

    let email_label = format!("ornot <{}>", &email_address);
    let to = format!("{}<{}>", email.to_name, email.to_address);

    let email = Message::builder()
        .from(email_label.parse().unwrap())
        .reply_to(email_label.parse().unwrap())
        .to(to.parse().unwrap())
        .subject(email.subject)
        .body(email.body)
        .unwrap();

    let creds = Credentials::new(email_address.to_string(), email_password.to_string());

    // Open a remote connection to gmail
    let mailer = SmtpTransport::relay(&smtp_host)
        .unwrap()
        .credentials(creds)
        .build();

    // Send the email
    match mailer.send(&email) {
        Ok(_) => println!("Email sent successfully!"),
        Err(e) => panic!("Could not send email: {:?}", e),
    }
}
