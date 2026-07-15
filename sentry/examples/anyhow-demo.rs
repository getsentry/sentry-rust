fn execute() -> anyhow::Result<usize> {
    let parsed = "NaN".parse()?;
    Ok(parsed)
}

fn main() {
    let _sentry = sentry::init(
        sentry::ClientOptions::new()
            .maybe_release(sentry::release_name!())
            .debug(true),
    );

    if let Err(err) = execute() {
        println!("error: {err}");
        sentry_anyhow::capture_anyhow(&err);
    }
}
