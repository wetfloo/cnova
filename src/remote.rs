use std::sync::LazyLock;

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);
