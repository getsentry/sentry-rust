fn execute() -> anyhow::Result<usize> {
    let parsed = "NaN".parse()?;
    Ok(parsed)
}

fn main() {
    let _sentry = sentry::init((
        "https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156",
        sentry::ClientOptions {
            release: sentry::release_name!(),
            ..Default::default()
        },
    ));

    if let Err(err) = execute() {
        println!("error: {}", err);
        sentry_anyhow::capture_anyhow(&err);
    }
}
