default:
  just --choose

check:
  cargo clippy --all-features --all-targets

coverage:
  cargo +nightly llvm-cov nextest --all-features --no-cfg-coverage --html

fmt:
  cargo +nightly fmt --all
