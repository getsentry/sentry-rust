# Changelog

## 0.15.0

- **breaking**: Remove usage of `failure`.
  `ParseAuthError`, `ParseDsnError`, `ParseProjectIdError`, and
  `ParseLevelError` now implement `std::error::Error`.
- **breaking**: Remove deprecated `with_serde` feature.
