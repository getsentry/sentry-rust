use sentry_types::protocol::v7::SampleRand;

#[test]
fn sample_rand_lower_bound() {
    assert_eq!(
        "0.000000".parse::<SampleRand>().unwrap().to_string(),
        "0.000000"
    );
}

#[test]
fn sample_rand_upper_bound() {
    assert_eq!(
        "0.999999".parse::<SampleRand>().unwrap().to_string(),
        "0.999999"
    );
}

#[test]
fn sample_rand_1_rejected() {
    assert!("1.000000".parse::<SampleRand>().is_err());
}

#[test]
fn sample_rand_few_digits() {
    assert_eq!("0.5".parse::<SampleRand>().unwrap().to_string(), "0.500000");
}

#[test]
fn sample_rand_truncates_too_many_digits() {
    assert_eq!(
        "0.1234567".parse::<SampleRand>().unwrap().to_string(),
        "0.123456"
    );
}
